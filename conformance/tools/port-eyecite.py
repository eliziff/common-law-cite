#!/usr/bin/env python3
"""Port eyecite's own test suite into language-neutral conformance cases.

Runs eyecite's tests (find, resolve, annotate, models, utils/clean) with the
public entry points wrapped, records every input the tests feed eyecite and
what *real* eyecite returns for it, and writes each as a conformance case
mapped onto the legal-citations model (form x authority, fields, spans,
pinpoints, parentheticals, court, year, parties, antecedents).

Every case is written with ``status: "pending"`` unless the existing case file
already has a status for the same name; run ``conformance/run.py --promote``
afterwards to mark the cases the engine already satisfies.

Run it with a Python that has eyecite installed (editable install of the
checkout you point at, so the tests and the library agree):

    python conformance/tools/port-eyecite.py --eyecite /path/to/eyecite

Mapping (eyecite -> legal-citations), asserted per class:

    FullCaseCitation     form=full    authority=case     span, fullSpan, fields.volume/reporter/
                                                         reporterCanonical/page/year, court.id,
                                                         parties, pinpoints, parentheticals
    ShortCaseCitation    form=short   authority=case     span, fields.volume/reporter, pinpoints
    FullLawCitation      form=full    authority=statute|regulation (reporters-db cite_type)
                                                         span, fields.volume(title)/series(reporter)/
                                                         chapter/section/year
    FullJournalCitation  form=full    authority=journal  span, fullSpan, fields.volume/reporter/page/year,
                                                         pinpoints
    IdCitation           form=ibid                       fullSpan (eyecite's span() includes the pin)
    SupraCitation        form=supra                      fullSpan (from the `supra` token through the pin)
    ReferenceCitation    form=reference                  fullSpan
    UnknownCitation      form=unknown                    span.start

Offsets are Unicode scalar ("char") offsets, eyecite's native unit. Inputs
eyecite cleaned (``clean_steps``) or read from markup are ported as the plain
text eyecite actually searched; the original is kept under ``upstream``.
"""

from __future__ import annotations

import argparse
import logging
import inspect
import json
import re
import subprocess
import sys
import unittest
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "conformance" / "cases"
FORMAT = "legal-citations-conformance:v1"


def current_test() -> str:
    for frame in inspect.stack():
        if frame.function.startswith("test_"):
            return f"{frame.frame.f_locals['self'].__class__.__name__}.{frame.function}"
    return "unknown"


# ---------------------------------------------------------------- mapping

PIN = re.compile(
    r"^(?:at\s+)?(?P<para>¶+\s*|paras?\.?\s+)?(?P<first>\d+)(?:\s*[-–—]\s*(?P<last>\d+))?$"
)


def pinpoint_expectation(pin_cite: str | None):
    """The first pinpoint of a simple eyecite pin cite, or None when the pin
    is too irregular to map without guessing."""
    if not pin_cite:
        return None
    text = pin_cite.strip().lstrip(",").strip()
    match = PIN.match(text)
    if not match:
        return None
    pinpoint = {
        "kind": "paragraph" if match["para"] else "page",
        "first": match["first"],
        "last": match["last"],
    }
    return {"$contains": [pinpoint]}


def law_authority(cite) -> str:
    kinds = {edition.reporter.cite_type for edition in getattr(cite, "all_editions", [])}
    if kinds and all(kind.startswith("admin") for kind in kinds):
        return "regulation"
    return "statute"


def span_of(text: str, start: int, end: int) -> dict:
    return {"start": start, "end": end, "text": text[start:end]}


