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

* A custom ``tokenizer`` is rejected: discovery is owned by the Rust engine.
* Built-in resolution runs separately in Rust; custom resolver callbacks run
  at this Python boundary.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
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
class Reporter:
    short_name: str
    name: str
    cite_type: str
    source: str


@dataclass(frozen=True)
class Edition:
    """Source edition metadata represented as Python objects and dates."""

    short_name: str
    reporter: Optional[Reporter] = None
    start: Optional[datetime] = None
    end: Optional[datetime] = None

    @classmethod
    def from_data(cls, data: dict) -> Edition:
        return cls(data["short_name"], Reporter(**data["reporter"]),
                   datetime.fromisoformat(data["start"]) if data.get("start") else None,
                   datetime.fromisoformat(data["end"]) if data.get("end") else None)


class CitationBase:
    """Common behaviour; see the module docstring for the attribute mapping."""

    def __init__(self, data: dict, text: str, citations: list):
        self.data = data
        self.document_text = text
        self._citations = citations
        self.index = data["index"]
        self.groups = self._groups()
        self.metadata = self._metadata()
        fields = data.get("fields") or {}
        self.year = fields.get("yearNumber")
        self.exact_editions = tuple(Edition.from_data(row) for row in fields.get("exactEditions", []))
        self.variation_editions = tuple(Edition.from_data(row) for row in fields.get("variationEditions", []))
        self.all_editions = self.exact_editions + self.variation_editions
        source_edition = fields.get("sourceEdition")
        self.edition_guess = Edition.from_data(source_edition) if source_edition else None
        if not self.all_editions and fields.get("reporterId") and fields.get("reporterCanonical"):
            self.edition_guess = Edition(fields["reporterCanonical"])
            self.all_editions = (self.edition_guess,)

    # -- spans

    def span(self) -> tuple[int, int]:
        source_name = (self.data.get("fields") or {}).get("sourceCaseName") or {}
        span = source_name.get("referenceSpan") or self.data["span"]
        return span["start"], span["end"]

    def full_span(self) -> tuple[int, int]:
        full = self.data["fullSpan"]
        name = (self.data.get("fields") or {}).get("sourceCaseName")
        if name is not None:
            return name["fullSpanStart"], name.get("fullSpanEnd") or full["end"]
        return full["start"], full["end"]

    def span_with_pincite(self) -> tuple[int, int]:
        start, end = self.span()
        pin_start, pin_end = self.metadata.pin_cite_span_start, self.metadata.pin_cite_span_end
        return min(start, pin_start if pin_start is not None else start), max(end, pin_end if pin_end is not None else end)

    def matched_text(self) -> str:
        source_name = (self.data.get("fields") or {}).get("sourceCaseName") or {}
        return (source_name.get("referenceSpan") or self.data["span"])["text"]

    # -- corrected forms

    def corrected_reporter(self) -> Optional[str]:
        return self.edition_guess.short_name if self.edition_guess else self.groups.get("reporter")

    def _correction(self) -> dict:
        start, end = self.full_span()
        core_start, core_end = self.span()
        kind = next((name for cls, name in (
            (ShortCaseCitation, "short_case"), (FullCaseCitation, "case"),
            (FullLawCitation, "law"), (FullJournalCitation, "journal"),
            (ResourceCitation, "resource"),
        ) if isinstance(self, cls)), "plain")
        return call("correctCitation", {
            "text": self.matched_text(), "kind": kind,
            "style": (self.data.get("style") or {}).get("text"),
            "jurisdiction": self.data.get("jurisdiction"),
            "sourceLayout": {
                "prefix": self.document_text[start:core_start],
                "suffix": self.document_text[core_end:end],
            },
            "reporter": self.groups.get("reporter"),
            "correctedReporter": self.edition_guess.short_name if self.edition_guess else None,
            "page": self.groups.get("page"),
            "metadata": {name: getattr(self.metadata, name, None) for name in (
                "pin_cite", "plaintiff", "defendant", "extra", "court", "year",
                "month", "day", "publisher", "parenthetical", "antecedent_guess",
            )},
        })

    def corrected_citation(self) -> str:
        return self._correction()["citation"]

    def corrected_citation_full(self) -> str:
        return self._correction()["full"]

    def corrected_page(self) -> Optional[str]:
        return self._correction()["page"]

    # -- mapping helpers

    def _groups(self) -> dict:
        return {}

    def _pin(self) -> dict:
        pins = self.data.get("pinpoints") or []
        if not pins:
            return {}
        start, end = pins[0]["span"]["start"], pins[-1]["span"]["end"]
        phrase = (self.data.get("fields") or {}).get("pinCite")
        if phrase and self.data["form"] != "short":
            start, end = phrase["start"], phrase["end"]
        return {
            "pin_cite": self.document_text[start:end],
            "pin_cite_span_start": start,
            "pin_cite_span_end": end,
        }

    def _metadata(self) -> Metadata:
        data = self.data
        fields = data.get("fields") or {}
        source_name = fields.get("sourceCaseName")
        if data["form"] == "reference" and source_name is not None:
            pin = source_name.get("pinCite")
            parties = data.get("parties") or {}
            return Metadata(plaintiff=parties.get("plaintiff"), defendant=parties.get("defendant"),
                            pin_cite=pin["text"] if pin else None)
        values: dict = dict(self._pin())
        for parenthetical in data.get("parentheticals") or []:
            if parenthetical["kind"] == "explanatory":
                values["parenthetical"] = parenthetical["content"]
                break
        for name in ("year", "month", "day", "extra"):
            if fields.get(name):
                values[name] = fields[name]
        if data.get("court"):
            values["court"] = data["court"]["id"]
        if data.get("parties"):
            values["plaintiff"] = data["parties"]["plaintiff"]
            values["defendant"] = data["parties"]["defendant"]
        if fields.get("publisher"):
            values["publisher"] = fields["publisher"]
        if data.get("form") == "reference" and data.get("shortName"):
            values["resolved_case_name_short"] = data["shortName"]
        antecedent = data.get("antecedent")
        target = next((item for item in self._citations if item["index"] == antecedent), None)
        if target is not None:
            values["resolved_case_name_short"] = target.get("shortName")
            if target.get("style"):
                values["resolved_case_name"] = target["style"]["text"].strip().rstrip(",")
        if data.get("form") in ("short", "supra", "reference") and data.get("style"):
            values["antecedent_guess"] = data["style"]["text"].strip().rstrip(",").strip()
        if source_name is not None:
            values["antecedent_guess"] = source_name["antecedentGuess"]
            if source_name.get("fullSpanEnd") is not None:
                pin = source_name.get("pinCite")
                values["pin_cite"] = pin["text"] if pin else None
                values["pin_cite_span_start"] = None
                values["pin_cite_span_end"] = source_name.get("pinCiteSpanEnd")
                values["parenthetical"] = source_name.get("parenthetical")
            if source_name.get("preCitation") is not None:
                pin = source_name.get("pinCite")
                values["pin_cite"] = pin["text"] if pin else None
                if pin:
                    values["pin_cite_span_start"] = source_name["preCitation"]["start"]
        if data.get("form") == "supra":
            values["volume"] = fields.get("volume")
        return Metadata(**values)

    # -- identity

    def _identity(self):
        key = self.data.get("key")
        if key:
            return type(self), key
        # An unkeyed citation has no shared authority identity. In particular,
        # matching text and document-local indices must not merge uncertain
        # citations from different documents (or cases with missing pages).
        return type(self), id(self)

    def comparison_hash(self) -> int:
        return hash(self._identity())

    def __hash__(self) -> int:
        return self.comparison_hash()

    def __eq__(self, other: object) -> bool:
        return isinstance(other, CitationBase) and self._identity() == other._identity()

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
        if fields.get("sourceGroups"):
            return dict(fields["sourceGroups"])
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
        if fields.get("sourceGroups"):
            return dict(fields["sourceGroups"])
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
    name_fields = ("plaintiff", "defendant", "resolved_case_name_short", "resolved_case_name")


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


