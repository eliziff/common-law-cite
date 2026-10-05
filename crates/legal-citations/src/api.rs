//! JSON-in, JSON-out surface shared by the Python, WASM and CLI bindings.
//!
//! Every binding forwards `(method, request JSON)` to [`call`] and parses the
//! response JSON, so behavior, validation and offset conversion live here once.
//! The typed helpers ([`extract`], [`key`], ...) are the same operations for
//! Rust callers that want to skip JSON.
//!
//! Requests are JSON objects with camelCase keys; unknown keys are rejected so
//! a typo never silently falls back to a default. Responses are JSON objects.
//! Citation JSON follows `conformance/schema/citation.schema.json`, versioned by
//! [`SCHEMA_VERSION`].
//!
//! | method            | request                                                        | response                                               |
//! |-------------------|----------------------------------------------------------------|--------------------------------------------------------|
//! | `extract`         | `{text, options?, offsetUnit?}`                                | `{schemaVersion, offsetUnit, citations, sourceParts}`  |
//! | `key`             | `{citation}`                                                   | `{keyVersion, key}`                                    |
//! | `keyForText`      | `{text, options?}`                                             | `{keyVersion, key, reason?, message?, keys}`           |
//! | `format`          | `{citation}` / `{text, options?}` / `{pinpoint}`, `language?`, `rangeDash?`, `style?` | `{citations: [{index, formatted}]}` / `{pinpoint}` |
//! | `formatDocument`  | `{documentType?, title?, citation?}`                            | `{title, citation, plain}`                              |
//! | `caseHeading`     | `{text}`                                                       | `{name, citation}`                                      |
//! | `url`             | `{citation}` / `{text, options?}`, `language?`, `anchor?`     | `{urls: [{index, url}]}`                               |
//! | `sourceCanliiRoutes` | `{}`                                                        | source court code to CanLII route                       |
//! | `annotate`        | `{text, annotations? \| before/after, span?, source?, cleanSteps?, unbalancedTags?, offsetUnit?}` | `{text}`          |
//! | `clean`           | `{text, steps}`                                                | `{text}`                                               |
//! | `registry`        | `{table?, surface?}`                                           | the registry, a table, or the entries a surface names  |
//! | `classifyExcerpt` | `{excerpt}`                                                    | `{kind, citeTokens, citeRuns, ...}`                    |
//! | `hasCitation`     | `{text}`                                                       | `{hasCitation}`                                        |
//! | `version`         | `{}`                                                           | `{version, schemaVersion, keyVersion, grammar, registry}` |
//!
//! Offsets in requests and responses are in the request's `offsetUnit`
//! (`byte`, `char` = Unicode scalar values, `utf16` = JavaScript code units),
//! default `byte`. Bindings set their language's natural unit.

use crate::format::{self as format_stage, Language};
use crate::model::{Citation, Form, PinpointKind, Span};
use crate::text::ScalarText;
use crate::{
    annotate as annotate_stage, clean as clean_stage, excerpt, key as key_stage,
    registry as registry_stage, url as url_stage, OffsetUnit, Options,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

/// Version of the JSON citation schema (`conformance/schema/citation.schema.json`).
/// Bumped on any breaking change to the serialized [`Citation`] shape; additive
/// optional fields do not bump it.
pub const SCHEMA_VERSION: u32 = 3;

/// Every method [`call`] accepts.
pub const METHODS: &[&str] = &[
    "extract",
    "resolve",
    "resolveReference",
    "inferShortForms",
    "referenceShortForms",
    "referenceInfo",
    "normalizeShortForm",
    "resolveInferredReference",
    "resolveRegistryReference",
    "supraHint",
    "reanchorReference",
    "splitSources",
    "pureReferenceClauses",
    "quotedTitles",
    "sourceFields",
    "bareCitation",
    "legislationLookup",
    "stripCitationTail",
    "correctCitation",
    "caseNamesAgree",
    "authorities",
    "captionStyleOfCause",
    "pinpointsAt",
    "key",
    "keyForText",
    "format",
    "formatArticle",
    "formatDocument",
    "caseHeading",
    "chooseCaseCitation",
    "pinpointLayouts",
    "url",
    "sourceCanliiRoutes",
    "canliiCitationUrl",
    "canliiAliasTarget",
    "canliiAliasTargetInfo",
    "annotate",
    "annotationRanges",
    "clean",
    "placeholderMarkup",
    "registry",
    "classifyExcerpt",
    "hasCitation",
    "hasCitationCue",
    "hasCitationSignal",
    "isCitationContinuation",
    "protectedCitationSpans",
    "matchesReporterHeader",
    "version",
];

/// Why a call failed. Serialized as `{"code": ..., "message": ...}`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub enum ErrorCode {
    /// The method name is not one of [`METHODS`].
    UnknownMethod,
    /// The request is not valid JSON or does not match the method's shape.
    InvalidRequest,
    /// An offset in the request is out of range or splits a character.
    InvalidOffset,
    /// The method is part of the surface but its engine stage is not built yet.
    Unimplemented,
}

impl ApiError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// The error as a JSON object string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("error serializes")
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

/// Dispatch one JSON request. `request_json` may be empty for methods that take
/// no arguments.
pub fn call(method: &str, request_json: &str) -> Result<String, ApiError> {
    let request = if request_json.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(request_json)
            .map_err(|error| ApiError::invalid(format!("request is not valid JSON: {error}")))?
    };
    let response = call_value(method, request)?;
    Ok(serde_json::to_string(&response).expect("response serializes"))
}