def expectation(cite, text: str) -> dict:
    """Partial legal-citations expectation for one eyecite citation."""
    kind = type(cite).__name__
    metadata = cite.metadata
    groups = cite.groups
    start, end = cite.span()
    full_start, full_end = cite.full_span()
    out: dict = {}

    if kind == "FullCaseCitation":
        out = {"form": "full", "authority": "case", "span": span_of(text, start, end)}
        out["fullSpan"] = {"start": full_start, "end": full_end}
        fields = {
            "volume": groups.get("volume"),
            "reporter": groups.get("reporter"),
            "page": groups.get("page"),
        }
        if getattr(cite, "edition_guess", None) is not None:
            fields["reporterCanonical"] = cite.edition_guess.short_name
        if metadata.year:
            fields["year"] = metadata.year
        out["fields"] = fields
        if metadata.court:
            out["court"] = {"id": metadata.court}
        if metadata.plaintiff or metadata.defendant:
            out["parties"] = {
                "plaintiff": metadata.plaintiff or "",
                "defendant": metadata.defendant or "",
            }
    elif kind == "ShortCaseCitation":
        out = {"form": "short", "authority": "case", "span": span_of(text, start, end)}
        out["fields"] = {"volume": groups.get("volume"), "reporter": groups.get("reporter")}
    elif kind == "FullLawCitation":
        out = {
            "form": "full",
            "authority": law_authority(cite),
            "span": span_of(text, start, end),
        }
        fields = {"series": groups.get("reporter")}
        for source, target in (("title", "volume"), ("chapter", "chapter"), ("section", "section")):
            if groups.get(source):
                fields[target] = groups[source]
        if metadata.year:
            fields["year"] = metadata.year
        out["fields"] = fields
    elif kind == "FullJournalCitation":
        out = {"form": "full", "authority": "journal", "span": span_of(text, start, end)}
        out["fullSpan"] = {"start": full_start, "end": full_end}
        fields = {
            "volume": groups.get("volume"),
            "reporter": groups.get("reporter"),
            "page": groups.get("page"),
        }
        if metadata.year:
            fields["year"] = metadata.year
        out["fields"] = fields
    elif kind == "IdCitation":
        out = {"form": "ibid", "fullSpan": span_of(text, start, end)}
    elif kind == "SupraCitation":
        out = {"form": "supra", "fullSpan": span_of(text, start, end)}
    elif kind == "ReferenceCitation":
        out = {"form": "reference", "fullSpan": span_of(text, start, end)}
    elif kind == "UnknownCitation":
        out = {"form": "unknown", "span": {"start": start}}
    else:
        raise ValueError(f"unmapped eyecite class {kind}")

    if kind != "UnknownCitation" and kind != "FullLawCitation":
        pinpoint = pinpoint_expectation(getattr(metadata, "pin_cite", None))
        if pinpoint:
            out["pinpoints"] = pinpoint
    parenthetical = getattr(metadata, "parenthetical", None)
    if parenthetical and kind in ("FullCaseCitation", "FullJournalCitation"):
        out["parentheticals"] = {
            "$contains": [{"kind": "explanatory", "content": parenthetical}]
        }
    return out


def upstream_record(cite) -> dict:
    metadata = {
        key: value
        for key, value in vars(cite.metadata).items()
        if value is not None and not key.startswith("pin_cite_span")
    }
    return {
        "class": type(cite).__name__,
        "span": list(cite.span()),
        "fullSpan": list(cite.full_span()),
        "groups": dict(cite.groups),
        "metadata": metadata,
    }


# ---------------------------------------------------------------- recording


class Recorder:
    def __init__(self, eyecite):
        self.eyecite = eyecite
        self.extract: dict[str, list] = defaultdict(list)
        self.annotate: dict[str, list] = defaultdict(list)
        self.clean: dict[str, list] = defaultdict(list)
        self.resolve: dict[str, list] = defaultdict(list)
        self.skipped: list[str] = []

    def get_citations(self, module: str):
        real = self.eyecite.get_citations
        from eyecite.tokenizers import AhocorasickTokenizer, Tokenizer

        def wrapped(plain_text="", remove_ambiguous=False, tokenizer=None, markup_text="", clean_steps=None, **kwargs):
            result = real(
                plain_text=plain_text,
                remove_ambiguous=remove_ambiguous,
                tokenizer=tokenizer or self.eyecite.tokenizers.default_tokenizer,
                markup_text=markup_text,
                clean_steps=clean_steps,
                **kwargs,
            )
            test = current_test()
            # test_custom_tokenizer rewrites the extractors; every other test
            # passes a stock Tokenizer instance.
            custom = test.endswith("test_custom_tokenizer") or (
                tokenizer is not None and type(tokenizer) not in (Tokenizer, AhocorasickTokenizer)
            )
            if custom or remove_ambiguous or kwargs:
                reason = "custom tokenizer" if custom else "remove_ambiguous" if remove_ambiguous else sorted(kwargs)
                self.skipped.append(f"{module} {test}: non-default get_citations arguments ({reason})")
                return result
            document = result[0].document if result else self.eyecite.models.Document(
                plain_text=plain_text, markup_text=markup_text, clean_steps=clean_steps
            )
            self.extract[module].append(
                {
                    "test": test,
                    "text": document.plain_text,
                    "source": markup_text or plain_text,
                    "markup": bool(markup_text),
                    "clean_steps": [step for step in (clean_steps or []) if isinstance(step, str)],
                    "callable_steps": any(not isinstance(step, str) for step in (clean_steps or [])),
                    "citations": result,
                }
            )
            return result

        return wrapped

    def annotate_citations(self, module: str):
        real = self.eyecite.annotate_citations

        def wrapped(plain_text, annotations, source_text=None, **kwargs):
            result = real(plain_text, annotations, source_text=source_text, **kwargs)
            self.annotate[module].append(
                {
                    "test": current_test(),
                    "text": plain_text,
                    "annotations": annotations,
                    "source": source_text,
                    "kwargs": kwargs,
                    "result": result,
                }
            )
            return result

        return wrapped

    def clean_text(self, module: str):
        real = self.eyecite.clean_text

        def wrapped(text, steps):
            test = current_test()
            try:
                result = real(text, steps)
            except Exception as error:  # noqa: BLE001 - recorded, then re-raised
                self.clean[module].append({"test": test, "text": text, "steps": steps, "error": error})
                raise
            self.clean[module].append({"test": test, "text": text, "steps": steps, "result": result})
            return result

        return wrapped


