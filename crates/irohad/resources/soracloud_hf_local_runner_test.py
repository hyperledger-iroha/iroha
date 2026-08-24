#!/usr/bin/env python3
"""Injected unit-test runner for Soracloud local HF worker supervision."""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

WORKER_INSTANCE_ID = f"hf-test-worker-{os.getpid()}-{time.time_ns()}"


def emit(payload: dict) -> int:
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=False))
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0


def fixture_config(source_files_dir: Path) -> dict:
    try:
        config = json.loads((source_files_dir / "config.json").read_text(encoding="utf-8"))
    except Exception:
        return {}
    fixture = config.get("_soracloud_fixture")
    return fixture if isinstance(fixture, dict) else {}


def handle_request(request: dict) -> dict:
    if request.get("schema_version") != 1:
        return {
            "ok": False,
            "error": {
                "code": "unsupported_schema",
                "message": "test runner request schema_version must be 1",
            },
        }
    source_files_dir = Path(request.get("source_files_dir", ""))
    if not source_files_dir.is_dir():
        return {
            "ok": False,
            "error": {"code": "source_missing", "message": "test source directory is missing"},
        }
    fixture = fixture_config(source_files_dir)
    mode = fixture.get("mode", "echo")
    if mode != "echo":
        return {
            "ok": False,
            "error": {
                "code": "local_execution_failed",
                "message": f"unsupported injected test-runner mode: {mode}",
            },
        }
    if request.get("probe_only"):
        response = {
            "backend": "injected_test_runner",
            "repo_id": request.get("repo_id"),
            "model_name": request.get("model_name"),
            "pipeline_tag": request.get("pipeline_tag"),
            "worker_instance_id": WORKER_INSTANCE_ID,
            "worker_pid": os.getpid(),
            "probe": "ready",
        }
    else:
        request_body = request.get("request_body") or {}
        if isinstance(request_body, dict):
            inputs = request_body.get("inputs")
            parameters = request_body.get("parameters")
        else:
            inputs = request_body
            parameters = None
        response = {
            "backend": "injected_test_runner",
            "repo_id": request.get("repo_id"),
            "model_name": request.get("model_name"),
            "pipeline_tag": request.get("pipeline_tag"),
            "worker_instance_id": WORKER_INSTANCE_ID,
            "worker_pid": os.getpid(),
            "inputs": inputs,
            "parameters": parameters,
            "request_query": request.get("request_query"),
            "text": f"{fixture.get('prefix', '')}{inputs}",
        }
    return {"ok": True, "content_type": "application/json", "response_json": response}


def serve_forever() -> int:
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            emit(handle_request(request))
        except Exception as exc:
            emit({"ok": False, "error": {"code": "invalid_request", "message": str(exc)}})
    return 0


def main() -> int:
    if "--server" in sys.argv[1:]:
        return serve_forever()
    try:
        request = json.load(sys.stdin)
    except Exception as exc:
        return emit({"ok": False, "error": {"code": "invalid_request", "message": str(exc)}})
    return emit(handle_request(request))


if __name__ == "__main__":
    raise SystemExit(main())
