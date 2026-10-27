//! Hand-built citations and a small registry for the downstream stage tests
//! (parallel, resolve, key, format, url), independent of find/classify.
#![allow(dead_code)]

use legal_citations::registry::Registry;
use serde_json::{json, Value};
use legal_citations::{
    Authority, Citation, CourtRef, Fields, Form, Format, Parties, Pinpoint, PinpointKind, Span,
};
use std::sync::LazyLock;

pub fn span(text: &str, start: usize) -> Span {
    Span {
        start,
        end: start + text.len(),
        text: text.to_owned(),
    }
}

/// Offset of the `occurrence`th (0-based) `needle` in `text`.
pub fn at(text: &str, needle: &str, occurrence: usize) -> usize {
    text.match_indices(needle)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("{needle:?} #{occurrence} not in {text:?}"))
        .0
}

pub struct B(pub Citation);

pub fn cite(index: usize, form: Form, authority: Authority, core: &str, start: usize) -> B {
    B(Citation {
        index,
        form,
        authority,
        format: None,
        span: span(core, start),
        signal: None,
        full_span: span(core, start),
        style: None,
        parties: None,
        fields: Fields::default(),
        court: None,
        jurisdiction: None,
        language: None,
        pinpoints: Vec::new(),
        parentheticals: Vec::new(),
        history: Vec::new(),
        short_name: None,
        explicit_short_name: None,
        parallel_group: None,
        alias: None,
        interpretations: Vec::new(),
        antecedent: None,
        key: None,
        reasons: Vec::new(),
    })
}

pub fn case(index: usize, core: &str, start: usize) -> B {
    cite(index, Form::Full, Authority::Case, core, start)
}

impl B {
    pub fn format(mut self, format: Format) -> Self {
        self.0.format = Some(format);
        self
    }
    pub fn authority(mut self, authority: Authority) -> Self {
        self.0.authority = authority;
        self
    }
    pub fn fields(mut self, edit: impl FnOnce(&mut Fields)) -> Self {
        edit(&mut self.0.fields);
        self
    }
    pub fn neutral(self, year: &str, series: &str, number: &str) -> Self {
        self.format(Format::Neutral).fields(|fields| {
            fields.year = Some(year.into());
            fields.series = Some(series.into());
            fields.number = Some(number.into());
        })
    }
    pub fn reporter(self, year: Option<&str>, volume: Option<&str>, reporter: &str, page: &str) -> Self {
        self.format(Format::Reporter).fields(|fields| {
            fields.year = year.map(Into::into);
            fields.volume = volume.map(Into::into);
            fields.reporter = Some(reporter.into());
            fields.page = Some(page.into());
        })
    }
    pub fn court(mut self, id: &str) -> Self {
        self.0.court = Some(CourtRef {
            id: id.into(),
            text: id.to_uppercase(),
        });
        self
    }
    pub fn jurisdiction(mut self, id: &str) -> Self {
        self.0.jurisdiction = Some(id.into());
        self
    }
    /// A style of cause immediately in front of the core, separated by `, `.
    pub fn style(mut self, style: &str) -> Self {
        let start = self.0.span.start.saturating_sub(style.len() + 2);
        self.0.style = Some(span(style, start));
        self.0.full_span = Span {
            start,
            end: self.0.full_span.end,
            text: format!("{style}, {}", self.0.full_span.text),
        };
        self.0.short_name = Some(style.into());
        self
    }
    pub fn parties(mut self, plaintiff: &str, defendant: &str) -> Self {
        self.0.parties = Some(Parties {
            plaintiff: Some(plaintiff.into()),
            defendant: Some(defendant.into()),
        });
        self
    }
    pub fn short(mut self, name: &str) -> Self {
        self.0.short_name = Some(name.into());
        self
    }
    pub fn explicit(mut self, name: &str) -> Self {
        self.0.explicit_short_name = Some(name.into());
        self
    }
    pub fn note(self, note: u32) -> Self {
        self.fields(|fields| fields.note = Some(note))
    }
    pub fn pin(mut self, kind: PinpointKind, first: &str, last: Option<&str>) -> Self {
        self.0.pinpoints.push(Pinpoint {
            kind,
            span: span(first, 0),
            first: first.into(),
            last: last.map(Into::into),
        });
        self
    }
    /// Extend the full span to `end` (pinpoints, parentheticals).
    pub fn full_to(mut self, text: &str, end: usize) -> Self {
        self.0.full_span = span(&text[self.0.full_span.start..end], self.0.full_span.start);
        self
    }
    pub fn language(mut self, language: &str) -> Self {
        self.0.language = Some(language.into());
        self
    }
    pub fn build(self) -> Citation {
        self.0
    }
}

