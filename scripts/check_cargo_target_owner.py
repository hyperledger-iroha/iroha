#!/usr/bin/env python3
"""Keep local Cargo commands out of foreign-source and authenticated release lanes.

Cargo dep-info can contain relative source paths. Sharing a checkout's target
directory with a frozen checkout can therefore make an older artifact appear
fresh. Cargo metadata resolves effective target/config precedence without a build
or dependency download. Role records are advisory wrapper coordination; direct
Cargo does not use this guard. No target or cache is created, removed or repaired.
"""

from __future__ import annotations

import argparse
from collections.abc import Sequence
import json
import os
import stat
import subprocess
from pathlib import Path


LANE_GUIDANCE = "use --target-slot <stable-name> or a dedicated external development lane"
BUILD_COMMANDS = {"build", "check", "test", "run", "bench", "rustc", "rustdoc", "doc", "clippy", "fix", "clean"}
NO_BUILD_COMMANDS = {"fmt", "metadata", "locate-project", "tree", "help", "version", "fetch", "update", "generate-lockfile"}


def check_lane_role(target: Path, source_root: Path) -> None:
    """Check the exact Cargo lane, allowing separate named child target lanes."""
    marker = target / ".taira-build-lane" / "role.json"
    if not os.path.lexists(marker):
        return
    try:
        fd = os.open(marker, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            info = os.fstat(fd)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                    or info.st_nlink != 1 or stat.S_IMODE(info.st_mode) != 0o600
                    or info.st_size > 16 * 1024):
                raise ValueError("unsafe lane role")
            role = json.loads(os.read(fd, 16 * 1024 + 1))
        finally:
            os.close(fd)
        if (not isinstance(role, dict) or set(role) != {"schema", "repo_root", "role"}
                or role["schema"] != "taira.cargo-lane.v1"
                or not isinstance(role["role"], str)
                or role["role"] not in {"development", "release"}
                or not isinstance(role["repo_root"], str)
                or not Path(role["repo_root"]).is_absolute()):
            raise ValueError("invalid lane role")
    except (OSError, ValueError) as error:
        raise ValueError(f"Cargo target {target} has an unreadable or invalid lane role; {LANE_GUIDANCE}") from error
    if role["role"] == "release":
        raise ValueError(f"Cargo target {target} is an authenticated release lane; {LANE_GUIDANCE}")
    if Path(role["repo_root"]).resolve() != source_root:
        raise ValueError(f"Cargo target {target} is assigned to another repository; {LANE_GUIDANCE}")


def cargo_selection(arguments: list[str]) -> tuple[list[str], Path | None, Path | None, Path | None] | None:
    """Extract selector arguments only; Cargo owns all TOML/config resolution."""
    prefix: list[str] = []
    manifest = target = build = None
    command = None
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        index += 1
        if argument == "--":
            break
        if argument.startswith("+") and command is None and not prefix:
            prefix.append(argument)
            continue
        if argument == "-C" or argument.startswith("-C"):
            raise ValueError("cargo-fast does not accept Cargo -C; invoke the selected source tree's wrapper")
        key, separator, value = argument.partition("=")
        if key in {"--config", "--manifest-path", "--target-dir", "--build-dir", "-Z", "--color"}:
            if not separator:
                if index == len(arguments):
                    raise ValueError(f"missing argument for {key}")
                value = arguments[index]
                index += 1
            if not value:
                raise ValueError(f"empty argument for {key}")
            if key in {"--config", "-Z"}:
                prefix.extend((key, value))
            elif key == "--manifest-path":
                manifest = Path(value)
            elif key == "--target-dir":
                target = Path(value)
            elif key == "--build-dir":
                build = Path(value)
            continue
        if argument.startswith("-Z"):
            prefix.append(argument)
            continue
        if command is None:
            if argument in {"--help", "-h", "--version", "-V", "--list"}:
                return None
            if argument in {"--locked", "--offline", "--frozen", "--quiet", "-q", "--verbose", "-v", "-vv"}:
                continue
            if argument not in BUILD_COMMANDS | NO_BUILD_COMMANDS:
                raise ValueError("cargo-fast cannot resolve target selectors hidden in a Cargo alias or external command; use an explicit Cargo build/check/test command")
            command = argument
    if command is None:
        raise ValueError("missing Cargo command")
    return None if command in NO_BUILD_COMMANDS else (prefix, manifest, target, build)


