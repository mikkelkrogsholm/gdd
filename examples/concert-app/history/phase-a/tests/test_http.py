"""Exercise the real threaded HTTP boundary, session authority and persistence."""

from concurrent.futures import ThreadPoolExecutor
from http.client import HTTPConnection
import json
from pathlib import Path
from threading import Thread

import pytest

from concert_app.api import make_server
from concert_app.identity import SessionStore, IdentityError
from concert_app import reservations


@pytest.fixture
def server(tmp_path):
    app = make_server(tmp_path / "concert.sqlite")
    worker = Thread(target=app.serve_forever, daemon=True)
    worker.start()
    yield app
    app.shutdown()
    app.server_close()
    worker.join(timeout=5)


def request(server, method, path, body=None, token=None, headers=None, raw=None):
    connection = HTTPConnection("127.0.0.1", server.server_port, timeout=5)
    values = {"Content-Type": "application/json"}
    if token:
        values["Cookie"] = "concert_session=" + token
    values.update(headers or {})
    data = raw if raw is not None else (json.dumps(body or {}).encode() if method == "POST" else None)
    connection.request(method, path, body=data, headers=values)
    response = connection.getresponse()
    payload = response.read()
    result = (response.status, dict(response.getheaders()), json.loads(payload) if "application/json" in response.getheader("Content-Type", "") else payload)
    connection.close()
    return result


def login(server, username="alice"):
    status, headers, body = request(server, "POST", "/api/login", {"username": username, "password": username + "-demo"})
    assert status == 200
    assert body == {"principal": {"id": username, "name": username.title()}}
    cookie = headers["Set-Cookie"]
    assert "HttpOnly" in cookie and "SameSite=Strict" in cookie and "Path=/" in cookie
    return cookie.split(";", 1)[0].split("=", 1)[1]


def reserve(server, token, key, concert="midnight", **extra):
    return request(server, "POST", "/api/reservations", {"concert_id": concert, "request_key": key, **extra}, token)


def test_public_catalog_and_private_session(server):
    status, headers, catalog = request(server, "GET", "/api/concerts")
    assert status == 200 and len(catalog["concerts"]) == 3
    assert headers["Cache-Control"] == "no-store"
    assert "Access-Control-Allow-Origin" not in headers
    assert request(server, "GET", "/api/session")[2] == {"principal": None}
    assert request(server, "GET", "/api/session", token="invented")[2] == {"principal": None}
    token = login(server)
    assert request(server, "GET", "/api/session", token=token)[2]["principal"]["id"] == "alice"


def test_login_failure_logout_and_session_replacement(server):
    token = login(server)
    failed = request(server, "POST", "/api/login", {"username": "alice", "password": "wrong"}, token)
    assert failed[0] == 401 and failed[2]["error"]["code"] == "InvalidCredentials"
    assert request(server, "GET", "/api/session", token=token)[2]["principal"] is None
    token, other = login(server), login(server, "bob")
    status, headers, body = request(server, "POST", "/api/logout", token=token)
    assert status == 200 and body == {"principal": None} and "Max-Age=0" in headers["Set-Cookie"]
    assert request(server, "POST", "/api/logout", token=token)[0] == 200
    assert request(server, "GET", "/api/session", token=other)[2]["principal"]["id"] == "bob"
    assert request(server, "GET", "/api/reservations", token=token)[0] == 401


@pytest.mark.parametrize("route,method", [("/api/reservations", "GET"), ("/api/reservations", "POST"), ("/api/reservations/missing/cancel", "POST")])
def test_private_operations_require_real_session(server, route, method):
    result = request(server, method, route, {"principal_id": "alice", "owner_id": "alice", "concert_id": "midnight", "request_key": "forged"})
    assert result[0] == 401 and result[2]["error"]["code"] == "Unauthenticated"


def test_foreign_cancel_is_indistinguishable_from_missing_and_history_is_private(server):
    alice, bob = login(server), login(server, "bob")
    created = reserve(server, alice, "a")[2]
    foreign = request(server, "POST", f"/api/reservations/{created['id']}/cancel", {"principal_id": "alice", "owner_id": "alice"}, bob)
    missing = request(server, "POST", "/api/reservations/missing/cancel", token=bob)
    assert foreign[0] == missing[0] == 404 and foreign[2] == missing[2]
    assert request(server, "GET", "/api/reservations", token=bob)[2]["reservations"] == []
    own = request(server, "GET", "/api/reservations", token=alice)[2]
    assert own["reservations"] == [{**created, "concert_id": "midnight"}]
    assert "principal_id" not in json.dumps(own) and "request_key" not in json.dumps(own)
    request(server, "POST", f"/api/reservations/{created['id']}/cancel", token=alice)
    assert request(server, "POST", f"/api/reservations/{created['id']}/cancel", token=bob)[2] == missing[2]


