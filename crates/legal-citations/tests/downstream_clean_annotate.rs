use legal_citations::annotate::{annotate, annotate_source, citation_annotations, Annotation, Extent, Unbalanced};
use legal_citations::clean::{clean, Cleaned, Step, TagKind};
use legal_citations::{Authority, Citation, Fields, Form, Span};

fn cleaned_range(cleaned: &Cleaned, needle: &str) -> std::ops::Range<usize> {
    let start = cleaned.text.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {:?}", cleaned.text));
    start..start + needle.len()
}

fn source_of<'a>(source: &'a str, cleaned: &Cleaned, needle: &str) -> &'a str {
    &source[cleaned.source_range(cleaned_range(cleaned, needle)).unwrap()]
}

#[test]
fn html_strips_tags_and_keeps_block_breaks() {
    let source = "<html><head><title>T</title><style>p{}</style></head><body><p>First <i>para</i>.</p><p>Second<br>line</p><script>var x = '<p>';</script></body></html>";
    let cleaned = clean(source, &[Step::Html]);
    assert_eq!(cleaned.text, "First para.\n\nSecond\nline\n\n");
}

#[test]
fn html_decodes_entities_and_maps_them_whole() {
    let source = "<p>Smith &amp; Jones v R&eacute;gie, 2016&nbsp;SCC&#160;27 &#x2014; &bogus; & more</p>";
    let cleaned = clean(source, &[Step::Html]);
    assert_eq!(cleaned.text, "Smith & Jones v R\u{e9}gie, 2016\u{a0}SCC\u{a0}27 \u{2014} &bogus; & more\n\n");
    assert_eq!(source_of(source, &cleaned, "&"), "&amp;");
    assert_eq!(source_of(source, &cleaned, "R\u{e9}gie"), "R&eacute;gie");
    assert_eq!(source_of(source, &cleaned, "2016\u{a0}SCC\u{a0}27"), "2016&nbsp;SCC&#160;27");
}

#[test]
fn html_collapses_source_whitespace_but_not_in_pre() {
    let source = "<div>\n   R v   Jordan,\n  2016 SCC 27\n</div><pre>a   b\n c</pre>";
    let cleaned = clean(source, &[Step::Html]);
    assert_eq!(cleaned.text, "R v Jordan, 2016 SCC 27\n\na   b\n c\n\n");
}

#[test]
fn html_records_tags_in_source_offsets() {
    let source = "<p class=\"a>b\">x <!-- c --> <i>y</i><br/></p>";
    let cleaned = clean(source, &[Step::Html]);
    let kinds = cleaned.tags.iter().map(|tag| (tag.name.as_str(), tag.kind)).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ("p", TagKind::Open),
            ("", TagKind::Other),
            ("i", TagKind::Open),
            ("i", TagKind::Close),
            ("br", TagKind::SelfClosing),
            ("p", TagKind::Close),
        ]
    );
    assert_eq!(&source[cleaned.tags[0].start..cleaned.tags[0].end], "<p class=\"a>b\">");
    assert_eq!(cleaned.text, "x y\n\n");
}

#[test]
fn stray_angle_brackets_are_text() {
    let cleaned = clean("a < b and 3<4 but <i>c</i>", &[Step::Html]);
    assert_eq!(cleaned.text, "a < b and 3<4 but c");
}

#[test]
fn inline_whitespace_keeps_newlines() {
    let source = "R  v\u{a0}\tJordan,\n\n2016   SCC 27";
    let cleaned = clean(source, &[Step::InlineWhitespace]);
    assert_eq!(cleaned.text, "R v Jordan,\n\n2016 SCC 27");
    assert_eq!(source_of(source, &cleaned, "v Jordan"), "v\u{a0}\tJordan");
}

#[test]
fn all_whitespace_collapses_everything() {
    let cleaned = clean(" a \n\n b\u{2003}c ", &[Step::AllWhitespace]);
    assert_eq!(cleaned.text, " a b c ");
}

#[test]
fn underscores_and_zero_width() {
    let source = "Signed ______ on _ day\u{200b} of Ju\u{ad}ly\u{feff}";
    let cleaned = clean(source, &[Step::Underscores, Step::ZeroWidth]);
    assert_eq!(cleaned.text, "Signed  on _ day of July");
    assert_eq!(source_of(source, &cleaned, "July"), "Ju\u{ad}ly");
}

#[test]
fn steps_compose_and_map_to_the_original_source() {
    let source = "<p>See  <b>R&nbsp;v   Jordan</b>,\n 2016 SCC 27</p>";
    let cleaned = clean(source, &[Step::Html, Step::AllWhitespace]);
    assert_eq!(cleaned.text, "See R v Jordan, 2016 SCC 27 ");
    assert_eq!(source_of(source, &cleaned, "2016 SCC 27"), "2016 SCC 27");
    assert_eq!(source_of(source, &cleaned, "R v Jordan"), "R&nbsp;v   Jordan");
    let then = clean(source, &[Step::Html]).then(&[Step::AllWhitespace]);
    assert_eq!(then.text, cleaned.text);
    assert_eq!(then.source_range(0..3), cleaned.source_range(0..3));
}

#[test]
fn identity_and_bounds() {
    let cleaned = Cleaned::identity("abc é");
    assert_eq!(cleaned.source_range(1..3), Some(1..3));
    assert_eq!(cleaned.source_range(4..6), Some(4..6));
    assert_eq!(cleaned.source_range(4..5), None);
    assert_eq!(cleaned.source_range(2..9), None);
    assert_eq!(cleaned.source_offset(cleaned.text.len()), Some(6));
    let span = Span { start: 0, end: 3, text: "abc".into() };
    assert_eq!(cleaned.source_span(&span, "abc é").unwrap().text, "abc");
}

