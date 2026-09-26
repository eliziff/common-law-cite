"""eyecite-compatible facade over legal_citations.

A migration aid: code written against eyecite's public API keeps working by
changing the import::

    # from eyecite import get_citations, resolve_citations, annotate_citations, clean_text
    from legal_citations.eyecite import get_citations, resolve_citations, annotate_citations, clean_text

Citations come back as eyecite-named classes (``FullCaseCitation``,
``ShortCaseCitation``, ``SupraCitation``, ``IdCitation``, ``ReferenceCitation``,
``FullLawCitation``, ``FullJournalCitation``, ``UnknownCitation``) with the
attributes eyecite users rely on: ``groups``, ``metadata.pin_cite``,
``metadata.parenthetical``, ``metadata.year``, ``metadata.court``,
``metadata.plaintiff``, ``metadata.defendant``, ``span()``, ``full_span()``,
``matched_text()``, ``corrected_citation()``, ``year``, ``index``.

Every object also carries the full legal-citations record as ``.data`` (with
``form``, ``authority``, ``key``, ``jurisdiction``, parallel groups, ...), which
is where Canadian and Commonwealth detail lives that eyecite's model has no
slot for. Citations eyecite has no class for (books, parliamentary papers,
short forms of statutes) map to ``FullCitation`` / ``UnknownCitation``.

Differences from eyecite, by design:

* ``tokenizer`` and ``remove_ambiguous`` are accepted and ignored; there is one
  engine.
* ``metadata.pin_cite`` is the pinpoint text as written (``"347-348"``,
  ``"para 5"``) without eyecite's leading ``"at "`` on ``Id.`` citations.
* Custom resolver callbacks to ``resolve_citations`` are not supported; the
  engine resolves while extracting.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Iterable, Optional, Union

from . import call, extract

__all__ = [
    "CitationBase",
    "ResourceCitation",
    "FullCitation",
    "CaseCitation",
    "FullCaseCitation",
    "ShortCaseCitation",
    "FullLawCitation",
    "FullJournalCitation",
    "SupraCitation",
    "IdCitation",
    "ReferenceCitation",
    "UnknownCitation",
    "Resource",
    "get_citations",
    "resolve_citations",
    "annotate_citations",
    "clean_text",
]

_LAW = {"statute", "regulation", "constitution", "court_rule", "treaty", "bill"}


class Metadata:
    """eyecite-style metadata: every attribute eyecite defines reads as None
    when legal-citations has no value for it."""

    _FIELDS = (
        "parenthetical",
        "pin_cite",
        "pin_cite_span_start",
        "pin_cite_span_end",
        "year",
        "month",
        "day",
        "court",
        "plaintiff",
        "defendant",
        "extra",
        "antecedent_guess",
        "resolved_case_name_short",
        "resolved_case_name",
        "publisher",
    )

    def __init__(self, **values: Any):
        for name in self._FIELDS:
            setattr(self, name, None)
        for name, value in values.items():
            setattr(self, name, value)

    def __repr__(self) -> str:
        shown = ", ".join(
            f"{name}={value!r}" for name, value in vars(self).items() if value is not None
        )
        return f"Metadata({shown})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Metadata) and vars(self) == vars(other)


@dataclass(frozen=True)
class Edition:
    """The part of eyecite's ``Edition`` callers read: ``short_name``."""

    short_name: str


