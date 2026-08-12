"""Focused cases for the protected Sumeragi v2 release-tool probes."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
PROBE_SCRIPT = REPO_ROOT / "scripts" / "probe_sumeragi_v2_release_tool_closure.py"
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")


def _load_probe_module():
    spec = importlib.util.spec_from_file_location("sumeragi_v2_tool_probe", PROBE_SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


PROBE = _load_probe_module()


_DISPATCHER_SOURCE = r"""
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/utsname.h>
#include <time.h>
#include <unistd.h>

static const char *tool_name(const char *path) {
    const char *slash = strrchr(path, '/');
    return slash == NULL ? path : slash + 1;
}

static int copy_fd(int input, int output) {
    char buffer[4096];
    for (;;) {
        ssize_t received = read(input, buffer, sizeof(buffer));
        if (received == 0) return 0;
        if (received < 0) return 1;
        ssize_t offset = 0;
        while (offset < received) {
            ssize_t written = write(output, buffer + offset, (size_t)(received - offset));
            if (written <= 0) return 1;
            offset += written;
        }
    }
}

static int copy_file(const char *source, const char *target) {
    int input = open(source, O_RDONLY);
    if (input < 0) return 1;
    int output = open(target, O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (output < 0) { close(input); return 1; }
    int result = copy_fd(input, output);
    if (close(input) != 0 || close(output) != 0) return 1;
    return result;
}

int main(int argc, char **argv) {
    const char *name = tool_name(argv[0]);
    if (strcmp(name, "awk") == 0) { puts("probe-ok"); return 0; }
    if (strcmp(name, "basename") == 0) { puts("beta"); return 0; }
    if (strcmp(name, "cargo") == 0) {
        puts("cargo 1.93.1 (083ac5135 2025-12-15)"); return 0;
    }
    if (strcmp(name, "cargo-verus") == 0) { puts("cargo verus help"); return 0; }
    if (strcmp(name, "cat") == 0) {
        int input = argc > 1 ? open(argv[1], O_RDONLY) : -1;
        int result = input < 0 ? 1 : copy_fd(input, STDOUT_FILENO);
        if (input >= 0) close(input);
        return result;
    }
    if (strcmp(name, "chmod") == 0) return argc == 3 && chmod(argv[2], 0400) == 0 ? 0 : 1;
    if (strcmp(name, "cmp") == 0) return 1;
    if (strcmp(name, "diff") == 0) { puts("Files left and right differ"); return 1; }
    if (strcmp(name, "cp") == 0) return argc == 3 ? copy_file(argv[1], argv[2]) : 1;
    if (strcmp(name, "cut") == 0) { fputs("b\n", stdout); return 0; }
    if (strcmp(name, "dirname") == 0) { puts("alpha"); return 0; }
    if (strcmp(name, "env") == 0) { puts("RELEASE_PROBE=closed"); return 0; }
    if (strcmp(name, "find") == 0) { puts("find-root/probe-file"); return 0; }
    if (strcmp(name, "grep") == 0) { puts("probe"); return 0; }
    if (strcmp(name, "git-index-pack") == 0) { fputs("fatal: early EOF\n", stderr); return 128; }
    if (strcmp(name, "git-upload-pack") == 0) {
        fputs("fatal: 'missing.git' does not appear to be a git repository\n", stderr);
        return 128;
    }
    if (strcmp(name, "java") == 0) { fputs("openjdk version \"21\"\n", stderr); return 0; }
    if (strcmp(name, "ln") == 0) return argc == 3 && link(argv[1], argv[2]) == 0 ? 0 : 1;
    if (strcmp(name, "ls") == 0) { puts("listed"); return 0; }
    if (strcmp(name, "mkdir") == 0) return argc == 2 && mkdir(argv[1], 0777) == 0 ? 0 : 1;
    if (strcmp(name, "mkfifo") == 0) return argc == 2 && mkfifo(argv[1], 0666) == 0 ? 0 : 1;
    if (strcmp(name, "mktemp") == 0) {
        if (argc != 2) return 1;
        int descriptor = mkstemp(argv[1]);
        if (descriptor < 0 || fchmod(descriptor, 0600) != 0 || close(descriptor) != 0) return 1;
        puts(argv[1]);
        return 0;
    }
    if (strcmp(name, "mv") == 0) return argc == 3 && rename(argv[1], argv[2]) == 0 ? 0 : 1;
    if (strcmp(name, "node") == 0) { puts(argv[0]); return 0; }
    if (strcmp(name, "openssl") == 0) {
        const unsigned char digest[32] = {
            0xe3,0xb0,0xc4,0x42,0x98,0xfc,0x1c,0x14,0x9a,0xfb,0xf4,0xc8,0x99,0x6f,0xb9,0x24,
            0x27,0xae,0x41,0xe4,0x64,0x9b,0x93,0x4c,0xa4,0x95,0x99,0x1b,0x78,0x52,0xb8,0x55
        };
        return write(STDOUT_FILENO, digest, sizeof(digest)) == (ssize_t)sizeof(digest) ? 0 : 1;
    }
    if (strcmp(name, "rm") == 0) return argc == 3 && unlink(argv[2]) == 0 ? 0 : 1;
    if (strcmp(name, "rmdir") == 0) return argc == 2 && rmdir(argv[1]) == 0 ? 0 : 1;
    if (strcmp(name, "rustc") == 0) {
        puts("rustc 1.93.1 (01f6ddf75 2026-02-11)"); return 0;
    }
    if (strcmp(name, "sed") == 0) { puts("first"); return 0; }
    if (strcmp(name, "sh") == 0) {
        puts(argc > 2 && strstr(argv[2], "xargs-ok") != NULL ? "xargs-ok" : "probe-ok");
        return 0;
    }
    if (strcmp(name, "sleep") == 0) {
        struct timespec request = {1, 0};
        return nanosleep(&request, NULL) == 0 ? 0 : 1;
    }
    if (strcmp(name, "tail") == 0) { puts("last"); return 0; }
    if (strcmp(name, "shasum") == 0 || strcmp(name, "sha256sum") == 0) {
        puts("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  -");
        return 0;
    }
    if (strcmp(name, "swift") == 0) { puts("Swift version 6.0"); return 0; }
    if (strcmp(name, "tee") == 0) {
        if (argc != 2) return 1;
        int output = open(argv[1], O_WRONLY | O_CREAT | O_EXCL, 0600);
        if (output < 0) return 1;
        char buffer[4096];
        for (;;) {
            ssize_t received = read(STDIN_FILENO, buffer, sizeof(buffer));
            if (received == 0) break;
            if (received < 0 || write(STDOUT_FILENO, buffer, (size_t)received) != received ||
                write(output, buffer, (size_t)received) != received) { close(output); return 1; }
        }
        return close(output) == 0 ? 0 : 1;
    }
    if (strcmp(name, "tlapm") == 0) { puts("3ab43c7"); return 0; }
    if (strcmp(name, "tr") == 0) {
        int byte;
        while ((byte = getchar()) != EOF) putchar(byte == 'a' ? 'b' : byte);
        return ferror(stdin) || ferror(stdout) ? 1 : 0;
    }
    if (strcmp(name, "uname") == 0) {
        struct utsname value;
        if (uname(&value) != 0) return 1;
        puts(value.sysname);
        return 0;
    }
    if (strcmp(name, "verus") == 0) { puts("Verus 0.2026.05.31.5dd6d83"); return 0; }
    if (strcmp(name, "wc") == 0) {
        size_t count = 0;
        while (getchar() != EOF) count++;
        printf("%zu\n", count);
        return ferror(stdin) ? 1 : 0;
    }
    if (strcmp(name, "xargs") == 0) {
        char input[6] = {0};
        if (argc != 6 || read(STDIN_FILENO, input, sizeof(input)) != 6 ||
            memcmp(input, "probe\0", 6) != 0) return 1;
        char *child[] = {argv[2], argv[3], argv[4], argv[5], "probe", NULL};
        execv(argv[2], child);
        return 1;
    }
    return 99;
}
"""


_VARIANT_SOURCES = {
    "noop": "int main(void) { return 0; }\n",
    "killed": "#include <signal.h>\nint main(void) { raise(SIGKILL); return 0; }\n",
    "noisy": (
        "#include <unistd.h>\n"
        "int main(void) { char x[4096] = {0}; for (int i=0;i<17;i++) "
        "if (write(1,x,sizeof(x)) != sizeof(x)) return 1; return 0; }\n"
    ),
    "status7": "int main(void) { return 7; }\n",
    "mutating": (
        "#include <sys/stat.h>\n"
        "int main(int argc, char **argv) { if (chmod(argv[0],0700)) return 1; "
        "return argc == 3 && chmod(argv[2],0400) == 0 ? 0 : 1; }\n"
    ),
    "hanging": "#include <unistd.h>\nint main(void) { for (;;) pause(); }\n",
}


def _compile(cc: str, root: Path, name: str, source: str) -> Path:
    source_path = root / f"{name}.c"
    executable = root / name
    source_path.write_text(source, encoding="utf-8")
    subprocess.run(
        [cc, "-std=c11", "-O2", str(source_path), "-o", str(executable)],
        check=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    executable.chmod(0o500)
    return executable


@pytest.fixture(scope="session")
def native_fixtures(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Path]:
    cc = shutil.which("cc")
    assert cc is not None, "focused release-tool probe cases require a C compiler"
    root = tmp_path_factory.mktemp("release-tool-native-fixtures")
    root.chmod(0o700)
    fixtures = {"dispatcher": _compile(cc, root, "dispatcher", _DISPATCHER_SOURCE)}
    fixtures.update(
        {
            name: _compile(cc, root, name, source)
            for name, source in _VARIANT_SOURCES.items()
        }
    )
    return fixtures


def _tool_closure(tmp_path: Path, dispatcher: Path) -> tuple[dict[str, Path], dict[str, str]]:
    tmp_path.chmod(0o700)
    root = tmp_path / "tool-archives"
    root.mkdir(mode=0o700)
    paths: dict[str, Path] = {}
    digests: dict[str, str] = {}
    for name in PROBE.REQUIRED_TOOL_NAMES:
        destination = root / name
        shutil.copyfile(dispatcher, destination)
        destination.chmod(0o500)
        paths[name] = destination
        digests[name] = hashlib.sha256(destination.read_bytes()).hexdigest()
    return paths, digests


def _replace_tool(paths: dict[str, Path], name: str, source: Path) -> None:
    destination = paths[name]
    destination.chmod(0o700)
    shutil.copyfile(source, destination)
    destination.chmod(0o500)


def _run_probe(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> tuple[dict[str, Path], dict[str, str], dict[str, object]]:
    paths, digests = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    result = PROBE.probe_release_tool_closure(
        paths,
        tmp_path / "probe-private",
        expected_sha256=digests,
    )
    return paths, digests, result


def test_probe_table_is_the_exact_platform_closure() -> None:
    assert len(PROBE.REQUIRED_TOOL_NAMES) == 41
    assert len(set(PROBE.REQUIRED_TOOL_NAMES)) == 41
    assert set(PROBE.PROBE_OPERATION_IDS) == set(PROBE.REQUIRED_TOOL_NAMES)
    assert set(PROBE._EXPECTED_EXIT_STATUSES) == set(PROBE.REQUIRED_TOOL_NAMES)
    assert ("shasum" in PROBE.REQUIRED_TOOL_NAMES) == (sys.platform == "darwin")
    assert ("sha256sum" in PROBE.REQUIRED_TOOL_NAMES) == (sys.platform != "darwin")


def test_probe_accepts_relocatable_native_closure_and_returns_no_paths(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> None:
    paths, digests, result = _run_probe(tmp_path, native_fixtures)
    assert not (tmp_path / "probe-private").exists()
    assert set(result) == {
        "format",
        "host_family",
        "probe_contract_sha256",
        "schema_version",
        "tool_count",
        "tools",
    }
    assert result["format"] == PROBE.PROBE_FORMAT
    assert result["schema_version"] == 1
    assert result["tool_count"] == 41
    assert _DIGEST_RE.fullmatch(result["probe_contract_sha256"])
    assert set(result["tools"]) == set(PROBE.REQUIRED_TOOL_NAMES)
    record_keys = {
        "archive_id",
        "exit_status",
        "invocation_sha256",
        "mode",
        "operation_id",
        "postcondition_sha256",
        "sha256",
        "size_bytes",
        "stderr_sha256",
        "stderr_size_bytes",
        "stdout_sha256",
        "stdout_size_bytes",
    }
    for name, record in result["tools"].items():
        assert set(record) == record_keys
        assert record["archive_id"] == f"release-tool.{name}.v1"
        assert record["mode"] == "0500"
        assert record["operation_id"] == PROBE.PROBE_OPERATION_IDS[name]
        assert record["sha256"] == digests[name]
        for field in (
            "invocation_sha256",
            "postcondition_sha256",
            "sha256",
            "stderr_sha256",
            "stdout_sha256",
        ):
            assert _DIGEST_RE.fullmatch(record[field])
    encoded = PROBE.canonical_json(result)
    assert os.fsencode(tmp_path) not in encoded
    assert all(os.fsencode(path) not in encoded for path in paths.values())


def test_standalone_cli_authenticates_manifest_and_emits_path_free_result(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> None:
    paths, digests = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    manifest = tmp_path / "manifest.json"
    manifest_bytes = PROBE.canonical_json(
        {
            "schema_version": 1,
            "tools": {
                name: {
                    "archive_id": f"release-runner-tool.{name}.v1",
                    "path": str(paths[name]),
                    "sha256": digests[name],
                }
                for name in PROBE.REQUIRED_TOOL_NAMES
            },
        }
    )
    manifest.write_bytes(manifest_bytes)
    manifest.chmod(0o400)
    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(PROBE_SCRIPT),
            "--tool-manifest",
            str(manifest),
            "--expected-tool-manifest-sha256",
            hashlib.sha256(manifest_bytes).hexdigest(),
            "--probe-root",
            str(tmp_path / "probe-private"),
        ],
        check=False,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={"LANG": "C", "LC_ALL": "C", "PATH": "", "TZ": "UTC"},
        timeout=30,
    )
    assert completed.returncode == 0, completed.stderr.decode("utf-8", "replace")
    assert completed.stderr == b""
    result = json.loads(completed.stdout)
    assert result["tool_count"] == 41
    assert os.fsencode(tmp_path) not in completed.stdout
    assert not (tmp_path / "probe-private").exists()


@pytest.mark.parametrize(
    ("shebang", "message"),
    [
        (b"#!/usr/bin/perl\nexit 0;\n", "forbidden absolute shebang"),
        (b"#!protected-perl\nexit 0;\n", "unlisted shebang"),
    ],
)
def test_probe_rejects_unclosed_script_interpreters_before_execution(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
    shebang: bytes,
    message: str,
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    tool = paths["shasum" if sys.platform == "darwin" else "sha256sum"]
    tool.chmod(0o700)
    tool.write_bytes(shebang)
    tool.chmod(0o500)
    with pytest.raises(PROBE.ToolProbeError, match=message):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


def test_probe_rejects_copied_executable_killed_at_launch(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, "awk", native_fixtures["killed"])
    with pytest.raises(PROBE.ToolProbeError, match="awk was terminated by signal 9"):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


def test_probe_rejects_status_zero_without_the_tool_postcondition(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, "chmod", native_fixtures["noop"])
    with pytest.raises(PROBE.ToolProbeError, match="chmod did not apply the exact mode"):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


@pytest.mark.parametrize(
    "name",
    ("cmp", "diff", "env", "find", "grep", "sh", "sleep", "tail", "xargs"),
)
def test_every_observable_utility_probe_rejects_a_status_zero_noop(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
    name: str,
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, name, native_fixtures["noop"])
    with pytest.raises(PROBE.ToolProbeError, match=f"release tool {name}"):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


def test_probe_rejects_tool_content_metadata_drift_after_execution(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, "chmod", native_fixtures["mutating"])
    with pytest.raises(PROBE.ToolProbeError, match="chmod changed across its functional probe"):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


@pytest.mark.parametrize(
    ("variant", "message"),
    [
        ("noisy", "exceeded its output bound"),
        ("status7", "awk returned unexpected status 7"),
    ],
)
def test_probe_enforces_bounded_output_and_exact_status(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
    variant: str,
    message: str,
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, "awk", native_fixtures[variant])
    with pytest.raises(PROBE.ToolProbeError, match=message):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()


def test_probe_enforces_bounded_runtime_and_reclaims_private_state(
    tmp_path: Path,
    native_fixtures: dict[str, Path],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    paths, _ = _tool_closure(tmp_path, native_fixtures["dispatcher"])
    _replace_tool(paths, "awk", native_fixtures["hanging"])
    monkeypatch.setattr(PROBE, "COMMAND_TIMEOUT_SECONDS", 0.05)
    with pytest.raises(PROBE.ToolProbeError, match="exceeded its runtime bound"):
        PROBE.probe_release_tool_closure(paths, tmp_path / "probe-private")
    assert not (tmp_path / "probe-private").exists()
