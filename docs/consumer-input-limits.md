# Consumer input limits

The validator's JSON/YAML loader accepts at most **256 nested containers**.
A root mapping or sequence has depth 1; each nested mapping or sequence adds
one. Scalars, mapping-key text and brackets inside strings do not add depth.
The limit covers model manifests, fragments, record files and other inputs
read through this loader. It is an implementation resource limit, not a change
to the GDD format or a declaration that deeper documents are invalid.

The native parser checks the container depth while reading JSON or YAML
events. Exceeding the limit yields `FORMAT_UNSUPPORTED` and UNKNOWN. Shallow
syntax errors and forbidden features retain their own diagnostics. Acceptance
by the loader does not guarantee capacity for downstream schema checks.

Graphify's raw-byte adapter has its own documented projection capacity and
retains original bytes when projection is unresolved. This loader limit does
not replace that adapter's policy.

Local path resolution detects symlink loops when reading model assets.
A loop is a known invalid local path (`ASSET_PATH_INVALID`, `FAIL`). A genuinely
missing input remains unavailable (`SOURCE_UNAVAILABLE`, `UNKNOWN`); resolution
does not require every referenced asset to exist. Valid symlinks remain subject
to the existing bundle-root containment checks.