/// [`call`] over parsed JSON values.
pub fn call_value(method: &str, request: Value) -> Result<Value, ApiError> {
    fn to_value<T: Serialize>(value: T) -> Result<Value, ApiError> {
        Ok(serde_json::to_value(value).expect("response serializes"))
    }
    match method {
        "extract" => to_value(extract(&parse(method, request)?)?),
        "resolve" => to_value(resolve(&parse(method, request)?)?),
        "resolveReference" => to_value(crate::resolve::resolve_reference(&parse(method, request)?)),
        "inferShortForms" => {
            let request: InferShortFormsRequest = parse(method, request)?;
            to_value(crate::short_forms::infer(&request.text, &request.kind))
        }
        "referenceShortForms" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::short_forms::reference_candidates(&request.text))
        }
        "referenceInfo" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::short_forms::reference_info(&request.text))
        }
        "bareCitation" => {
            let request: InferShortFormsRequest = parse(method, request)?;
            to_value(crate::source::derive_bare_citation(&request.text, &request.kind))
        }
        "legislationLookup" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(url_stage::legislation_lookup(&request.text))
        }
        "stripCitationTail" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::source::strip_administrative_tail(&request.text))
        }
        "correctCitation" => to_value(crate::format::correct_citation(&parse(method, request)?)),
        "authorities" => to_value(crate::authorities::authorities(&parse(method, request)?)),
        "captionStyleOfCause" => to_value(crate::authorities::caption_style_of_cause(&parse(method, request)?)),
        "pinpointsAt" => to_value(crate::authorities::pinpoints_at(&parse(method, request)?)),
        "caseNamesAgree" => to_value(crate::format::case_names_agree(&parse(method, request)?)),
        "normalizeShortForm" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::short_forms::normalize(&request.text))
        }
        "resolveInferredReference" => {
            let request: InferredReferenceRequest = parse(method, request)?;
            to_value(crate::short_forms::resolve_after_strict_abstention(
                &request.text, &request.registry, &request.inferred_forms))
        }
        "resolveRegistryReference" => {
            let request: RegistryReferenceRequest = parse(method, request)?;
            to_value(crate::short_forms::resolve_registry(&request.text, &request.registry, request.aggressive))
        }
        "reanchorReference" => {
            let request: ReanchorRequest = parse(method, request)?;
            to_value(crate::short_forms::reanchor_reference(&request.link, &request.text))
        },
        "supraHint" => {
            let request: SupraHintRequest = parse(method, request)?;
            to_value(if request.fallback {
                crate::short_forms::fallback_hint(&request.text)
            } else {
                crate::short_forms::supra_hint(&request.text, request.aggressive)
            })
        }
        "splitSources" => {
            let request: SplitSourcesRequest = parse(method, request)?;
            to_value(split_sources(&request))
        }
        "splitNotes" => to_value(split_notes(&parse(method, request)?)?),
        "quotedTitles" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::source::quoted_titles(&request.text))
        }
        "pureReferenceClauses" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(crate::source::pure_reference_clauses(&request.text))
        }
        "noteReferences" => {
            let request: NoteReferencesRequest = parse(method, request)?;
            to_value(crate::source::references(&request.parts, request.citations.as_deref(),
                request.kinds.as_deref(), request.styles.as_deref(), |prefix| match request.offset_unit {
                    OffsetUnit::Byte => prefix.len(),
                    OffsetUnit::Char => prefix.chars().count(),
                    OffsetUnit::Utf16 => prefix.encode_utf16().count(),
                }))
        }
        "sourceFields" => {
            let request: SourceFieldsRequest = parse(method, request)?;
            match (&request.text, &request.part) {
                (Some(text), None) => to_value(crate::source::extract_text_fields(text, request.extended_us.unwrap_or(false))),
                (None, Some(part)) => to_value(crate::source::extract_fields(part, request.extended_us.unwrap_or(part.extended_us))),
                _ => Err(ApiError::invalid("sourceFields: pass exactly one of text or part")),
            }
        }
        "key" => to_value(key(&parse(method, request)?)),
        "matchesReporterHeader" => {
            let request: ReporterHeaderRequest = parse(method, request)?;
            to_value(crate::cues::matches_reporter_header(&request.text, &request.citations))
        }
        "keyForText" => to_value(key_for_text(&parse(method, request)?)?),
        "format" => to_value(format(&parse(method, request)?)?),
        "formatArticle" => to_value(format_stage::article(&parse(method, request)?)),
        "chooseCaseCitation" => to_value(format_stage::choose_case_citation(&parse(method, request)?)),
        "formatDocument" => to_value(format_stage::document(&parse(method, request)?)),
        "caseHeading" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Request { text: String }
            let request: Request = parse(method, request)?;
            to_value(format_stage::case_heading(&request.text))
        },
        "pinpointLayouts" => {
            let request: PinpointLayoutsRequest = parse(method, request)?;
            if !matches!(request.style.as_deref(), None | Some("short" | "full")) {
                return Err(ApiError::invalid("pinpointLayouts: style must be short or full"));
            }
            to_value(request.values.iter().map(|values| format_stage::pinpoint_layout(&request.kind, values, request.style.as_deref() == Some("full"))).collect::<Vec<_>>())
        },
        "url" => to_value(url(&parse(method, request)?)?),
        "canliiCitationUrl" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Request { text: String, language: Option<String> }
            let request: Request = parse(method, request)?;
            to_value(url_stage::canlii_citation_url(&request.text, request.language.as_deref().unwrap_or("en")))
        },
        "sourceCanliiRoutes" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Empty {}
            let _: Empty = parse(method, request)?;
            to_value(url_stage::source_canlii_routes())
        },
        "canliiAliasTarget" => {
            let request: AliasTargetRequest = parse(method, request)?;
            let page = url_stage::canlii_alias_target(&request.target, api_language("canliiAliasTarget", request.language.as_deref())?);
            to_value(if request.pdf { page.and_then(|page| url_stage::canlii_pdf_url(&page)) } else { page })
        },
        "canliiAliasTargetInfo" => {
            let request: AliasTargetRequest = parse(method, request)?;
            let mut info = url_stage::canlii_alias_target_info(&request.target, api_language("canliiAliasTargetInfo", request.language.as_deref())?);
            if request.pdf {
                info = info.and_then(|mut info| {
                    info.url = url_stage::canlii_pdf_url(&info.url)?;
                    Some(info)
                });
            }
            to_value(info)
        },
        "annotate" => to_value(annotate(&parse(method, request)?)?),
        "annotationRanges" => to_value(annotation_ranges(&parse(method, request)?)?),
        "clean" => to_value(clean(&parse(method, request)?)?),
        "placeholderMarkup" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(clean_stage::placeholder_markup(&request.text))
        }
        "registry" => registry(&parse(method, request)?),
        "classifyExcerpt" => to_value(classify_excerpt(&parse(method, request)?)),
        "hasCitationCue" | "hasCitationSignal" | "isCitationContinuation" => {
            let request: HasCitationRequest = parse(method, request)?;
            to_value(match method {
                "hasCitationCue" => crate::cues::has_citation_cue(&request.text),
                "hasCitationSignal" => crate::cues::has_citation_signal(&request.text),
                _ => crate::cues::is_citation_continuation(&request.text),
            })
        },
        "protectedCitationSpans" => to_value(protected_citation_spans(&parse(method, request)?)),
        "hasCitation" => to_value(has_citation(&parse(method, request)?)),
        "version" => {
            parse::<Empty>(method, request)?;
            to_value(version())
        }
        other => Err(ApiError::new(
            ErrorCode::UnknownMethod,
            format!(
                "unknown method {other:?}; expected one of {}",
                METHODS.join(", ")
            ),
        )),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(method: &str, mut request: Value) -> Result<T, ApiError> {
    if !request.is_object() {
        return Err(ApiError::invalid(format!(
            "{method}: request must be a JSON object"
        )));
    }
    normalize_options(method, &mut request)?;
    serde_json::from_value(request).map_err(|error| ApiError::invalid(format!("{method}: {error}")))
}

