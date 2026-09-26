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
//! | `extract`         | `{text, options?, offsetUnit?}`                                | `{schemaVersion, offsetUnit, citations}`               |
//! | `key`             | `{citation}`                                                   | `{keyVersion, key}`                                    |
//! | `keyForText`      | `{text, options?}`                                             | `{keyVersion, key, reason?, message?, keys}`           |
//! | `format`          | `{citation}` / `{text, options?}` / `{pinpoint}`, `language?`, `rangeDash?`, `style?` | `{citations: [{index, formatted}]}` / `{pinpoint}` |
//! | `url`             | `{citation}` / `{text, options?}`, `language?`, `anchor?`     | `{urls: [{index, url}]}`                               |
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
pub const SCHEMA_VERSION: u32 = 1;

/// Every method [`call`] accepts.
pub const METHODS: &[&str] = &[
    "extract",
    "key",
    "keyForText",
    "format",
    "url",
    "annotate",
    "clean",
    "registry",
    "classifyExcerpt",
    "hasCitation",
    "version",
];

/// Why a call failed. Serialized as `{"code": ..., "message": ...}`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
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
        "key" => to_value(key(&parse(method, request)?)),
        "keyForText" => to_value(key_for_text(&parse(method, request)?)),
        "format" => to_value(format(&parse(method, request)?)?),
        "url" => to_value(url(&parse(method, request)?)?),
        "annotate" => to_value(annotate(&parse(method, request)?)?),
        "clean" => to_value(clean(&parse(method, request)?)?),
        "registry" => registry(&parse(method, request)?),
        "classifyExcerpt" => to_value(classify_excerpt(&parse(method, request)?)),
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

fn parse<T: for<'de> Deserialize<'de>>(method: &str, request: Value) -> Result<T, ApiError> {
    if !request.is_object() {
        return Err(ApiError::invalid(format!(
            "{method}: request must be a JSON object"
        )));
    }
    reject_unknown_options(method, &request)?;
    serde_json::from_value(request).map_err(|error| ApiError::invalid(format!("{method}: {error}")))
}

