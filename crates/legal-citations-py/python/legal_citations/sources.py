"""Source splitting with ALR's existing Python result shapes.

Parsing and boundary decisions belong to the shared Rust engine. This module
only converts records, integer representations and Python string offsets.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass

from . import call


@dataclass(frozen=True)
class DeterministicPart:
    start: int
    end: int
    text: str
    anchors: tuple[str, ...]


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


def _split(text: str, recall_first: bool, extended_us: bool) -> DeterministicSplit:
    result = call("splitSources", {
        "text": text if isinstance(text, str) else "",
        "recallFirst": recall_first, "offsetUnit": "char",
        "extendedUs": extended_us,
    })
    return DeterministicSplit(
        result["status"],
        tuple(DeterministicPart(**{**part, "anchors": tuple(part["anchors"])})
              for part in result["parts"]),
        tuple(tuple(delimiter) for delimiter in result["delimiters"]),
        tuple(result["reasons"]),
    )


def split_footnote(text: str, *, extended_us: bool = False) -> DeterministicSplit:
    return _split(text, False, extended_us)


def split_footnote_recall_first(text: str, *, extended_us: bool = False) -> DeterministicSplit:
    return _split(text, True, extended_us)


def _fields(request: dict) -> DeterministicFields:
    result = call("sourceFields", request)
    result["pinpoint_fragments"] = tuple(result["pinpoint_fragments"])
    result["page_pinpoints"] = tuple(int(value) for value in result["page_pinpoints"])
    result["reasons"] = tuple(result["reasons"])
    return DeterministicFields(**result)


def extract_fields(part: DeterministicPart, *, extended_us: bool = False) -> DeterministicFields:
    return _fields({"part": asdict(part), "extendedUs": extended_us})


def extract_text_fields(text: str, *, extended_us: bool = False) -> DeterministicFields:
    return _fields({"text": str(text or ""), "extendedUs": extended_us})
