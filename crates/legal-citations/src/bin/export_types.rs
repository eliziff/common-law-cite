//! Package-time declarations from the Rust wire types; never part of runtime builds.
use legal_citations::api::*;
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var(
        "TS_RS_EXPORT_DIR",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bindings"),
    );
    let config = Config::default();
    macro_rules! export { ($($type:ty),+ $(,)?) => { $(<$type>::export_all(&config)?;)+ }; }
    export!(
        ExtractRequest,
        ExtractResponse,
        ResolveRequest,
        ResolveResponse,
        legal_citations::resolve::ReferenceRequest,
        InferShortFormsRequest,
        InferredReferenceRequest,
        RegistryReferenceRequest,
        SupraHintRequest,
        ReanchorRequest,
        SplitSourcesRequest,
        SourceFieldsRequest,
        legal_citations::source::SourceSplit,
        legal_citations::source::SourceFields,
        legal_citations::short_forms::InferredShortForm,
        legal_citations::short_forms::ReferenceInfo,
        legal_citations::format::CitationCorrection,
        legal_citations::format::CorrectedCitation,
        KeyRequest,
        KeyResponse,
        TextRequest,
        KeyForTextResponse,
        FormatRequest,
        FormatResponse,
        legal_citations::format::Article,
        legal_citations::format::DocumentCitation,
        legal_citations::format::CaseCitationRequest,
        legal_citations::format::FormattedDocument,
        legal_citations::format::CaseHeading,
        PinpointLayoutsRequest,
        legal_citations::format::PinpointLayout,
        UrlRequest,
        UrlResponse,
        AliasTargetRequest,
        legal_citations::url::AliasTargetInfo,
        AnnotateRequest,
        AnnotateResponse,
        legal_citations::annotate::PreparedAnnotation,
        CleanRequest,
        CleanResponse,
        RegistryRequest,
        ExcerptRequest,
        HasCitationRequest,
        HasCitationResponse,
        ProtectedSpansRequest,
        ReporterHeaderRequest,
        VersionResponse,
        ApiError,
        legal_citations::excerpt::ExcerptClassification,
        legal_citations::url::LegislationLookup
    );
    for entry in std::fs::read_dir(config.out_dir())? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "ts")
            && !path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".d.ts")
        {
            std::fs::rename(&path, path.with_extension("d.ts"))?;
        }
    }
    Ok(())
}
