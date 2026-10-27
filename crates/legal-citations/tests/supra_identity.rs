//! Independently invented sources and references; no private document or report data.
use legal_citations::api::call_value;
use serde_json::{json, Value};

#[test]
fn reference_occurrences_keep_written_names_and_separate_markers() {
    let references = legal_citations::find::find_references("Ibid at 17; Kestrel, supra note 4 at 42.");
    assert_eq!(references.len(), 2);
    assert_eq!(references[1].full_span.text, "Kestrel, supra note 4 at 42");
    assert_eq!(references[1].fields.inline_reference.as_ref().unwrap().span.text, "supra note 4");
    assert_eq!(references[1].pinpoints[0].span.text, "42");
}

fn extract(notes: &[(u32, u32, &str)]) -> (Value, Value) {
    let mut offset = 0;
    let ranges = notes.iter().map(|&(number, sequence, text)| {
        let start = offset;
        offset += text.len() + 2;
        json!({"number": number, "sequence": sequence, "start": start, "end": start + text.len()})
    }).collect::<Vec<_>>();
    let text = notes.iter().map(|note| note.2).collect::<Vec<_>>().join("\n\n");
    let ranges = json!(ranges);
    (call_value("extract", json!({"text": text, "options": {"resolve": false, "notes": ranges}})).unwrap(), ranges)
}

fn resolve(extracted: &Value, notes: &Value, parts: bool, mode: &str) -> Value {
    call_value("resolve", json!({"citations": extracted["citations"], "notes": notes,
        "sourceParts": if parts { extracted["sourceParts"].clone() } else { json!([]) },
        "supraHintMode": "aggressive", "supraLinkingMode": mode})).unwrap()
}

#[test]
fn recognized_sources_resolve_without_urls() {
    for (source, name) in [
        ("Polder Council v Mere Holdings, 2036 WL 671823.", "Mere Holdings"),
        ("Wetland Corridors Act, SC 2034, c 18.", "Wetland Corridors Act"),
        ("Wetland Corridor Regulations, SOR/2034-118.", "Wetland Corridor Regulations"),
        ("Milo Kestrel, Riparian Licences, 3rd ed (Halifax: Marsh Press, 2034).", "Kestrel"),
        ("Nessa Wren, \"Riverbank Licensing\" (2034) 18:2 Harbour LJ 61.", "Wren"),
    ] {
        for reference in ["Supra note 4 at 9.".to_owned(), format!("{name}, supra note 4 at 9.")] {
            let (extracted, notes) = extract(&[(4, 0, source), (8, 0, &reference)]);
            for parts in [false, true] {
                for mode in ["safe", "aggressive"] {
                    let result = resolve(&extracted, &notes, parts, mode);
                    assert_eq!(result["resolutions"][0]["antecedent"], 0, "{source}; {reference}; {parts}; {mode}: {result}");
                    assert!(result["resolutions"][0]["url"].is_null(), "{result}");
                    assert_eq!(result["authorities"], json!([[0, 1]]));
                }
            }
        }
    }
}

#[test]
fn consecutive_ibids_inherit_an_identity_without_a_link() {
    let (extracted, notes) = extract(&[
        (4, 0, "Lena Tern, Shore Access (Reed Press, 2036)."),
        (8, 0, "Ibid at 54."), (12, 0, "Ibid at 58."),
    ]);
    for parts in [false, true] {
        let result = resolve(&extracted, &notes, parts, "safe");
        assert_eq!(result["authorities"], json!([[0, 1, 2]]), "{result}");
        assert!(result["resolutions"].as_array().unwrap().iter().all(|reference|
            reference["antecedent"] == 0 && reference["url"].is_null()), "{result}");
    }
}

#[test]
fn ibid_inherits_the_authority_of_a_resolved_reporter_short_form() {
    let (extracted, notes) = extract(&[
        (4, 0, "Basin Agency v Cedar Weir, 287 F.3d 618 (9th Cir. 2017)."),
        (8, 0, "287 F.3d at 622."), (12, 0, "Ibid at 625."),
    ]);
    assert_eq!(extracted["citations"].as_array().unwrap().len(), 3);
    for parts in [false, true] {
        let result = resolve(&extracted, &notes, parts, "safe");
        assert_eq!(result["authorities"], json!([[0,1,2]]), "{parts}: {result}");
        assert_eq!(result["resolutions"][1]["antecedent"], 0, "{parts}: {result}");
    }
}

