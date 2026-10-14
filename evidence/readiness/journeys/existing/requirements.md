# Accepted Reservation requirements

Human task: build a Reservation lifecycle preserving the public
reserve(value: Reservation) -> Reservation signature. Reservation defaults to
draft. reserve changes draft to reserved and returns the same object. Repeated
reserve on reserved is harmless. cancel changes reserved to cancelled; repeated
cancel is harmless. reserve of cancelled rejects with InvalidTransition.

The task authorizes modeling, implementation, verification, and acceptance in
this isolated project. The observed Graphify graph and current implementation
are provenance, not accepted requirements.

Bounded agent decisions: expose cancel(value: Reservation) -> Reservation as a
module function returning the same object. Reject cancel of draft and unsupported
state values with InvalidTransition, preserving state. Use in-memory, sequential
calls only; no persistence, identity permissions, or thread-safety guarantee.
