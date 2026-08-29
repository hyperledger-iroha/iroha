"""Focused tests for the immutable TLAPM builder download boundary.

The production boundary excludes cross-user/path ambiguity through an
invocation-local owner-private root; it does not claim hostile same-UID safety.
"""

from __future__ import annotations

from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import socket
import subprocess
import sys
import threading
from typing import Iterator

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
BUILDER = ROOT_DIR / "scripts/formal/build_sumeragi_v2_tlapm_from_source.sh"
LOCK = ROOT_DIR / "scripts/formal/sumeragi_v2_tlapm_source_build_lock.json"
HELPER = ROOT_DIR / "scripts/formal/sumeragi_v2_tlapm_source_lock.py"
PAYLOAD = bytes(range(256)) * 256
CUTOFF = 8192
RETRY_FLAG = re.compile(r"--retry(?:\b|=)")
FROZEN_BUILDER_SHA256 = (
    "370f51fb84902544d7bd70ebdc37ebdf83cf1801c224ded5861ed452da2a98c0"
)


def _function(source: str, name: str, next_name: str) -> str:
    start = source.index(f"{name}() {{")
    end = source.index(f"\n{next_name}() {{", start)
    return source[start:end]


BUILDER_SOURCE = BUILDER.read_text(encoding="utf-8")
DOWNLOAD_BOUNDARY_SOURCE = _function(
    BUILDER_SOURCE,
    "download_curl_attempt",
    "verify_checked_file",
)


def _assert_frozen_builder(source: str) -> None:
    assert hashlib.sha256(source.encode()).hexdigest() == FROZEN_BUILDER_SHA256


def _assert_build_job_partition(source: str) -> None:
    build_jobs_assignment = (
        'readonly BUILD_JOBS="${TLAPM_SOURCE_BUILD_JOBS:-2}"'
    )
    build_jobs_validation = (
        '[[ "$BUILD_JOBS" =~ ^[1-9][0-9]*$ ]] && '
        '((BUILD_JOBS <= 64)) || {'
    )
    opam_jobs = (
        'OPAMDOWNLOADJOBS=1 OPAMJOBS="$BUILD_JOBS" '
        'OPAMPRECISETRACKING=1 \\'
    )
    release_make = 'make --jobs=1 -C "$SOURCE_DIR" release \\'

    assert source.count(build_jobs_assignment) == 1
    assert source.count(build_jobs_validation) == 1
    assert source.count(opam_jobs) == 1
    assert source.count(release_make) == 1
    release_make_lines = [
        line.strip()
        for line in source.splitlines()
        if re.match(r"^[ \t]*make\b.*\brelease[ \t]*\\$", line)
    ]
    assert release_make_lines == [release_make]
    assert "BUILD_JOBS" not in release_make_lines[0]


def _assert_vcs_build_source_contract(source: str) -> None:
    normalized = " ".join(source.replace("\\\n", "").split())
    helpers = source[
        source.index("verify_build_source_checkout() {"):
        source.index("\ndownload_curl_attempt() {")
    ]
    lifecycle = source[
        source.index('echo "[tlapm] fetching immutable source commit'):
        source.index('\necho "[tlapm] downloading pinned opam')
    ]
    identity_gates = source[
        source.index('readonly UPSTREAM_ARCHIVE='):
        source.index('\nreadonly ATTESTATION=')
    ]
    local_checkout = (
        'checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR" '
        '"$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE" "$SOURCE_DIR"'
    )
    normalized_lifecycle = " ".join(lifecycle.replace("\\\n", "").split())

    assert hashlib.sha256(helpers.encode()).hexdigest() == "182a8338f62b69348d0a1d28c95595e294e2bbf34f474906b1cd91fd94874f67"
    assert hashlib.sha256(lifecycle.encode()).hexdigest() == "8e0e7c3aec75067ab2cffec5a2bff1164fde3c995043046fe59bfe0201fff0e1"
    assert hashlib.sha256(identity_gates.encode()).hexdigest() == "436886d45aa3fa0b124ab6cd6a3a5b07acf4d02d51ee49d06a54a260767370e8"
    assert normalized.count(local_checkout) == 1
    for marker in ("TLAPM source\"", local_checkout, "seal_build_source_checkout", "opam repository"):
        assert marker in normalized_lifecycle
    assert normalized_lifecycle.index("TLAPM source\"") < normalized_lifecycle.index(local_checkout)
    assert normalized_lifecycle.index(local_checkout) < normalized_lifecycle.index("seal_build_source_checkout")
    assert normalized_lifecycle.index("seal_build_source_checkout") < normalized_lifecycle.index("opam repository")
    assert source.count("verify_build_source_checkout") == 4
    assert source.count("seal_build_source_checkout") == 2
    assert source.count("\nverify_build_source_checkout\n") == 2
    assert 'describe --always --dirty --abbrev=7' in helpers
    assert 'rev-parse --is-shallow-repository' in helpers
    assert '${git_dir}/objects/info/alternates' in helpers
    assert '${git_dir}/commondir' in helpers
    assert '${git_dir}/shallow' in helpers
    assert '! -type d ! -type f -print -quit' in normalized
    assert 'actual_shallow_sha="$(hash_file' in helpers
    assert '-type f -links +1 -print -quit' in normalized
    assert 'remote get-url --all origin' in normalized
    assert 'remote get-url --push --all origin' in normalized
    assert 'remote remove origin' in normalized
    assert '[[ -z "$actual_remotes" ]]' in helpers
    assert '[[ -d "$git_dir" && ! -L "$git_dir"' in helpers
    for forbidden in (
        "SOURCE_ARCHIVE",
        "git archive",
        "git worktree",
        "git clone",
        "--shared",
        "ln -s",
        "development",
    ):
        assert forbidden not in source
    assert source.count('"RELEASE_VERSION=${TLAPM_SOURCE_VERSION}"') == 1
    assert "RELEASE_VERSION=${TLAPM_SOURCE_COMMIT" not in source
    assert (
        'tlapm-${TLAPM_SOURCE_VERSION}-${PLATFORM}.tar.gz' in identity_gates
    )
    assert identity_gates.count('${TLAPM_SOURCE_COMMIT:0:7}') == 6
    assert '[[ "$built_version" ==' in identity_gates
    assert '[[ "$projected_version" ==' in identity_gates
    assert '[[ "$archived_version" ==' in identity_gates
    assert 'echo "actual:   ${built_version}"' in identity_gates
    assert 'echo "actual:   ${projected_version}"' in identity_gates
    assert 'echo "actual:   ${archived_version}"' in identity_gates
    assert 'readonly BUILT_ISABELLE="${SOURCE_DIR}/_build/default/deps/isabelle/Isabelle"' in identity_gates
    assert 'readonly PROJECTED_ISABELLE="${AUTHENTICATED_DISTRIBUTION}/lib/tlapm/backends/Isabelle"' in identity_gates
    assert '/bin/rm -rf -- "$PROJECTED_ISABELLE"' in identity_gates
    assert '/bin/cp -R "$BUILT_ISABELLE" "$PROJECTED_ISABELLE"' in identity_gates
    assert '/bin/cp -p "$BUILT_ISABELLE_EXEC_FILES" "$PROJECTED_ISABELLE_EXEC_FILES"' in identity_gates
    projection_markers = (
        'dune install --root "$SOURCE_DIR" --relocatable',
        '/bin/rm -rf -- "$PROJECTED_ISABELLE"',
        '/bin/cp -R "$BUILT_ISABELLE" "$PROJECTED_ISABELLE"',
        '/bin/cp -p "$BUILT_ISABELLE_EXEC_FILES" "$PROJECTED_ISABELLE_EXEC_FILES"',
        'clean_command make --jobs=1 -C "$AUTHENTICATED_DISTRIBUTION/lib/tlapm"',
    )
    assert all(identity_gates.count(marker) == 1 for marker in projection_markers)
    assert [identity_gates.index(marker) for marker in projection_markers] == sorted(
        identity_gates.index(marker) for marker in projection_markers
    )