class CitationBase:
    """Common behaviour; see the module docstring for the attribute mapping."""

    def __init__(self, data: dict, text: str, citations: list):
        self.data = data
        self.document_text = text
        self._citations = citations
        self.index = data["index"]
        self.groups = self._groups()
        self.metadata = self._metadata()
        year = (data.get("fields") or {}).get("year")
        self.year = int(year) if year and str(year).isdigit() else None
        reporter = (data.get("fields") or {}).get("reporterCanonical")
        self.edition_guess = Edition(reporter) if reporter else None
        self.all_editions = [self.edition_guess] if self.edition_guess else []

    # -- spans

    def span(self) -> tuple[int, int]:
        return self.data["span"]["start"], self.data["span"]["end"]

    def full_span(self) -> tuple[int, int]:
        full = self.data["fullSpan"]
        return full["start"], full["end"]

    def span_with_pincite(self) -> tuple[int, int]:
        start, end = self.span()
        pins = self.data.get("pinpoints") or []
        if pins:
            end = max(end, pins[-1]["span"]["end"])
        return start, end

    def matched_text(self) -> str:
        return self.data["span"]["text"]

    # -- corrected forms

    def corrected_reporter(self) -> Optional[str]:
        fields = self.data.get("fields") or {}
        return fields.get("reporterCanonical") or fields.get("reporter")

    def corrected_citation(self) -> str:
        text = self.matched_text()
        fields = self.data.get("fields") or {}
        written, canonical = fields.get("reporter"), fields.get("reporterCanonical")
        if written and canonical and written != canonical and written in text:
            text = text.replace(written, canonical, 1)
        return " ".join(text.split())

    def corrected_citation_full(self) -> str:
        start, end = self.full_span()
        core_start, core_end = self.span()
        full = self.document_text[start:end]
        corrected = (
            full[: core_start - start] + self.corrected_citation() + full[core_end - start :]
        )
        return " ".join(corrected.split())

    # -- mapping helpers

    def _groups(self) -> dict:
        return {}

    def _pin(self) -> dict:
        pins = self.data.get("pinpoints") or []
        if not pins:
            return {}
        start, end = pins[0]["span"]["start"], pins[-1]["span"]["end"]
        return {
            "pin_cite": self.document_text[start:end],
            "pin_cite_span_start": start,
            "pin_cite_span_end": end,
        }

    def _metadata(self) -> Metadata:
        data = self.data
        values: dict = dict(self._pin())
        for parenthetical in data.get("parentheticals") or []:
            if parenthetical["kind"] == "explanatory":
                values["parenthetical"] = parenthetical["content"]
                break
        fields = data.get("fields") or {}
        if fields.get("year"):
            values["year"] = fields["year"]
        if data.get("court"):
            values["court"] = data["court"]["id"]
        if data.get("parties"):
            values["plaintiff"] = data["parties"]["plaintiff"] or None
            values["defendant"] = data["parties"]["defendant"] or None
        if fields.get("publisher"):
            values["publisher"] = fields["publisher"]
        antecedent = data.get("antecedent")
        if antecedent is not None and antecedent < len(self._citations):
            target = self._citations[antecedent]
            values["resolved_case_name_short"] = target.get("shortName")
            if target.get("style"):
                values["resolved_case_name"] = target["style"]["text"].strip().rstrip(",")
        if data.get("form") in ("short", "supra", "reference") and data.get("style"):
            values["antecedent_guess"] = data["style"]["text"].strip().rstrip(",").strip()
        return Metadata(**values)

    # -- identity

    def comparison_hash(self) -> int:
        key = self.data.get("key")
        if key:
            return hash((type(self).__name__, key))
        return hash((type(self).__name__, self.corrected_citation(), self.index))

    def __hash__(self) -> int:
        return self.comparison_hash()

    def __eq__(self, other: object) -> bool:
        return (
            isinstance(other, CitationBase)
            and type(self) is type(other)
            and self.comparison_hash() == other.comparison_hash()
        )

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.matched_text()!r}, groups={self.groups!r}, metadata={self.metadata!r})"

    @property
    def form(self) -> str:
        return self.data["form"]

    @property
    def authority(self) -> str:
        return self.data["authority"]


class ResourceCitation(CitationBase):
    def _groups(self) -> dict:
        fields = self.data.get("fields") or {}
        return {
            "volume": fields.get("volume"),
            "reporter": fields.get("reporter"),
            "page": fields.get("page"),
        }


class FullCitation(ResourceCitation):
    pass


class CaseCitation(ResourceCitation):
    pass


class FullCaseCitation(CaseCitation, FullCitation):
    pass


class ShortCaseCitation(CaseCitation):
    def _groups(self) -> dict:
        groups = super()._groups()
        pins = self.data.get("pinpoints") or []
        groups["page"] = pins[0]["first"] if pins else groups.get("page")
        return groups


class FullLawCitation(FullCitation):
    def _groups(self) -> dict:
        fields = self.data.get("fields") or {}
        groups = {
            "title": fields.get("volume"),
            "reporter": fields.get("series") or fields.get("reporter"),
            "chapter": fields.get("chapter"),
            "section": fields.get("section"),
        }
        return {name: value for name, value in groups.items() if value is not None}


class FullJournalCitation(FullCitation):
    pass


class SupraCitation(CitationBase):
    pass


class IdCitation(CitationBase):
    pass


class ReferenceCitation(CitationBase):
    pass


class UnknownCitation(CitationBase):
    pass


def _class_for(data: dict) -> type:
    form, authority = data["form"], data["authority"]
    if form == "full":
        if authority == "case":
            return FullCaseCitation
        if authority in _LAW:
            return FullLawCitation
        if authority == "journal":
            return FullJournalCitation
        return FullCitation
    if form == "short":
        return ShortCaseCitation if authority in ("case", "unknown") else UnknownCitation
    return {
        "supra": SupraCitation,
        "ibid": IdCitation,
        "reference": ReferenceCitation,
    }.get(form, UnknownCitation)


def clean_text(text: str, steps: Iterable[Union[str, Callable[[str], str]]]) -> str:
    """eyecite's ``clean_text``; named steps run in the engine, callables in Python."""
    pending: list = []
    for step in steps:
        if callable(step):
            if pending:
                text = call("clean", {"text": text, "steps": pending})["text"]
                pending = []
            text = step(text)
        else:
            pending.append(step)
    if pending:
        text = call("clean", {"text": text, "steps": pending})["text"]
    return text


