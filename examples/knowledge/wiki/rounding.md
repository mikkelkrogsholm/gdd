---
type: engineering-standard
title: Invoice rounding
description: The calculation used by the invoice total action.
tags: [billing, rounding]
---
# Invoice rounding

## Calculation

Sum line amounts in signed tenths of a minor currency unit. Round the sum to the nearest whole minor unit, with exact ties away from zero. Two amounts of 15 tenths give a total of 3 minor units. Rounding the two lines separately would give 4.

Apply this rule to the [invoice total](../gdd-graph.md#gdd-node-action.total). The [two-line example](../gdd-graph.md#gdd-node-scenario.total) distinguishes the two algorithms.

## Presentation

The presentation layer chooses the currency symbol. That choice is outside the calculation.