DOWNLOAD_FUNCTIONS = "\n\n".join(
    (
        _function(
            BUILDER_SOURCE,
            "download_curl_attempt",
            "download_curl_status_is_transient",
        ),
        _function(
            BUILDER_SOURCE,
            "download_curl_status_is_transient",
            "download_retry_sleep",
        ),
        _function(BUILDER_SOURCE, "download_checked", "verify_checked_file"),
    )
)
VCS_VERIFY_FUNCTIONS = _function(
    BUILDER_SOURCE,
    "verify_exact_tree",
    "seal_build_source_checkout",
)


VCS_VERIFY_HARNESS = r"""
set -euo pipefail
umask 077
readonly SOURCE_DIR="$1"
readonly TLAPM_SOURCE_COMMIT="$2"
readonly TLAPM_SOURCE_TREE="$3"
readonly LOCK_PYTHON="$4"
readonly LOCK_HELPER="$5"
readonly LOCK_MANIFEST="$6"
readonly PLATFORM="arm64-darwin"

clean_command() {
  "$@"
}

hash_file() {
  "$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
    --lock "$LOCK_MANIFEST" --platform "$PLATFORM" hash-file --path "$1"
}
""" + VCS_VERIFY_FUNCTIONS + r"""

verify_build_source_checkout
"""


HARNESS = r"""
set -euo pipefail
umask 077
tmp_dir="$1"
destination="$2"
url="$3"
expected_sha256="$4"
readonly LOCK_PYTHON="$5"
readonly LOCK_HELPER="$6"
readonly LOCK_MANIFEST="$7"
readonly PLATFORM="arm64-darwin"

hash_file() {
  "$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
    --lock "$LOCK_MANIFEST" --platform "$PLATFORM" hash-file --path "$1"
}

validate_download_directory() {
  "$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
    --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
    validate-private-directory --directory "$1"
}

download_retry_sleep() {
  printf 'sleep:%s\n' "$1" >> "$TEST_EVENT_LOG"
}

clean_command() {
  if [[ "$1" != curl ]]; then
    "$@"
    return
  fi
  shift
  printf 'curl\n' >> "$TEST_EVENT_LOG"
  local output="" previous="" argument
  for argument in "$@"; do
    if [[ "$previous" == --output ]]; then
      output="$argument"
    fi
    previous="$argument"
  done
  case "$TEST_CURL_MODE" in
    real)
      local transformed=()
      for argument in "$@"; do
        if [[ "$argument" == =https ]]; then
          transformed+=(=http)
        else
          transformed+=("$argument")
        fi
      done
      "$TEST_CURL_BIN" "${transformed[@]}"
      ;;
    status:*)
      local status_payload="${TEST_CURL_MODE#status:}"
      local status="${status_payload%%:*}"
      local http_status="${status_payload#*:}"
      printf '%s' "$http_status"
      return "$status"
      ;;
    complete-status:*)
      local status_payload="${TEST_CURL_MODE#complete-status:}"
      local status="${status_payload%%:*}"
      local http_status="${status_payload#*:}"
      /bin/cp "$TEST_SPECIAL_TARGET" "$output"
      printf '%s' "$http_status"
      return "$status"
      ;;
    symlink)
      /bin/rm -f -- "$output"
      /bin/ln -s "$TEST_SPECIAL_TARGET" "$output"
      printf '200'
      ;;
    fifo)
      /bin/rm -f -- "$output"
      /usr/bin/mkfifo "$output"
      printf '200'
      ;;
    hardlink)
      /bin/rm -f -- "$output"
      /bin/ln "$TEST_SPECIAL_TARGET" "$output"
      printf '200'
      ;;
    oversized)
      /usr/bin/truncate -s 4294967297 "$output"
      printf '200'
      ;;
    destination-race)
      /bin/cp "$TEST_SPECIAL_TARGET" "$output"
      printf 'winner\n' > "$destination"
      printf '200'
      ;;
    *)
      printf 'unknown test curl mode\n' >&2
      return 2
      ;;
  esac
}
""" + DOWNLOAD_FUNCTIONS + r"""

download_checked fixture "$url" "$expected_sha256" "$destination"
"""


class _RangeServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, mode: str) -> None:
        super().__init__(("127.0.0.1", 0), _RangeHandler)
        self.mode = mode
        self.requests: list[str | None] = []


