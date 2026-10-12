"""Smoke tests for the Python binding.

Behavioral coverage lives in conformance/ (run with `python conformance/run.py`);
these only check the binding's own plumbing.
"""

import threading

import pytest

import legal_citations

TEXT = "See R v Jordan, 2016 SCC 27 at para 5; Roe v. Wade, 410 U.S. 113, 115 (1973)."


def test_version():
    info = legal_citations.version()
    assert info["version"] == legal_citations.__version__
    assert info["schemaVersion"] >= 1


def test_extract_offsets_are_python_indices():
    text = "Voilà — 😀 R c Jordan, 2016 CSC 27"
    citations = legal_citations.extract(text)
    assert citations, "expected a citation"
    span = citations[0]["span"]
    assert text[span["start"] : span["end"]] == span["text"]
    utf16 = legal_citations.extract(text, offset_unit="utf16")[0]["span"]
    assert utf16["start"] == span["start"] + 1  # the emoji is two UTF-16 units


def test_unknown_option_is_rejected():
    with pytest.raises(TypeError):
        legal_citations.extract("x", resolv=True)
    with pytest.raises(legal_citations.LegalCitationsError) as error:
        legal_citations.call("nope", {})
    assert error.value.code == "unknown_method"


def test_has_citation_and_registry():
    assert legal_citations.has_citation(TEXT)
    assert not legal_citations.has_citation("nothing to see")
    assert set(legal_citations.registry()) >= {"courts", "reporters", "series", "jurisdictions"}


def test_threads():
    results = []
    threads = [
        threading.Thread(target=lambda: results.append(len(legal_citations.extract(TEXT))))
        for _ in range(4)
    ]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    assert len(set(results)) == 1