#[test]
fn step_names() {
    assert_eq!(Step::from_name("html"), Some(Step::Html));
    assert_eq!(Step::from_name("xml"), Some(Step::Html));
    assert_eq!(Step::from_name("inline_whitespace"), Some(Step::InlineWhitespace));
    assert_eq!(Step::from_name("all_whitespace"), Some(Step::AllWhitespace));
    assert_eq!(Step::from_name("underscores"), Some(Step::Underscores));
    assert_eq!(Step::from_name("zero_width"), Some(Step::ZeroWidth));
    assert_eq!(Step::from_name("nope"), None);
}

#[test]
fn annotate_plain_text() {
    let text = "See 2016 SCC 27 and 2015 SCC 5.";
    let annotated = annotate(
        text,
        &[Annotation::new(20, 30, "<a>", "</a>"), Annotation::new(4, 15, "<a>", "</a>")],
    );
    assert_eq!(annotated, "See <a>2016 SCC 27</a> and <a>2015 SCC 5</a>.");
}

#[test]
fn annotate_drops_overlaps_and_bad_ranges() {
    let text = "abcdef é";
    let annotated = annotate(
        text,
        &[
            Annotation::new(0, 4, "[", "]"),
            Annotation::new(2, 5, "{", "}"),
            Annotation::new(7, 8, "<", ">"),
            Annotation::new(5, 99, "(", ")"),
        ],
    );
    assert_eq!(annotated, "[abcd]ef é");
}

#[test]
fn annotate_adjacent_annotations() {
    let annotated = annotate("abcd", &[Annotation::new(0, 2, "[", "]"), Annotation::new(2, 4, "(", ")")]);
    assert_eq!(annotated, "[ab](cd)");
}

#[test]
fn annotate_source_balanced_span() {
    let source = "<p>See <i>R v Jordan</i>, 2016&nbsp;SCC 27.</p>";
    let cleaned = clean(source, &[Step::Html]);
    let range = cleaned_range(&cleaned, "2016\u{a0}SCC 27");
    let annotated = annotate_source(source, &cleaned, &[Annotation::new(range.start, range.end, "<a>", "</a>")], Unbalanced::Skip);
    assert_eq!(annotated, "<p>See <i>R v Jordan</i>, <a>2016&nbsp;SCC 27</a>.</p>");
}

#[test]
fn annotate_source_unbalanced_modes() {
    let source = "<p>See <i>R v Jordan</i>, 2016 SCC 27.</p>";
    let cleaned = clean(source, &[Step::Html]);
    let range = cleaned_range(&cleaned, "R v Jordan, 2016 SCC 27");
    let annotation = [Annotation::new(range.start, range.end, "<a>", "</a>")];
    assert_eq!(
        annotate_source(source, &cleaned, &annotation, Unbalanced::Unchecked),
        "<p>See <i><a>R v Jordan</i>, 2016 SCC 27</a>.</p>"
    );
    assert_eq!(annotate_source(source, &cleaned, &annotation, Unbalanced::Skip), source);
    assert_eq!(
        annotate_source(source, &cleaned, &annotation, Unbalanced::Wrap),
        "<p>See <i><a>R v Jordan</a></i><a>, 2016 SCC 27</a>.</p>"
    );
}

#[test]
fn annotate_source_span_containing_a_whole_element_is_balanced() {
    let source = "<p>R v <b>Jordan</b>, 2016 SCC 27</p>";
    let cleaned = clean(source, &[Step::Html]);
    let range = cleaned_range(&cleaned, "R v Jordan, 2016 SCC 27");
    let annotated = annotate_source(source, &cleaned, &[Annotation::new(range.start, range.end, "<a>", "</a>")], Unbalanced::Skip);
    assert_eq!(annotated, "<p><a>R v <b>Jordan</b>, 2016 SCC 27</a></p>");
}

#[test]
fn annotate_source_wraps_across_paragraphs() {
    let source = "<p>R v Jordan,</p><p>2016 SCC 27</p>";
    let cleaned = clean(source, &[Step::Html, Step::AllWhitespace]);
    assert_eq!(cleaned.text, "R v Jordan, 2016 SCC 27 ");
    let range = cleaned_range(&cleaned, "R v Jordan, 2016 SCC 27");
    let annotated = annotate_source(source, &cleaned, &[Annotation::new(range.start, range.end, "<a>", "</a>")], Unbalanced::Wrap);
    assert_eq!(annotated, "<p><a>R v Jordan,</a></p><p><a>2016 SCC 27</a></p>");
}

fn citation(start: usize, end: usize, text: &str, full_start: usize) -> Citation {
    Citation {
        index: 0,
        form: Form::Full,
        authority: Authority::Case,
        format: None,
        span: Span { start, end, text: text[start..end].into() },
        signal: None,
        full_span: Span { start: full_start, end, text: text[full_start..end].into() },
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
        antecedent: None,
        key: None,
        reasons: Vec::new(),
    }
}

#[test]
fn citation_annotations_use_core_or_full_extent() {
    let text = "R v Jordan, 2016 SCC 27";
    let citations = [citation(12, 23, text, 0)];
    let core = citation_annotations(&citations, Extent::Core, |_| Some(("<a>".into(), "</a>".into())));
    assert_eq!(annotate(text, &core), "R v Jordan, <a>2016 SCC 27</a>");
    let full = citation_annotations(&citations, Extent::Full, |_| Some(("<a>".into(), "</a>".into())));
    assert_eq!(annotate(text, &full), "<a>R v Jordan, 2016 SCC 27</a>");
    let none = citation_annotations(&citations, Extent::Full, |_| None);
    assert!(none.is_empty());
}