def run_module(recorder: Recorder, name: str, patch_resolve=False):
    module = __import__(f"tests.{name}", fromlist=["*"])
    if hasattr(module, "tested_tokenizers"):
        from eyecite.tokenizers import Tokenizer

        module.tested_tokenizers = [Tokenizer()]
    if hasattr(module, "get_citations"):
        module.get_citations = recorder.get_citations(name)
    if hasattr(module, "annotate_citations"):
        module.annotate_citations = recorder.annotate_citations(name)
    if hasattr(module, "clean_text"):
        module.clean_text = recorder.clean_text(name)
    if patch_resolve:
        test_class = module.ResolveTest
        original = test_class.checkResolution
        original_reference = test_class.checkReferenceResolution

        def check_resolution(self, *rows):
            recorder.resolve[name].append({"test": current_test(), "parts": [text for _, text in rows]})
            return original(self, *rows)

        def check_reference(self, expected, text, resolved_case_name_short=None):
            if resolved_case_name_short:
                recorder.skipped.append(
                    f"{name} {current_test()}: simulated resolved_case_name_short ({text!r})"
                )
            else:
                recorder.resolve[name].append({"test": current_test(), "parts": [text]})
            return original_reference(self, expected, text, resolved_case_name_short)

        test_class.checkResolution = check_resolution
        test_class.checkReferenceResolution = check_reference
    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
    result = unittest.TextTestRunner(stream=open("/dev/null", "w"), verbosity=0).run(suite)
    return len(result.failures) + len(result.errors), result.testsRun


# ---------------------------------------------------------------- case building


def extract_case(name: str, text: str, citations, upstream_extra: dict) -> dict:
    return {
        "name": name,
        "input": text,
        "status": "pending",
        "expect": {"citations": [expectation(cite, text) for cite in citations]},
        "upstream": {**upstream_extra, "citations": [upstream_record(cite) for cite in citations]},
    }


MAX_INPUT = 20_000


def extract_cases(records: list, skipped: list) -> list:
    cases, seen, counters = [], set(), defaultdict(int)
    for record in records:
        if record["callable_steps"]:
            skipped.append(f"extract {record['test']}: callable clean step")
            continue
        if len(record["text"]) > MAX_INPUT:
            skipped.append(f"extract {record['test']}: input longer than {MAX_INPUT} characters")
            continue
        if record["text"] in seen:
            continue
        seen.add(record["text"])
        counters[record["test"]] += 1
        extra = {"test": record["test"]}
        if record["source"] != record["text"]:
            extra["source"] = record["source"]
            extra["cleanSteps"] = record["clean_steps"]
            extra["markup"] = record["markup"]
        cases.append(
            extract_case(f"{record['test']}#{counters[record['test']]}", record["text"], record["citations"], extra)
        )
    return cases


