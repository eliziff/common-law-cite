"""Source splitting with Python result shapes.

Parsing and boundary decisions belong to the shared Rust engine. This module
only converts records, integer representations and Python string offsets.
"""
from __future__ import annotations

from dataclasses import dataclass

from . import call


@dataclass(frozen=True)
class DeterministicPart:
    start: int
    end: int
    text: str
    anchors: tuple[str, ...]
    jurisdiction_priority: tuple[str, ...] | None = None
    anchor_spans: tuple[tuple[int, int], ...] = ()
    resolved_url: str | None = None


@dataclass(frozen=True)
class DeterministicSplit:
    status: str
    parts: tuple[DeterministicPart, ...] = ()
    delimiters: tuple[tuple[int, int, str], ...] = ()
    reasons: tuple[str, ...] = ()


@dataclass(frozen=True)
class DeterministicFields:
    status: str
    corrected: str
    kind: str
    link_candidate: str
    pinpoint_fragments: tuple[str, ...]
    page_pinpoints: tuple[int, ...]
    bare_citation: str
    citation_with_style: str
    short_form: str
    reasons: tuple[str, ...] = ()


def _split(text: str, recall_first: bool, jurisdiction_priority: tuple[str, ...] | None) -> DeterministicSplit:
    result = call("splitSources", {
        "text": text if isinstance(text, str) else "",
        "recallFirst": recall_first, "offsetUnit": "char",
        **({"jurisdictionPriority": list(jurisdiction_priority)} if jurisdiction_priority is not None else {}),
    })
    return DeterministicSplit(
        result["status"],
        tuple(DeterministicPart(part["start"], part["end"], part["text"], tuple(part["anchors"]),
                                tuple(part["jurisdictionPriority"]), tuple(tuple(span) for span in part["anchorSpans"]),
                                part.get("resolvedUrl")) for part in result["parts"]),
        tuple(tuple(delimiter) for delimiter in result["delimiters"]),
        tuple(result["reasons"]),
    )


def split_footnote(text: str, *, jurisdiction_priority: tuple[str, ...] | None = None) -> DeterministicSplit:
    return _split(text, False, jurisdiction_priority)


def split_footnote_recall_first(text: str, *, jurisdiction_priority: tuple[str, ...] | None = None) -> DeterministicSplit:
    return _split(text, True, jurisdiction_priority)


def _fields(request: dict) -> DeterministicFields:
    result = call("sourceFields", request)
    result["pinpoint_fragments"] = tuple(result["pinpoint_fragments"])
    result["page_pinpoints"] = tuple(int(value) for value in result["page_pinpoints"])
    result["reasons"] = tuple(result["reasons"])
    return DeterministicFields(**result)


def extract_fields(part: DeterministicPart, *, jurisdiction_priority: tuple[str, ...] | None = None) -> DeterministicFields:
    wire = {"start": part.start, "end": part.end, "text": part.text, "anchors": list(part.anchors),
            "anchorSpans": list(part.anchor_spans),
            **({"jurisdictionPriority": list(part.jurisdiction_priority)} if part.jurisdiction_priority is not None else {}),
            **({"resolvedUrl": part.resolved_url} if part.resolved_url is not None else {})}
    return _fields({"part": wire,
                    **({"jurisdictionPriority": list(jurisdiction_priority)} if jurisdiction_priority is not None else {})})


def extract_text_fields(text: str, *, jurisdiction_priority: tuple[str, ...] | None = None) -> DeterministicFields:
    return _fields({"text": str(text or ""),
                    **({"jurisdictionPriority": list(jurisdiction_priority)} if jurisdiction_priority is not None else {})})