def get_citations(
    plain_text: str = "",
    remove_ambiguous: bool = False,
    tokenizer: Any = None,
    markup_text: str = "",
    clean_steps: Optional[Iterable[Union[str, Callable[[str], str]]]] = None,
) -> list:
    """eyecite's ``get_citations``: citations in document order."""
    if tokenizer is not None:
        raise NotImplementedError("custom tokenizers are not supported by the Rust citation engine")
    clean_steps = list(clean_steps) if clean_steps is not None else None
    if plain_text and not markup_text:
        text = clean_text(plain_text, clean_steps) if clean_steps else plain_text
    elif markup_text and not plain_text:
        steps = list(clean_steps or [])
        if "html" not in steps:
            steps.insert(0, "html")
        text = clean_text(markup_text, steps)
    elif not plain_text and not markup_text:
        raise ValueError("Both `markup_text` and `plain_text` are empty")
    else:
        if clean_steps:
            raise ValueError("Both `markup_text` and `plain_text` were passed. Not clear which to apply `clean_steps` to")
        text = plain_text
    records = call("extract", {"text": text, "markupText": markup_text or None,
        "options": {"resolve": False, "removeAmbiguous": remove_ambiguous}, "offsetUnit": "char"})["citations"]
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


def resolve_citations(
    citations: list,
    resolve_full_citation=None,
    resolve_shortcase_citation=None,
    resolve_supra_citation=None,
    resolve_reference_citation=None,
    resolve_id_citation=None,
) -> dict:
    """Resolve the supplied records, invoking custom callbacks in Python."""
    resources: dict = {}
    resource_tokens: dict = {}
    resource_objects: list = []
    value_tokens: dict = {}
    full_cites = []
    last_resource = None

    def token(value, mapping):
        return mapping.setdefault(value, len(mapping))

    def resource_token(resource):
        if resource not in resource_tokens:
            resource_tokens[resource] = len(resource_objects)
            resource_objects.append(resource)
        return resource_tokens[resource]

    def record(citation, include_values=False):
        metadata = vars(citation.metadata)
        interpretations = citation.data.get("interpretations", [])
        selected = {item["kind"] for item in interpretations if item["selected"]}
        return {
            "form": citation.form, "authority": citation.authority,
            "format": citation.data.get("format"), "jurisdiction": citation.data.get("jurisdiction"),
            "reporter": citation.corrected_reporter(),
            "volume": citation.groups.get("volume"), "page": citation.groups.get("page"),
            "plaintiff": metadata.get("plaintiff"), "defendant": metadata.get("defendant"),
            "antecedentGuess": metadata.get("antecedent_guess"), "pinCite": metadata.get("pin_cite"),
            "metadataValues": [token(value, value_tokens) for value in metadata.values() if value] if include_values else [],
            "nameValues": [token(metadata[name], value_tokens) for name in ReferenceCitation.name_fields if metadata.get(name)] if include_values else [],
            "ambiguous": any(item["kind"] not in selected for item in interpretations),
        }

    callbacks = (
        (ShortCaseCitation, resolve_shortcase_citation),
        (SupraCitation, resolve_supra_citation),
        (ReferenceCitation, resolve_reference_citation),
    )
    for citation in citations:
        if isinstance(citation, FullCitation):
            resource = resolve_full_citation(citation) if resolve_full_citation else Resource(citation)
            full_cites.append((citation, resource))
        elif isinstance(citation, IdCitation) and resolve_id_citation is not None:
            resource = resolve_id_citation(citation, last_resource, resources)
        else:
            callback = next((callback for kind, callback in callbacks if isinstance(citation, kind)), None)
            if callback is not None:
                resource = callback(citation, full_cites)
            else:
                # Re-read live metadata after callbacks. Python objects are
                # represented by equality-preserving tokens, never repr strings.
                include_values = isinstance(citation, ReferenceCitation)
                request = {"citation": record(citation, include_values)}
                if isinstance(citation, IdCitation):
                    if last_resource:
                        request["previous"] = (record(resources[last_resource][0]), resource_token(last_resource))
                else:
                    request["fullCitations"] = [(record(full, include_values), resource_token(resource)) for full, resource in full_cites]
                selected = call("resolveReference", request)
                resource = resource_objects[selected] if selected is not None else None
        last_resource = resource
        if resource:
            resources.setdefault(resource, []).append(citation)
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
    from bisect import bisect_left, bisect_right
    source = source_text or plain_text
    annotations = sorted(annotations)
    alignment = None
    source_offsets = None
    if offset_updater is not None:
        source_offsets = [(offset_updater.update(start, bisect_right), offset_updater.update(end, bisect_left))
                          for (start, end), _, _ in annotations]
    elif not use_dmp and source != plain_text:
        # The pinned SpanUpdater's stdlib diff steps. Rust owns their offset
        # transitions, boundary selection and annotation handling.
        from difflib import SequenceMatcher
        placeholder = call("placeholderMarkup", {"text": source})
        alignment = []
        for operation, a1, a2, b1, b2 in SequenceMatcher(a=plain_text, b=placeholder, autojunk=False).get_opcodes():
            if operation == "insert":
                alignment.append(("+", b2 - b1))
            elif operation == "replace":
                alignment.extend((("-", a2 - a1), ("+", b2 - b1)))
            elif operation == "delete":
                alignment.append(("-", a2 - a1))
            elif operation == "equal":
                alignment.append(("=", a2 - a1))
    request = {"text": plain_text, "source": source, "offsetUnit": "char",
               "unbalancedTags": unbalanced_tags,
               "alignment": alignment, "sourceOffsets": source_offsets,
               "annotations": [{"start": start, "end": end,
                                "before": str(before), "after": str(after)}
                               for (start, end), before, after in annotations]}
    if annotator is None:
        return call("annotate", request)["text"]
    pieces, cursor = [], 0
    for item in call("annotationRanges", request):
        start, end = item["start"], item["end"]
        _, before, after = annotations[item["index"]]
        pieces.extend((source[cursor:start], annotator(before, item["text"], after)))
        cursor = end
    pieces.append(source[cursor:])
    return "".join(pieces)