# The named clean steps get_citations() applied to each source it cleaned, so
# annotate_citations(plain_text, ..., source_text) can map offsets back
# exactly (legal-citations keeps an offset map instead of diffing).
_CLEANED_FROM: dict = {}
_CANDIDATE_STEPS = (
    ["html", "inline_whitespace"],
    ["html", "all_whitespace"],
    ["html"],
    ["all_whitespace"],
    ["inline_whitespace"],
    ["html", "underscores", "inline_whitespace"],
    ["html", "underscores", "all_whitespace"],
    ["underscores"],
)


def _remember(source: str, steps: list, text: str) -> None:
    if len(_CLEANED_FROM) > 64:
        _CLEANED_FROM.clear()
    if all(isinstance(step, str) for step in steps):
        _CLEANED_FROM[(source, text)] = list(steps)


def get_citations(
    plain_text: str = "",
    remove_ambiguous: bool = False,
    tokenizer: Any = None,
    markup_text: str = "",
    clean_steps: Optional[Iterable[Union[str, Callable[[str], str]]]] = None,
) -> list:
    """eyecite's ``get_citations``: citations in document order."""
    if plain_text and not markup_text:
        text = clean_text(plain_text, clean_steps) if clean_steps else plain_text
        if clean_steps:
            _remember(plain_text, list(clean_steps), text)
    elif markup_text and not plain_text:
        steps = list(clean_steps or [])
        if "html" not in steps:
            steps.insert(0, "html")
        text = clean_text(markup_text, steps)
        _remember(markup_text, steps, text)
    elif not plain_text and not markup_text:
        raise ValueError("Both `markup_text` and `plain_text` are empty")
    else:
        text = plain_text
    records = extract(text)
    return [_class_for(record)(record, text, records) for record in records]


class Resource:
    """eyecite's default resource: one per cited authority."""

    def __init__(self, citation: CitationBase):
        self.citation = citation

    def __hash__(self) -> int:
        return hash(self.citation)

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Resource) and self.citation == other.citation

    def __repr__(self) -> str:
        return f"Resource({self.citation!r})"


def resolve_citations(citations: list, **resolvers: Any) -> dict:
    """eyecite's ``resolve_citations``: ``{Resource: [citations]}``.

    Resolution already happened during extraction (each record's
    ``antecedent``); this regroups the objects the way eyecite does. Full
    citations with the same key share one resource.
    """
    if resolvers:
        raise NotImplementedError(
            "custom resolver callbacks are not supported; legal_citations resolves while extracting"
        )
    by_index = {citation.index: citation for citation in citations}
    resources: dict = {}
    resource_of_index: dict = {}
    for citation in citations:
        if isinstance(citation, FullCitation):
            resource = Resource(citation)
            resources.setdefault(resource, []).append(citation)
            resource_of_index[citation.index] = next(
                existing for existing in resources if existing == resource
            )
    for citation in citations:
        if isinstance(citation, FullCitation):
            continue
        antecedent = citation.data.get("antecedent")
        if antecedent is None or antecedent not in by_index:
            continue
        resource = resource_of_index.get(antecedent)
        if resource is not None:
            resources[resource].append(citation)
    for members in resources.values():
        members.sort(key=lambda citation: citation.index)
    return resources


def annotate_citations(
    plain_text: str,
    annotations: Iterable,
    source_text: str = "",
    unbalanced_tags: str = "unchecked",
    use_dmp: bool = True,
    annotator: Optional[Callable[[Any, str, Any], str]] = None,
    offset_updater: Any = None,
) -> str:
    """eyecite's ``annotate_citations``: insert ``before``/``after`` around each
    ``((start, end), before, after)`` span of ``plain_text``; with
    ``source_text`` the annotations are placed in the original markup."""
    annotations = sorted(annotations, key=lambda item: item[0])
    if not source_text or source_text == plain_text:
        pieces, cursor = [], 0
        for (start, end), before, after in annotations:
            pieces.append(plain_text[cursor:start])
            inner = plain_text[start:end]
            pieces.append(
                annotator(before, inner, after) if annotator else f"{before}{inner}{after}"
            )
            cursor = end
        pieces.append(plain_text[cursor:])
        return "".join(pieces)
    if annotator is not None:
        raise NotImplementedError("annotator callbacks are only supported without source_text")
    steps = _CLEANED_FROM.get((source_text, plain_text))
    if steps is None:
        steps = next(
            (
                candidate
                for candidate in _CANDIDATE_STEPS
                if call("clean", {"text": source_text, "steps": candidate})["text"] == plain_text
            ),
            None,
        )
    if steps is None:
        raise ValueError(
            "plain_text is not source_text cleaned with a known step list; "
            "extract with legal_citations.eyecite.get_citations() first, or use "
            "legal_citations.annotate(..., source=..., clean_steps=[...])"
        )
    request = {
        "text": plain_text,
        "annotations": [
            {"start": start, "end": end, "before": str(before), "after": str(after)}
            for (start, end), before, after in annotations
        ],
        "source": source_text,
        "cleanSteps": steps,
        "unbalancedTags": unbalanced_tags,
        "offsetUnit": "char",
    }
    return call("annotate", request)["text"]
