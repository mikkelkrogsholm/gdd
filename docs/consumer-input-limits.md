# Consumer input limits

The validator's JSON/YAML loader accepts at most **256 nested containers**.
A root mapping or sequence has depth 1; each nested mapping or sequence adds
one. Scalars, mapping-key text and brackets inside strings do not add depth.
The limit covers model manifests, fragments, record files and other inputs
read through this loader. It is an implementation resource limit, not a change
to the GDD format or a declaration that deeper documents are invalid.

JSON receives a string-aware depth preflight before recursive decoding. YAML
receives an event-based depth check before recursive composition. No process
recursion limit is changed. Exceeding this limit produces `FORMAT_UNSUPPORTED`
and an `UNKNOWN` result; defensive handling of a runtime `RecursionError`
retains the same classification. Acceptance by the loader does not guarantee
that every recursive schema or later consumer operation has sufficient runtime
capacity. Shallow invalid syntax and forbidden features retain their existing
failure diagnostics.

Graphify's raw-byte adapter has its own documented projection capacity and
retains original bytes when projection is unresolved. This loader limit does
not replace that adapter's policy.

Local path resolution detects symlink loops before reading assets, including
on Python versions whose non-strict `Path.resolve()` suppresses loop errors.
A loop is a known invalid local path (`ASSET_PATH_INVALID`, `FAIL`). A genuinely
missing input remains unavailable (`SOURCE_UNAVAILABLE`, `UNKNOWN`); resolution
does not require every referenced asset to exist. Valid symlinks remain subject
to the existing bundle-root containment checks.
