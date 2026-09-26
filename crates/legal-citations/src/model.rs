//! The typed citation model every consumer reads.
//!
//! A [`Citation`] answers two independent questions, mirroring eyecite's class
//! hierarchy without inheriting its US-only assumptions:
//!
//! * [`Form`]: how the text refers to the authority (a full citation, a short
//!   form, `supra`, `ibid`/`Id.`, a bare case-name reference).
//! * [`Authority`]: what kind of authority it is (a case, a statute, a
//!   regulation, a journal article, a book, a parliamentary paper, ...).
//!
//! Offsets are byte offsets into the text passed to [`crate::extract`]; the
//! JSON surface converts them to the unit a caller asks for (see
//! [`crate::OffsetUnit`]).

use serde::{Deserialize, Serialize};

/// How a citation refers to its authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Form {
    /// A complete first reference: `R v Jordan, 2016 SCC 27`.
    Full,
    /// A reporter short form: `123 F.3d at 456`, `Jordan at para 12` is a
    /// [`Form::Reference`].
    Short,
    /// `Jordan, supra note 4`, `Jordan, above n 4`, `Jordan (n 4)`.
    Supra,
    /// `Ibid`, `Id.`, `Ibid at para 5`.
    Ibid,
    /// A bare case-name reference to an earlier full citation: `Jordan at para 12`.
    Reference,
    /// A section or paragraph reference with no identifiable authority (`§ 1983`),
    /// kept so an `Id.` never attaches to the wrong citation.
    Unknown,
}

/// What kind of authority a citation identifies.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Authority {
    Case,
    Statute,
    Regulation,
    Constitution,
    CourtRule,
    Treaty,
    Bill,
    Debate,
    ParliamentaryPaper,
    GovernmentDocument,
    Journal,
    Book,
    BookChapter,
    Webpage,
    /// The grammar found a citation but could not tell what it cites.
    Unknown,
}

impl Authority {
    /// Legislation of any kind: statutes, regulations, constitutions, rules,
    /// treaties and bills.
    pub fn is_legislation(self) -> bool {
        matches!(
            self,
            Self::Statute
                | Self::Regulation
                | Self::Constitution
                | Self::CourtRule
                | Self::Treaty
                | Self::Bill
        )
    }

    /// Commentary and other secondary material.
    pub fn is_secondary(self) -> bool {
        matches!(
            self,
            Self::Journal
                | Self::Book
                | Self::BookChapter
                | Self::Webpage
                | Self::Debate
                | Self::ParliamentaryPaper
                | Self::GovernmentDocument
        )
    }
}

/// The surface shape of a case citation or legislative citation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Format {
    /// `2016 SCC 27`, `[2019] UKSC 5`, `2020 Comp Trib 6`.
    Neutral,
    /// `[2016] 1 SCR 631`, `(1992) 175 CLR 1`, `410 U.S. 113`.
    Reporter,
    /// `2004 CanLII 12345 (ON CA)`.
    CanLii,
    /// Commercial database identifiers: `2019 CarswellOnt 123`, `[2019] OJ No 45`,
    /// `2019 WL 123456`.
    Database,
    /// Court file or docket numbers.
    Docket,
    /// Revised or annual statute volumes: `RSC 1985, c C-46`, `SO 2006, c 21`.
    StatuteVolume,
    /// Regulation series: `SOR/2002-227`, `O Reg 191/11`, `CRC, c 870`.
    RegulationSeries,
    /// US code-style citations: `42 U.S.C. § 1983`.
    Code,
    /// A publication block: journal volume/page, book imprint, paper number.
    Publication,
    Url,
}

/// A range of the source text.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// What a pinpoint locates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PinpointKind {
    Paragraph,
    Page,
    Section,
    Subsection,
    Rule,
    Article,
    Schedule,
    Footnote,
    Clause,
}