/// [`Options`] tolerates unknown keys; the API does not, so a misspelt option
/// is an error rather than a silent default. The allowed keys are read from the
/// serialized default, so they never drift from the struct.
fn reject_unknown_options(method: &str, request: &Value) -> Result<(), ApiError> {
    let Some(Value::Object(options)) = request.get("options") else {
        return Ok(());
    };
    let Value::Object(known) = serde_json::to_value(Options::default()).expect("options serialize")
    else {
        unreachable!("options serialize to an object")
    };
    for name in options.keys() {
        if !known.contains_key(name) {
            let mut allowed = known.keys().map(String::as_str).collect::<Vec<_>>();
            allowed.sort_unstable();
            return Err(ApiError::invalid(format!(
                "{method}: unknown option {name:?}; expected one of {}",
                allowed.join(", ")
            )));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

// ---------------------------------------------------------------- extract

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractRequest {
    pub text: String,
    #[serde(default)]
    pub options: Options,
    /// Unit of every offset in the response, and of `options.notes` ranges.
    #[serde(default)]
    pub offset_unit: OffsetUnit,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResponse {
    pub schema_version: u32,
    pub offset_unit: OffsetUnit,
    pub citations: Vec<Citation>,
}

/// [`crate::extract`] with offsets converted to `request.offset_unit`.
pub fn extract(request: &ExtractRequest) -> Result<ExtractResponse, ApiError> {
    let citations = extract_citations(&request.text, &request.options, request.offset_unit)?;
    Ok(ExtractResponse {
        schema_version: SCHEMA_VERSION,
        offset_unit: request.offset_unit,
        citations,
    })
}

fn extract_citations(
    text: &str,
    options: &Options,
    unit: OffsetUnit,
) -> Result<Vec<Citation>, ApiError> {
    let document = ScalarText::new(text);
    let mut options = options.clone();
    if let Some(notes) = options.notes.as_mut() {
        for note in notes.iter_mut() {
            note.start = to_byte(&document, note.start, unit, "options.notes[].start")?;
            note.end = to_byte(&document, note.end, unit, "options.notes[].end")?;
            if note.start > note.end {
                return Err(ApiError::new(
                    ErrorCode::InvalidOffset,
                    format!("note {}: start is after end", note.number),
                ));
            }
        }
    }
    let mut citations = crate::extract(text, &options);
    convert_citations(&document, &mut citations, unit);
    Ok(citations)
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
    let convert = |byte: usize| -> usize {
        match unit {
            OffsetUnit::Byte => Some(byte),
            OffsetUnit::Char => document.scalar_at_byte(byte),
            OffsetUnit::Utf16 => document.utf16_at_byte(byte),
        }
        .expect("engine offsets are character boundaries")
    };
    let convert_span = |span: &mut Span| {
        span.start = convert(span.start);
        span.end = convert(span.end);
    };
    for citation in citations {
        convert_span(&mut citation.span);
        convert_span(&mut citation.full_span);
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

// ---------------------------------------------------------------- key

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRequest {
    pub citation: Citation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub struct TextRequest {
    pub text: String,
    #[serde(default)]
    pub options: Options,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CitationKey {
    pub index: usize,
    /// The citation core as written.
    pub text: String,
    pub key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub fn key_for_text(request: &TextRequest) -> KeyForTextResponse {
    let (key, reason, message) = match key_stage::key_for_text(&request.text) {
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
    let citations = crate::extract(&request.text, &request.options);
    let keys = citations
        .iter()
        .map(|citation| CitationKey {
            index: citation.index,
            text: citation.span.text.clone(),
            key: citation
                .key
                .clone()
                .or_else(|| citations.get(citation.authority_index())?.key.clone()),
        })
        .collect();
    KeyForTextResponse {
        key_version: key_stage::KEY_VERSION.to_owned(),
        key,
        reason,
        message,
        keys,
    }
}

// ---------------------------------------------------------------- format

/// Format citations (`citation` or every citation in `text`) or a bare
/// pinpoint, in McGill style.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormatRequest {
    #[serde(default)]
    pub citation: Option<Citation>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub pinpoint: Option<PinpointRequest>,
    #[serde(default)]
    pub options: Options,
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

/// `{kind: "paragraph", locators: [{first: "12", last: "14"}]}` →
/// `at paras 12-14`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PinpointRequest {
    pub kind: PinpointKind,
    pub locators: Vec<Locator>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Locator {
    pub first: String,
    #[serde(default)]
    pub last: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Formatted {
    pub index: usize,
    /// `null` for forms the formatter does not render (short forms,
    /// references, unknown).
    pub formatted: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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
    let language = request
        .language
        .as_deref()
        .map_or(Language::En, Language::from_code);
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

fn request_citations(
    method: &str,
    citation: &Option<Citation>,
    text: &Option<String>,
    options: &Options,
) -> Result<Vec<Citation>, ApiError> {
    match (citation, text) {
        (Some(citation), None) => Ok(vec![citation.clone()]),
        (None, Some(text)) => Ok(crate::extract(text, options)),
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
    let citations =
        request_citations("format", &request.citation, &request.text, &request.options)?;
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
pub struct UrlRequest {
    #[serde(default)]
    pub citation: Option<Citation>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub options: Options,
    /// `en` (default) or `fr`: the language of the page to link.
    #[serde(default)]
    pub language: Option<String>,
    /// Append the anchor of a single pinpoint (`#par12`) when the source has one.
    #[serde(default)]
    pub anchor: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CitationUrl {
    pub index: usize,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UrlResponse {
    pub urls: Vec<CitationUrl>,
}

/// The public-source URL of each citation (CanLII, Justice Laws, Find Case
/// Law, legislation.gov.uk, CourtListener). A back reference gets its
/// antecedent's URL; `null` when no source is certain.
pub fn url(request: &UrlRequest) -> Result<UrlResponse, ApiError> {
    let citations = request_citations("url", &request.citation, &request.text, &request.options)?;
    let language = request
        .language
        .as_deref()
        .map_or(Language::En, Language::from_code);
    let urls = citations
        .iter()
        .map(|citation| {
            let authority = citations
                .iter()
                .find(|candidate| candidate.index == citation.authority_index())
                .unwrap_or(citation);
            let url = url_stage::url(authority, language).map(|page| {
                if request.anchor {
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
pub struct Annotation {
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnnotateResponse {
    pub text: String,
}

fn clean_steps(method: &str, names: &[String]) -> Result<Vec<clean_stage::Step>, ApiError> {
    names
        .iter()
        .map(|name| {
            clean_stage::Step::from_name(name).ok_or_else(|| {
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

pub fn annotate(request: &AnnotateRequest) -> Result<AnnotateResponse, ApiError> {
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
    let annotations = match &request.annotations {
        Some(annotations) => {
            let document = ScalarText::new(&request.text);
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
            let citations = crate::extract(&request.text, &request.options);
            annotate_stage::citation_annotations(&citations, extent, |citation| {
                Some((
                    template(&request.before, citation),
                    template(&request.after, citation),
                ))
            })
        }
    };
    let text = match &request.source {
        None => annotate_stage::annotate(&request.text, &annotations),
        Some(source) => {
            let cleaned =
                clean_stage::clean(source, &clean_steps("annotate", &request.clean_steps)?);
            if cleaned.text != request.text {
                return Err(ApiError::invalid(
                    "annotate: cleaning `source` with `cleanSteps` does not produce `text`",
                ));
            }
            annotate_stage::annotate_source(source, &cleaned, &annotations, unbalanced)
        }
    };
    Ok(AnnotateResponse { text })
}

// ---------------------------------------------------------------- clean

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanRequest {
    pub text: String,
    /// eyecite-compatible step names: `html` (= `xml`), `inline_whitespace`,
    /// `all_whitespace`, `underscores`, plus `zero_width`; applied in order.
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        "series" => to_json(registry.series_by_surface(surface).into_iter().collect::<Vec<_>>()),
        "journals" => to_json(registry.journal_by_surface(surface).into_iter().collect::<Vec<_>>()),
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
pub struct HasCitationRequest {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HasCitationResponse {
    pub has_citation: bool,
}

pub fn has_citation(request: &HasCitationRequest) -> HasCitationResponse {
    HasCitationResponse {
        has_citation: crate::has_citation(&request.text),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
