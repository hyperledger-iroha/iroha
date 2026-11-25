from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread
from unittest.mock import patch

import pytest

from iroha_torii_client import AnonymousBallotClientV1, AnonymousBallotRefusal


def test_whole_public_records_and_credential_free_submission():
    seen = []
    fixture = Path(__file__).resolve().parents[3] / "fixtures/governance/anonymous_ballot_http_v1"
    ballot_frame, board_frame, attempt_frame = ((fixture / name).read_bytes() for name in ("ballot.nrt", "board.nrt", "attempt.nrt"))

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            self.respond()

        def do_POST(self):
            self.respond()

        def respond(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            seen.append((self.command, self.path, self.headers, body))
            mode = self.path.split("/")[1]
            data = b"" if self.command == "POST" else (board_frame if "/boards/" in self.path else attempt_frame)
            status = 202 if self.command == "POST" else 200
            if mode == "redirect":
                status = 307
            if mode == "refusal":
                status, data = 429, b'{"code":"anonymous_ballot_throttled","message":"remote text"}'
            if mode == "wrong":
                status = 200
            if mode == "body":
                data = b"x"
            if mode == "empty":
                data = b""
            if mode in ("oversized", "compressed"):
                status = 503
                data = bytes(65537) if mode == "oversized" else b"compressed proxy message"
            self.send_response(status)
            self.send_header("Content-Type", "application/json" if mode in ("wrong", "refusal") else "application/x-norito")
            if mode == "compressed":
                self.send_header("Content-Encoding", "gzip")
            self.send_header("Content-Length", "1048577" if mode == "large" else str(len(data)))
            self.send_header("Set-Cookie", "identity=never")
            self.send_header("Retry-After", "3")
            if mode == "redirect":
                self.send_header("Location", "/unexpected")
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *_):
            pass

    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base = f"http://127.0.0.1:{server.server_port}"
        def client(mode):
            return AnonymousBallotClientV1(base + "/" + mode, allow_local_http=True)
        try:
            for invalid in (base, "https://user:pass@example.invalid", "https://example.invalid/?token=value"):
                with pytest.raises(ValueError):
                    AnonymousBallotClientV1(invalid)
            api = client("good")
            scope = bytes.fromhex("856bb8bcfa0804b4df16c68d0aa758937016ac87a3e783fa4b434ebeb5f4cd584e4abe3bf0b859b89863fb653f620aba7b7ea2ac4343c8723a7021a15cc01d2d")
            attempt = "c53a4fa9eae4964c1f6b2e60740ee721971fa82748df98af93bc606bea20fcc4"
            assert api.submit_anonymous_ballot(ballot_frame) is None
            assert api.get_anonymous_ballot_board(scope) == board_frame
            assert api.get_anonymous_parliament_attempt(attempt) == attempt_frame
            assert [(method, path) for method, path, _, _ in seen] == [
                ("POST", "/good/v1/gov/anonymous-ballots"),
                ("GET", "/good/v1/gov/anonymous-ballots/boards/" + scope.hex()),
                ("GET", "/good/v1/gov/anonymous-ballots/attempts/" + attempt),
            ]
            assert seen[0][3] == ballot_frame
            assert seen[0][2]["Content-Type"] == "application/x-norito"
            for _, _, headers, _ in seen:
                assert all(headers.get(name) is None for name in ("Authorization", "Cookie", "X-API-Token", "X-Iroha-Account", "Referer"))
            with pytest.raises(ValueError):
                api.get_anonymous_ballot_board("AB" * 64)
            with pytest.raises(ValueError):
                api.submit_anonymous_ballot(bytes(262145))
            for mode in ("wrong", "body"):
                with pytest.raises(ValueError):
                    client(mode).submit_anonymous_ballot(b"x")
            for mode in ("wrong", "empty", "large"):
                with pytest.raises(ValueError):
                    client(mode).get_anonymous_ballot_board(scope)
            for mode, status in (("redirect", 307), ("refusal", 429)):
                with pytest.raises(AnonymousBallotRefusal) as error:
                    client(mode).submit_anonymous_ballot(b"x")
                assert error.value.status == status
                if status == 429:
                    assert (error.value.code, error.value.retry_after_seconds) == ("anonymous_ballot_throttled", 3)
                    assert "remote text" not in str(error.value)
            for mode in ("oversized", "compressed"):
                with pytest.raises(AnonymousBallotRefusal) as error:
                    client(mode).submit_anonymous_ballot(ballot_frame)
                assert (error.value.status, error.value.code, error.value.retry_after_seconds) == (503, None, 3)
            with patch("iroha_torii_client.anonymous_ballot._envelope_fields", side_effect=RecursionError):
                with pytest.raises(AnonymousBallotRefusal) as error:
                    client("refusal").submit_anonymous_ballot(ballot_frame)
                assert (error.value.status, error.value.code, error.value.retry_after_seconds) == (429, None, 3)
            assert all(path != "/unexpected" for _, path, _, _ in seen)
        finally:
            server.shutdown()
            thread.join()