/// Normalize convenience-call options once, at the engine boundary.
fn normalize_options(method: &str, request: &mut Value) -> Result<(), ApiError> {
    let object = request.as_object_mut().ok_or_else(|| ApiError::invalid("request must be an object"))?;
    if !matches!(method, "extract" | "keyForText" | "format" | "url" | "annotate" | "annotationRanges") { return Ok(()); }
    static OPTION_NAMES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        serde_json::to_value(Options::default()).expect("options serialize")
            .as_object().unwrap().keys().cloned().collect()
    });
    let mut options = object.remove("options").unwrap_or_else(|| serde_json::json!({}));
    let target = options.as_object_mut().ok_or_else(|| ApiError::invalid("options must be an object"))?;
    for name in OPTION_NAMES.iter() {
        if let Some(value) = object.remove(name) {
            if target.insert(name.clone(), value).is_some() {
                return Err(ApiError::invalid(format!("duplicate option {name:?}")));
            }
        }
    }
    let has_extraction_options = !target.is_empty();
    let parsed: Options = serde_json::from_value(options.clone()).map_err(|error| ApiError::invalid(error.to_string()))?;
    for priority in &parsed.jurisdiction_priority {
        if registry_stage::registry().jurisdiction(priority).is_none() {
            return Err(ApiError::invalid(format!("unknown jurisdiction priority {priority:?}")));
        }
    }
    let extracts = match method {
        "format" | "url" => object.get("text").is_some_and(|value| !value.is_null()),
        "annotate" | "annotationRanges" => object.get("annotations").is_none_or(Value::is_null),
        _ => true,
    };
    if !extracts && has_extraction_options {
        return Err(ApiError::invalid(format!("{method}: extraction options require text extraction")));
    }
    if matches!(method, "format" | "url") && !extracts && object.contains_key("offsetUnit") {
        return Err(ApiError::invalid(format!("{method}: offsetUnit requires text extraction")));
    }
    object.insert("options".into(), options);
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

