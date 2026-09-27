"""Legal citation extraction, resolution and formatting for Canadian,
Commonwealth and US law.

Every function is a thin wrapper over one JSON API shared with the Rust crate,
the CLI and the npm package, so all of them return the same data. Citations
are plain ``dict`` objects following ``conformance/schema/citation.schema.json``
(camelCase keys). Offsets are Python string indices (``offset_unit="char"``)
unless you ask for ``"byte"`` or ``"utf16"``.

>>> import legal_citations
>>> [c["span"]["text"] for c in legal_citations.extract("R v Jordan, 2016 SCC 27 at para 5")]
['2016 SCC 27']

For code written against eyecite, ``legal_citations.eyecite`` offers
``get_citations``, ``resolve_citations``, ``annotate_citations`` and
``clean_text`` with eyecite's class and attribute names.
"""

from __future__ import annotations

import json
from typing import Any, Iterable, Mapping, Optional, Sequence, Union

from ._native import __version__
from ._native import call as _native_call

__all__ = [
    "LegalCitationsError",
    "__version__",
    "annotate",
    "call",
    "classify_excerpt",
    "clean",
    "extract",
    "format",
    "format_pinpoint",
    "has_citation",
    "has_citation_cue",
    "has_citation_signal",
    "is_citation_continuation",
    "protected_citation_spans",
    "key",
    "key_for_text",
    "keys",
    "registry",
    "resolve",
    "url",
    "version",
]



_OPTION_NAMES = {
    "resolve": "resolve",
    "parallel": "parallel",
    "extended_us": "extendedUs",
    "notes": "notes",
    "remove_ambiguous": "removeAmbiguous",
    "jurisdiction_priority": "jurisdictionPriority",
    "supra_hint_mode": "supraHintMode",
    "supra_linking_mode": "supraLinkingMode",
}


class LegalCitationsError(ValueError):
    """A request the engine rejected. ``code`` is one of ``unknown_method``,
    ``invalid_request``, ``invalid_offset``, ``unimplemented``."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def call(method: str, request: Optional[Mapping[str, Any]] = None) -> Any:
    """Call any API method with a request object; returns the response object."""
    ok, payload = _native_call(method, json.dumps(request or {}))
    if not ok:
        error = json.loads(payload)
        raise LegalCitationsError(error["code"], error["message"])
    return json.loads(payload)


def _options(options: Mapping[str, Any]) -> dict:
    out = {}
    for name, value in options.items():
        if name not in _OPTION_NAMES:
            raise TypeError(
                f"unknown option {name!r}; expected one of {', '.join(sorted(_OPTION_NAMES))}"
            )
        if name == "notes" and value is not None:
            value = [_note(note) for note in value]
        out[_OPTION_NAMES[name]] = value
    return out


def _note(note: Any) -> dict:
    if isinstance(note, Mapping):
        return dict(note)
    number, start, end, *rest = note
    out = {"number": number, "start": start, "end": end}
    if rest:
        out["sequence"] = rest[0]
    return out


def resolve(citations: Iterable[Mapping[str, Any]], *, notes=None, alias_groups=(),
            source_parts=(), reading_order=None, supra_hint_mode="aggressive",
            supra_linking_mode="safe") -> dict:
    """Resolve existing records using extract's source parts and the same offset unit."""
    return call("resolve", {"citations": list(citations), "notes": None if notes is None else [_note(note) for note in notes],
                            "aliasGroups": list(alias_groups), "sourceParts": list(source_parts),
                            "readingOrder": None if reading_order is None else list(reading_order),
                            "supraHintMode": supra_hint_mode, "supraLinkingMode": supra_linking_mode})


def extract(text: str, *, markup_text: Optional[str] = None, offset_unit: str = "char", **options: Any) -> list[dict]:
    """Find, classify, group and resolve every citation in ``text``.

    Options: ``resolve`` (default True), ``parallel`` (True), ``extended_us``
    (True), ``notes`` (a list of ``{"number", "start", "end"}`` footnote ranges,
    or ``(number, start, end)`` tuples, in ``offset_unit``),
    ``supra_hint_mode`` ("aggressive") and ``supra_linking_mode`` ("safe").
    """
    response = call(
        "extract", {"text": text, "markupText": markup_text, "options": _options(options), "offsetUnit": offset_unit}
    )
    return response["citations"]


def key(citation: Mapping[str, Any]) -> Optional[str]:
    """The versioned identity key of one extracted citation."""
    return call("key", {"citation": citation})["key"]


def key_for_text(text: str, **options: Any) -> Optional[str]:
    """The key of the one full citation ``text`` holds (``"R v Jordan, 2016 SCC 27"``);
    ``None`` when it holds none, several, or one without a stable identity
    (``call("keyForText", ...)["reason"]`` says which)."""
    return call("keyForText", {"text": text, "options": _options(options)})["key"]


def keys(text: str, **options: Any) -> list[dict]:
    """``[{"index", "text", "key"}]`` for every citation in ``text``."""
    return call("keyForText", {"text": text, "options": _options(options)})["keys"]


def _citation_or_text(citation, text, options) -> dict:
    if (citation is None) == (text is None):
        raise TypeError("pass exactly one of citation= or text=")
    request = {"options": _options(options)}
    if citation is not None:
        request["citation"] = citation
    else:
        request["text"] = text
    return request


