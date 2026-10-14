# Concert reservations: accepted local demonstration

The human authorized a working GDD 0.3 concert app, an explicit requirement change,
and an injected architecture violation. This document is intent, not execution evidence.
The app runs on loopback with Python, a file-backed SQLite database, and a plain browser
interface. Use only fictional demo identities and seeded fictional concerts. Payment,
ticket delivery, expiry, external authentication and public deployment are outside scope.

## Phase A

One reservation occupies one seat. Capacity is fixed. Several reservations with
different request keys are permitted, with no per-person limit in phase A.

Authenticate demo credentials on the server and issue unpredictable opaque cookies.
Only a server-resolved principal can reserve, cancel or list personal history. Alice
and Bob are separate fictional accounts. Their documented demo passwords provide a
reproducible local identity flow. Tokens are held in memory and restart signs users out.
Never accept owner_id or principal_id from a reservation request as authentication.

Successful request keys are scoped to the principal and reserve operation and retained
permanently with their reservation. An identical retry returns the same ID and current
state before checking capacity. A successful key reused for another concert conflicts.
A cancelled reservation is never reactivated by replay. Failed creates bind no key.

Capacity validation, creation and key binding form one atomic transaction. Concurrent
connections must behave as a valid sequential order. Failure before commit rolls back
all effects. Count actual reserved rows; do not maintain a divergent seat counter.

Only the owner can cancel. Missing and foreign IDs return the same ReservationNotFound
response, without reservation data, including already cancelled foreign reservations.
Repeated cancellation has no additional effect. Successful keys/history remain stored.

The public catalog shows fictional event metadata and current availability. Availability
is a snapshot; the server decides whether a reservation succeeds. Personal listing
returns only the authenticated principal's reserved/cancelled history and active count.

## Requirement change: Phase B

Each principal may have at most two reserved reservations across all concerts.
Cancelled history does not consume allowance. Keep successful replay and key-conflict
handling before the new limit. For a new key: confirm concert existence, check the
global principal limit, then check capacity, and insert/bind in the same protected
transaction. ActiveReservationLimitReached wins over SoldOut if both apply.

Cancellation frees allowance. Failed-limit keys remain unbound and may succeed after
cancellation. Parallel requests across different concerts compete for one remaining
allowance; exactly one may succeed. Publish the domain-owned nullable active_limit in
the personal-list response (null in A, 2 in B); the UI must not duplicate the policy.

Do not silently delete or cancel pre-existing history to meet a changed policy.
Validate existing data before serving phase B; an over-limit database requires explicit
reconciliation and must be left unchanged. The experiment demonstrates a compatible
phase-A database transitioning to B and separately tests rejection of incompatible data.

## Boundaries and acceptance

API translates HTTP and calls public Identity/Reservations operations. Identity owns
credential checking and session revocation. Reservations owns domain decisions.
Storage owns physical SQL and transaction mechanics. Frontend owns presentation.
API must never import Storage directly. A permitted indirect API -> Reservations ->
Storage path remains legal. Extract actual static imports using explicit Python bindings.
Inject a forbidden direct import in an isolated copy, observe FAIL, repair and rescan.
Missing files, ambiguous bindings and unsupported dynamic imports remain UNKNOWN.

Accept each phase only after actual database/HTTP tests, actual browser flows, current
architecture evidence, and a concrete advisory design review. Preserve phase-A
executions and revisions when B changes the graph. Advisory review is not proof of
correctness or a replacement for blocking checks.