def resolve_cases(eyecite, records: list, skipped: list) -> list:
    from eyecite.models import FullCitation

    cases, seen, counters = [], set(), defaultdict(int)
    for record in records:
        text = " ".join(record["parts"])
        if text in seen:
            continue
        seen.add(text)
        citations = eyecite.get_citations(text)
        if len(record["parts"]) > 1 and len(citations) != len(record["parts"]):
            skipped.append(f"resolve {record['test']}: joined text {text!r} changes the citation count")
            continue
        resolution = eyecite.resolve_citations(citations)
        position = {id(cite): index for index, cite in enumerate(citations)}
        antecedent: dict[int, object] = {}
        for resource_citations in resolution.values():
            indices = [position[id(cite)] for cite in resource_citations]
            fulls = [index for index in indices if isinstance(citations[index], FullCitation)]
            for index in indices:
                if isinstance(citations[index], FullCitation) or not fulls:
                    continue
                antecedent[index] = fulls[0] if len(fulls) == 1 else {"$in": fulls}
        expect = []
        for index, cite in enumerate(citations):
            form = expectation(cite, text)["form"]
            expect.append({"form": form, "antecedent": antecedent.get(index)})
        counters[record["test"]] += 1
        cases.append(
            {
                "name": f"{record['test']}#{counters[record['test']]}",
                "input": text,
                "status": "pending",
                "expect": {"citations": expect},
                "upstream": {
                    "test": record["test"],
                    "citations": [upstream_record(cite) for cite in citations],
                    "resolution": [
                        [position[id(cite)] for cite in cites] for cites in resolution.values()
                    ],
                },
            }
        )
    return cases


def clean_steps_for(record: dict, extracts: list):
    """The clean steps eyecite used to derive ``record["text"]`` from its
    source, read from the get_citations() call the same test made."""
    for extract in reversed(extracts):
        if extract["test"] == record["test"] and extract["text"] == record["text"]:
            if extract["callable_steps"]:
                return None
            steps = list(extract["clean_steps"])
            if extract["markup"] and "html" not in steps:
                steps.insert(0, "html")
            return steps
    return []


def annotate_cases(records: list, extracts: list, skipped: list) -> list:
    cases, counters = [], defaultdict(int)
    for record in records:
        if len(record["source"] or record["text"]) > MAX_INPUT:
            skipped.append(f"annotate {record['test']}: input longer than {MAX_INPUT} characters")
            continue
        kwargs = dict(record["kwargs"])
        unbalanced = kwargs.pop("unbalanced_tags", None)
        if kwargs:
            skipped.append(f"annotate {record['test']}: unsupported annotate arguments {sorted(kwargs)}")
            continue
        # Explicit spans (eyecite's own), so the case tests annotation and
        # markup mapping independently of extraction.
        request = {
            "text": record["text"],
            "annotations": [
                {"start": start, "end": end, "before": before, "after": after}
                for (start, end), before, after in record["annotations"]
            ],
            "offsetUnit": "char",
        }
        if record["source"] is not None and record["source"] != record["text"]:
            steps = clean_steps_for(record, extracts)
            if steps is None:
                skipped.append(f"annotate {record['test']}: callable clean step")
                continue
            request["source"] = record["source"]
            request["cleanSteps"] = steps
        if unbalanced:
            request["unbalancedTags"] = unbalanced
        counters[record["test"]] += 1
        cases.append(
            {
                "name": f"{record['test']}#{counters[record['test']]}",
                "method": "annotate",
                "request": request,
                "status": "pending",
                "expect": {"text": record["result"]},
                "upstream": {"test": record["test"]},
            }
        )
    return cases


def clean_cases(records: list, skipped: list) -> list:
    cases, counters = [], defaultdict(int)
    for record in records:
        if any(not isinstance(step, str) for step in record["steps"]):
            skipped.append(f"clean {record['test']}: callable clean step")
            continue
        if len(record["text"]) > MAX_INPUT:
            skipped.append(f"clean {record['test']}: input longer than {MAX_INPUT} characters")
            continue
        counters[record["test"]] += 1
        expect = (
            {"error": {"code": "invalid_request"}}
            if "error" in record
            else {"text": record["result"]}
        )
        cases.append(
            {
                "name": f"{record['test']}#{counters[record['test']]}",
                "method": "clean",
                "request": {"text": record["text"], "steps": list(record["steps"])},
                "status": "pending",
                "expect": expect,
                "upstream": {"test": record["test"]},
            }
        )
    return cases


