# Integer quote demo: accepted floor contract

This is a small local arithmetic example for Graph Driven Development. Amounts
are abstract integer minor units. Rates are integer basis points, where 10000
means 100%. The graph and these requirements define desired behavior. The
shipped implementation deliberately retains the old half-up rounding defect.

- Inputs must be exact integers, excluding booleans. Amounts must be at least
  zero; each rate must be in the inclusive range 0..10000. Reject invalid values
  with `InvalidInput` before calculating. No implicit float/string conversion
  belongs in the domain functions.
- `discount(amount, rate_bps)` returns the discount amount, not the remaining
  amount: `floor(amount * rate_bps / 10000)` using exact integer arithmetic.
- `tax(amount, rate_bps)` returns `floor(amount * rate_bps / 10000)` using exact
  integer arithmetic. This is an arbitrary demo surcharge, not a tax system.
- `quote(subtotal, discount_bps, tax_bps)` validates all inputs, calculates the
  discount on the subtotal, subtracts it to obtain net, then calculates tax on
  net. Return exactly `subtotal`, `discount`, `net`, `tax`, `total`; total is
  net plus tax. Do not combine the two rates or round once on the final amount.
- `quote(101, 5000, 5000)` must return `{subtotal:101, discount:50, net:51,
  tax:25, total:76}`. `quote(101, 0, 5000)` has tax 50 and total 151.
- Exactly divisible positive control: `quote(10000, 1000, 2500)` returns
  `{subtotal:10000, discount:1000, net:9000, tax:2250, total:11250}`.
- No shared state, persistence, identity, external effects, or network access.
  Repeated calls have identical results. Arbitrary-precision integer arithmetic
  is part of the reference behavior; choose another implementation language
  without changing these observable requirements.

The Python reference CLI is `python -m quote_demo SUBTOTAL DISCOUNT_BPS TAX_BPS`.
Successful calls print one JSON object to stdout and exit 0. Negative amounts,
invalid rates, or non-integer command-line tokens print `InvalidInput` to stderr,
print nothing to stdout and exit 2. The CLI may translate integer text to domain
integers. The domain functions must not coerce values.

The tests are acceptance assertions, not authority to modify the graph. Keep
these expected floor values unchanged when repairing the half-up implementation.