class _RangeHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    @property
    def fixture(self) -> _RangeServer:
        return self.server  # type: ignore[return-value]

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def _headers(
        self,
        status: int,
        length: int,
        *,
        content_range: str | None = None,
    ) -> None:
        self.send_response(status)
        self.send_header("Content-Length", str(length))
        self.send_header("Accept-Ranges", "bytes")
        if content_range is not None:
            self.send_header("Content-Range", content_range)
        self.send_header("Connection", "close")
        self.end_headers()

    def _interrupt(self, start: int) -> None:
        body = PAYLOAD[start : start + CUTOFF]
        status = 206 if start else 200
        content_range = None
        if start:
            content_range = f"bytes {start}-{len(PAYLOAD) - 1}/{len(PAYLOAD)}"
        self._headers(status, len(PAYLOAD) - start, content_range=content_range)
        self.wfile.write(body)
        self.wfile.flush()
        try:
            self.connection.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.connection.close()

    def do_GET(self) -> None:
        range_header = self.headers.get("Range")
        self.fixture.requests.append(range_header)
        start = 0
        if range_header is not None:
            assert range_header.startswith("bytes=") and range_header.endswith("-")
            start = int(range_header.removeprefix("bytes=").removesuffix("-"))
        mode = self.fixture.mode

        if mode == "normal":
            self._headers(200, len(PAYLOAD))
            self.wfile.write(PAYLOAD)
            return
        if mode == "resume" and range_header is None:
            self._interrupt(0)
            return
        if mode == "resume":
            body = PAYLOAD[start:]
            self._headers(
                206,
                len(body),
                content_range=f"bytes {start}-{len(PAYLOAD) - 1}/{len(PAYLOAD)}",
            )
            self.wfile.write(body)
            return
        if mode == "cap":
            self._interrupt(start)
            return
        if range_header is None and mode in {"ignore", "wrong", "short", "416"}:
            self._interrupt(0)
            return
        if mode == "ignore":
            self._headers(200, len(PAYLOAD))
            self.wfile.write(PAYLOAD)
            return
        if mode == "wrong":
            wrong_start = start + 1
            body = PAYLOAD[wrong_start:]
            self._headers(
                206,
                len(body),
                content_range=(
                    f"bytes {wrong_start}-{len(PAYLOAD) - 1}/{len(PAYLOAD)}"
                ),
            )
            self.wfile.write(body)
            return
        if mode == "short":
            body = PAYLOAD[start : start + 17]
            self._headers(
                206,
                len(body),
                content_range=f"bytes {start}-{start + len(body) - 1}/{len(PAYLOAD)}",
            )
            self.wfile.write(body)
            return
        if mode == "416":
            self._headers(416, 0, content_range=f"bytes */{len(PAYLOAD)}")
            return
        if mode == "hash-mismatch":
            wrong = bytes(reversed(PAYLOAD))
            self._headers(200, len(wrong))
            self.wfile.write(wrong)
            return
        if mode.startswith("http-"):
            self._headers(int(mode.removeprefix("http-")), 0)
            return
        raise AssertionError(mode)


@contextmanager
def _server(mode: str) -> Iterator[_RangeServer]:
    server = _RangeServer(mode)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()


def _run_harness(
    tmp_path: Path,
    *,
    url: str,
    expected: bytes = PAYLOAD,
    mode: str = "real",
    preexisting_destination: bytes | None = None,
    stale_partial: bytes | None = None,
    symlink_parent: bool = False,
) -> tuple[subprocess.CompletedProcess[str], Path, Path, Path]:
    root = (tmp_path / "private-root").resolve()
    root.mkdir(mode=0o700)
    root.chmod(0o700)
    destination = root / "cache" / "artifact.tar.gz"
    event_log = root / "events.log"
    special_target = root / "special-target"
    special_target.write_bytes(expected)
    if symlink_parent:
        cache_target = root / "cache-target"
        cache_target.mkdir(mode=0o700)
        destination.parent.symlink_to(cache_target, target_is_directory=True)
    elif preexisting_destination is not None or stale_partial is not None:
        destination.parent.mkdir(mode=0o700)
    if preexisting_destination is not None:
        destination.write_bytes(preexisting_destination)
    if stale_partial is not None:
        destination.with_name(f"{destination.name}.partial.previous").write_bytes(
            stale_partial
        )
    environment = {
        "PATH": os.defpath,
        "TEST_CURL_BIN": shutil.which("curl") or "/usr/bin/curl",
        "TEST_CURL_MODE": mode,
        "TEST_EVENT_LOG": str(event_log),
        "TEST_SPECIAL_TARGET": str(special_target),
    }
    result = subprocess.run(
        [
            "/bin/bash",
            "-c",
            HARNESS,
            "harness",
            str(root),
            str(destination),
            url,
            hashlib.sha256(expected).hexdigest(),
            sys.executable,
            str(HELPER),
            str(LOCK),
        ],
        cwd=ROOT_DIR,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=20,
        check=False,
    )
    return result, destination, event_log, special_target


