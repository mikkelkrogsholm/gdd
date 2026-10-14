# Preserved concert experiment

The complete `examples/concert-app/` tree at Git commit
`7e27757c34b6fa57ecc69660e263c99e1420649b` is preserved here:

- [Deterministic compressed bundle](concert-7e27757.tar.gz): 197 files, 1,732,377 bytes.
- [Byte index](concert-7e27757.index.json): original/retrieval paths, SHA256, sizes,
  Git blob identities, file modes, commit and source-tree identity.
- [Recovery verification](concert-7e27757.recovery-verification.json): actual new
  replay of the recovered original tools with explicit installed runtime paths.
- [Curation manifest](concert-7e27757.curation.json): exact removed paths and the
  recovery/replay gate satisfied before removal.

The bundle SHA256 is
`4888e79870a157e9521e2a5ef226bcbd076fda73c45e2c3c7ee9291d34b50d2d`.
Compression uses zero timestamps, sorted files and normalized owner metadata.
All recovered content was compared with its Git blob identity and byte index;
no historical receipt, model or asset was translated or rewritten.

From the repository root:

```sh
python scripts/concert_archive.py check \
  --archive archive/experiments/concert-7e27757.tar.gz \
  --index archive/experiments/concert-7e27757.index.json
python scripts/concert_archive.py restore \
  --archive archive/experiments/concert-7e27757.tar.gz \
  --index archive/experiments/concert-7e27757.index.json \
  --output "$PWD/../concert-original-recovered"
```

Use a new destination whose parent already exists. Output and its ancestors must
not be symlinks; use a canonical parent path on systems where `/tmp` or `/var` is
a symlink. The helper verifies every indexed regular file before writing, rejects
links/traversal/duplicate members, and refuses all existing output paths. The
index is the integrity trust input; keep its Git provenance when downloading.

Recovered files appear under `examples/concert-app/`. Original validator,
schema and standard dependencies remain retrievable from the same pinned Git
commit (`git show COMMIT:PATH` or `git archive COMMIT`); the bundle contains only
the concert experiment subtree. The recovery trial deliberately used the
current copied validator. Its exact dependency hashes, command, configuration,
results and separate execution location are in the recovery verification.

To replay the recovered original tools, first install GDD and test dependencies
in the checkout and configure Node/Playwright/browser executables as described
in the [current reference](../../examples/concert-app/README.md). Set both
`GDD_PLAYWRIGHT` and `GDD_CHROME` explicitly: recovered historical scripts retain
their original personal defaults. Use a fresh output destination:

```sh
uv run python ../concert-original-recovered/examples/concert-app/tools/replay.py \
  --output /tmp/concert-original-replay --node node
```

This creates a new execution using recovered delivered patches. The saved
original journals and records still describe their own historical execution.
App/model revisions may match while checker revisions differ; never silently
rebind old source hashes to portable current tools.
