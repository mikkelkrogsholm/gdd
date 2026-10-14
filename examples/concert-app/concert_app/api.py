"""Translate local HTTP requests into identity and reservation public operations."""

from http.cookies import SimpleCookie, CookieError
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import re
from urllib.parse import urlsplit

from . import identity, reservations

STATIC = Path(__file__).parent / "static"
MAX_BODY = 16 * 1024
STATUS = {
    "Unauthenticated": 401, "InvalidCredentials": 401, "InvalidInput": 400,
    "ConcertNotFound": 404, "ReservationNotFound": 404, "SoldOut": 409,
    "IdempotencyConflict": 409, "ActiveReservationLimitReached": 409,
    "PersistenceUnavailable": 503, "Forbidden": 403, "NotFound": 404, "SessionChanged": 409,
}


class RequestError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code, self.message = code, message


class ConcertServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, database, host, port):
        reservations.initialize(database)
        self.database = str(database)
        self.sessions = identity.SessionStore()
        super().__init__((host, port), Handler)


class Handler(BaseHTTPRequestHandler):
    server: ConcertServer

    def log_message(self, format, *args):
        """Do not log URLs, credentials, cookies, or personal identifiers."""

    def _headers(self, status, content_type, cookie=None, length=None):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        if length is not None:
            self.send_header("Content-Length", str(length))
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "same-origin")
        self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'")
        if cookie:
            self.send_header("Set-Cookie", cookie)
        self.end_headers()

    def _json(self, status, value, cookie=None):
        payload = json.dumps(value, ensure_ascii=False, allow_nan=False).encode("utf-8")
        self._headers(status, "application/json; charset=utf-8", cookie, len(payload))
        self.wfile.write(payload)

    def _token(self):
        if len(self.headers.get_all("Cookie", [])) > 1:
            return None
        cookie = SimpleCookie()
        try:
            cookie.load(self.headers.get("Cookie", ""))
        except CookieError:
            return None
        morsel = cookie.get("concert_session")
        return morsel.value if morsel else None

    def _principal(self):
        principal = self.server.sessions.principal(self._token())
        if principal is None:
            raise RequestError("Unauthenticated", "Sign in to manage your reservations.")
        self._expect_principal(principal)
        return principal

    def _expect_principal(self, principal):
        expectations = self.headers.get_all("X-Concert-Principal", [])
        if len(expectations) > 1:
            raise RequestError("InvalidInput", "Send at most one identity expectation.")
        if principal is not None and expectations and expectations[0] != principal:
            raise RequestError("SessionChanged", "The session changed in another tab. Sign in again.")

    def _local_request(self):
        hosts = self.headers.get_all("Host", [])
        port = self.server.server_port
        allowed = {f"127.0.0.1:{port}", f"localhost:{port}"}
        if len(hosts) != 1 or hosts[0] not in allowed:
            raise RequestError("Forbidden", "Use the local app address.")
        origins = self.headers.get_all("Origin", [])
        if len(origins) > 1 or (origins and origins[0] != "http://" + hosts[0]):
            raise RequestError("Forbidden", "Use the app from its local origin.")

    def _body(self):
        lengths = self.headers.get_all("Content-Length", [])
        types = self.headers.get_all("Content-Type", [])
        if self.headers.get("Transfer-Encoding") or len(lengths) != 1 or len(types) != 1:
            raise RequestError("InvalidInput", "Send a JSON object with a content length.")
        if types[0].split(";", 1)[0].strip().lower() != "application/json":
            raise RequestError("InvalidInput", "Send an application/json object.")
        try:
            length = int(lengths[0])
            if not 0 <= length <= MAX_BODY:
                raise ValueError()
            data = self.rfile.read(length)
            if len(data) != length:
                raise ValueError()
            def reject_constant(value):
                raise ValueError("Non-finite JSON is unsupported")
            value = json.loads(data.decode("utf-8"), parse_constant=reject_constant)
            if not isinstance(value, dict):
                raise ValueError()
            return value
        except (ValueError, UnicodeError):
            raise RequestError("InvalidInput", "Send a valid JSON object of at most 16 KiB.") from None

    def _perform(self, method):
        try:
            self._local_request()
            path = urlsplit(self.path).path
            if method == "GET":
                if path == "/api/session":
                    self._json(200, {"principal": self.server.sessions.profile(self._token())})
                elif path == "/api/concerts":
                    self._json(200, reservations.catalog(self.server.database))
                elif path == "/api/reservations":
                    self._json(200, reservations.list_own(self.server.database, self._principal()))
                elif path in ("/", "/app.js", "/style.css"):
                    filename = "index.html" if path == "/" else path[1:]
                    content_type = {"index.html": "text/html", "app.js": "text/javascript", "style.css": "text/css"}[filename]
                    payload = (STATIC / filename).read_bytes()
                    self._headers(200, content_type + "; charset=utf-8", length=len(payload))
                    self.wfile.write(payload)
                else:
                    raise RequestError("NotFound", "This route does not exist.")
                return
            body = self._body()
            if path == "/api/login":
                # An unsuccessful identity change revokes the presented session too.
                self.server.sessions.logout(self._token())
                token, profile = self.server.sessions.login(body.get("username"), body.get("password"))
                self._json(200, {"principal": profile}, f"concert_session={token}; HttpOnly; SameSite=Strict; Path=/")
            elif path == "/api/logout":
                self._expect_principal(self.server.sessions.principal(self._token()))
                self.server.sessions.logout(self._token())
                self._json(200, {"principal": None}, "concert_session=; Max-Age=0; HttpOnly; SameSite=Strict; Path=/")
            elif path == "/api/reservations":
                result = reservations.reserve(self.server.database, self._principal(), body.get("concert_id"), body.get("request_key"))
                self._json(200, result)
            elif re.fullmatch(r"/api/reservations/[A-Za-z0-9_-]+/cancel", path):
                result = reservations.cancel(self.server.database, self._principal(), path.split("/")[3])
                self._json(200, result)
            else:
                raise RequestError("NotFound", "This route does not exist.")
        except (RequestError, identity.IdentityError, reservations.DomainError) as error:
            self._json(STATUS.get(error.code, 500), {"error": {"code": error.code, "message": error.message}})
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception:
            self._json(500, {"error": {"code": "InternalError", "message": "The request could not be completed."}})

    def do_GET(self):
        self._perform("GET")

    def do_POST(self):
        self._perform("POST")


def make_server(database, host="127.0.0.1", port=0):
    if host not in ("127.0.0.1", "localhost"):
        raise ValueError("The demo must bind to loopback.")
    return ConcertServer(database, host, port)