def test_http_replay_cancel_and_cross_principal_key(server):
    alice, bob = login(server), login(server, "bob")
    first = reserve(server, alice, "same")[2]
    assert reserve(server, alice, "same")[2] == first
    assert reserve(server, alice, "same", "northern")[0] == 409
    assert reserve(server, bob, "same")[2]["id"] != first["id"]
    cancelled = request(server, "POST", f"/api/reservations/{first['id']}/cancel", token=alice)[2]
    assert cancelled["state"] == "cancelled" and reserve(server, alice, "same")[2] == cancelled


def test_current_policy_and_ignored_body_identity(server):
    token = login(server, "bob")
    for i, concert in enumerate(("midnight", "northern", "matinee")):
        result = reserve(server, token, f"limit-{i}", concert, principal_id="alice", owner_id="alice")
        assert result[0] == (409 if reservations.ACTIVE_LIMIT == 2 and i == 2 else 200)
    own = request(server, "GET", "/api/reservations", token=token)[2]
    assert own["active_count"] == (2 if reservations.ACTIVE_LIMIT == 2 else 3)
    assert own["active_limit"] == reservations.ACTIVE_LIMIT
    assert request(server, "GET", "/api/reservations", token=login(server))[2]["reservations"] == []


@pytest.mark.parametrize("raw", [b"[1]", b"{broken", b'"text"', b'{"x":NaN}', b'{"x":Infinity}', b"\xff", b"x" * 16385], ids=["array", "malformed", "string", "nan", "infinity", "utf8", "oversized"])
def test_malformed_and_oversized_json(server, raw):
    result = request(server, "POST", "/api/login", raw=raw)
    assert result[0] == 400 and result[2]["error"]["code"] == "InvalidInput"


def test_content_type_and_cross_origin_host(server):
    assert request(server, "POST", "/api/login", headers={"Content-Type": "text/plain"})[0] == 400
    for header in ({"Origin": "https://evil.invalid"}, {"Origin": "null"}, {"Host": "evil.invalid"}):
        assert request(server, "POST", "/api/login", {"username": "alice", "password": "alice-demo"}, headers=header)[0] == 403
    good = {"Origin": f"http://127.0.0.1:{server.server_port}"}
    assert request(server, "POST", "/api/login", {"username": "alice", "password": "alice-demo"}, headers=good)[0] == 200


def test_duplicate_headers_and_transfer_encoding_are_rejected(server):
    for name, values, expected in [("Host", ["evil.invalid"], 403), ("Origin", ["http://evil.invalid", "null"], 403), ("Content-Length", ["2", "2"], 400), ("Transfer-Encoding", ["chunked"], 400)]:
        connection = HTTPConnection("127.0.0.1", server.server_port, timeout=5)
        connection.putrequest("POST", "/api/login")
        connection.putheader("Content-Type", "application/json")
        for value in values:
            connection.putheader(name, value)
        if name != "Content-Length":
            connection.putheader("Content-Length", "2")
        connection.endheaders(b"{}")
        response = connection.getresponse()
        assert response.status == expected
        response.read()
        connection.close()


def test_static_allowlist_and_unknown_routes(server):
    assert request(server, "GET", "/")[0] == 200
    for route in ("/../identity.py", "/%2e%2e/identity.py", "/api/unknown"):
        assert request(server, "GET", route)[0] == 404
    assert request(server, "POST", "/api/unknown")[0] == 404


def test_restart_keeps_reservations_and_revokes_in_memory_sessions(tmp_path):
    database = tmp_path / "restart.sqlite"
    first = make_server(database)
    worker = Thread(target=first.serve_forever, daemon=True)
    worker.start()
    token = login(first)
    created = reserve(first, token, "persist")[2]
    first.shutdown()
    first.server_close()
    worker.join()
    second = make_server(database)
    worker = Thread(target=second.serve_forever, daemon=True)
    worker.start()
    try:
        assert request(second, "GET", "/api/session", token=token)[2]["principal"] is None
        own = request(second, "GET", "/api/reservations", token=login(second))[2]
        assert own["reservations"][0]["id"] == created["id"]
    finally:
        second.shutdown()
        second.server_close()
        worker.join()


def test_session_registry_concurrency_and_public_copy():
    sessions = SessionStore()
    with ThreadPoolExecutor(max_workers=8) as pool:
        pairs = list(pool.map(lambda _: sessions.login("alice", "alice-demo"), range(32)))
    assert len({token for token, _ in pairs}) == 32
    token, profile = pairs[0]
    profile["id"] = "bob"
    assert sessions.principal(token) == "alice"
    sessions.profile(token)["id"] = "bob"
    assert sessions.principal(token) == "alice"
    with pytest.raises(IdentityError):
        sessions.login(["alice"], "alice-demo")


def test_public_bind_is_rejected(tmp_path):
    with pytest.raises(ValueError, match="loopback"):
        make_server(tmp_path / "unused.sqlite", "0.0.0.0")