pub fn ibid(index: usize, start: usize) -> Citation {
    cite(index, Form::Ibid, Authority::Unknown, "Ibid", start).build()
}

pub fn supra(index: usize, name: Option<&str>, note: Option<u32>, start: usize) -> Citation {
    let text = match note {
        Some(note) => format!("supra note {note}"),
        None => "supra".to_owned(),
    };
    let mut citation = cite(index, Form::Supra, Authority::Unknown, &text, start).build();
    citation.fields.note = note;
    citation.short_name = name.map(Into::into);
    citation
}

/// A small registry in the embedded JSON format, so the tests keep compiling
/// while the registry schema grows optional fields.
pub static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let route = |jurisdiction: &str, database: &str| json!({"jurisdiction": jurisdiction, "database": database});
    let court = |id: &str, jurisdiction: &str, level: &str, neutral: &[&str], aliases: &[&str], canlii: Option<Value>, canlii_fr: Option<Value>| {
        let mut value = json!({
            "id": id, "name": {"en": id.to_uppercase()}, "jurisdiction": jurisdiction, "level": level,
            "neutral": neutral, "aliases": aliases,
        });
        if let Some(canlii) = canlii {
            value["canlii"] = canlii;
        }
        if let Some(canlii_fr) = canlii_fr {
            value["canlii_fr"] = canlii_fr;
        }
        value
    };
    let reporter = |id: &str, kind: &str, jurisdiction: &str, editions: &[&str], variations: &[(&str, &str)], year_volume: bool, source: &str| {
        json!({
            "id": id, "name": {"en": id}, "kind": kind, "jurisdiction": jurisdiction,
            "editions": editions.iter().map(|abbreviation| json!({"abbreviation": abbreviation})).collect::<Vec<_>>(),
            "variations": variations.iter().map(|(written, canonical)| ((*written).to_owned(), json!(canonical))).collect::<serde_json::Map<_, _>>(),
            "year_volume": year_volume, "source": source,
        })
    };
    let series = |id: &str, kind: &str, jurisdiction: &str, abbreviation: &str, variations: &[&str], canlii: Option<Value>| {
        let mut value = json!({
            "id": id, "name": {"en": id}, "kind": kind, "jurisdiction": jurisdiction,
            "abbreviation": abbreviation, "variations": variations,
        });
        if let Some(canlii) = canlii {
            value["canlii"] = canlii;
        }
        value
    };
    let courts = vec![
        court("scc", "ca", "apex", &["SCC", "CSC"], &["S.C.C."], Some(route("ca", "scc")), Some(route("ca", "csc"))),
        court("fca", "ca", "appellate", &["FCA", "CAF"], &[], Some(route("ca", "fca")), Some(route("ca", "caf"))),
        court("fc", "ca", "superior_trial", &["FC", "CF", "FCT", "CFPI"], &[], Some(route("ca", "fct")), Some(route("ca", "cf"))),
        court("tcc", "ca", "superior_trial", &["TCC", "CCI"], &[], Some(route("ca", "tcc")), None),
        court("onca", "ca-on", "appellate", &["ONCA"], &["ON CA", "Ont CA", "CA Ont"], Some(route("on", "onca")), None),
        court("onsc", "ca-on", "superior_trial", &["ONSC"], &["ON SC"], Some(route("on", "onsc")), None),
        court("hrto", "ca-on", "tribunal", &["HRTO"], &[], Some(route("on", "onhrt")), None),
        court("nbqb", "ca-nb", "superior_trial", &["NBQB", "NBBR"], &[], Some(route("nb", "nbqb")), Some(route("nb", "NBQB"))),
        court("ukpc", "uk", "apex", &["UKPC"], &["UKJCPC"], Some(route("ca", "ukjcpc")), None),
        court("nwtca", "ca-nt", "appellate", &["NWTCA"], &[], Some(route("nt", "ntca")), None),
        court("cact", "ca", "tribunal", &["Comp Trib", "Trib conc"], &[], Some(route("ca", "cact")), None),
        court("uksc", "uk", "apex", &["UKSC"], &[], None, None),
        court("ewca-civ", "uk-ew", "appellate", &["EWCA Civ"], &[], None, None),
        court("bcca", "ca-bc", "appellate", &["BCCA"], &["BC CA"], None, None),
    ];
    let reporters = vec![
        reporter("scr", "official", "ca", &["SCR"], &[("S.C.R.", "SCR"), ("R.C.S.", "SCR"), ("RCS", "SCR")], true, "authored"),
        reporter("dlr", "general", "ca", &["DLR", "DLR (2d)", "DLR (3d)", "DLR (4th)"], &[("D.L.R. (4th)", "DLR (4th)"), ("D.L.R.", "DLR")], false, "authored"),
        reporter("ccc", "specialty", "ca", &["CCC", "CCC (2d)", "CCC (3d)"], &[("C.C.C. (3d)", "CCC (3d)")], false, "authored"),
        reporter("acws", "digest", "ca", &["ACWS", "ACWS (3d)"], &[], false, "authored"),
        reporter("carswell", "database", "ca", &["CarswellOnt", "CarswellBC"], &[], false, "authored"),
        reporter("oj", "database", "ca-on", &["OJ No"], &[("O.J. No.", "OJ No")], false, "authored"),
        reporter("or", "general", "ca-on", &["OR", "OR (2d)", "OR (3d)"], &[], false, "authored"),
        reporter("ac", "official", "uk", &["AC"], &[("A.C.", "AC")], true, "authored"),
        reporter("us", "official", "us", &["U.S."], &[("US", "U.S."), ("U. S.", "U.S.")], false, "reporters-db"),
        reporter("f", "general", "us", &["F.", "F.2d", "F.3d"], &[("F. 3d", "F.3d"), ("F3d", "F.3d")], false, "reporters-db"),
        reporter("f-supp", "general", "us", &["F. Supp.", "F. Supp. 2d"], &[("F.Supp.2d", "F. Supp. 2d")], false, "reporters-db"),
    ];
    let series = vec![
        series("rsc", "revised_statutes", "ca", "RSC", &["R.S.C.", "LRC", "L.R.C."], Some(route("ca", "cas"))),
        series("sc", "annual_statutes", "ca", "SC", &["S.C.", "LC", "L.C."], Some(route("ca", "caa"))),
        series("rso", "revised_statutes", "ca-on", "RSO", &["R.S.O.", "LRO"], Some(route("on", "ons"))),
        series("so", "annual_statutes", "ca-on", "SO", &["S.O.", "LO"], None),
        series("sor", "regulations", "ca", "SOR", &["DORS", "S.O.R."], Some(route("ca", "car"))),
        series("si", "regulations", "ca", "SI", &["TR", "S.I."], None),
        series("crc", "regulations", "ca", "CRC", &["C.R.C."], Some(route("ca", "car"))),
        series("oreg", "regulations", "ca-on", "O Reg", &["O. Reg.", "Ont Reg", "Règl de l'Ont"], Some(route("on", "onr"))),
        series("rro", "regulations", "ca-on", "RRO", &["R.R.O."], None),
        series("cqlr", "revised_statutes", "ca-qc", "CQLR", &["RLRQ"], Some(route("qc", "qcs"))),
        series("usc", "code", "us", "USC", &["U.S.C."], None),
    ];
    let journals = vec![json!({
        "id": "mcgill-lj", "abbreviation": "McGill LJ", "variations": ["McGill L.J.", "RD McGill"],
        "jurisdiction": "ca", "source": "mcgill",
    })];
    serde_json::from_value(json!({
        "jurisdictions": [], "courts": courts, "reporters": reporters, "series": series, "journals": journals,
    }))
    .expect("test registry")
});

pub fn registry() -> &'static Registry {
    &REGISTRY
}
