#!/usr/bin/env python3
"""Local Soracloud Hugging Face inference runner.

This runner executes against an already-imported local HF source directory.
It accepts one JSON request on stdin and emits one JSON response on stdout.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def emit(payload: dict) -> int:
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=False))
    sys.stdout.flush()
    return 0


def error(code: str, message: str) -> int:
    return emit({"ok": False, "error": {"code": code, "message": message}})


def load_json(path: Path) -> dict | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    except Exception:
        return None


def maybe_run_fixture(source_files_dir: Path, request_body: object, request: dict) -> dict | None:
    config = load_json(source_files_dir / "config.json") or {}
    fixture = config.get("_soracloud_fixture")
    if not isinstance(fixture, dict):
        return None

    mode = fixture.get("mode", "echo")
    if mode != "echo":
        raise RuntimeError(f"unsupported _soracloud_fixture mode: {mode}")

    if isinstance(request_body, dict):
        inputs = request_body.get("inputs")
        parameters = request_body.get("parameters")
    else:
        inputs = request_body
        parameters = None

    prefix = fixture.get("prefix", "")
    return {
        "backend": "local_fixture",
        "repo_id": request.get("repo_id"),
        "model_name": request.get("model_name"),
        "pipeline_tag": request.get("pipeline_tag"),
        "inputs": inputs,
        "parameters": parameters,
        "request_query": request.get("request_query"),
        "text": f"{prefix}{inputs}",
    }


def build_transformers_pipeline(source_files_dir: Path, pipeline_tag: str):
    try:
        os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
        import transformers  # type: ignore
    except ImportError as exc:
        raise RuntimeError(
            "local HF execution requires the `transformers` Python package on the node"
        ) from exc

    try:
        import torch  # type: ignore

        try:
            torch.set_num_threads(1)
        except Exception:
            pass
    except ImportError:
        pass

    kwargs = {
        "task": pipeline_tag,
        "model": str(source_files_dir),
        "local_files_only": True,
        "trust_remote_code": False,
    }

    if (source_files_dir / "tokenizer.json").exists() or (
        source_files_dir / "tokenizer_config.json"
    ).exists():
        kwargs["tokenizer"] = str(source_files_dir)
    if (source_files_dir / "preprocessor_config.json").exists():
        kwargs["feature_extractor"] = str(source_files_dir)
        kwargs["image_processor"] = str(source_files_dir)

    try:
        return transformers.pipeline(**kwargs)
    except Exception as exc:
        raise RuntimeError(
            f"failed to build local transformers pipeline `{pipeline_tag}` from {source_files_dir}: {exc}"
        ) from exc


def run_transformers(request: dict, request_body: object) -> dict:
    pipeline_tag = request.get("pipeline_tag")
    if not isinstance(pipeline_tag, str) or not pipeline_tag.strip():
        raise RuntimeError("generated HF source does not expose a usable pipeline_tag")

    source_files_dir = Path(request["source_files_dir"])
    pipeline = build_transformers_pipeline(source_files_dir, pipeline_tag)

    parameters = {}
    inputs = request_body
    if isinstance(request_body, dict):
        parameters = request_body.get("parameters") or {}
        if not isinstance(parameters, dict):
            raise RuntimeError("HF request field `parameters` must be a JSON object")
        inputs = request_body["inputs"] if "inputs" in request_body else request_body

    try:
        if parameters:
            response = pipeline(inputs, **parameters)
        else:
            response = pipeline(inputs)
    except Exception as exc:
        raise RuntimeError(f"local pipeline execution failed: {exc}") from exc

    return {
        "backend": "local_transformers",
        "repo_id": request.get("repo_id"),
        "pipeline_tag": pipeline_tag,
        "response": response,
    }


def main() -> int:
    try:
        request = json.load(sys.stdin)
    except Exception as exc:
        return error("invalid_request", f"failed to decode local HF runner request: {exc}")

    if request.get("schema_version") != 1:
        return error("unsupported_schema", "local HF runner request schema_version must be 1")

    source_files_dir_raw = request.get("source_files_dir")
    if not isinstance(source_files_dir_raw, str) or not source_files_dir_raw.strip():
        return error("invalid_request", "local HF runner requires `source_files_dir`")
    source_files_dir = Path(source_files_dir_raw)
    if not source_files_dir.is_dir():
        return error(
            "source_missing",
            f"local HF source directory is missing: {source_files_dir}",
        )

    request_body = request.get("request_body")
    if request_body is None:
        request_body = {}

    try:
        fixture_response = maybe_run_fixture(source_files_dir, request_body, request)
        if fixture_response is not None:
            response = fixture_response
        else:
            response = run_transformers(request, request_body)
    except Exception as exc:
        return error("local_execution_failed", str(exc))

    return emit(
        {
            "ok": True,
            "content_type": "application/json",
            "response_json": response,
        }
    )


if __name__ == "__main__":
    raise SystemExit(main())