#[test]
fn stored_note_offsets_do_not_override_the_note_number_order() {
    let (extracted, notes) = extract(&[
        (8, 0, "Ibid at 72."),
        (4, 0, "Lena Tern, Shore Access (Reed Press, 2036)."),
    ]);
    for parts in [false, true] {
        let result = resolve(&extracted, &notes, parts, "safe");
        assert_eq!(result["resolutions"][0]["antecedent"], 1, "{parts}: {result}");
        assert_eq!(result["authorities"], json!([[0,1]]), "{parts}: {result}");
    }
}

#[test]
fn note_numbers_constrain_named_and_inferred_matches() {
    let (extracted, notes) = extract(&[
        (4, 0, "Milo Kestrel, Riparian Licences (Halifax: Marsh Press, 2034)."),
        (8, 0, "Nessa Wren, \"Riverbank Licensing\" (2034) 18:2 Harbour LJ 61."),
        (12, 0, "Kestrel, supra note 8 at 42."),
    ]);
    for parts in [false, true] {
        for mode in ["safe", "aggressive"] {
            let result = resolve(&extracted, &notes, parts, mode);
            assert!(result["resolutions"][0]["antecedent"].is_null(), "{parts}; {mode}: {result}");
            assert!(result["resolutions"][0]["sourcePart"].is_null(), "{result}");
        }
    }
}

#[test]
fn references_can_name_a_note_containing_a_resolved_supra_or_ibid() {
    for intermediate in ["Kestrel, supra note 4 at 19.", "Ibid at 19."] {
        let (extracted, notes) = extract(&[
            (4, 0, "Milo Kestrel, Riparian Licences (Halifax: Marsh Press, 2034)."),
            (8, 0, intermediate), (12, 0, "Kestrel, supra note 8 at 42."),
        ]);
        for parts in [false, true] {
            let result = resolve(&extracted, &notes, parts, "safe");
            assert_eq!(result["resolutions"][0]["antecedent"], 0, "{result}");
            assert_eq!(result["resolutions"][1]["antecedent"], 0, "{parts}; {intermediate}: {result}");
            if parts { assert_eq!(result["resolutions"][1]["sourcePart"], 0, "{result}"); }
        }
    }
}

#[test]
fn matching_a_source_does_not_make_two_distinct_sources_unambiguous() {
    for reference in ["Supra note 4 at 42.", "Kestrel, supra note 4 at 42."] {
        let (mut extracted, notes) = extract(&[
            (4, 0, "Milo Kestrel, \"Riparian Licences\" (2034) 18:2 Harbour LJ 31; Milo Kestrel, \"Harbour Permits\" (2034) 18:2 Harbour LJ 65."),
            (8, 0, reference),
        ]);
        // Distinct articles can share the PDF of a complete journal issue.
        for part in extracted["sourceParts"].as_array_mut().unwrap() {
            part["resolvedUrl"] = json!("https://example.test/journal/2034/issue-2.pdf");
        }
        for parts in [false, true] {
            for mode in ["safe", "aggressive"] {
                let result = resolve(&extracted, &notes, parts, mode);
                assert!(result["resolutions"][0]["antecedent"].is_null(), "{parts}; {mode}: {result}");
                assert!(result["resolutions"][0]["url"].is_null(), "{result}");
            }
        }
    }
}

#[test]
fn optional_urls_enrich_the_same_association() {
    let (mut extracted, notes) = extract(&[
        (4, 0, "Milo Kestrel, Riparian Licences (Halifax: Marsh Press, 2034)."),
        (8, 0, "Kestrel, supra note 4 at 42."),
    ]);
    let unlinked = resolve(&extracted, &notes, true, "safe");
    extracted["sourceParts"][0]["resolvedUrl"] = json!("https://example.test/library/riparian");
    let linked = resolve(&extracted, &notes, true, "safe");
    assert_eq!(linked["authorities"], unlinked["authorities"]);
    assert_eq!(linked["resolutions"][0]["antecedent"], unlinked["resolutions"][0]["antecedent"]);
    assert_eq!(linked["resolutions"][0]["sourcePart"], unlinked["resolutions"][0]["sourcePart"]);
    assert_eq!(linked["resolutions"][0]["url"], "https://example.test/library/riparian");
}