def _events(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines() if path.exists() else []


def _nested_make_recipe_runs(tmp_path: Path, jobs: int) -> list[str]:
    make_binary = shutil.which("make", path=os.defpath)
    sleep_binary = shutil.which("sleep", path=os.defpath)
    touch_binary = shutil.which("touch", path=os.defpath)
    assert make_binary is not None
    assert sleep_binary is not None
    assert touch_binary is not None

    fixture = tmp_path / f"jobs-{jobs}"
    fixture.mkdir(mode=0o700)
    parent_makefile = fixture / "parent.mk"
    child_makefile = fixture / "child.mk"
    run_directory = fixture / "run"
    run_directory.mkdir(mode=0o700)
    parent_makefile.write_text(
        '.PHONY: all\n\nall:\n\t+@$(MAKE) -f "$(CHILD_MAKEFILE)" all\n',
        encoding="utf-8",
    )
    child_makefile.write_text(
        ".PHONY: all\n\n"
        "all: Isabelle Isabelle-test\n\n"
        "Isabelle Isabelle-test:\n"
        "\t@printf '%s\\n' \"$@\" >> runs.log\n"
        f"\t@{sleep_binary} 0.5\n"
        f"\t@{touch_binary} Isabelle Isabelle-test\n",
        encoding="utf-8",
    )
    environment = {
        "PATH": os.defpath,
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "TMPDIR": str(fixture),
    }
    result = subprocess.run(
        [
            make_binary,
            f"--jobs={jobs}",
            "-f",
            str(parent_makefile),
            f"CHILD_MAKEFILE={child_makefile}",
        ],
        cwd=run_directory,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert "jobserver unavailable" not in result.stderr.lower()
    assert (run_directory / "Isabelle").is_file()
    assert (run_directory / "Isabelle-test").is_file()
    return (run_directory / "runs.log").read_text(encoding="utf-8").splitlines()


def _run_git(
    git_binary: str,
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [git_binary, *arguments],
        cwd=cwd,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )


def _require_git_success(result: subprocess.CompletedProcess[str]) -> str:
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def _run_production_vcs_verifier(
    build_source: Path,
    commit: str,
    tree: str,
    fixture: Path,
) -> subprocess.CompletedProcess[str]:
    environment = {
        "PATH": os.defpath,
        "HOME": str(fixture),
        "TMPDIR": str(fixture),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_NO_REPLACE_OBJECTS": "1",
    }
    return subprocess.run(
        [
            "/bin/bash",
            "-c",
            VCS_VERIFY_HARNESS,
            "vcs-verifier",
            str(build_source),
            commit,
            tree,
            sys.executable,
            str(HELPER),
            str(LOCK),
        ],
        cwd=fixture,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )


def test_release_build_is_serial_while_opam_jobs_remain_configurable() -> None:
    _assert_build_job_partition(BUILDER_SOURCE)


@pytest.mark.parametrize(
    "needle,replacement",
    (
        (
            'make --jobs=1 -C "$SOURCE_DIR" release \\',
            'make --jobs="$BUILD_JOBS" -C "$SOURCE_DIR" release \\',
        ),
        (
            'make --jobs=1 -C "$SOURCE_DIR" release \\',
            'make --jobs=2 -C "$SOURCE_DIR" release \\',
        ),
        (
            'make --jobs=1 -C "$SOURCE_DIR" release \\',
            'make -C "$SOURCE_DIR" release \\',
        ),
        (
            'OPAMDOWNLOADJOBS=1 OPAMJOBS="$BUILD_JOBS" '
            'OPAMPRECISETRACKING=1 \\',
            'OPAMDOWNLOADJOBS=1 OPAMJOBS=1 OPAMPRECISETRACKING=1 \\',
        ),
        (
            '((BUILD_JOBS <= 64)) || {',
            '((BUILD_JOBS <= 1)) || {',
        ),
    ),
)
def test_build_job_partition_rejects_mutations(
    needle: str,
    replacement: str,
) -> None:
    assert BUILDER_SOURCE.count(needle) == 1
    mutated = BUILDER_SOURCE.replace(needle, replacement, 1)
    syntax = subprocess.run(
        ["/bin/bash", "-n"],
        input=mutated,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert syntax.returncode == 0, syntax.stderr
    with pytest.raises(AssertionError):
        _assert_build_job_partition(mutated)


def test_serial_nested_make_avoids_ordinary_multi_target_duplicate_recipe(
    tmp_path: Path,
) -> None:
    parallel_runs = _nested_make_recipe_runs(tmp_path, 2)
    serial_runs = _nested_make_recipe_runs(tmp_path, 1)

    assert sorted(parallel_runs) == ["Isabelle", "Isabelle-test"]
    assert len(serial_runs) == 1
    assert serial_runs[0] in {"Isabelle", "Isabelle-test"}


def test_vcs_build_source_contract_is_frozen_and_identity_domains_stay_distinct(
) -> None:
    _assert_frozen_builder(BUILDER_SOURCE)
    _assert_vcs_build_source_contract(BUILDER_SOURCE)
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    helper_source = HELPER.read_text(encoding="utf-8")
    source = lock["source"]

    assert source["version"] == "1.6.0-pre"
    assert source["commit"] == "3ab43c7ff31db4ced850619d4746fa4c841a7681"
    assert source["version"] != source["commit"][:7]
    assert '"binary_identity": source["commit"][:7]' in helper_source
    assert '"source_commit": source["commit"]' in helper_source
    assert '"source_version": source["version"]' in helper_source


@pytest.mark.parametrize(
    "needle,replacement",
    (
        (
            'checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR" \\\n',
            'checkout_exact_tree "TLAPM build source" '
            '"$TLAPM_SOURCE_REPOSITORY_URL" \\\n',
        ),
        (
            'echo "[tlapm] preparing the independent VCS-bearing build checkout"',
            'clean_command git -C "$SOURCE_PIN_DIR" archive --format=tar HEAD '
            '>/dev/null\n'
            'echo "[tlapm] preparing the independent VCS-bearing build checkout"',
        ),
        (
            'checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR" \\\n'
            '  "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE" "$SOURCE_DIR"',
            'clean_command git -C "$SOURCE_PIN_DIR" worktree add --detach \\\n'
            '  "$SOURCE_DIR" "$TLAPM_SOURCE_COMMIT"',
        ),
        (
            'checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR" \\\n'
            '  "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE" "$SOURCE_DIR"',
            'clean_command git clone --shared "$SOURCE_PIN_DIR" "$SOURCE_DIR"',
        ),
        (
            'seal_build_source_checkout\necho "[tlapm] fetching immutable opam',
            'clean_command ln -s "$SOURCE_PIN_DIR/.git" "$SOURCE_DIR/.git"\n'
            'seal_build_source_checkout\necho "[tlapm] fetching immutable opam',
        ),
        (
            '\nseal_build_source_checkout\necho "[tlapm] fetching immutable opam',
            '\n:\necho "[tlapm] fetching immutable opam',
        ),
        (
            'describe --always --dirty --abbrev=7',
            'rev-parse --short=7 HEAD',
        ),
        (
            'rev-parse --is-shallow-repository',
            'rev-parse --is-inside-work-tree',
        ),
        (
            '! -L "${git_dir}/objects/info/alternates"',
            '! -L "${git_dir}/objects/info/unused-alternates"',
        ),
        (
            '! -L "${git_dir}/commondir"',
            '! -L "${git_dir}/unused-commondir"',
        ),
        (
            '! -type d ! -type f -print -quit',
            '-type f -print -quit',
        ),
        (
            '-type f -links 1 -print -quit',
            '-type f -links +9 -print -quit',
        ),
        (
            '-type f -links +1 -print -quit',
            '-type f -links +9 -print -quit',
        ),
        (
            '\nverify_build_source_checkout\nverify_exact_tree "opam repository"',
            '\n:\nverify_exact_tree "opam repository"',
        ),
        (
            '"RELEASE_VERSION=${TLAPM_SOURCE_VERSION}"\n\n'
            'verify_build_source_checkout\n"$LOCK_PYTHON"',
            '"RELEASE_VERSION=${TLAPM_SOURCE_VERSION}"\n\n:\n'
            '"$LOCK_PYTHON"',
        ),
        (
            '"RELEASE_VERSION=${TLAPM_SOURCE_VERSION}"',
            '"RELEASE_VERSION=${TLAPM_SOURCE_COMMIT:0:7}"',
        ),
            (
                'readonly UPSTREAM_ARCHIVE="${SOURCE_DIR}/_build/'
                'tlapm-${TLAPM_SOURCE_VERSION}-${PLATFORM}.tar.gz"',
                'readonly UPSTREAM_ARCHIVE="${SOURCE_DIR}/_build/'
                'tlapm-${TLAPM_SOURCE_COMMIT:0:7}-${PLATFORM}.tar.gz"',
            ),
            (
                'readonly BUILT_ARCHIVE="${tmp_dir}/'
                'tlapm-${TLAPM_SOURCE_VERSION}-${PLATFORM}.tar.gz"',
                'readonly BUILT_ARCHIVE="${tmp_dir}/'
                'tlapm-${TLAPM_SOURCE_COMMIT:0:7}-${PLATFORM}.tar.gz"',
            ),
        (
            '[[ "$built_version" == "${TLAPM_SOURCE_COMMIT:0:7}" ]]',
            '[[ "$built_version" == "$TLAPM_SOURCE_VERSION" ]]',
        ),
        (
            '[[ "$archived_version" == "${TLAPM_SOURCE_COMMIT:0:7}" ]]',
            '[[ "$archived_version" == development ]]',
        ),
        (
            'clean_command git -C "$SOURCE_DIR" remote remove origin',
            'clean_command git -C "$SOURCE_DIR" remote get-url origin >/dev/null',
        ),
    ),
)
def test_vcs_build_source_contract_rejects_mutations(
    needle: str,
    replacement: str,
) -> None:
    assert BUILDER_SOURCE.count(needle) == 1
    mutated = BUILDER_SOURCE.replace(needle, replacement, 1)
    syntax = subprocess.run(
        ["/bin/bash", "-n"],
        input=mutated,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert syntax.returncode == 0, syntax.stderr
    with pytest.raises(AssertionError):
        _assert_vcs_build_source_contract(mutated)


def test_vcs_build_source_contract_rejects_late_identity_preflight() -> None:
    early = '\nseal_build_source_checkout\necho "[tlapm] fetching immutable opam'
    late = (
        '  "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"\n'
        'chmod 0500 "$OPAM_BINARY"'
    )
    assert BUILDER_SOURCE.count(early) == 1
    assert BUILDER_SOURCE.count(late) == 1
    mutated = BUILDER_SOURCE.replace(
        early,
        '\n:\necho "[tlapm] fetching immutable opam',
        1,
    ).replace(
        late,
        '  "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"\n'
        'seal_build_source_checkout\nchmod 0500 "$OPAM_BINARY"',
        1,
    )
    syntax = subprocess.run(
        ["/bin/bash", "-n"],
        input=mutated,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert syntax.returncode == 0, syntax.stderr
    with pytest.raises(AssertionError):
        _assert_vcs_build_source_contract(mutated)


def test_local_exact_checkout_preserves_vcs_identity_while_archive_loses_it(
    tmp_path: Path,
) -> None:
    git_binary = shutil.which("git", path=os.defpath)
    tar_binary = shutil.which("tar", path=os.defpath)
    assert git_binary is not None
    assert tar_binary is not None
    fixture = tmp_path.resolve()
    environment = {
        "PATH": os.defpath,
        "HOME": str(fixture),
        "TMPDIR": str(fixture),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_AUTHOR_DATE": "2026-07-19T10:03:28Z",
        "GIT_COMMITTER_DATE": "2026-07-19T10:03:28Z",
    }
    source_pin = fixture / "source-pin"
    source_pin.mkdir(mode=0o700)
    _require_git_success(_run_git(git_binary, ["init", "--quiet"], cwd=source_pin, environment=environment))
    tracked = source_pin / "source.txt"
    tracked.write_text("immutable source\n", encoding="utf-8")
    _require_git_success(_run_git(git_binary, ["add", "source.txt"], cwd=source_pin, environment=environment))
    _require_git_success(
        _run_git(
            git_binary,
            ["-c", "user.name=TLAPM Test", "-c", "user.email=tlapm@example.invalid", "commit", "--quiet", "-m", "pin"],
            cwd=source_pin,
            environment=environment,
        )
    )
    commit = _require_git_success(_run_git(git_binary, ["rev-parse", "HEAD"], cwd=source_pin, environment=environment))
    tree = _require_git_success(_run_git(git_binary, ["rev-parse", "HEAD^{tree}"], cwd=source_pin, environment=environment))
    short_commit = commit[:7]

    archive = fixture / "source.tar"
    archive_tree = fixture / "archive-tree"
    archive_tree.mkdir(mode=0o700)
    _require_git_success(
        _run_git(git_binary, ["archive", "--format=tar", f"--output={archive}", "HEAD"], cwd=source_pin, environment=environment)
    )
    extracted = subprocess.run(
        [tar_binary, "-xf", str(archive), "-C", str(archive_tree)],
        cwd=fixture,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    assert extracted.returncode == 0, extracted.stderr
    assert not (archive_tree / ".git").exists()
    archive_description = _run_git(
        git_binary,
        ["describe", "--always", "--dirty", "--abbrev=7"],
        cwd=archive_tree,
        environment=environment,
    )
    assert archive_description.returncode != 0

    build_source = fixture / "build-source"
    build_source.mkdir(mode=0o700)
    _require_git_success(_run_git(git_binary, ["init", "--quiet"], cwd=build_source, environment=environment))
    _require_git_success(_run_git(git_binary, ["remote", "add", "origin", str(source_pin.resolve())], cwd=build_source, environment=environment))
    _require_git_success(
        _run_git(
            git_binary,
            ["-c", "protocol.version=2", "fetch", "--quiet", "--no-tags", "--depth=1", "origin", commit],
            cwd=build_source,
            environment=environment,
        )
    )
    _require_git_success(_run_git(git_binary, ["checkout", "--quiet", "--detach", "FETCH_HEAD"], cwd=build_source, environment=environment))
    origin = _require_git_success(_run_git(git_binary, ["remote", "get-url", "--all", "origin"], cwd=build_source, environment=environment))
    assert origin == str(source_pin.resolve())
    _require_git_success(_run_git(git_binary, ["remote", "remove", "origin"], cwd=build_source, environment=environment))

    git_dir = build_source / ".git"
    assert git_dir.is_dir() and not git_dir.is_symlink()
    assert not os.path.samefile(git_dir, source_pin / ".git")
    assert _require_git_success(_run_git(git_binary, ["remote"], cwd=build_source, environment=environment)) == ""
    assert _require_git_success(_run_git(git_binary, ["rev-parse", "HEAD"], cwd=build_source, environment=environment)) == commit
    assert _require_git_success(_run_git(git_binary, ["rev-parse", "HEAD^{tree}"], cwd=build_source, environment=environment)) == tree
    assert _require_git_success(_run_git(git_binary, ["rev-parse", "--is-shallow-repository"], cwd=build_source, environment=environment)) == "true"
    assert _require_git_success(_run_git(git_binary, ["tag", "--points-at", "HEAD"], cwd=build_source, environment=environment)) == ""
    assert _require_git_success(_run_git(git_binary, ["describe", "--always", "--dirty", "--abbrev=7"], cwd=build_source, environment=environment)) == short_commit
    assert not (git_dir / "objects/info/alternates").exists()
    assert all(path.stat().st_nlink == 1 for path in (git_dir / "objects").rglob("*") if path.is_file())
    verified = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert verified.returncode == 0, verified.stderr

    build_output = build_source / "_build/output"
    build_output.parent.mkdir(mode=0o700)
    build_output.write_text("untracked build output\n", encoding="utf-8")
    assert _require_git_success(_run_git(git_binary, ["status", "--porcelain=v1", "--untracked-files=no"], cwd=build_source, environment=environment)) == ""
    assert _require_git_success(_run_git(git_binary, ["describe", "--always", "--dirty", "--abbrev=7"], cwd=build_source, environment=environment)) == short_commit
    verified = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert verified.returncode == 0, verified.stderr
    (build_source / "source.txt").write_text("tracked mutation\n", encoding="utf-8")
    assert _require_git_success(_run_git(git_binary, ["status", "--porcelain=v1", "--untracked-files=no"], cwd=build_source, environment=environment)) != ""
    assert _require_git_success(_run_git(git_binary, ["describe", "--always", "--dirty", "--abbrev=7"], cwd=build_source, environment=environment)) == f"{short_commit}-dirty"
    assert _run_production_vcs_verifier(build_source, commit, tree, fixture).returncode != 0
    (build_source / "source.txt").write_text("immutable source\n", encoding="utf-8")

    _require_git_success(_run_git(git_binary, ["config", "core.worktree", str(source_pin)], cwd=build_source, environment=environment))
    (build_source / "source.txt").write_text("hidden tracked mutation\n", encoding="utf-8")
    escaped_worktree = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert escaped_worktree.returncode != 0
    assert "escaped its canonical checkout" in escaped_worktree.stderr
    _require_git_success(_run_git(git_binary, ["config", "--unset", "core.worktree"], cwd=build_source, environment=environment))
    (build_source / "source.txt").write_text("immutable source\n", encoding="utf-8")

    _require_git_success(_run_git(git_binary, ["repack", "-ad"], cwd=build_source, environment=environment))
    pack_dir = git_dir / "objects/pack"
    external_pack = fixture / "external-pack"
    external_pack.mkdir(mode=0o700)
    pack_entries = list(pack_dir.iterdir())
    assert pack_entries
    for entry in pack_entries:
        shutil.move(str(entry), external_pack / entry.name)
    pack_dir.rmdir()
    pack_dir.symlink_to(external_pack, target_is_directory=True)
    nested_pack = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert nested_pack.returncode != 0
    assert "link or special entry" in nested_pack.stderr
    pack_dir.unlink()
    pack_dir.mkdir(mode=0o700)
    for entry in external_pack.iterdir():
        shutil.move(str(entry), pack_dir / entry.name)

    loose_external = fixture / "external-loose"
    loose_external.mkdir(mode=0o700)
    loose_link = git_dir / "objects/aa"
    loose_link.symlink_to(loose_external, target_is_directory=True)
    assert _run_production_vcs_verifier(build_source, commit, tree, fixture).returncode != 0
    loose_link.unlink()

    common_dir = git_dir / "commondir"
    common_dir.write_text(f"{source_pin / '.git'}\n", encoding="utf-8")
    external_common = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert external_common.returncode != 0
    assert "shared Git common directory" in external_common.stderr
    common_dir.unlink()

    shallow = git_dir / "shallow"
    shallow_bytes = shallow.read_bytes()
    external_shallow = fixture / "external-shallow"
    shallow.replace(external_shallow)
    shallow.symlink_to(external_shallow)
    symlink_shallow = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert symlink_shallow.returncode != 0
    assert "link or special entry" in symlink_shallow.stderr
    shallow.unlink()
    external_shallow.replace(shallow)

    external_shallow.write_bytes(shallow_bytes)
    shallow.unlink()
    os.link(external_shallow, shallow)
    hardlink_shallow = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert hardlink_shallow.returncode != 0
    assert "hard-linked Git metadata" in hardlink_shallow.stderr
    shallow.unlink()
    external_shallow.replace(shallow)

    shallow.write_text(f"{'0' * 40}\n", encoding="utf-8")
    wrong_shallow = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert wrong_shallow.returncode != 0
    assert "does not name the pinned commit" in wrong_shallow.stderr
    shallow.write_bytes(shallow_bytes)
    verified = _run_production_vcs_verifier(build_source, commit, tree, fixture)
    assert verified.returncode == 0, verified.stderr


def test_download_resume_static_contract_is_exact() -> None:
    normalized = " ".join(BUILDER_SOURCE.replace("\\\n", "").split())
    _assert_frozen_builder(BUILDER_SOURCE)
    assert hashlib.sha256(DOWNLOAD_BOUNDARY_SOURCE.encode()).hexdigest() == (
        "2fbda6879085a14e2241ea406cde7ffb12ef2e3ae25985c95a9562bfc92637dd"
    )
    assert "18|28|35|52|55|56) return 0" in normalized
    assert "local -r max_attempts=4" in BUILDER_SOURCE
    assert "1|2|4) /bin/sleep" in normalized
    assert "--disable --proto '=https' --proto-redir '=https'" in normalized
    assert "--tlsv1.2 --fail --location" in normalized
    assert "--continue-at -" in BUILDER_SOURCE
    assert "--write-out '%{http_code}'" in BUILDER_SOURCE
    assert RETRY_FLAG.search(BUILDER_SOURCE) is None
    for forbidden in (
        "--retry-all-errors",
        "--retry-connrefused",
        "--fail-with-body",
        "--remove-on-error",
    ):
        assert forbidden not in BUILDER_SOURCE
    assert len(
        re.findall(
            r"(?m)^[ \t]*(?:clean_command[ \t]+)?(?:/usr/bin/)?curl\b",
            BUILDER_SOURCE,
        )
    ) == 1
    assert len(
        re.findall(
            r"(?m)^[ \t]*download_checked(?:\(\)|[ \t])",
            BUILDER_SOURCE,
        )
    ) == 3
    assert (
        'tmp_dir="$(/usr/bin/mktemp -d '
        '"${OUTPUT_PARENT}/.sumeragi-v2-tlapm-source.XXXXXX")"'
        in BUILDER_SOURCE
    )
    assert 'tmp_dir="$(cd -P -- "$tmp_dir" && pwd)"' in BUILDER_SOURCE
    assert '/bin/chmod 0700 "$tmp_dir"' in BUILDER_SOURCE
    assert 'trap \'/bin/rm -rf -- "$tmp_dir"\' EXIT' in BUILDER_SOURCE
    assert '/usr/bin/mktemp "${destination}.partial.XXXXXX"' in BUILDER_SOURCE
    assert 'case "$destination" in "${tmp_dir}"/*)' in normalized
    assert "validate-private-directory --directory" in normalized
    assert '[[ "$http_status" == "$expected_http_status" ]]' in BUILDER_SOURCE
    assert (
        '[[ "$http_status" == 000 || "$http_status" == '
        '"$expected_http_status" ]]' in BUILDER_SOURCE
    )
    assert normalized.count('actual_sha256="$(hash_file "$partial")"') >= 4
    final_hash = normalized.rindex('actual_sha256="$(hash_file "$partial")"')
    final_absence = normalized.index(
        '[[ ! -e "$destination" && ! -L "$destination" ]]', final_hash
    )
    promotion = normalized.index('clean_command mv "$partial" "$destination"')
    assert final_hash < final_absence < promotion


@pytest.mark.parametrize(
    "forbidden",
    ("--retry 3", "--retry=3", "--retry-all-errors"),
)
def test_retry_flag_guard_rejects_mutations_outside_pinned_download_helpers(
    forbidden: str,
) -> None:
    insertion = BUILDER_SOURCE.index("\nfor required_command")
    mutated = (
        BUILDER_SOURCE[:insertion]
        + f"\nretry_mutation=({forbidden})\n"
        + BUILDER_SOURCE[insertion:]
    )

    assert RETRY_FLAG.search(mutated) is not None


@pytest.mark.parametrize(
    "needle,replacement",
    (
        (
            'echo "[tlapm] downloading pinned opam ${TLAPM_OPAM_VERSION}"',
            'exit 0\necho "[tlapm] downloading pinned opam '
            '${TLAPM_OPAM_VERSION}"',
        ),
        (
            'download_checked "opam ${TLAPM_OPAM_VERSION}" '
            '"$TLAPM_OPAM_BINARY_URL" \\\n'
            '  "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"',
            'if false; then\n  download_checked "opam '
            '${TLAPM_OPAM_VERSION}" "$TLAPM_OPAM_BINARY_URL" \\\n'
            '    "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"\nfi',
        ),
        (
            'trap \'/bin/rm -rf -- "$tmp_dir"\' EXIT',
            'if false; then\n  trap \'/bin/rm -rf -- "$tmp_dir"\' EXIT\nfi',
        ),
        (
            "\nfor required_command in awk",
            '\nextra_probe="$(curl --proto \'=https\' '
            'https://example.invalid)"\nfor required_command in awk',
        ),
        (
            'download_checked "opam ${TLAPM_OPAM_VERSION}" '
            '"$TLAPM_OPAM_BINARY_URL" \\\n'
            '  "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"',
            'if download_checked "opam ${TLAPM_OPAM_VERSION}" '
            '"$TLAPM_OPAM_BINARY_URL" \\\n'
            '    "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"; then\n  :\nfi',
        ),
    ),
)
def test_frozen_builder_pin_rejects_top_level_lifecycle_mutations(
    needle: str,
    replacement: str,
) -> None:
    assert BUILDER_SOURCE.count(needle) == 1
    mutated = BUILDER_SOURCE.replace(needle, replacement, 1)
    syntax = subprocess.run(
        ["/bin/bash", "-n"],
        input=mutated,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert syntax.returncode == 0, syntax.stderr
    with pytest.raises(AssertionError):
        _assert_frozen_builder(mutated)


@pytest.mark.parametrize("mode", ("normal", "resume"))
def test_download_checked_completes_normal_and_resumed_range(
    tmp_path: Path,
    mode: str,
) -> None:
    with _server(mode) as server:
        url = f"http://127.0.0.1:{server.server_port}/artifact"
        result, destination, event_log, _ = _run_harness(tmp_path, url=url)

    assert result.returncode == 0, result.stderr
    assert destination.read_bytes() == PAYLOAD
    assert destination.stat().st_mode & 0o777 == 0o400
    if mode == "normal":
        assert server.requests == [None]
        assert _events(event_log) == ["curl"]
    else:
        assert server.requests == [None, f"bytes={CUTOFF}-"]
        assert _events(event_log) == ["curl", "sleep:1", "curl"]


def test_download_checked_stops_at_exact_attempt_cap(tmp_path: Path) -> None:
    with _server("cap") as server:
        url = f"http://127.0.0.1:{server.server_port}/artifact"
        result, destination, event_log, _ = _run_harness(tmp_path, url=url)

    assert result.returncode in {18, 56}
    assert not destination.exists()
    assert server.requests == [
        None,
        f"bytes={CUTOFF}-",
        f"bytes={2 * CUTOFF}-",
        f"bytes={3 * CUTOFF}-",
    ]
    assert _events(event_log) == [
        "curl",
        "sleep:1",
        "curl",
        "sleep:2",
        "curl",
        "sleep:4",
        "curl",
    ]
    assert "exhausted 4 bounded download attempts" in result.stderr


@pytest.mark.parametrize("status", (18, 28, 35, 52, 55, 56))
def test_download_checked_retries_only_each_allowlisted_transport_class(
    tmp_path: Path,
    status: int,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        mode=f"status:{status}:000",
    )

    assert result.returncode == status
    assert not destination.exists()
    assert _events(event_log) == [
        "curl",
        "sleep:1",
        "curl",
        "sleep:2",
        "curl",
        "sleep:4",
        "curl",
    ]


@pytest.mark.parametrize(
    "mode,succeeds,diagnostic",
    (
        ("complete-status:56:200", True, ""),
        ("complete-status:56:401", False, "unexpected HTTP 401"),
        ("complete-status:60:200", False, "terminal curl status 60"),
        ("complete-status:22:503", False, "terminal curl status 22"),
        ("complete-status:0:416", False, "unexpected HTTP 416"),
    ),
)
def test_exact_hash_never_overrides_http_tls_or_terminal_curl_status(
    tmp_path: Path,
    mode: str,
    succeeds: bool,
    diagnostic: str,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        mode=mode,
    )

    assert (result.returncode == 0) is succeeds
    assert destination.exists() is succeeds
    assert _events(event_log) == ["curl"]
    if succeeds:
        assert destination.read_bytes() == PAYLOAD
    else:
        assert diagnostic in result.stderr


@pytest.mark.parametrize(
    "mode, diagnostic",
    (
        ("ignore", "terminal curl status 33"),
        ("wrong", "terminal curl status 33"),
        ("short", "checksum mismatch"),
        ("416", "unexpected HTTP 416"),
        ("hash-mismatch", "checksum mismatch"),
        ("http-401", "terminal curl status 22"),
        ("http-408", "terminal curl status 22"),
        ("http-429", "terminal curl status 22"),
        ("http-503", "terminal curl status 22"),
    ),
)
def test_download_checked_rejects_http_range_and_hash_failures(
    tmp_path: Path,
    mode: str,
    diagnostic: str,
) -> None:
    with _server(mode) as server:
        url = f"http://127.0.0.1:{server.server_port}/artifact"
        result, destination, event_log, _ = _run_harness(tmp_path, url=url)

    assert result.returncode != 0
    assert not destination.exists()
    assert diagnostic in result.stderr
    expected_requests = 2 if mode in {"ignore", "wrong", "short", "416"} else 1
    assert len(server.requests) == expected_requests
    assert _events(event_log).count("curl") == expected_requests


@pytest.mark.parametrize(
    "status,http_status",
    (
        (6, "000"),
        (7, "000"),
        (16, "000"),
        (22, "503"),
        (23, "200"),
        (33, "206"),
        (36, "206"),
        (47, "000"),
        (60, "000"),
        (67, "401"),
        (77, "000"),
        (92, "000"),
    ),
)
def test_download_checked_does_not_retry_terminal_curl_classes(
    tmp_path: Path,
    status: int,
    http_status: str,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        mode=f"status:{status}:{http_status}",
    )

    assert result.returncode == status
    assert not destination.exists()
    assert _events(event_log) == ["curl"]
    assert f"terminal curl status {status}" in result.stderr


@pytest.mark.parametrize("mode", ("symlink", "fifo", "hardlink", "oversized"))
def test_download_checked_rejects_replaced_or_special_partial(
    tmp_path: Path,
    mode: str,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        mode=mode,
    )

    assert result.returncode != 0
    assert not destination.exists()
    assert _events(event_log) == ["curl"]
    assert "is not one bounded regular archive" in result.stderr


def test_download_checked_preserves_destination_race_winner(tmp_path: Path) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        mode="destination-race",
    )

    assert result.returncode != 0
    assert destination.read_bytes() == b"winner\n"
    assert _events(event_log) == ["curl"]
    assert "cache destination appeared before promotion" in result.stderr


def test_download_checked_rejects_preexisting_destination_without_network(
    tmp_path: Path,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        preexisting_destination=b"preexisting\n",
    )

    assert result.returncode != 0
    assert destination.read_bytes() == b"preexisting\n"
    assert _events(event_log) == []
    assert "cache destination already exists" in result.stderr


def test_download_checked_never_reuses_stale_partial_from_prior_invocation(
    tmp_path: Path,
) -> None:
    with _server("normal") as server:
        url = f"http://127.0.0.1:{server.server_port}/artifact"
        result, destination, event_log, _ = _run_harness(
            tmp_path,
            url=url,
            stale_partial=b"untrusted prior bytes",
        )

    assert result.returncode == 0, result.stderr
    assert destination.read_bytes() == PAYLOAD
    assert server.requests == [None]
    assert _events(event_log) == ["curl"]
    assert destination.with_name(
        f"{destination.name}.partial.previous"
    ).read_bytes() == b"untrusted prior bytes"


def test_download_checked_rejects_symlinked_destination_parent_before_network(
    tmp_path: Path,
) -> None:
    result, destination, event_log, _ = _run_harness(
        tmp_path,
        url="http://unused.invalid/artifact",
        symlink_parent=True,
    )

    assert result.returncode != 0
    assert not destination.exists()
    assert _events(event_log) == []


def test_hash_helper_rejects_device_input(tmp_path: Path) -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(HELPER),
            "--lock",
            str(LOCK),
            "--platform",
            "arm64-darwin",
            "hash-file",
            "--path",
            "/dev/null",
        ],
        cwd=ROOT_DIR,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )

    assert result.returncode != 0
    assert "is not one bounded regular archive" in result.stderr


def test_hash_helper_rejects_replacement_during_hash(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    namespace = runpy.run_path(str(HELPER))
    hash_regular_file = namespace["_hash_regular_file"]
    lock_error = namespace["LockError"]
    helper_os = hash_regular_file.__globals__["os"]
    original_read = helper_os.read
    target = tmp_path / "partial"
    replacement = tmp_path / "replacement"
    target.write_bytes(PAYLOAD)
    replacement.write_bytes(b"replacement")
    replaced = False

    def replace_after_first_read(descriptor: int, size: int) -> bytes:
        nonlocal replaced
        block = original_read(descriptor, size)
        if not replaced:
            replacement.replace(target)
            replaced = True
        return block

    monkeypatch.setattr(helper_os, "read", replace_after_first_read)
    with pytest.raises(lock_error, match="changed while it was hashed"):
        hash_regular_file(target)

    assert replaced
