# Registry source data

`upstream/reporters-db/` contains the original reporters, laws, journals and
regexes files. `upstream/courts-db/` contains the original courts and states
files. Their bytes match `upstream.lock`; `tools/sync-upstream.py --check` checks
the hashes and generated grammar. Update the sources with that helper.

The `registry` API exposes `courts`, `reporters`, `laws`, `journals`,
`jurisdictions`, `states` and `regexes`. Reporters, laws and journals retain
their abbreviation keys and arrays of records. Courts retain courts-db IDs.
Every upstream field passes through, including fields unused by extraction.
There is no separate renamed upstream schema or exported CanLII route file.

Authored Canadian and Commonwealth records use string names, keyed reporter
editions and courts-db level codes. Optional extensions carry French names,
neutral identifiers, CanLII routes, numbering regimes and verified status.
`region` describes geographic scope; courts-db's `jurisdiction` is retained
with its original meaning. `hierarchy` is a derived ranking category, distinct
from a court's original `level` and `type`.

`build_registry.rs` compiles matching records and compresses the complete
source metadata separately. Extraction loads only the matching records.
Registry inspection loads the source metadata on demand. Generated snapshots
live in Cargo's canonical output directory, not in the checkout.

Source identity and authority identity are separate. A reporter's public
identity is its original abbreviation key and record; its `citation_id` in
authored build input supplies the existing key-v3 encoding. That encoding is
not exposed as a replacement source ID. Keeping it stable preserves saved
authority keys and product output while the registry schema changes.

`edition_order` preserves authored edition precedence independently of JSON
object ordering. Upstream edition precedence comes from dates and abbreviation.
CanLII source routes and court hierarchy consumers read this registry; they do
not maintain another court table.
