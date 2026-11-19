<!-- gdd-navigation/0.1 project=project.invoices sha256=99a1cba5dfff29b301962c821693265afc9cb1addbc52f033c5bbb790e7d8e98 -->
# Invoice calculation

Graph navigation for `project.invoices`.

## Nodes

<a name="gdd-node-action.total"></a>

### `action.total`

action · Calculate the invoice total.

- [Invoice display request · Invoice display request / Per-line amounts](<./wiki/requests.md>) · provenance (pinned)
- [Invoice rounding · Invoice rounding / Calculation](<./wiki/rounding.md#calculation>) · contract (pinned), provenance (pinned)

<a name="gdd-node-component.billing"></a>

### `component.billing`

component · Own invoice calculations.

<a name="gdd-node-scenario.total"></a>

### `scenario.total`

scenario · Two halfway line amounts round once after summation.

- [Invoice rounding · Invoice rounding / Calculation](<./wiki/rounding.md#calculation>) · provenance (pinned)

<a name="gdd-node-scenario.whole-units"></a>

### `scenario.whole-units`

scenario · Whole-unit amounts are preserved in the total.

## Edges

<a name="gdd-edge-edge.owner"></a>

### `edge.owner`

[`action.total`](#gdd-node-action.total) → [`component.billing`](#gdd-node-component.billing) · owned\_by

<a name="gdd-edge-edge.scenario"></a>

### `edge.scenario`

[`scenario.total`](#gdd-node-scenario.total) → [`action.total`](#gdd-node-action.total) · exercises

<a name="gdd-edge-edge.whole-units"></a>

### `edge.whole-units`

[`scenario.whole-units`](#gdd-node-scenario.whole-units) → [`action.total`](#gdd-node-action.total) · exercises

## Questions

<a name="gdd-question-question.rounding"></a>

### `question.rounding`

open · Should displaying rounded lines also change the amount charged?

- [Invoice display request · Invoice display request / Per-line amounts](<./wiki/requests.md>) · provenance (pinned)
