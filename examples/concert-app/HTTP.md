# HTTP and Python boundary contracts

All routes are same-origin local HTTP. JSON request bodies are UTF-8 objects, limited
to 16 KiB. Use cookies with HttpOnly, SameSite=Strict and Path=/. Set no CORS permission.
Reject unexpected Host and Origin before mutation. Request-body identity fields are
ignored; a resolved session supplies the principal. All messages and assets are English.

## HTTP

| Route | Request | Successful JSON |
| --- | --- | --- |
| GET /api/session | cookie if present | {"principal": null} or {"principal": {"id":"alice","name":"Alice"}} |
| POST /api/login | {"username":"alice","password":"alice-demo"} | {"principal":{"id":"alice","name":"Alice"}} plus an opaque session cookie |
| POST /api/logout | {} and cookie | {"principal":null}; revoke token and expire cookie |
| GET /api/concerts | none; public | {"concerts":[{"id":"midnight","title":"Midnight Sessions","date":"2026-11-14","venue":"Harbor Hall","capacity":8,"available":8}]} |
| GET /api/reservations | valid session | {"reservations":[{"id":"...","concert_id":"midnight","state":"reserved"}],"active_count":1,"active_limit":null} |
| POST /api/reservations | {"concert_id":"midnight","request_key":"unique-client-key"} | {"id":"...","state":"reserved"}; replay can return cancelled |
| POST /api/reservations/ID/cancel | {} and valid session | {"id":"...","state":"cancelled"} |

Bob's fictional credentials are bob / bob-demo. Invalid credentials yield 401
InvalidCredentials and establish no session. A session query with an unknown token
returns principal=null. Logout is idempotent. Authenticated reads and mutations without
a valid session yield 401 Unauthenticated. Login failure must not preserve a previously
authenticated UI presentation as if the new identity had succeeded.

Errors use {"error":{"code":"SoldOut","message":"No seats are available."}}.
InvalidInput and malformed/oversized JSON:400; missing concert/reservation:404;
SoldOut, IdempotencyConflict and ActiveReservationLimitReached:409;
PersistenceUnavailable:503; rejected cross-origin/host:403. Unsupported routes:404.
Do not expose SQL, stack traces, passwords, tokens or another person's reservation.

Render catalog/history as text. Disable a pending action; retain its reserve key on
an uncertain network outcome, so retry cannot double-book. Clear personal data when
identity changes and ignore stale responses from a previous identity. Display a
server-provided active limit; preserve access to cancellation when at the limit.

## Fixed Python interfaces for parallel agent work

`concert_app.storage` owns SQL. It exports initialize(database),
transaction(database, write=False) (a context manager yielding a connection),
get_concert(connection, concert_id), list_concerts(connection),
find_by_key(connection, principal_id, request_key),
get_reservation(connection, reservation_id), list_for_principal(connection, principal_id),
count_active(connection, concert_id=None, principal_id=None),
insert_reservation(connection, principal_id, concert_id, request_key),
cancel_reservation(connection, reservation_id), and validate_active_limit(database, limit).
Rows are dictionaries. Reservation rows include id, concert_id, principal_id,
request_key and state; public responses omit private principal/key fields.
PersistenceError is its SQLite error type. initialize seeds the documented three
fictional concerts only when missing and never overwrites existing capacity/history.
Write transactions use BEGIN IMMEDIATE and rollback on every precommit failure.

`concert_app.reservations` exports initialize(database), catalog(database),
list_own(database, principal_id), reserve(database, principal_id, concert_id, request_key),
cancel(database, principal_id, reservation_id), DomainError, and ACTIVE_LIMIT.
ACTIVE_LIMIT is None in A and 2 in B. initialize calls storage initialization and
validates compatibility with the current limit. catalog returns the HTTP catalog
wrapper; list_own returns the HTTP history/count/limit wrapper. reserve/cancel return
only the public id/state pair. DomainError has code and message. Validate identity
before any persistence access. Translate database failures to PersistenceUnavailable.

`concert_app.identity` exports SessionStore with login(username,password),
profile(token), principal(token), logout(token). login returns (opaque_token, profile)
or raises IdentityError(code,message); profile returns {id,name} or None; principal
returns the server-owned principal ID or None. SessionStore owns its thread-safe map.

`concert_app.api` provides make_server(database, host='127.0.0.1', port=0) returning
a ThreadingHTTPServer. Static files are served from concert_app/static. The main
entry point starts this server with an explicit database and port. Tests can use
make_server with an ephemeral port and call shutdown/server_close after use.

## Phase-B session presentation consistency

Browser tabs share the session cookie. On private reads, reserve, cancel and logout,
the browser sends X-Concert-Principal with the identity currently presented to the
user. The server resolves the cookie first, then rejects a different optional
expectation with 409 SessionChanged before reading private data or applying effects.
This header has no authentication or ownership authority; headerless script clients
still use the server-resolved cookie principal. Unknown sessions remain 401.
After SessionChanged the browser clears personal presentation and requires explicit
sign-in. Logout captures its expected identity before clearing the local view and
must not revoke an unexpectedly changed session. The real browser suite exercises
two tabs sharing a context, including a stale reserve and logout.