/// One located pinpoint. A range such as `paras 62-64` is one pinpoint with
/// `first = 62` and `last = 64`; a list such as `paras 20, 23 and 25` is three.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pinpoint {
    pub kind: PinpointKind,
    pub span: Span,
    /// The first locator of the range, normalized (`7(2)`, `12`, `xii`).
    pub first: String,
    /// The last locator of a range; `None` for a single locator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// What a trailing parenthetical carries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ParentheticalKind {
    /// `(ON CA)`, `(HL)`, `(2d Cir. 1999)`: court and/or jurisdiction, possibly with a date.
    Court,
    /// `(1998)`: a date only.
    Date,
    /// `(holding that ...)`, `(Abella J, dissenting)`.
    Explanatory,
    /// `(QL)`, `(WL Can)`, `(CanLII)`: the database the citation comes from.
    Source,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Parenthetical {
    pub kind: ParentheticalKind,
    pub span: Span,
    /// The text inside the parentheses.
    pub content: String,
}

/// A subsequent- or prior-history relation: `aff'd`, `rev'd`, `leave to appeal
/// to SCC refused`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct History {
    /// Normalized relation: `affirmed`, `reversed`, `leave_refused`,
    /// `leave_granted`, `varied`, `overruled`, `appeal_dismissed`, ...
    pub relation: String,
    pub span: Span,
    /// Index of the citation the relation points to, when it was extracted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<usize>,
}

/// Which way a note cross-reference points.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NoteDirection {
    /// `supra note 4`, `above n 4`, `(n 4)`, `op. cit. note 4`, `note 4 ci-dessus`.
    Back,
    /// `infra note 12`, `below n 12`, `note 12 ci-dessous`.
    Forward,
    /// `see footnote 7`, `see also note 7`.
    Unspecified,
}

/// A reference to another footnote by number (see [`crate::find::note_references`]).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NoteReference {
    pub span: Span,
    pub note: u32,
    pub direction: NoteDirection,
}

/// A court resolved against the registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CourtRef {
    /// Registry id, e.g. `scc`, `onca`, `ewca-civ`, `ca9`.
    pub id: String,
    /// The surface text the court was read from.
    pub text: String,
}

/// Parsed components. Every field is optional; which fields are filled depends
/// on [`Authority`] and [`Format`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
    /// The reporter, journal or series abbreviation as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporter: Option<String>,
    /// The registry's canonical abbreviation for [`Fields::reporter`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporter_canonical: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Neutral-citation or database decision number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    /// Neutral court identifier or statute/regulation series as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regnal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docket: Option<String>,
    /// The note a `supra`/`above n` reference points to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
}

/// The parties of a two-party style of cause.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Parties {
    pub plaintiff: String,
    pub defendant: String,
}

/// One citation found in a text.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    /// Position in the returned list; other citations refer to it by this index.
    pub index: usize,
    pub form: Form,
    pub authority: Authority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    /// The citation core: `2016 SCC 27`, `RSC 1985, c C-46`, `Ibid`.
    pub span: Span,
    /// An introductory signal in front of the citation (`See`, `See also`,
    /// `Cf`, `But see`, `Voir`), normalized to lowercase without punctuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<Span>,
    /// Style, core, pinpoints, parentheticals and a bracketed short form.
    pub full_span: Span,
    /// The style of cause, statute title or author/title in front of the core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parties: Option<Parties>,
    pub fields: Fields,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub court: Option<CourtRef>,
    /// Registry jurisdiction id: `ca`, `ca-on`, `uk`, `uk-ew`, `au-nsw`, `us`, `us-ca`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// `en` or `fr` when the citation form is language-specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinpoints: Vec<Pinpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parentheticals: Vec<Parenthetical>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<History>,
    /// The name later references use: the observed style, or an explicit
    /// bracketed short form (`[Jordan]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_short_name: Option<String>,
    /// Citations to the same decision printed side by side share a group id:
    /// the index of the group's first citation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_group: Option<usize>,
    /// For a short form, supra, ibid or reference: the full citation it resolves to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antecedent: Option<usize>,
    /// The versioned identity key of the authority (see [`crate::key`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Which grammar rules produced this reading, for audit and debugging.
    pub reasons: Vec<String>,
}

impl Citation {
    /// The index of the full citation this one ultimately refers to.
    pub fn authority_index(&self) -> usize {
        self.antecedent.unwrap_or(self.index)
    }
}