def format(
    citation: Optional[Mapping[str, Any]] = None,
    *,
    text: Optional[str] = None,
    style: Optional[str] = None,
    language: Optional[str] = None,
    range_dash: Optional[str] = None,
    **options: Any,
) -> list[dict]:
    """``[{"index", "formatted"}]``: one citation, or every citation in
    ``text``, rendered in McGill style (``language="fr"`` for French)."""
    request = _citation_or_text(citation, text, options)
    request.update(_format_options(style, language, range_dash))
    return call("format", request)["citations"]


def format_pinpoint(
    kind: str,
    locators: Iterable[Union[str, Sequence[Optional[str]], Mapping[str, Any]]],
    *,
    style: Optional[str] = None,
    language: Optional[str] = None,
    range_dash: Optional[str] = None,
) -> str:
    """``format_pinpoint("paragraph", [("12", "14")])`` -> ``"at paras 12-14"``."""
    items = []
    for locator in locators:
        if isinstance(locator, str):
            items.append({"first": locator})
        elif isinstance(locator, Mapping):
            items.append(dict(locator))
        else:
            first, last = (list(locator) + [None])[:2]
            items.append({"first": first, "last": last})
    request = {"pinpoint": {"kind": kind, "locators": items}}
    request.update(_format_options(style, language, range_dash))
    return call("format", request)["pinpoint"]


def _format_options(style, language, range_dash) -> dict:
    out = {}
    if style is not None:
        out["style"] = style
    if language is not None:
        out["language"] = language
    if range_dash is not None:
        out["rangeDash"] = range_dash
    return out


def url(
    citation: Optional[Mapping[str, Any]] = None,
    *,
    text: Optional[str] = None,
    language: Optional[str] = None,
    anchor: bool = False,
    **options: Any,
) -> list[dict]:
    """``[{"index", "url"}]``: the public-source URL of each citation (a back
    reference gets its antecedent's), ``None`` when no source is certain."""
    request = _citation_or_text(citation, text, options)
    if language is not None:
        request["language"] = language
    request["anchor"] = anchor
    return call("url", request)["urls"]


def annotate(
    text: str,
    before: str = "",
    after: str = "",
    *,
    annotations: Optional[Iterable[Union[Mapping[str, Any], Sequence[Any]]]] = None,
    span: str = "span",
    source: Optional[str] = None,
    clean_steps: Optional[Iterable[str]] = None,
    unbalanced_tags: Optional[str] = None,
    offset_unit: str = "char",
    **options: Any,
) -> str:
    """Insert markup around citations.

    Without ``annotations`` every citation found in ``text`` is wrapped in
    ``before``/``after`` (templates; ``{index}``, ``{key}``, ``{form}``,
    ``{authority}``, ``{url}`` are substituted). With ``annotations`` —
    ``{"start", "end", "before", "after"}`` or ``((start, end), before, after)``
    — exactly those spans are wrapped. ``source`` is the markup ``text`` was
    cleaned from with ``clean_steps`` (see :func:`clean`); the result is then
    that markup, annotated, with offsets mapped exactly (no diffing).
    """
    request: dict = {
        "text": text,
        "before": before,
        "after": after,
        "span": span,
        "options": _options(options),
        "offsetUnit": offset_unit,
    }
    if annotations is not None:
        request["annotations"] = [_annotation(item) for item in annotations]
    if source is not None:
        request["source"] = source
        request["cleanSteps"] = list(clean_steps or [])
    if unbalanced_tags is not None:
        request["unbalancedTags"] = unbalanced_tags
    return call("annotate", request)["text"]


def _annotation(item: Any) -> dict:
    if isinstance(item, Mapping):
        return dict(item)
    (start, end), before, after = item
    return {"start": start, "end": end, "before": before, "after": after}


def clean(text: str, steps: Iterable[str]) -> str:
    """Clean text with eyecite-compatible steps (``html``/``xml``,
    ``inline_whitespace``, ``all_whitespace``, ``underscores``) or ``zero_width``."""
    return call("clean", {"text": text, "steps": list(steps)})["text"]


def has_citation_cue(text: str) -> bool:
    return call("hasCitationCue", {"text": text})


def has_citation_signal(text: str) -> bool:
    return call("hasCitationSignal", {"text": text})


def is_citation_continuation(text: str) -> bool:
    return call("isCitationContinuation", {"text": text})


def protected_citation_spans(text: str, *, offset_unit: str = "char") -> tuple[tuple[int, int], ...]:
    return tuple(map(tuple, call("protectedCitationSpans", {"text": text, "offsetUnit": offset_unit})))


def registry(table: Optional[str] = None, surface: Optional[str] = None) -> Any:
    """The embedded registry (``jurisdictions``, ``courts``, ``reporters``,
    ``series``, ``journals``), one table, or the entries of ``table`` that a
    surface form names (``registry("courts", "FCA")``), preferred first."""
    request = {}
    if table is not None:
        request["table"] = table
    if surface is not None:
        request["surface"] = surface
    return call("registry", request)


def classify_excerpt(excerpt: str) -> dict:
    """Is a quoted excerpt prose, an authority list, or a mix?"""
    return call("classifyExcerpt", {"excerpt": excerpt})


def has_citation(text: str) -> bool:
    """Whether ``text`` holds a citation or a two-party case name."""
    return call("hasCitation", {"text": text})["hasCitation"]


def version() -> dict:
    """Crate, schema, key, grammar and registry versions."""
    return call("version")