def write(path: Path, source: str, description: str, cases: list) -> None:
    previous = {}
    if path.exists():
        previous = {case["name"]: case for case in json.loads(path.read_text())["cases"]}
    for case in cases:
        old = previous.get(case["name"])
        if old and old.get("input", old.get("request")) == case.get("input", case.get("request")):
            case["status"] = old.get("status", "pending")
            if "note" in old:
                case["note"] = old["note"]
            # A reviewed, intentional difference from eyecite: keep the
            # hand-edited expectation instead of regenerating it.
            if "divergence" in old:
                case["divergence"] = old["divergence"]
                case["expect"] = old["expect"]
    document = {
        "format": FORMAT,
        "source": source,
        "description": description,
        "generatedBy": "conformance/tools/port-eyecite.py",
        "cases": cases,
    }
    path.write_text(json.dumps(document, ensure_ascii=False, indent=1) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--eyecite", type=Path, required=True, help="eyecite checkout (with tests/)")
    arguments = parser.parse_args()
    sys.path.insert(0, str(arguments.eyecite))
    import eyecite

    # eyecite logs every overlap it resolves; the reports carry what matters.
    logging.getLogger("eyecite").setLevel(logging.ERROR)
    import eyecite.models
    import eyecite.tokenizers

    revision = subprocess.run(
        ["git", "-C", str(arguments.eyecite), "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    from importlib.metadata import version

    source = f"eyecite {version('eyecite')} (git {revision or 'unknown'}), reporters-db {version('reporters-db')}, courts-db {version('courts-db')}"
    commit = subprocess.run(
        ["git", "-C", str(arguments.eyecite), "rev-parse", "HEAD"], capture_output=True, text=True, check=False
    ).stdout.strip()
    lock_path = Path(__file__).with_name("eyecite.lock.json")
    lock = json.loads(lock_path.read_text()) if lock_path.exists() else {}
    lock.update(
        {
            "commit": commit or lock.get("commit"),
            "eyecite": version("eyecite"),
            "reporters-db": version("reporters-db"),
            "courts-db": version("courts-db"),
        }
    )
    lock_path.write_text(json.dumps(lock, indent=2) + "\n")
    recorder = Recorder(eyecite)
    summary = {}
    for module in ("test_FindTest", "test_ModelsTest", "test_AnnotateTest", "test_UtilsTest"):
        summary[module] = run_module(recorder, module)
    summary["test_ResolveTest"] = run_module(recorder, "test_ResolveTest", patch_resolve=True)

    CASES.mkdir(parents=True, exist_ok=True)
    outputs = {
        "eyecite-find.json": (
            "eyecite tests/test_FindTest.py: every text the find tests pass to get_citations(), with real eyecite output mapped to the legal-citations model.",
            extract_cases(recorder.extract["test_FindTest"], recorder.skipped),
        ),
        "eyecite-models.json": (
            "eyecite tests/test_ModelsTest.py and test_UtilsTest.py extraction inputs (missing pages, corrected reporters, full spans).",
            extract_cases(recorder.extract["test_ModelsTest"] + recorder.extract["test_UtilsTest"], recorder.skipped),
        ),
        "eyecite-resolve.json": (
            "eyecite tests/test_ResolveTest.py: each checkResolution() row list joined into one text; antecedents from real eyecite resolve_citations().",
            resolve_cases(eyecite, recorder.resolve["test_ResolveTest"], recorder.skipped),
        ),
        "eyecite-annotate.json": (
            "eyecite tests/test_AnnotateTest.py annotate_citations() calls as annotate requests.",
            annotate_cases(recorder.annotate["test_AnnotateTest"], recorder.extract["test_AnnotateTest"], recorder.skipped),
        ),
        "eyecite-clean.json": (
            "eyecite clean_text() calls from tests/test_UtilsTest.py and test_AnnotateTest.py.",
            clean_cases(recorder.clean["test_UtilsTest"] + recorder.clean["test_AnnotateTest"], recorder.skipped),
        ),
    }
    for file_name, (description, cases) in outputs.items():
        write(CASES / file_name, source, description, cases)
        print(f"{file_name}: {len(cases)} cases")
    for module, (failed, ran) in summary.items():
        print(f"{module}: {ran} upstream tests ran, {failed} failed under the recorder")
    for line in sorted(set(recorder.skipped)):
        print(f"skipped: {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
