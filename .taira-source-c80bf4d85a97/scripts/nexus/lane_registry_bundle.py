#!/usr/bin/env python3
"""Bundle Nexus lane manifests and governance modules for distribution (NX-2)."""

from __future__ import annotations

import argparse
import json
import shutil
import tarfile
from dataclasses import dataclass
from hashlib import blake2b, sha256
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple


@dataclass(frozen=True)
class ModuleEntry:
    """Represents a governance module definition for the overlay."""

    name: str
    module_type: str
    params: Dict[str, str]


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create a lane registry bundle containing manifests and an optional governance "
            "catalog overlay so operators can distribute manifests across nodes."
        ),
    )
    parser.add_argument(
        "--manifest",
        action="append",
        required=True,
        help="Path to a lane manifest JSON file (repeatable).",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        required=True,
        help="Directory where bundle contents should be written.",
    )
    parser.add_argument(
        "--bundle-out",
        type=Path,
        help="Optional .tar/.tar.gz/.tar.xz archive path capturing the bundle.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path for the summary JSON (defaults to <output-dir>/summary.json).",
    )
    parser.add_argument(
        "--default-module",
        help="Default governance module identifier recorded in the overlay.",
    )
    parser.add_argument(
        "--module",
        action="append",
        default=[],
        help=(
            "Governance module spec in the form "
            "'name=<id>,module_type=<type>[,param.<key>=<value>...]' (repeatable)."
        ),
    )
    parser.add_argument(
        "--module-file",
        type=Path,
        action="append",
        default=[],
        help=(
            "Path to a governance catalog overlay JSON file whose entries should be merged "
            "into the generated overlay (repeatable)."
        ),
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing output directories or bundle archives.",
    )
    parser.add_argument(
        "--skip-overlay",
        action="store_true",
        help="Skip writing cache/governance_catalog.json even when modules are provided.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def slugify_alias(alias: str) -> str:
    sanitized = "".join(ch.lower() if ch.isalnum() else "_" for ch in alias).strip("_")
    return sanitized or "lane"


def parse_module_spec(entry: str) -> ModuleEntry:
    if not entry or "=" not in entry:
        raise ValueError("module spec must contain key=value assignments")
    name: Optional[str] = None
    module_type: Optional[str] = None
    params: Dict[str, str] = {}
    for raw in entry.split(","):
        part = raw.strip()
        if not part:
            continue
        if "=" not in part:
            raise ValueError(f"invalid token `{part}` in module spec `{entry}`")
        key, value = part.split("=", 1)
        key = key.strip()
        value = value.strip()
        if key in {"name"}:
            name = value
        elif key in {"module_type", "type"}:
            module_type = value
        elif key.startswith("param.") or key.startswith("params."):
            param_key = key.split(".", 1)[1]
            if not param_key:
                raise ValueError(f"module spec `{entry}` contains blank param key")
            params[param_key] = value
        else:
            raise ValueError(f"unknown field `{key}` in module spec `{entry}`")
    if not name or not module_type:
        raise ValueError("module spec must include name=<id> and module_type=<type>")
    return ModuleEntry(name=name, module_type=module_type, params=params)


def load_overlay_file(path: Path) -> Dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"overlay file {path} must contain a JSON object")
    modules = data.get("modules", {})
    if modules is not None and not isinstance(modules, dict):
        raise ValueError(f"overlay file {path} has invalid `modules` entry")
    return data


def compute_digests(payload: bytes) -> Tuple[str, str]:
    return sha256(payload).hexdigest(), blake2b(payload, digest_size=32).hexdigest()


def summarize_privacy_commitments(manifest: Dict) -> List[Dict[str, str]]:
    commits = []
    for entry in manifest.get("privacy_commitments") or []:
        cid = entry.get("id")
        scheme = entry.get("scheme")
        if cid is None or scheme is None:
            continue
        try:
            cid_int = int(cid)
        except (TypeError, ValueError):
            continue
        commits.append({"id": cid_int, "scheme": str(scheme)})
    commits.sort(key=lambda item: item["id"])
    return commits


def write_manifest(manifest_path: Path, manifests_dir: Path, force: bool) -> Dict:
    data = json.loads(manifest_path.read_text(encoding="utf-8"))
    alias = data.get("lane")
    if not isinstance(alias, str) or not alias.strip():
        raise ValueError(f"manifest {manifest_path} is missing a `lane` field")
    slug = slugify_alias(alias)
    dest = manifests_dir / f"{slug}.manifest.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and not force:
        raise FileExistsError(f"{dest} already exists (rerun with --force to overwrite)")
    serialized = json.dumps(data, indent=2, sort_keys=True)
    dest.write_text(serialized + "\n", encoding="utf-8")
    payload = dest.read_bytes()
    sha_hex, blake_hex = compute_digests(payload)
    return {
        "alias": alias,
        "slug": slug,
        "source": str(manifest_path),
        "path": str(dest),
        "sha256": sha_hex,
        "blake2b": blake_hex,
        "privacy_commitments": summarize_privacy_commitments(data),
    }


