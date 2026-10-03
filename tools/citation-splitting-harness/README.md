# Citation splitting harness drafts

This directory preserves a draft evaluator and experiment ledger for integration
review. It does not change the production parser. No tests, builds, or corpus
evaluations were run for this preservation handoff, and no readiness or accuracy
improvement is claimed.

The six Python files are unchanged from the retained implementation. The two
contract documents were sanitized to remove run-specific paths, deadlines,
corpus references, status claims, and historical result counts.

## Retained work

- `evaluate.py` calls the real CLI and measures exact spans, internal boundaries,
  and complete partitions, with mandatory raw character preservation. Extraction
  coverage is scored separately.
- `ledger.py` records hashed experiment evidence, binds builds to source, and
  requires strict gains while preserving original and incumbent successes.
- `check_conformance.py` checks for regressions against separately captured
  baseline metadata while retaining the original runner's return code.
- `test_evaluate.py`, `test_ledger.py`, and `test_check_conformance.py` preserve
  authored harness tests. Embedded strings, synthetic records, and `.invalid`
  URL/email values are invented test doubles, not corpus-derived examples.
- `evaluator-contract.txt` and `PROTOCOL.txt` document proposed scoring and
  experiment controls. They are drafts, not frozen benchmark contracts.

## Unfinished portability

The original layout assumed a sibling `common-law-cite` checkout. In this new
directory, defaults using `ROOT.parent / "common-law-cite"` do not locate the
repository correctly. `evaluate.py` also assumes fixed fixture, sealed-input,
manifest, initial-source, receipt, and lock paths beside the script. The ledger's
path overrides are not fully connected to evaluator inputs. Conformance checks
require fresh runtime baseline metadata. These limitations remain unresolved.

Before executing an experiment, integrate repository and private runtime path
configuration, adapt the authorized corpus schema without changing established
source-family assignments, capture fresh baseline evidence, and review and
freeze the evaluator and manifests. Keep private runtime inputs and outputs
outside tracked source. The commands in the draft contracts are illustrative
and require that integration work first.

No documents, corpus fixtures, gold or sealed data, raw logs, receipts, attempt
history, credentials, signed URLs, binaries, caches, toolchains, or download
material are included. References to runtime filenames do not supply those
files. No source corpus is needed to preserve or review these drafts.
