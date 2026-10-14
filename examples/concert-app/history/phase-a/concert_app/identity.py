"""Own fictional credential verification and a thread-safe opaque session registry."""

from hmac import compare_digest
from secrets import token_urlsafe
from threading import Lock


class IdentityError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code, self.message = code, message


class SessionStore:
    def __init__(self):
        self._sessions = {}
        self._lock = Lock()
        self._accounts = {
            "alice": ("alice-demo", {"id": "alice", "name": "Alice"}),
            "bob": ("bob-demo", {"id": "bob", "name": "Bob"}),
        }

    def login(self, username, password):
        account = self._accounts.get(username) if isinstance(username, str) else None
        candidate = password.encode("utf-8") if isinstance(password, str) else b""
        expected = account[0].encode("utf-8") if account else b"invalid-account"
        matches = compare_digest(candidate, expected)
        if not account or not matches:
            raise IdentityError("InvalidCredentials", "The demo credentials did not match.")
        token = token_urlsafe(32)
        with self._lock:
            self._sessions[token] = dict(account[1])
        return token, dict(account[1])

    def profile(self, token):
        with self._lock:
            profile = self._sessions.get(token)
            return dict(profile) if profile else None

    def principal(self, token):
        profile = self.profile(token)
        return profile["id"] if profile else None

    def logout(self, token):
        with self._lock:
            self._sessions.pop(token, None)
