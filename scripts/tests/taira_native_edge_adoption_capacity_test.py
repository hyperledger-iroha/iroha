"""Maximum opaque-journal custody under an ordinary 256-descriptor child.

This reuses the explicitly isolated adoption authority and native-effect fixture;
it does not prove Rust signature or independent OS-parent authorization. All
files and native child effects stay in that fixture's temporary directory. The
private child marker below is supplied only by this test's subprocess launcher;
no live service, credential, or controller resource limit is changed.
"""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import resource
import subprocess
import sys

from taira_native_edge_adoption_test import IsolatedAdoption, MODULE, OWNER, pin, run
from taira_native_nginx_apply_test import native_apply


CHILD_MARKER = "TAIRA_NATIVE_ADOPTION_CAPACITY_TEST_CHILD"


def test_maximum_opaque_adoption_archives_and_replays_with_256_descriptors(native_apply, monkeypatch):
    if os.environ.get(CHILD_MARKER) != "1":
        directory = Path(__file__).resolve().parent
        program = """import os, resource, sys
resource.setrlimit(resource.RLIMIT_NOFILE, (256, resource.getrlimit(resource.RLIMIT_NOFILE)[1]))
sys.path.insert(0, sys.argv[1])
os.environ['PYTEST_DISABLE_PLUGIN_AUTOLOAD'] = '1'
os.environ['TAIRA_NATIVE_ADOPTION_CAPACITY_TEST_CHILD'] = '1'
import pytest
raise SystemExit(pytest.main(['-q', '--tb=short', sys.argv[1] +
    '/taira_native_edge_adoption_capacity_test.py::test_maximum_opaque_adoption_archives_and_replays_with_256_descriptors']))
"""
        result = subprocess.run([sys.executable, "-I", "-c", program, str(directory)],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
        assert result.returncode == 0, result.stdout.decode()[:16384]
        assert b"1 passed" in result.stdout
        return

    assert resource.getrlimit(resource.RLIMIT_NOFILE)[0] == 256
    request, root, candidate = native_apply
    original = OWNER.remote_apply(request)
    assert original["exit_code"] == 0, original
    admission = IsolatedAdoption(request, root)
    try:
        # Existing isolated authority retains two incidents; fill the admitted
        # maximum with distinct opaque public bodies, never decoded as owners.
        for ordinal in range(30):
            path = root / (".taira-native-nginx-apply-" + f"{ordinal + 1:032x}" + ".receipt.ndjson")
            body = b"opaque capacity incident, deliberately not JSON: " + str(ordinal).encode() + b"\n"
            path.write_bytes(body)
            path.chmod(0o600)
            retained = pin(path)
            admission.fd_handles.append(retained["fd"])
            admission.packet["opaque_journals"].append(retained)
        admission.packet["opaque_journals"].sort(key=lambda item: item["reference"]["file"]["path"])
        admission.plan["opaque_journals"] = [copy.deepcopy(item["reference"])
            for item in admission.packet["opaque_journals"]]
        assert len(admission.plan["opaque_journals"]) == 32
        admission.refresh_directory()

        include = copy.deepcopy(admission.plan["publication"])
        opaque = [Path(item["file"]["path"]).read_bytes() for item in admission.plan["opaque_journals"]]
        reload_count = (root / "reload-count").read_bytes()
        signals = []
        def forbid_signal(*args):
            signals.append(args)
            raise AssertionError("isolated adoption must not request a master signal")
        monkeypatch.setattr(OWNER, "signal_master", forbid_signal)
        loads = OWNER.json.loads
        def refuse_incident_decode(body, *args, **kwargs):
            assert body not in opaque, "opaque incidents must not enter the canonical owner decoder"
            return loads(body, *args, **kwargs)
        monkeypatch.setattr(OWNER.json, "loads", refuse_incident_decode)

        result = run(admission)
        assert result["status"] == "adopted_unqualified", result
        assert result["error_code"] is None
        assert result["owned_publication"]["operation_id"] == "c" * 32
        assert result["owned_publication"]["publication"] == include
        assert len(result["archived_journals"]) == 32
        for ordinal, archive in enumerate(result["archived_journals"]):
            original_ref = admission.plan["opaque_journals"][ordinal]
            assert archive["file"]["path"] == MODULE.adoption_archive_path(admission, ordinal)
            assert Path(archive["file"]["path"]).read_bytes() == opaque[ordinal]
            assert all(archive["file"]["identity"][key] == original_ref["file"]["identity"][key]
                for key in MODULE.IDENTITY_KEYS - {"ctime_ns"})
            assert not Path(original_ref["file"]["path"]).exists()

        owner_reference = result["owned_publication"]["journal"]
        owner_path = Path(owner_reference["file"]["path"])
        owner_body = owner_path.read_bytes()
        owner_rows = [json.loads(line) for line in owner_body.splitlines()]
        assert len(owner_rows) == 1
        assert owner_rows[0]["phase"] == "awaiting_readiness"
        assert owner_rows[0]["qualified"] is False
        assert owner_rows[0]["journal_write_intent"] is not None
        assert owner_rows[0]["journal_publication_identity"]["inode"] == str(owner_path.stat().st_ino)
        authority_path = admission.path / "native-adoption.ndjson"
        authority_body = authority_path.read_bytes()
        authority_rows = [json.loads(line) for line in authority_body.splitlines()]
        assert sum(row["phase"] == "archive_requested" for row in authority_rows) == 32
        assert sum(row["phase"] == "archived" for row in authority_rows) == 32
        assert sum(row["write_intent"] is not None for row in authority_rows) == 1
        assert authority_rows[-1]["phase"] == "adopted_unqualified"
        assert len(authority_body) <= MODULE.MAX_PUBLIC_BYTES
        assert (root / "context-completed").exists()
        assert Path(include["file"]["path"]).read_bytes() == candidate
        assert MODULE.identity(Path(include["file"]["path"]).stat()) == include["file"]["identity"]
        assert (root / "reload-count").read_bytes() == reload_count and signals == []

        # Retain the original 32 incident descriptors while reopening all 32
        # archived references, as a genuine interrupted-owner replay does.
        admission.reopen_packet()
        again = run(admission)
        assert again == result
        assert authority_path.read_bytes() == authority_body
        assert owner_path.read_bytes() == owner_body
        assert MODULE.identity(owner_path.stat()) == owner_reference["file"]["identity"]
        assert Path(include["file"]["path"]).read_bytes() == candidate
        assert MODULE.identity(Path(include["file"]["path"]).stat()) == include["file"]["identity"]
        assert (root / "reload-count").read_bytes() == reload_count and signals == []
    finally:
        admission.close()