def check_cargo_command(working_directory: Path, arguments: list[str]) -> None:
    selection = cargo_selection(arguments)
    if selection is None:
        return
    prefix, manifest, target, build = selection
    working_directory = working_directory.resolve()
    environment = os.environ.copy()
    # Metadata has no --target-dir option. Reproduce this highest-priority CLI
    # selector at both Cargo's legacy environment and final config boundaries.
    # Cargo still resolves every other inherited/file/config input itself.
    for key, variable, selected in (("target-dir", "CARGO_TARGET_DIR", target),
                                    ("build-dir", "CARGO_BUILD_BUILD_DIR", build)):
        if selected is not None:
            value = str((working_directory / selected).resolve())
            environment[variable] = value
            prefix.extend(("--config", f"build.{key}={json.dumps(value, ensure_ascii=False)}"))
    result = subprocess.run(
        ["cargo", *prefix, "metadata", "--locked", "--offline", "--no-deps", "--format-version=1",
         "--manifest-path", str(working_directory / (manifest or Path("Cargo.toml")))],
        cwd=working_directory, env=environment, stdin=subprocess.DEVNULL,
        capture_output=True, text=True, check=True, timeout=30,
    )
    metadata = json.loads(result.stdout)
    if not isinstance(metadata, dict) or metadata.get("version") != 1:
        raise ValueError("Cargo metadata did not return the supported workspace schema")
    source_root = Path(metadata["workspace_root"])
    if not source_root.is_absolute() or not (source_root / "Cargo.toml").is_file():
        raise ValueError("Cargo metadata did not return an existing absolute workspace")
    # Pinned Cargo exposes both final artifacts and intermediate build output.
    # Do not guess when a different Cargo cannot report that physical boundary.
    checked: set[Path] = set()
    for field in ("target_directory", "build_directory"):
        selected = metadata.get(field)
        if not isinstance(selected, str) or not Path(selected).is_absolute():
            raise ValueError(f"Cargo metadata cannot resolve {field}; use the repository's pinned Cargo")
        output = Path(selected).resolve()
        if output not in checked:
            check_resolved_target_owner(working_directory, output, source_root.resolve(), prefix)
            checked.add(output)


def workspace_root(manifest: Path, working_directory: Path, cargo_prefix: Sequence[str] = ()) -> Path:
    # Let Cargo resolve membership, including explicit workspace pointers and
    # nested independent workspaces. This command does not build or resolve deps.
    result = subprocess.run(
        ["cargo", *cargo_prefix, "locate-project", "--workspace", "--message-format", "plain",
         "--manifest-path", str(manifest)],
        cwd=working_directory, capture_output=True, text=True, check=True, timeout=30,
    )
    located = Path(result.stdout.strip())
    if not located.is_absolute() or not located.is_file():
        raise ValueError("cargo locate-project did not return an existing absolute manifest")
    # Cargo resolves source paths from the manifest's directory even when the
    # manifest file is a symlink. Resolving the file would alias distinct inputs.
    return located.parent.resolve()


def check_target_owner(working_directory: Path, target: Path, manifest: Path) -> None:
    working_directory = working_directory.resolve()
    manifest = working_directory / manifest
    source_root = workspace_root(manifest.parent.resolve() / manifest.name, working_directory)
    target = (working_directory / target).resolve()
    check_resolved_target_owner(working_directory, target, source_root)


def check_resolved_target_owner(working_directory: Path, target: Path, source_root: Path, cargo_prefix: Sequence[str] = ()) -> None:
    check_lane_role(target, source_root)
    for candidate in (target, *target.parents):
        candidate_manifest = candidate / "Cargo.toml"
        if not candidate_manifest.is_file():
            continue
        owner = workspace_root(candidate_manifest, working_directory, cargo_prefix)
        if owner != source_root:
            raise ValueError(
                f"Cargo target {target} belongs to another source tree ({owner}); "
                f"use the warm target for {source_root} or a dedicated external build lane"
            )
        return


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--manifest-path", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        if args.cargo_args:
            if args.target_dir is not None:
                raise ValueError("direct target checks cannot also select a Cargo command")
            arguments = args.cargo_args[1:] if args.cargo_args[0] == "--" else args.cargo_args
            check_cargo_command(args.source_root, arguments)
        elif args.target_dir is not None:
            check_target_owner(args.source_root, args.target_dir, args.manifest_path)
        else:
            raise ValueError("supply --target-dir or a Cargo command after --")
    except subprocess.CalledProcessError:
        parser.exit(1, "error: Cargo could not resolve the workspace and target; check the manifest/config with the pinned Cargo\n")
    except subprocess.TimeoutExpired:
        parser.exit(1, "error: timed out determining Cargo workspace\n")
    except (OSError, RuntimeError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"error: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