#[test]
fn a_shared_url_never_merges_distinct_resolved_authorities() {
    let (mut extracted, notes) = extract(&[
        (4, 0, "Lena Tern, \"Shore Access\" (2036) 22:1 Harbour LJ 31."),
        (8, 0, "Ivo Gull, \"Reed Routes\" (2036) 22:1 Harbour LJ 65."),
        (12, 0, "Tern, supra note 4 at 54."),
        (16, 0, "Gull, supra note 8 at 58."),
    ]);
    for part in extracted["sourceParts"].as_array_mut().unwrap() {
        part["resolvedUrl"] = json!("https://example.test/journal/2036/issue-1.pdf");
    }
    let result = resolve(&extracted, &notes, true, "safe");
    assert_eq!(result["authorities"], json!([[0, 2], [1, 3]]), "{result}");
    assert_eq!(result["resolutions"][0]["antecedent"], 0);
    assert_eq!(result["resolutions"][1]["antecedent"], 1);
}

#[test]
fn a_caller_identified_source_part_needs_no_url_or_citation_core() {
    let source = "Wetland Board, Advisory Memorandum, File WB-62 (2036).";
    let (mut extracted, notes) = extract(&[(4, 0, source),
        (8, 0, "Advisory Memorandum, supra note 4 at 7.")]);
    extracted["sourceParts"] = json!([{"start":0, "end": source.len(), "text":source,
        "anchors":["document"]}]);
    let result = resolve(&extracted, &notes, true, "safe");
    assert!(result["resolutions"][0]["antecedent"].is_null(), "{result}");
    assert_eq!(result["resolutions"][0]["sourcePart"], 0, "{result}");
    assert!(result["resolutions"][0]["url"].is_null(), "{result}");
    assert_eq!(result["authorities"], json!([[0]]), "{result}");
}

#[test]
fn a_leading_ibid_obeys_body_reading_order_and_unresolved_barriers() {
    let blocks = [
        "Cedar Weir Ltd v Basin Agency, 2037 BCSC 842.",
        "Channel Trust v Wicket Board, 2038 BCSC 523.",
        "Ibid at para 17.",
        "Cedar Weir, supra note 99 at para 8; Ibid at para 11.",
    ];
    let text = blocks.join("\n\n");
    let start = blocks[0].len() + blocks[1].len() + 4;
    let notes = json!([
        {"number":12,"start":start,"end":start+blocks[2].len()},
        {"number":16,"start":start+blocks[2].len()+2,"end":text.len()},
    ]);
    let extracted = call_value("extract", json!({"text":text,
        "options":{"resolve":false,"notes":notes}})).unwrap();
    assert_eq!(extracted["citations"].as_array().unwrap().len(), 5);
    for parts in [false, true] {
        let result = call_value("resolve", json!({"citations":extracted["citations"], "notes":notes,
            "sourceParts":if parts { extracted["sourceParts"].clone() } else { json!([]) },
            "readingOrder":[0,2,1,3,4],"supraLinkingMode":"safe"})).unwrap();
        assert_eq!(result["resolutions"][0]["antecedent"], 0, "{parts}: {result}");
        for reference in &result["resolutions"].as_array().unwrap()[1..] {
            assert!(reference["antecedent"].is_null(), "{parts}: {result}");
        }
    }
}

#[test]
fn restarted_and_ambiguous_note_numbers_are_preserved() {
    for second_sequence in [0, 1] {
        let (extracted, notes) = extract(&[
            (4, 0, "Milo Kestrel, Riparian Licences (Halifax: Marsh Press, 2034)."),
            (4, second_sequence, "Nessa Wren, \"Riverbank Licensing\" (2034) 18:2 Harbour LJ 61."),
            (8, second_sequence, "Kestrel, supra note 4 at 42."),
        ]);
        for parts in [false, true] {
            for mode in ["safe", "aggressive"] {
                let result = resolve(&extracted, &notes, parts, mode);
                assert!(result["resolutions"][0]["antecedent"].is_null(), "{parts}; {mode}: {result}");
            }
        }
    }
}

#[test]
fn registry_matching_returns_source_identity_without_a_link() {
    let result = call_value("resolveRegistryReference", json!({"text": "Wren, supra note 4 at 75.",
        "aggressive": false, "registry": [{"note": 4, "target": "essay:riverbank", "short_form": "Wren"}]})).unwrap();
    assert_eq!(result, json!(["essay:riverbank", "note_number"]));
}
