# Existing code → Graphify → GDD proposal

[code/reservation.py](code/reservation.py) is a small code fixture, not a concert application. It was actually extracted by installed Graphify 0.9.75 without an LLM pipeline.

| Artifact | Nodes | Edges | Production |
| --- | ---: | ---: | --- |
| [raw.ast.json](raw.ast.json) | 3 | 5 | extract_python |
| [export.directed.json](export.directed.json) | 3 | 5 | directed build + to_json(original_links) |
| [export.undirected.json](export.undirected.json) | 3 | 5 | undirected build + to_json(original_links) |
| [export.collapsed.json](export.collapsed.json) | 3 | 3 | simple graph + to_json without original_links |

The three parallel reserve→Reservation records are parameter_type-reference, return_type-reference, and calls. The last export demonstrates loss before the adapter's input; the adapter cannot invent the missing records. [producer-evidence.json](producer-evidence.json) records actual input/artifact hashes, package versions, and producer source hashes. There is no known Git code revision or performed business verification.

[The observation bundle](observations/observations.json) is the importer's actual output from raw.ast.json. [mapping.json](mapping.json) is an explicit, informative interpretation of the module, reserve, and the constructor. It is used for the [unaccepted proposal](observations/proposal.json), which contains GDD sketches and open contract questions. It is not an automatically generated accepted standard model.

Use the [installed Rust CLI](../../INSTALL.md#install-the-rust-cli-and-viewer)
from the repository root. Choose new paths for the recovered graph and proposal.

```sh
gdd validate examples/graphify/observations --profile import.graphify
gdd graphify export examples/graphify/observations --output recovered.json
gdd graphify propose examples/graphify/observations \
  --mapping examples/graphify/mapping.json --output proposal.json
```

To reproduce the original extraction, use the [v0.8.0 producer script](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/conformance/generate_graphify_fixture.py)
in a separate checkout. Set `GRAPHIFY_PYTHON` to the absolute path of an
interpreter with `graphifyy==0.9.75`, `networkx==3.7`, `tree-sitter==0.25.2`
and `tree-sitter-python==0.25.0`. The script rewrites the producer artifacts
and evidence in that checkout.

```sh
GDD_PRODUCER_TREE="$(mktemp -d)/gdd"
git worktree add --detach "$GDD_PRODUCER_TREE" 0dbb10b8c193f55c9777405de3f4ae22ac2d0825
(
  set -e
  cd "$GDD_PRODUCER_TREE"
  "$GRAPHIFY_PYTHON" conformance/generate_graphify_fixture.py
  git diff -- examples/graphify
)
```

Graphify is not a runtime dependency of the GDD adapter. See [GRAPHIFY.md](../../GRAPHIFY.md) for profiles, direction, trust boundaries, and the agent workflow.