// ---------------------------------------------------------------- extract

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct InferShortFormsRequest {
    pub text: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct InferredReferenceRequest {
    pub text: String,
    pub registry: Vec<crate::short_forms::ReferenceSource>,
    pub inferred_forms: Vec<crate::short_forms::ReferenceSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct RegistryReferenceRequest {
    pub text: String,
    pub registry: Vec<crate::short_forms::ReferenceSource>,
    pub aggressive: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SupraHintRequest {
    pub text: String,
    pub aggressive: bool,
    #[serde(default)]
    pub fallback: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReanchorRequest {
    pub link: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SplitSourcesRequest {
    pub text: String,
    #[serde(default)]
    pub recall_first: bool,
    #[serde(default)]
    pub extended_us: bool,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SourceFieldsRequest {
    pub text: Option<String>,
    pub part: Option<crate::source::SourcePart>,
    #[serde(default)]
    pub extended_us: Option<bool>,
}

pub fn split_sources(request: &SplitSourcesRequest) -> crate::source::SourceSplit {
    let mut result = crate::source::split(&request.text, request.recall_first, request.extended_us);
    let document = ScalarText::new(&request.text);
    let convert = |byte| match request.offset_unit {
        OffsetUnit::Byte => byte,
        OffsetUnit::Char => document.scalar_at_byte(byte).unwrap(),
        OffsetUnit::Utf16 => document.utf16_at_byte(byte).unwrap(),
    };
    for part in &mut result.parts {
        for (start, end) in &mut part.anchor_spans {
            *start = convert(*start);
            *end = convert(*end);
        }
        part.start = convert(part.start);
        part.end = convert(part.end);
    }
    for (start, end, _) in &mut result.delimiters {
        *start = convert(*start);
        *end = convert(*end);
    }
    result
}

/// Note splitting: every part of every note, prose included, as `extract` splits them.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct SplitNotesRequest {
    pub text: String,
    pub notes: Vec<crate::NoteRange>,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

/// Authority references: what the split parts cite (see [`crate::source::references`]).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct NoteReferencesRequest {
    pub parts: Vec<crate::source::SourcePart>,
    #[serde(default)]
    pub citations: Option<Vec<Citation>>,
    /// The kinds of authority wanted ("case", "statute", "journal", ...); every kind when absent.
    #[serde(default)]
    pub kinds: Option<Vec<crate::Authority>>,
    /// The citation guides whose own forms count ("mcgill", "coal", "bluebook", "aglc", "oscola",
    /// "nzlsg"); every guide's when absent. Supplied citations were found under the extract's own.
    #[serde(default)]
    pub styles: Option<Vec<crate::CitationStyle>>,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

pub fn split_notes(request: &SplitNotesRequest) -> Result<Vec<crate::source::SourcePart>, ApiError> {
    let document = ScalarText::new(&request.text);
    let options = byte_options(&document, &Options { notes: Some(request.notes.clone()), ..Options::default() },
        request.offset_unit)?;
    let mut parts = crate::source::split_notes(&request.text, options.notes.as_deref().unwrap_or(&[]));
    convert_parts(&document, &mut parts, request.offset_unit);
    Ok(parts)
}

fn convert_parts(document: &ScalarText<'_>, parts: &mut [crate::source::SourcePart], unit: OffsetUnit) {
    for part in parts {
        for (start, end) in &mut part.anchor_spans {
            *start = from_byte(document, *start, unit);
            *end = from_byte(document, *end, unit);
        }
        part.start = from_byte(document, part.start, unit);
        part.end = from_byte(document, part.end, unit);
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ExtractRequest {
    pub text: String,
    #[serde(default)]
    pub markup_text: Option<String>,
    #[serde(default)]
    pub options: Options,
    /// Unit of every offset in the response, and of `options.notes` ranges.
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ReporterHeaderRequest {
    pub text: String,
    pub citations: Vec<Citation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ExtractResponse {
    pub schema_version: u32,
    pub offset_unit: OffsetUnit,
    pub citations: Vec<Citation>,
    pub source_parts: Vec<crate::source::SourcePart>,
    pub authorities: Vec<Vec<usize>>,
    /// How each reference resolved (with `options.resolve`), its source part indexed into
    /// `source_parts`: the antecedent of a reference to a book, report or page no citation names.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolutions: Vec<crate::resolve::Resolution>,
}

/// [`crate::extract`] with offsets converted to `request.offset_unit`.
pub fn extract(request: &ExtractRequest) -> Result<ExtractResponse, ApiError> {
    let document = ScalarText::new(&request.text);
    let (citations, mut source_parts, resolutions) = extract_citations(&document, request.markup_text.as_deref(), &request.options, request.offset_unit)?;
    convert_parts(&document, &mut source_parts, request.offset_unit);
    Ok(ExtractResponse {
        schema_version: SCHEMA_VERSION,
        offset_unit: request.offset_unit,
        authorities: crate::resolve::authorities_with_resolutions(&citations, &[], &resolutions),
        citations,
        source_parts,
        resolutions,
    })
}

fn from_byte(document: &ScalarText<'_>, byte: usize, unit: OffsetUnit) -> usize {
    match unit {
        OffsetUnit::Byte => byte,
        OffsetUnit::Char => document.scalar_at_byte(byte).expect("source boundary"),
        OffsetUnit::Utf16 => document.utf16_at_byte(byte).expect("source boundary"),
    }
}

fn extract_citations(
    document: &ScalarText<'_>,
    markup: Option<&str>,
    options: &Options,
    unit: OffsetUnit,
) -> Result<(Vec<Citation>, Vec<crate::source::SourcePart>, Vec<crate::resolve::Resolution>), ApiError> {
    let options = byte_options(document, options, unit)?;
    let (mut citations, parts, resolutions) = crate::extract_markup_with_parts(document.value, markup, &options);
    convert_citations(document, &mut citations, unit);
    Ok((citations, parts, resolutions))
}

fn byte_options(document: &ScalarText<'_>, options: &Options, unit: OffsetUnit) -> Result<Options, ApiError> {
    let mut options = options.clone();
    if let Some(notes) = options.notes.as_mut() {
        for note in notes.iter_mut() {
            note.start = to_byte(document, note.start, unit, "options.notes[].start")?;
            note.end = to_byte(document, note.end, unit, "options.notes[].end")?;
            if let Some(anchor) = note.anchor {
                note.anchor = Some(to_byte(document, anchor, unit, "options.notes[].anchor")?);
            }
            if note.start > note.end {
                return Err(ApiError::new(
                    ErrorCode::InvalidOffset,
                    format!("note {}: start is after end", note.number),
                ));
            }
        }
        let mut ranges = notes.iter().map(|note| (note.start, note.end)).collect::<Vec<_>>();
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(ApiError::invalid("extract: note ranges overlap"));
        }
    }
    Ok(options)
}

fn to_byte(
    document: &ScalarText<'_>,
    offset: usize,
    unit: OffsetUnit,
    what: &str,
) -> Result<usize, ApiError> {
    let byte = match unit {
        OffsetUnit::Byte => document.value.is_char_boundary(offset).then_some(offset),
        OffsetUnit::Char => document.byte_at_scalar(offset),
        OffsetUnit::Utf16 => document.byte_at_utf16(offset),
    };
    byte.ok_or_else(|| {
        ApiError::new(
            ErrorCode::InvalidOffset,
            format!("{what}: {offset} is not a {unit:?} boundary inside the text"),
        )
    })
}

/// Convert every byte offset in `citations` (spans, pinpoints, parentheticals,
/// history) to `unit`. Offsets produced by the engine are always character
/// boundaries of `document`.
pub fn convert_citations(document: &ScalarText<'_>, citations: &mut [Citation], unit: OffsetUnit) {
    if unit == OffsetUnit::Byte {
        return;
    }
    let convert = |byte| from_byte(document, byte, unit);
    let convert_span = |span: &mut Span| {
        span.start = convert(span.start);
        span.end = convert(span.end);
    };
    for citation in citations {
        convert_span(&mut citation.span);
        convert_span(&mut citation.full_span);
        if let Some(pin_cite) = citation.fields.pin_cite.as_mut() {
            convert_span(pin_cite);
        }
        if let Some(short) = citation.fields.explicit_short_span.as_mut() {
            convert_span(short);
        }
        if let Some(reference) = citation.fields.inline_reference.as_mut() {
            convert_span(&mut reference.span);
        }
        if let Some(title) = citation.fields.anchor_title.as_mut() {
            convert_span(title);
        }
        if let Some(mention) = citation.fields.anchor_mention.as_mut() {
            convert_span(mention);
        }
        if let Some(name) = citation.fields.source_case_name.as_mut() {
            name.full_span_start = convert(name.full_span_start);
            name.full_span_end = name.full_span_end.map(convert);
            name.pin_cite_span_end = name.pin_cite_span_end.map(convert);
            if let Some(pre) = name.pre_citation.as_mut() { convert_span(pre); }
            if let Some(pin) = name.pin_cite.as_mut() { convert_span(pin); }
            if let Some(reference) = name.reference_span.as_mut() { convert_span(reference); }
            if let Some(token) = name.token_span.as_mut() { convert_span(token); }
        }
        if let Some(signal) = citation.signal.as_mut() {
            convert_span(signal);
        }
        if let Some(style) = citation.style.as_mut() {
            convert_span(style);
        }
        for pinpoint in &mut citation.pinpoints {
            convert_span(&mut pinpoint.span);
        }
        for parenthetical in &mut citation.parentheticals {
            convert_span(&mut parenthetical.span);
        }
        for history in &mut citation.history {
            convert_span(&mut history.span);
        }
    }
}

// ---------------------------------------------------------------- resolve

/// Resolve an existing citation list without extracting it again. Note ranges
/// and citation spans must use the same offset unit; resolution does not slice
/// the document, so no conversion or original text is needed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ResolveRequest {
    pub citations: Vec<Citation>,
    /// Source boundaries emitted by extract, in the same offset unit as citations and notes.
    #[serde(default)]
    pub source_parts: Vec<crate::source::SourcePart>,
    #[serde(default)]
    pub supra_hint_mode: Option<crate::SupraMode>,
    #[serde(default)]
    pub supra_linking_mode: Option<crate::SupraMode>,
    /// See [`crate::Options::split_tier`].
    #[serde(default)]
    pub split_tier: Option<crate::SupraMode>,
    /// Citation indices in document reading order, when footnote anchors are known.
    #[serde(default)]
    pub reading_order: Option<Vec<usize>>,
    #[serde(default)]
    pub notes: Option<Vec<crate::NoteRange>>,
    #[serde(default)]
    pub alias_groups: Vec<crate::aliases::SourceAliasGroup>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ResolveResponse {
    pub citations: Vec<Citation>,
    pub resolutions: Vec<crate::resolve::Resolution>,
    pub authorities: Vec<Vec<usize>>,
}

pub fn resolve(request: &ResolveRequest) -> Result<ResolveResponse, ApiError> {
    let mut citations = request.citations.clone();
    let mut indices = std::collections::HashSet::new();
    for citation in &mut citations {
        if !indices.insert(citation.index) {
            return Err(ApiError::invalid("resolve: citation indices must be unique"));
        }
        if citation.span.start > citation.span.end {
            return Err(ApiError::invalid("resolve: citation start is after its end"));
        }
        // Recompute identity and references from the supplied records. A caller
        // may have removed or edited citations since the previous resolution.
        citation.antecedent = None;
        citation.reasons.retain(|reason| reason != "source_alias_conflict");
        citation.alias = crate::aliases::resolve(citation).cloned();
        citation.key = citation.alias.as_ref().map(|target| target.key.clone())
            .or_else(|| key_stage::key_in(citation, crate::registry::registry()));
    }
    if request.notes.as_ref().is_some_and(|notes| notes.iter().any(|note| note.start > note.end)) {
        return Err(ApiError::invalid("resolve: note start is after its end"));
    }
    if let Some(notes) = request.notes.as_deref() {
        let mut ranges = notes.iter().map(|note| (note.start, note.end)).collect::<Vec<_>>();
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(ApiError::invalid("resolve: note ranges overlap"));
        }
    }
    if !request.source_parts.is_empty() {
        let Some(notes) = request.notes.as_deref().filter(|notes| !notes.is_empty()) else {
            return Err(ApiError::invalid("resolve: sourceParts require note ranges"));
        };
        let mut parts = request.source_parts.iter().collect::<Vec<_>>();
        parts.sort_by_key(|part| (part.start, part.end));
        if parts.windows(2).any(|pair| pair[0].end > pair[1].start) {
            return Err(ApiError::invalid("resolve: source parts overlap"));
        }
        for part in &parts {
            if part.start >= part.end || !notes.iter().any(|note|
                note.start <= part.start && part.end <= note.end)
                || part.anchor_spans.iter().any(|&(start, end)|
                    start >= end || start < part.start || end > part.end)
                || part.anchor_spans.windows(2).any(|pair| pair[0] > pair[1]) {
                return Err(ApiError::invalid("resolve: invalid source part extent or anchors"));
            }
        }
    }
    citations.sort_by_key(|citation| (citation.span.start, citation.index));
    if request.alias_groups.iter().any(|group| !indices.contains(&group.index)) {
        return Err(ApiError::invalid("resolve: alias group refers to an unknown citation index"));
    }
    let links = crate::aliases::source_links(&mut citations, &request.alias_groups);
    let order = request.reading_order.as_ref().map(|order| {
        let positions = citations.iter().enumerate().map(|(position, citation)| (citation.index, position))
            .collect::<std::collections::HashMap<_, _>>();
        if order.len() != citations.len() || order.iter().copied().collect::<std::collections::HashSet<_>>() != indices {
            return Err(ApiError::invalid("resolve: readingOrder must contain every citation index exactly once"));
        }
        Ok(order.iter().map(|index| positions[index]).collect::<Vec<_>>())
    }).transpose()?;
    let resolutions = crate::resolve::resolve_with_sources(&citations, request.notes.as_deref(), &links,
        order, &request.source_parts, request.supra_hint_mode.unwrap_or(crate::SupraMode::Aggressive),
        request.supra_linking_mode.unwrap_or(crate::SupraMode::Safe), request.split_tier);
    for resolution in &resolutions {
        if let Some(citation) = citations.iter_mut().find(|citation| citation.index == resolution.index) {
            citation.antecedent = resolution.antecedent;
        }
    }
    let authorities = crate::resolve::authorities_with_resolutions(&citations, &links, &resolutions);
    Ok(ResolveResponse { citations, resolutions, authorities })
}

// ---------------------------------------------------------------- key

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct KeyRequest {
    pub citation: Citation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct KeyResponse {
    pub key_version: String,
    pub key: Option<String>,
}

/// The identity key of one full citation (see [`crate::key`]); `null` for
/// back references and authorities without a stable identity.
pub fn key(request: &KeyRequest) -> KeyResponse {
    KeyResponse {
        key_version: key_stage::KEY_VERSION.to_owned(),
        key: key_stage::key(&request.citation),
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct TextRequest {
    pub text: String,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CitationKey {
    pub index: usize,
    /// The citation core as written.
    pub text: String,
    pub key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct KeyForTextResponse {
    pub key_version: String,
    /// The key of the one full citation `text` holds (`R v Jordan, 2016 SCC
    /// 27`), per [`crate::key::key_for_text`]; `null` with a `reason` otherwise.
    pub key: Option<String>,
    /// `no_citation`, `multiple_citations` or `no_identity` when `key` is null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Every citation's key in document order; a back reference carries the
    /// key of the authority it resolves to.
    pub keys: Vec<CitationKey>,
}

/// The key of the single citation in `text`, plus every citation's key.
pub fn key_for_text(request: &TextRequest) -> Result<KeyForTextResponse, ApiError> {
    let citations = extract_citations(&ScalarText::new(&request.text), None, &request.options, request.offset_unit)?.0;
    let (key, reason, message) = match key_stage::single_key(&citations) {
        Ok(key) => (Some(key), None, None),
        Err(error) => {
            let reason = match error {
                key_stage::KeyError::NoCitation => "no_citation",
                key_stage::KeyError::Multiple(_) => "multiple_citations",
                key_stage::KeyError::NoIdentity => "no_identity",
            };
            (None, Some(reason.to_owned()), Some(error.to_string()))
        }
    };
    let keys = citations
        .iter()
        .map(|citation| CitationKey {
            index: citation.index,
            text: citation.span.text.clone(),
            key: citation
                .key
                .clone()
                .or_else(|| citations.iter().find(|target| target.index == citation.authority_index())?.key.clone()),
        })
        .collect();
    Ok(KeyForTextResponse {
        key_version: key_stage::KEY_VERSION.to_owned(),
        key,
        reason,
        message,
        keys,
    })
}

// ---------------------------------------------------------------- format

/// Format citations (`citation` or every citation in `text`) or a bare
/// pinpoint, in McGill style.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct FormatRequest {
    #[serde(default)]
    pub citation: Option<Citation>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub pinpoint: Option<PinpointRequest>,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
    /// Citation style; only `mcgill` (the default) today.
    #[serde(default)]
    pub style: Option<String>,
    /// `en` (default) or `fr`.
    #[serde(default)]
    pub language: Option<String>,
    /// Range separator: `-` (default) or `–` (U+2013).
    #[serde(default)]
    pub range_dash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct PinpointLayoutsRequest {
    pub kind: String,
    pub values: Vec<Vec<String>>,
    #[serde(default)] pub style: Option<String>,
}

/// `{kind: "paragraph", locators: [{first: "12", last: "14"}]}` →
/// `at paras 12-14`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct PinpointRequest {
    pub kind: PinpointKind,
    pub locators: Vec<Locator>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Locator {
    pub first: String,
    #[serde(default)]
    pub last: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Formatted {
    pub index: usize,
    /// `null` for forms the formatter does not render (short forms,
    /// references, unknown).
    pub formatted: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct FormatResponse {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Formatted>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinpoint: Option<String>,
}

fn format_style(request: &FormatRequest) -> Result<format_stage::Style, ApiError> {
    if let Some(style) = request
        .style
        .as_deref()
        .filter(|style| !style.eq_ignore_ascii_case("mcgill"))
    {
        return Err(ApiError::invalid(format!(
            "format: unsupported style {style:?}; expected mcgill"
        )));
    }
    let language = api_language("format", request.language.as_deref())?;
    let range_dash = match request.range_dash.as_deref() {
        None | Some("-") => "-",
        Some("\u{2013}") => "\u{2013}",
        Some(other) => {
            return Err(ApiError::invalid(format!(
                "format: rangeDash must be \"-\" or \"\u{2013}\", got {other:?}"
            )))
        }
    };
    Ok(format_stage::Style {
        language,
        range_dash,
    })
}

fn api_language(method: &str, code: Option<&str>) -> Result<Language, ApiError> {
    let Some(code) = code else { return Ok(Language::En); };
    match code.trim().to_ascii_lowercase().split('-').next() {
        Some("en") => Ok(Language::En),
        Some("fr") => Ok(Language::Fr),
        _ => Err(ApiError::invalid(format!("{method}: language must be en or fr"))),
    }
}

fn request_citations(
    method: &str,
    citation: &Option<Citation>,
    text: &Option<String>,
    options: &Options,
    offset_unit: OffsetUnit,
) -> Result<(Vec<Citation>, Vec<crate::resolve::Resolution>), ApiError> {
    match (citation, text) {
        (Some(citation), None) => Ok((vec![citation.clone()], Vec::new())),
        (None, Some(text)) => {
            let (citations, _, resolutions) = extract_citations(&ScalarText::new(text), None, options, offset_unit)?;
            Ok((citations, resolutions))
        }
        _ => Err(ApiError::invalid(format!(
            "{method}: pass exactly one of `citation` or `text`"
        ))),
    }
}

/// One citation rendered in McGill form: full citations with their pinpoints,
/// `Ibid` and `supra note N` back references. `citations` is the document the
/// citation came from, used to name a supra's antecedent.
pub fn format_citation(
    citation: &Citation,
    citations: &[Citation],
    style: format_stage::Style,
) -> Option<String> {
    let label =
        Some(format_stage::pinpoints(&citation.pinpoints, style)).filter(|label| !label.is_empty());
    match citation.form {
        Form::Full => Some(format_stage::full(citation, style)),
        Form::Ibid => Some(format_stage::ibid(label.as_deref())),
        Form::Supra => {
            let note = citation.fields.note?;
            let short = citation
                .antecedent
                .and_then(|antecedent| {
                    citations
                        .iter()
                        .find(|candidate| candidate.index == antecedent)
                })
                .map(|antecedent| format_stage::short_label(antecedent, style.language))
                .or_else(|| {
                    citation
                        .style
                        .as_ref()
                        .map(|span| span.text.trim().trim_end_matches(',').trim().to_owned())
                        .filter(|name| !name.is_empty())
                })?;
            Some(format_stage::supra(&short, note, label.as_deref()))
        }
        _ => None,
    }
}

pub fn format(request: &FormatRequest) -> Result<FormatResponse, ApiError> {
    let style = format_style(request)?;
    if let Some(pinpoint) = &request.pinpoint {
        if request.citation.is_some() || request.text.is_some() {
            return Err(ApiError::invalid(
                "format: pass one of `citation`, `text` or `pinpoint`",
            ));
        }
        let items = pinpoint
            .locators
            .iter()
            .map(|locator| (locator.first.as_str(), locator.last.as_deref()))
            .collect::<Vec<_>>();
        return Ok(FormatResponse {
            citations: Vec::new(),
            pinpoint: Some(format_stage::pinpoint(pinpoint.kind, &items, style)),
        });
    }
    let (citations, _) =
        request_citations("format", &request.citation, &request.text, &request.options, request.offset_unit)?;
    Ok(FormatResponse {
        citations: citations
            .iter()
            .map(|citation| Formatted {
                index: citation.index,
                formatted: format_citation(citation, &citations, style),
            })
            .collect(),
        pinpoint: None,
    })
}

// ---------------------------------------------------------------- url

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct UrlRequest {
    #[serde(default)]
    pub citation: Option<Citation>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
    /// `en` (default) or `fr`: the language of the page to link.
    #[serde(default)]
    pub language: Option<String>,
    /// Append the anchor of a single pinpoint (`#par12`) when the source has one.
    #[serde(default)]
    pub anchor: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct AliasTargetRequest {
    pub target: String,
    #[serde(default)] pub language: Option<String>,
    #[serde(default)] pub pdf: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CitationUrl {
    pub index: usize,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct UrlResponse {
    pub urls: Vec<CitationUrl>,
}

/// The public-source URL of each citation (CanLII, Justice Laws, Find Case
/// Law, legislation.gov.uk, CourtListener). A back reference gets its
/// antecedent's URL; `null` when no source is certain.
pub fn url(request: &UrlRequest) -> Result<UrlResponse, ApiError> {
    let (citations, resolutions) = request_citations("url", &request.citation, &request.text, &request.options, request.offset_unit)?;
    let language = api_language("url", request.language.as_deref())?;
    let source_urls = resolutions.into_iter().filter_map(|resolution|
        resolution.url.map(|url| (resolution.index, url)))
        .collect::<std::collections::HashMap<_, _>>();
    let urls = citations
        .iter()
        .map(|citation| {
            let authority = citations
                .iter()
                .find(|candidate| candidate.index == citation.authority_index())
                .unwrap_or(citation);
            let explicit = source_urls.get(&citation.index).cloned();
            let url = explicit.or_else(|| url_stage::url(authority, language)).map(|page| {
                if request.anchor && !source_urls.contains_key(&citation.index) {
                    url_stage::with_pinpoint(&page, citation)
                } else {
                    page
                }
            });
            CitationUrl {
                index: citation.index,
                url,
            }
        })
        .collect();
    Ok(UrlResponse { urls })
}

// ---------------------------------------------------------------- annotate

/// Insert markup into `text`, or into `source` (the markup `text` was cleaned
/// from with `cleanSteps`), preserving the source's tags.
///
/// Either pass explicit `annotations` (spans of `text` in `offsetUnit`, like
/// eyecite's `annotate_citations`), or leave them out and every citation found
/// in `text` is wrapped in `before`/`after`, templates in which `{index}`,
/// `{key}`, `{form}`, `{authority}` and `{url}` are substituted.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct AnnotateRequest {
    pub text: String,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub annotations: Option<Vec<Annotation>>,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
    /// Which extent of each found citation to wrap: `span` (the core,
    /// eyecite's `span()`, default) or `fullSpan`.
    #[serde(default)]
    pub span: Option<String>,
    /// Original markup `text` was cleaned from; the result is that markup,
    /// annotated. Cleaning `source` with `cleanSteps` must give `text`.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub clean_steps: Vec<String>,
    /// Character-count diff steps supplied by a host's stdlib aligner.
    #[serde(default)]
    pub alignment: Option<Vec<clean_stage::DiffStep>>,
    /// Source spans returned by a host-provided offset updater, in input order
    /// and offsetUnit. Ordering still uses the original extraction spans.
    #[serde(default)]
    pub source_offsets: Option<Vec<[usize; 2]>>,
    /// How to treat an annotation whose source range crosses tags unevenly
    /// (eyecite's `unbalanced_tags`): `unchecked` (default), `skip`, `wrap`.
    #[serde(default)]
    pub unbalanced_tags: Option<String>,
    /// Unit of `annotations` offsets.
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct Annotation {
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct AnnotateResponse {
    pub text: String,
}

fn clean_steps(method: &str, names: &[String]) -> Result<Vec<clean_stage::Step>, ApiError> {
    names
        .iter()
        .map(|name| {
            clean_stage::Step::from_eyecite_name(name).ok_or_else(|| {
                ApiError::invalid(format!(
                    "{method}: unknown clean step {name:?}; expected html, xml, inline_whitespace, all_whitespace, underscores or zero_width"
                ))
            })
        })
        .collect()
}

fn template(value: &str, citation: &Citation) -> String {
    let mut output = value.replace("{index}", &citation.index.to_string());
    if output.contains("{key}") {
        output = output.replace("{key}", citation.key.as_deref().unwrap_or_default());
    }
    if output.contains("{form}") {
        output = output.replace("{form}", &serde_value_name(&citation.form));
    }
    if output.contains("{authority}") {
        output = output.replace("{authority}", &serde_value_name(&citation.authority));
    }
    if output.contains("{url}") {
        let url = url_stage::url(citation, Language::En).unwrap_or_default();
        output = output.replace("{url}", &url);
    }
    output
}

fn serde_value_name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}

fn planned_annotations(request: &AnnotateRequest) -> Result<Vec<annotate_stage::PreparedAnnotation>, ApiError> {
    let unbalanced = match request.unbalanced_tags.as_deref() {
        None | Some("unchecked") => annotate_stage::Unbalanced::Unchecked,
        Some("skip") => annotate_stage::Unbalanced::Skip,
        Some("wrap") => annotate_stage::Unbalanced::Wrap,
        Some(other) => {
            return Err(ApiError::invalid(format!(
                "annotate: unbalancedTags must be unchecked, skip or wrap, got {other:?}"
            )))
        }
    };
    let extent = match request.span.as_deref() {
        None | Some("span") => annotate_stage::Extent::Core,
        Some("fullSpan") => annotate_stage::Extent::Full,
        Some(other) => {
            return Err(ApiError::invalid(format!(
                "annotate: span must be span or fullSpan, got {other:?}"
            )))
        }
    };
    let document = ScalarText::new(&request.text);
    let annotations = match &request.annotations {
        Some(annotations) => {
            annotations
                .iter()
                .map(|annotation| {
                    let start = to_byte(
                        &document,
                        annotation.start,
                        request.offset_unit,
                        "annotations[].start",
                    )?;
                    let end = to_byte(
                        &document,
                        annotation.end,
                        request.offset_unit,
                        "annotations[].end",
                    )?;
                    if start > end {
                        return Err(ApiError::new(
                            ErrorCode::InvalidOffset,
                            "annotation start is after its end",
                        ));
                    }
                    Ok(annotate_stage::Annotation::new(
                        start,
                        end,
                        annotation.before.clone(),
                        annotation.after.clone(),
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let options = byte_options(&document, &request.options, request.offset_unit)?;
            let citations = crate::extract(&request.text, &options);
            annotate_stage::citation_annotations(&citations, extent, |citation| {
                Some((
                    template(&request.before, citation),
                    template(&request.after, citation),
                ))
            })
        }
    };
    let source = request.source.as_deref().unwrap_or(&request.text);
    if let Some(offsets) = &request.source_offsets {
        if request.alignment.is_some() || !request.clean_steps.is_empty() {
            return Err(ApiError::invalid("annotate: sourceOffsets cannot be combined with alignment or cleanSteps"));
        }
        if offsets.len() != annotations.len() {
            return Err(ApiError::invalid("annotate: sourceOffsets must have one span per annotation"));
        }
        let document = ScalarText::new(source);
        let mapped = offsets.iter().map(|[start, end]| Ok(
            to_byte(&document, *start, request.offset_unit, "sourceOffsets[].start")?..
            to_byte(&document, *end, request.offset_unit, "sourceOffsets[].end")?
        )).collect::<Result<Vec<_>, ApiError>>()?;
        return Ok(annotate_stage::prepare(source, &annotations, unbalanced, |index, _| Some(mapped[index].clone())));
    }
    if request.alignment.is_some() && !request.clean_steps.is_empty() {
        return Err(ApiError::invalid("annotate: alignment and cleanSteps are mutually exclusive"));
    }
    let cleaned = if source == request.text && request.alignment.is_none() {
        clean_stage::Cleaned::identity(source)
    } else if request.clean_steps.is_empty() {
        let map = clean_stage::SpanUpdater::new(&request.text, source,
            &clean_stage::placeholder_markup(source), request.alignment.as_deref()).map_err(ApiError::invalid)?;
        return Ok(annotate_stage::prepare(source, &annotations, unbalanced, |_, annotation|
            Some(map.byte(annotation.start, true)?..map.byte(annotation.end, false)?)));
    } else {
        let cleaned = clean_stage::clean(source, &clean_steps("annotate", &request.clean_steps)?);
        if cleaned.text != request.text {
            return Err(ApiError::invalid("annotate: cleaning source with cleanSteps does not produce text"));
        }
        cleaned
    };
    Ok(annotate_stage::source_annotations(source, &cleaned, &annotations, unbalanced))
}

pub fn annotate(request: &AnnotateRequest) -> Result<AnnotateResponse, ApiError> {
    Ok(AnnotateResponse { text: annotate_stage::render(
        request.source.as_deref().unwrap_or(&request.text), &planned_annotations(request)?) })
}

pub fn annotation_ranges(request: &AnnotateRequest) -> Result<Vec<annotate_stage::PreparedAnnotation>, ApiError> {
    let annotations = planned_annotations(request)?;
    let document = ScalarText::new(request.source.as_deref().unwrap_or(&request.text));
    Ok(annotations.into_iter().map(|mut annotation| {
        annotation.start = from_byte(&document, annotation.start, request.offset_unit);
        annotation.end = from_byte(&document, annotation.end, request.offset_unit);
        annotation
    }).collect())
}

// ---------------------------------------------------------------- clean

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CleanRequest {
    pub text: String,
    /// eyecite-compatible step names: `html`, `xml`, `inline_whitespace`,
    /// `all_whitespace`, `underscores`, plus `zero_width`; applied in order.
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct CleanResponse {
    pub text: String,
}

pub fn clean(request: &CleanRequest) -> Result<CleanResponse, ApiError> {
    let steps = clean_steps("clean", &request.steps)?;
    Ok(CleanResponse {
        text: clean_stage::clean(&request.text, &steps).text,
    })
}

// ---------------------------------------------------------------- registry

/// `{}` returns the whole registry; `{table}` one table (`jurisdictions`,
/// `courts`, `reporters`, `series`, `journals`); `{table, surface}` the
/// entries of that table a surface form names, preferred first
/// (`{"table": "courts", "surface": "FCA"}` → Canadian `fca`, then `fca-au`;
/// `{"table": "reporters", "surface": "S.C.R."}` →
/// `[{"reporter": {...}, "canonical": "SCR"}]`).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct RegistryRequest {
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
}

/// The table names [`registry`] accepts, read from the serialized registry so
/// a new table is picked up without touching this file.
pub fn registry_tables() -> Vec<String> {
    match serde_json::to_value(registry_stage::Registry::default()) {
        Ok(Value::Object(tables)) => tables.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn unknown_table(table: &str) -> ApiError {
    ApiError::invalid(format!(
        "registry: unknown table {table:?}; expected one of {}",
        registry_tables().join(", ")
    ))
}

fn to_json<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("registry serializes")
}

/// The embedded registry as JSON: whole, one table, or a surface lookup.
pub fn registry(request: &RegistryRequest) -> Result<Value, ApiError> {
    let registry = registry_stage::registry();
    let Some(table) = request.table.as_deref() else {
        if request.surface.is_some() {
            return Err(ApiError::invalid("registry: `surface` needs a `table`"));
        }
        return Ok(to_json(registry));
    };
    let Some(surface) = request.surface.as_deref() else {
        return to_json(registry)
            .get(table)
            .cloned()
            .ok_or_else(|| unknown_table(table));
    };
    Ok(match table {
        "courts" => to_json(registry.courts_by_surface(surface)),
        "reporters" => Value::Array(
            registry
                .reporters_by_surface(surface)
                .into_iter()
                .map(|(reporter, canonical)| serde_json::json!({"reporter": reporter, "canonical": canonical}))
                .collect(),
        ),
        "series" => to_json(registry.series_candidates(surface)),
        "journals" => to_json(registry.journals_by_surface(surface)),
        "jurisdictions" => to_json(registry.jurisdiction(surface).into_iter().collect::<Vec<_>>()),
        other if registry_tables().iter().any(|name| name == other) => {
            return Err(ApiError::invalid(format!("registry: table {other:?} has no surface lookup")))
        }
        other => return Err(unknown_table(other)),
    })
}

// ---------------------------------------------------------------- misc

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ExcerptRequest {
    pub excerpt: String,
}

/// [`excerpt::classify_citator_excerpt`]. `cite_char_coverage` and the
/// `prose_window` bounds are measured in UTF-16 code units, as in the source
/// implementation.
pub fn classify_excerpt(request: &ExcerptRequest) -> excerpt::ExcerptClassification {
    excerpt::classify_citator_excerpt(&request.excerpt)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct HasCitationRequest {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct HasCitationResponse {
    pub has_citation: bool,
}

pub fn has_citation(request: &HasCitationRequest) -> HasCitationResponse {
    HasCitationResponse {
        has_citation: crate::has_citation(&request.text),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct ProtectedSpansRequest {
    pub text: String,
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

pub fn protected_citation_spans(request: &ProtectedSpansRequest) -> Vec<[usize; 2]> {
    let text = ScalarText::new(&request.text);
    crate::cues::protected_spans(&request.text).into_iter()
        .map(|span| [from_byte(&text, span.start, request.offset_unit),
            from_byte(&text, span.end, request.offset_unit)]).collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct GrammarVersion {
    /// `legal-grammar-corpus:v1`.
    pub format: String,
    /// Number of grammar entries across all tables.
    pub entries: usize,
    /// SHA-256 of the embedded `grammar-corpus.json`.
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct RegistryVersion {
    pub jurisdictions: usize,
    pub courts: usize,
    pub reporters: usize,
    pub series: usize,
    pub journals: usize,
    /// Reporter and journal entries by data source (`mcgill`, `reporters-db`, ...).
    pub sources: std::collections::BTreeMap<String, usize>,
    /// Upstream data versions (`reporters-db`, `courts-db`) the generated
    /// entries were built from, as pinned in `upstream.lock`.
    pub upstream: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "binding-types", derive(ts_rs::TS))]
pub struct VersionResponse {
    /// The `legal-citations` crate version.
    pub version: String,
    pub schema_version: u32,
    pub key_version: String,
    pub grammar: GrammarVersion,
    pub registry: RegistryVersion,
}

pub fn version() -> VersionResponse {
    let registry = registry_stage::registry();
    let mut sources = std::collections::BTreeMap::new();
    for source in registry
        .reporters
        .iter()
        .map(|reporter| &reporter.source)
        .chain(registry.journals.iter().map(|journal| &journal.source))
    {
        *sources.entry(source.clone()).or_insert(0) += 1;
    }
    VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: SCHEMA_VERSION,
        key_version: key_stage::KEY_VERSION.to_owned(),
        grammar: GrammarVersion {
            format: legal_grammar::GRAMMAR_CORPUS_FORMAT.to_owned(),
            entries: legal_grammar::load_tables().map_or(0, |tables| tables.len()),
            sha256: {
                use sha2::{Digest, Sha256};
                Sha256::digest(legal_grammar::GRAMMAR_CORPUS_JSON.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            },
        },
        registry: RegistryVersion {
            jurisdictions: registry.jurisdictions.len(),
            courts: registry.courts.len(),
            reporters: registry.reporters.len(),
            series: registry.series.len(),
            journals: registry.journals.len(),
            sources,
            upstream: serde_json::from_str(include_str!("../registry/upstream/pins.json"))
                .expect("registry/upstream/pins.json"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_method_and_option_are_errors() {
        assert_eq!(
            call("nope", "{}").unwrap_err().code,
            ErrorCode::UnknownMethod
        );
        let error = call("extract", r#"{"text":"x","options":{"resolv":true}}"#).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest);
        assert!(error.message.contains("resolv"));
    }

    #[test]
    fn extract_envelope() {
        let response: Value = serde_json::from_str(
            &call("extract", r#"{"text":"nothing here","offsetUnit":"utf16"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(response["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(response["offsetUnit"], "utf16");
        assert!(response["citations"].is_array());
    }

    #[test]
    fn note_offsets_are_validated() {
        let error = call(
            "extract",
            r#"{"text":"é","offsetUnit":"byte","options":{"notes":[{"number":1,"start":1,"end":2}]}}"#,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidOffset);
    }

    #[test]
    fn pinpoint_formatting_and_clean() {
        let response: Value = serde_json::from_str(
            &call(
                "format",
                r#"{"pinpoint":{"kind":"paragraph","locators":[{"first":"12","last":"14"}]}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(response["pinpoint"], "at paras 12-14");
        let response: Value = serde_json::from_str(
            &call(
                "clean",
                r#"{"text":"a  <i>b</i>","steps":["html","all_whitespace"]}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(response["text"], "a b");
        assert_eq!(
            call("clean", r#"{"text":"x","steps":["bogus"]}"#)
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }

    #[test]
    fn explicit_annotations_use_the_offset_unit() {
        let request = serde_json::json!({
            "text": "é 1 U.S. 1",
            "annotations": [{"start": 2, "end": 10, "before": "<a>", "after": "</a>"}],
            "offsetUnit": "char",
        });
        let response = call_value("annotate", request).unwrap();
        assert_eq!(response["text"], "é <a>1 U.S. 1</a>");
    }

    #[test]
    fn registry_tables_and_errors() {
        assert!(registry_tables().iter().any(|table| table == "journals"));
        let error = call("registry", r#"{"table":"nope"}"#).unwrap_err();
        assert!(error.message.contains("journals"));
        assert!(call("registry", r#"{"table":"courts","surface":"SCC"}"#).is_ok());
    }

    #[test]
    fn version_reports_schema() {
        let response: Value = serde_json::from_str(&call("version", "").unwrap()).unwrap();
        assert_eq!(response["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(response["version"], env!("CARGO_PKG_VERSION"));
    }
}
