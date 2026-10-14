# Parcel price CLI

Use Python 3.11 or newer with no application dependencies:

```sh
PYTHONPATH=src python3 -B -m parcel.cli 1001 domestic
# 200
PYTHONPATH=src python3 -B -m parcel.cli 1001 remote
# 350
python3 -B tests/test_b.py src
python3 -B tests/check_architecture.py src
```

The current contract B adds one 50-cent remote handling fee. Amounts are integer
cents. Weight must be a positive integer in grams; invalid input exits 2.
The rate table remains 100 cents domestic and 150 cents remote per begun kg.

`tests/test_b.py` is the current fixed acceptance suite. `tests/test_a.py` is the
preserved historical A contract and is intentionally not selected for B. Run
it against `history/a/src` to reproduce A. Fixed B tests against
`history/b-red/src` reproduce the old-code defect. The explicit commands above
are the supported checks; generic test discovery is not configured.

See [JOURNEY.md](JOURNEY.md) for model, workflow, revisions, trust boundaries,
raw check receipts, negative controls and limitations. All artifacts are flat
files. The offline graph is [evidence/b-graph.html](evidence/b-graph.html).
