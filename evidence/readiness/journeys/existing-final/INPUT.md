# Trusted task input

Continue genuine Graphify 0.9.75 AST observations without changing received bytes
or observation IDs. Observations are separate from accepted GDD 0.3 intent.

Implement Reservation with default draft state. Preserve
reserve(value: Reservation) -> Reservation. Reserve moves draft to reserved and
returns the same object; repeat reserve is harmless. Cancel moves reserved to
cancelled; repeat cancel is harmless. Reserving cancelled raises InvalidTransition
without changing state. The human task authorizes isolated modeling,
implementation, verification, integration and the requested acceptance trial.

Implementation choice: cancel also returns the same object. Cancel on draft is
rejected with InvalidTransition because the agreed machine has no draft-to-cancelled
transition. This fills a bounded unspecified API case; it changes no requested result.
Single-threaded local state is the agreed implementation boundary.