def merge_overlay(
    base: Dict[str, Dict],
    overlay: Dict,
) -> None:
    if "default_module" in overlay:
        base["default_module"] = overlay["default_module"]
    modules = overlay.get("modules") or {}
    if not isinstance(modules, dict):
        raise ValueError("overlay modules entry must be an object")
    target_modules = base.setdefault("modules", {})
    for name, module in modules.items():
        if not isinstance(module, dict):
            raise ValueError(f"module `{name}` must be an object")
        module_type = module.get("module_type")
        if not isinstance(module_type, str) or not module_type.strip():
            raise ValueError(f"module `{name}` is missing module_type")
        params = module.get("params", {})
        if params is not None and not isinstance(params, dict):
            raise ValueError(f"module `{name}` params must be an object")
        entry = {
            "module_type": module_type,
            "params": params or {},
        }
        target_modules[name] = entry


def build_overlay(
    args: argparse.Namespace,
) -> Tuple[Optional[Dict], Optional[Path]]:
    overlay: Dict[str, Dict] = {"modules": {}}
    for overlay_path in args.module_file:
        merge_overlay(overlay, load_overlay_file(overlay_path))
    for spec in args.module:
        module_entry = parse_module_spec(spec)
        overlay["modules"][module_entry.name] = {
            "module_type": module_entry.module_type,
            "params": module_entry.params,
        }
    if args.default_module:
        overlay["default_module"] = args.default_module
    if args.skip_overlay or (
        not overlay.get("modules") and "default_module" not in overlay
    ):
        return None, None
    return overlay, Path("cache/governance_catalog.json")


def infer_tar_mode(path: Path) -> str:
    lower = path.name.lower()
    if lower.endswith(".tar.gz") or lower.endswith(".tgz"):
        return "w:gz"
    if lower.endswith(".tar.xz") or lower.endswith(".txz"):
        return "w:xz"
    if lower.endswith(".tar.bz2") or lower.endswith(".tbz2"):
        return "w:bz2"
    if lower.endswith(".tar"):
        return "w"
    raise ValueError(
        f"unsupported archive extension for {path}; use .tar, .tar.gz, .tgz, .tar.xz, or .tar.bz2",
    )


def create_archive(bundle_path: Path, root_dir: Path, force: bool) -> None:
    if bundle_path.exists():
        if not force:
            raise FileExistsError(f"{bundle_path} exists (rerun with --force)")
        bundle_path.unlink()
    bundle_path.parent.mkdir(parents=True, exist_ok=True)
    mode = infer_tar_mode(bundle_path)
    with tarfile.open(bundle_path, mode) as tar:
        tar.add(root_dir, arcname=root_dir.name)


def prepare_output_dir(output_dir: Path, force: bool) -> None:
    if output_dir.exists():
        if not force:
            raise FileExistsError(
                f"{output_dir} already exists (rerun with --force to overwrite)",
            )
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)


def bundle_registry(args: argparse.Namespace) -> Dict:
    output_dir = args.output_dir.expanduser().resolve()
    prepare_output_dir(output_dir, args.force)
    manifests_dir = output_dir / "manifests"
    manifests_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = output_dir / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)

    manifest_summaries = []
    for manifest_path_str in args.manifest:
        manifest_path = Path(manifest_path_str).expanduser().resolve()
        if not manifest_path.exists():
            raise FileNotFoundError(f"manifest file {manifest_path} does not exist")
        manifest_summaries.append(
            write_manifest(manifest_path, manifests_dir, args.force),
        )

    overlay, overlay_rel_path = build_overlay(args)
    overlay_path: Optional[Path] = None
    if overlay and overlay_rel_path:
        overlay_path = (output_dir / overlay_rel_path).resolve()
        overlay_path.parent.mkdir(parents=True, exist_ok=True)
        overlay_path.write_text(json.dumps(overlay, indent=2) + "\n", encoding="utf-8")

    summary_path = args.summary_out or (output_dir / "summary.json")
    summary_path = summary_path.expanduser().resolve()
    summary = {
        "output_dir": str(output_dir),
        "manifests": manifest_summaries,
        "governance_overlay": {
            "path": str(overlay_path) if overlay_path else None,
            "default_module": overlay.get("default_module") if overlay else None,
            "modules": overlay.get("modules") if overlay else {},
        },
        "bundle_path": None,
    }
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    if args.bundle_out:
        bundle_path = args.bundle_out.expanduser().resolve()
        create_archive(bundle_path, output_dir, args.force)
        summary["bundle_path"] = str(bundle_path)
        summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    return summary


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    summary = bundle_registry(args)
    print(f"[nexus] registry bundle written to {summary['output_dir']}")
    if summary.get("bundle_path"):
        print(f"[nexus] archive written to {summary['bundle_path']}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
