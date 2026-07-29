"""Shared fail-closed preflight checks for SoraFS evidence runners."""

from __future__ import annotations

import argparse
import ipaddress
import json
import os
import re
import shlex
import stat
import subprocess
import sys
import unicodedata
from collections.abc import Callable, Iterable, Mapping, Sequence
from html import unescape
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urlsplit

from sorafs_evidence_sensitivity import (
    COMMON_SENSITIVE_KEYS,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES,
    normalize_sensitive_key,
)
from sorafs_path_identity import (
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_path_identity import resolve_path_identity


RUNNER_ARG_FIELD_RE = re.compile(r"[a-z][a-z0-9_]*\Z")
PLAN_RENDERED_PATH_ERROR = (
    "SoraFS runner plan-rendered paths must not contain secret-looking, "
    "control-character, parent, current, drive-prefix, or platform-specific "
    "components"
)
RUNNER_URL_ARG_ERROR = (
    "SoraFS runner URL arguments must not contain userinfo, query strings, "
    "fragments, control characters, encoded separators, drive prefixes, "
    "URI-scheme-like host/path tokens, or secret-looking host/path components"
)
RUNNER_CANONICAL_ORIGIN_ERROR = (
    "SoraFS runner service origins must be exact canonical bare HTTPS origins; "
    "HTTP is permitted only for localhost or literal loopback fixtures"
)
RUNNER_PASSTHROUGH_ARG_ERROR = (
    "SoraFS runner passthrough arguments must not contain secret-looking "
    "option names, values, paths, URLs, or control characters"
)
PATH_SENSITIVE_KEY_FRAGMENTS = HIGH_RISK_SENSITIVE_KEY_FRAGMENTS - frozenset(
    {"requestbody", "responsebody"}
)


def _contains_control_character(value: str) -> bool:
    """Return whether text contains ASCII or Unicode control-like characters."""

    return any(
        ord(character) < 32
        or ord(character) == 127
        or unicodedata.category(character).startswith("C")
        for character in value
    )


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("runner preflight errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("runner preflight errors must be a list of strings")
        if not diagnostic_text_is_canonical(error):
            raise ValueError(
                "runner preflight errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if not diagnostic_text_is_canonical(label):
        raise ValueError("runner preflight label must be a non-empty canonical string")
    return label


def _require_runner_arg_field(field: Any) -> str:
    if not isinstance(field, str) or RUNNER_ARG_FIELD_RE.fullmatch(field) is None:
        raise ValueError("runner argument field must be a snake_case string")
    return field


def _runner_error_messages(errors: Any) -> tuple[str, ...]:
    """Return runner error messages or reject scalar/object containers."""

    if isinstance(errors, (str, bytes, bytearray, Mapping)) or not isinstance(
        errors,
        Iterable,
    ):
        raise ValueError("runner error messages must be a sequence of strings")
    messages = tuple(errors)
    for error in messages:
        if not isinstance(error, str):
            raise ValueError("runner error messages must be a sequence of strings")
        if not diagnostic_text_is_canonical(error):
            raise ValueError(
                "runner error message must be a non-empty canonical string"
            )
    return messages


def _runner_notice_message(message: Any) -> str:
    """Return a runner notice message or reject unsafe stderr text."""

    if not diagnostic_text_is_canonical(message):
        raise ValueError("runner notice message must be a non-empty canonical string")
    return message


def runner_path_size_open_flags() -> int:
    """Return descriptor flags for no-follow runner artifact size checks."""

    flags = os.O_RDONLY
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags


def is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    """Return whether a sensitive-looking path token is an allowed digest label."""

    return any(
        normalized_key.endswith(suffix)
        for suffix in PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES
    )


def _decoded_text_variants(value: str) -> tuple[str, ...]:
    """Return raw plus decoded and Unicode-normalized text variants."""

    variants: list[str] = []
    seen: set[str] = set()
    current = value
    for _ in range(5):
        for candidate in (current, unicodedata.normalize("NFKC", current)):
            if candidate not in seen:
                variants.append(candidate)
                seen.add(candidate)
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        current = decoded
    return tuple(variants)


def _path_component_has_windows_drive_prefix(component: str) -> bool:
    """Return whether a path component starts with a Windows drive prefix."""

    return (
        len(component) >= 2
        and component[1] == ":"
        and component[0].isascii()
        and component[0].isalpha()
    )


def _path_component_has_uri_scheme_prefix(component: str) -> bool:
    """Return whether a path component looks like a URI scheme prefix."""

    scheme, separator, _rest = component.partition(":")
    return bool(
        separator
        and scheme
        and scheme[0].isalpha()
        and all(character.isalnum() or character in "+-." for character in scheme)
    )


def is_sensitive_path_component(component: str) -> bool:
    """Return whether a path component looks like runtime secret material."""

    exact_keys = frozenset(key.lower() for key in COMMON_SENSITIVE_KEYS)
    normalized_keys = frozenset(normalize_sensitive_key(key) for key in exact_keys)
    for variant in _decoded_text_variants(component):
        component_lower = variant.lower()
        normalized_component = normalize_sensitive_key(variant)
        if (
            component_lower in exact_keys
            or normalized_component in normalized_keys
            or any(
                fragment in normalized_component
                and not is_payload_free_sensitive_reference(normalized_component)
                for fragment in PATH_SENSITIVE_KEY_FRAGMENTS
            )
        ):
            return True
    return False


def _path_like_value_has_sensitive_component(value: Any) -> bool:
    """Return whether a Path or path-like string contains secret-looking text."""

    if isinstance(value, Path):
        path = value
    elif isinstance(value, str) and value:
        path = Path(value)
    else:
        return False
    for component in path.parts:
        if component in {path.anchor, "/", ""}:
            continue
        if is_sensitive_path_component(component):
            return True
    return False


def _path_component_is_plan_safe(component: str) -> bool:
    """Return whether a path component is safe in raw or decoded form."""

    for variant in _decoded_text_variants(component):
        if (
            variant in {".", "..", ""}
            or "/" in variant
            or "\\" in variant
            or _path_component_has_windows_drive_prefix(variant)
            or _path_component_has_uri_scheme_prefix(variant)
            or _contains_control_character(variant)
            or is_sensitive_path_component(variant)
        ):
            return False
    return True


def _url_host_component_is_plan_safe(component: str) -> bool:
    """Return whether a URL host label is safe in raw or decoded form."""

    for variant in _decoded_text_variants(component):
        if (
            variant in {".", "..", ""}
            or "/" in variant
            or "\\" in variant
            or _path_component_has_windows_drive_prefix(variant)
            or _path_component_has_uri_scheme_prefix(variant)
            or _contains_control_character(variant)
            or is_sensitive_path_component(variant)
        ):
            return False
    return True


def _value_variants_are_passthrough_safe(value: str) -> bool:
    """Return whether raw or percent/HTML-decoded passthrough values are safe."""

    for variant in _decoded_text_variants(value):
        if _contains_control_character(variant):
            return False
        if variant.startswith(("http://", "https://")):
            if not runner_url_arg_is_plan_safe(variant):
                return False
            continue
        if (
            any(separator in variant for separator in ("/", "\\"))
            or _path_component_has_uri_scheme_prefix(variant)
        ):
            if not plan_rendered_path_is_safe(Path(variant)):
                return False
            continue
        if is_sensitive_path_component(variant):
            return False
    return True


def _key_variants_are_passthrough_safe(value: str) -> bool:
    """Return whether raw or percent/HTML-decoded passthrough key names are safe."""

    return all(
        not _contains_control_character(variant)
        and not is_sensitive_path_component(variant)
        for variant in _decoded_text_variants(value)
    )


def _option_variants_are_passthrough_safe(value: str) -> bool:
    """Return whether raw or percent/HTML-decoded option names are safe."""

    return all(
        not _contains_control_character(variant)
        and not is_sensitive_path_component(variant.lstrip("-").replace("-", "_"))
        for variant in _decoded_text_variants(value)
    )


def plan_rendered_path_is_safe(path: Path) -> bool:
    """Return whether a path can be rendered in runner plans."""

    if not isinstance(path, Path) or not path.name:
        return False
    for component in path.parts:
        if component in {path.anchor, "/", ""}:
            continue
        if not _path_component_is_plan_safe(component):
            return False
    return True


def validate_plan_rendered_paths(paths: Iterable[Any], errors: list[str]) -> None:
    """Reject unsafe path components before runner paths enter command plans."""

    error_list = _require_error_list(errors)
    if any(
        isinstance(path, Path) and not plan_rendered_path_is_safe(path)
        for path in paths
    ):
        error_list.append(PLAN_RENDERED_PATH_ERROR)


def runner_url_arg_is_plan_safe(value: str) -> bool:
    """Return whether a URL can be rendered in runner plans."""

    if not diagnostic_text_is_canonical(value) or "\\" in value:
        return False
    try:
        parsed = urlsplit(value)
    except ValueError:
        return False
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        return False
    if parsed.username is not None or parsed.password is not None or "@" in parsed.netloc:
        return False
    if parsed.query or parsed.fragment:
        return False
    host = parsed.hostname or ""
    for component in host.split("."):
        if component and not _url_host_component_is_plan_safe(component):
            return False
    for component in parsed.path.split("/"):
        if component and not _path_component_is_plan_safe(component):
            return False
    return True


def require_runner_url_args(
    args: argparse.Namespace,
    fields: Sequence[str],
    errors: list[str],
) -> None:
    """Reject unsafe URL arguments before they enter command plans."""

    error_list = _require_error_list(errors)
    for field in fields:
        field_name = _require_runner_arg_field(field)
        value = getattr(args, field_name, None)
        if value is None:
            continue
        if not runner_url_arg_is_plan_safe(value):
            if RUNNER_URL_ARG_ERROR not in error_list:
                error_list.append(RUNNER_URL_ARG_ERROR)


def runner_url_arg_is_canonical_service_origin(
    value: str,
    *,
    allow_loopback_http: bool,
) -> bool:
    """Return whether a URL is an exact HTTPS origin or allowed loopback HTTP."""

    if not isinstance(allow_loopback_http, bool):
        return False
    if not runner_url_arg_is_plan_safe(value):
        return False
    try:
        parsed = urlsplit(value)
        port = parsed.port
    except ValueError:
        return False
    if parsed.scheme not in {"http", "https"}:
        return False
    if not value.startswith(f"{parsed.scheme}://"):
        return False
    if parsed.path not in {"", "/"} or parsed.query or parsed.fragment:
        return False
    if parsed.username is not None or parsed.password is not None or "@" in parsed.netloc:
        return False
    if port == 0:
        return False
    host = parsed.hostname
    if not host or not host.isascii() or "%" in host:
        return False

    address: ipaddress.IPv4Address | ipaddress.IPv6Address | None
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        address = None
    if address is None:
        final_host_label = host.rsplit(".", 1)[-1]
        if (
            host.replace(".", "").isdigit()
            or final_host_label.isdigit()
            or (
                final_host_label.lower().startswith("0x")
                and len(final_host_label) > 2
                and all(
                    character in "0123456789abcdef"
                    for character in final_host_label[2:].lower()
                )
            )
        ):
            return False
        if (
            host != host.lower()
            or host.startswith(".")
            or host.endswith(".")
            or any(
                not component
                or not _url_host_component_is_plan_safe(component)
                for component in host.split(".")
            )
        ):
            return False
        canonical_host = host
        loopback = host == "localhost"
    else:
        canonical_address = address.compressed
        canonical_host = (
            f"[{canonical_address}]"
            if isinstance(address, ipaddress.IPv6Address)
            else canonical_address
        )
        loopback = address.is_loopback

    if parsed.scheme == "http" and not (allow_loopback_http and loopback):
        return False
    default_port = 443 if parsed.scheme == "https" else 80
    port_suffix = "" if port is None or port == default_port else f":{port}"
    canonical_origin = f"{parsed.scheme}://{canonical_host}{port_suffix}"
    return value in {canonical_origin, f"{canonical_origin}/"}


def require_runner_canonical_service_origin_args(
    args: argparse.Namespace,
    fields: Sequence[str],
    errors: list[str],
    *,
    allow_loopback_http: bool = False,
) -> None:
    """Require exact secure bare origins before a runner mutates output state."""

    error_list = _require_error_list(errors)
    for field in fields:
        field_name = _require_runner_arg_field(field)
        value = getattr(args, field_name, None)
        if value is None:
            continue
        if not runner_url_arg_is_canonical_service_origin(
            value,
            allow_loopback_http=allow_loopback_http,
        ):
            if RUNNER_CANONICAL_ORIGIN_ERROR not in error_list:
                error_list.append(RUNNER_CANONICAL_ORIGIN_ERROR)


def runner_passthrough_arg_is_plan_safe(value: str) -> bool:
    """Return whether a passthrough CLI argument can be rendered in plans."""

    if not diagnostic_text_is_canonical(value):
        return False
    if value.startswith(("http://", "https://")):
        return runner_url_arg_is_plan_safe(value)

    key, separator, raw_value = value.partition("=")
    if value.startswith("-"):
        if not _option_variants_are_passthrough_safe(key):
            return False
    elif separator and not _key_variants_are_passthrough_safe(key):
        return False

    checked_values = [raw_value] if separator else [value]
    for checked_value in checked_values:
        if not checked_value:
            continue
        if not _value_variants_are_passthrough_safe(checked_value):
            return False
    return True


def require_runner_passthrough_args(
    args: argparse.Namespace,
    fields: Sequence[str],
    sequence_fields: Sequence[str],
    errors: list[str],
) -> None:
    """Reject unsafe passthrough command arguments before dry-run rendering."""

    error_list = _require_error_list(errors)
    values: list[Any] = []
    for field in fields:
        field_name = _require_runner_arg_field(field)
        value = getattr(args, field_name, None)
        if value is not None:
            values.append(value)
    for field in sequence_fields:
        field_name = _require_runner_arg_field(field)
        sequence = getattr(args, field_name, ())
        if isinstance(sequence, (str, bytes, bytearray, Mapping)) or not isinstance(
            sequence,
            Sequence,
        ):
            if RUNNER_PASSTHROUGH_ARG_ERROR not in error_list:
                error_list.append(RUNNER_PASSTHROUGH_ARG_ERROR)
            continue
        values.extend(sequence)
    if any(
        not isinstance(value, str) or not runner_passthrough_arg_is_plan_safe(value)
        for value in values
    ):
        if RUNNER_PASSTHROUGH_ARG_ERROR not in error_list:
            error_list.append(RUNNER_PASSTHROUGH_ARG_ERROR)


def _runner_path_sequence(paths: Any, errors: list[str], *, label: str) -> Sequence[Any] | None:
    if isinstance(paths, (str, bytes, bytearray, Mapping)) or not isinstance(
        paths, Sequence
    ):
        errors.append(f"{label} paths must be a sequence")
        return None
    return paths


def _runner_input_identity_map(
    seen: Any,
    errors: list[str],
    *,
    label: str,
) -> dict[Path, tuple[str, Path]] | None:
    if seen is None:
        return {}
    if not isinstance(seen, dict):
        errors.append(f"{label} identity map must be a dictionary")
        return None
    for identity, previous in seen.items():
        if (
            not isinstance(identity, Path)
            or not isinstance(previous, tuple)
            or len(previous) != 2
        ):
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
        previous_label, previous_path = previous
        try:
            _require_label(previous_label)
        except ValueError:
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
        if not isinstance(previous_path, Path):
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
    return seen


def _reserved_output_path_sequence(
    paths: Any,
    errors: list[str],
) -> Sequence[Any] | None:
    return _runner_path_sequence(paths, errors, label="reserved output")


def _record_path_inspection_failure(
    errors: list[str],
    *,
    label: str,
    path: Any,
    error: BaseException,
) -> None:
    path_display = path_diagnostic_label(path)
    errors.append(
        f"{label} `{path_display}` cannot be inspected: "
        f"{error_diagnostic_label(error, path_label=path_display)}"
    )


def inspect_runner_path_exists(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path exists, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.exists()
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_is_symlink(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a symlink, recording failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return stat.S_ISLNK(os.lstat(path).st_mode)
    except (FileNotFoundError, NotADirectoryError):
        return False
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_is_file(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a file, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.is_file()
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_size(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> int | None:
    """Return a path size in bytes, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    path_is_symlink = inspect_runner_path_is_symlink(path, error_list, label=path_label)
    if path_is_symlink is None:
        return None
    if path_is_symlink:
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must not be a symlink"
        )
        return None
    if not validate_runner_input_parent_chain(path, error_list, label=path_label):
        return None
    fd = -1
    try:
        fd = os.open(path, runner_path_size_open_flags())
        return os.fstat(fd).st_size
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None
    finally:
        if fd >= 0:
            os.close(fd)


def inspect_runner_path_is_dir(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a directory, recording failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def validate_runner_output_parent(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool:
    """Validate an output path's parent chain before creating files."""

    error_list = _require_error_list(errors)
    output_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{output_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return False
    for parent in (path.parent, *path.parent.parents):
        parent_label = f"{output_label} parent"
        parent_is_symlink = inspect_runner_path_is_symlink(
            parent,
            error_list,
            label=parent_label,
        )
        if parent_is_symlink is None:
            return False
        if parent_is_symlink:
            error_list.append(
                f"{parent_label} `{path_diagnostic_label(parent)}` "
                "must not be a symlink"
            )
            return False
        parent_exists = inspect_runner_path_exists(
            parent,
            error_list,
            label=parent_label,
        )
        if parent_exists is None:
            return False
        if parent_exists:
            parent_is_dir = inspect_runner_path_is_dir(
                parent,
                error_list,
                label=parent_label,
            )
            if parent_is_dir is None:
                return False
            if parent_is_dir is False:
                error_list.append(
                    f"{parent_label} `{path_diagnostic_label(parent)}` "
                    "must be a directory when it exists"
                )
                return False
    return True


def validate_runner_input_parent_chain(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool:
    """Validate an input path's parent chain before trusting its identity."""

    return validate_runner_output_parent(path, errors, label=label)


def validate_runner_output_dir(
    out_dir: Path,
    errors: list[str],
    *,
    label: str = "--out-dir",
    require_exists: bool = False,
) -> bool:
    """Validate a runner output directory target before or after command execution."""

    error_list = _require_error_list(errors)
    output_label = _require_label(label)
    if not isinstance(out_dir, Path):
        error_list.append(
            f"{output_label} `{path_diagnostic_label(out_dir)}` must be a path"
        )
        return False
    out_dir_is_symlink = inspect_runner_path_is_symlink(
        out_dir,
        error_list,
        label=output_label,
    )
    if out_dir_is_symlink is None:
        return False
    if out_dir_is_symlink:
        error_list.append(
            f"{output_label} `{path_diagnostic_label(out_dir)}` "
            "must not be a symlink"
        )
        return False
    if not validate_runner_output_parent(out_dir, error_list, label=output_label):
        return False
    out_dir_exists = inspect_runner_path_exists(out_dir, error_list, label=output_label)
    if out_dir_exists is None:
        return False
    if out_dir_exists:
        out_dir_is_dir = inspect_runner_path_is_dir(
            out_dir,
            error_list,
            label=output_label,
        )
        if out_dir_is_dir is None:
            return False
        if out_dir_is_dir is False:
            error_list.append(
                f"{output_label} `{path_diagnostic_label(out_dir)}` "
                "must be a directory when it exists"
            )
            return False
        return True
    if require_exists:
        error_list.append(
            f"{output_label} `{path_diagnostic_label(out_dir)}` "
            "must exist and be a directory"
        )
        return False
    return validate_runner_output_parent(out_dir, error_list, label=output_label)


def validate_runner_preflight(
    args: argparse.Namespace,
    *,
    summary_filename: str,
) -> list[str]:
    """Validate verifier and output targets before building a command plan."""

    errors: list[str] = []
    verifier = getattr(args, "verifier", None)
    out_dir = getattr(args, "out_dir", None)
    configured_summary_out = getattr(args, "summary_out", None)
    has_topology_qualification = hasattr(args, "topology_qualification_summary")
    topology_qualification_summary = getattr(
        args,
        "topology_qualification_summary",
        None,
    )
    summary_out = (
        configured_summary_out
        if configured_summary_out is not None
        else out_dir / summary_filename
        if isinstance(out_dir, Path)
        else configured_summary_out
    )
    rendered_paths = [verifier, out_dir, summary_out]
    if has_topology_qualification:
        rendered_paths.append(topology_qualification_summary)
    validate_plan_rendered_paths(rendered_paths, errors)
    if errors:
        return errors

    if has_topology_qualification:
        topology_file_errors = require_existing_files(
            [topology_qualification_summary],
            "--topology-qualification-summary",
        )
        errors.extend(topology_file_errors)
        if not topology_file_errors and isinstance(
            topology_qualification_summary,
            Path,
        ):
            # Imported lazily because evidence validation also imports this
            # preflight module for its URL guard.
            from sorafs_topology_qualification import (
                load_topology_qualification_binding,
            )

            _binding, topology_errors = load_topology_qualification_binding(
                topology_qualification_summary
            )
            errors.extend(topology_errors)

    if hasattr(args, "now_unix") and getattr(args, "now_unix") is None:
        errors.append("--now-unix is required")

    if not isinstance(verifier, Path):
        errors.append(
            f"--verifier `{path_diagnostic_label(verifier)}` "
            "must exist and be a file"
        )
    else:
        verifier_is_symlink = inspect_runner_path_is_symlink(
            verifier,
            errors,
            label="--verifier",
        )
        if verifier_is_symlink is not None:
            if verifier_is_symlink:
                errors.append(
                    f"--verifier `{path_diagnostic_label(verifier)}` "
                    "must not be a symlink"
                )
            elif validate_runner_input_parent_chain(
                verifier,
                errors,
                label="--verifier",
            ):
                verifier_is_file = inspect_runner_path_is_file(
                    verifier,
                    errors,
                    label="--verifier",
                )
                if verifier_is_file is False:
                    errors.append(
                        f"--verifier `{path_diagnostic_label(verifier)}` "
                        "must exist and be a file"
                    )

    if not isinstance(out_dir, Path):
        errors.append(f"--out-dir `{path_diagnostic_label(out_dir)}` must be a path")
        return errors
    out_dir_identity = resolve_runner_output_path(out_dir, errors)
    out_dir_valid = validate_runner_output_dir(out_dir, errors)

    if configured_summary_out is None and not out_dir_valid:
        return errors
    if not isinstance(summary_out, Path):
        errors.append(
            f"--summary-out `{path_diagnostic_label(summary_out)}` must be a path"
        )
    else:
        summary_out_is_symlink = inspect_runner_path_is_symlink(
            summary_out,
            errors,
            label="--summary-out",
        )
        if summary_out_is_symlink is None:
            pass
        elif summary_out_is_symlink:
            errors.append(
                f"--summary-out `{path_diagnostic_label(summary_out)}` "
                "must not be a symlink"
            )
        elif validate_runner_output_parent(summary_out, errors, label="--summary-out"):
            summary_out_exists = inspect_runner_path_exists(
                summary_out,
                errors,
                label="--summary-out",
            )
            if summary_out_exists:
                summary_out_is_dir = inspect_runner_path_is_dir(
                    summary_out,
                    errors,
                    label="--summary-out",
                )
                if summary_out_is_dir:
                    errors.append(
                        f"--summary-out `{path_diagnostic_label(summary_out)}` "
                        "must not be a directory"
                    )
                elif summary_out_is_dir is False:
                    summary_out_identity = resolve_runner_output_path(summary_out, errors)
                    if (
                        out_dir_identity is not None
                        and summary_out_identity is not None
                        and summary_out_identity == out_dir_identity
                    ):
                        errors.append(
                            "--summary-out `{}` must not be the same path as --out-dir `{}`".format(
                                path_diagnostic_label(summary_out),
                                path_diagnostic_label(out_dir),
                            )
                        )
            elif summary_out_exists is False:
                summary_out_identity = resolve_runner_output_path(summary_out, errors)
                if (
                    out_dir_identity is not None
                    and summary_out_identity is not None
                    and summary_out_identity == out_dir_identity
                ):
                    errors.append(
                        "--summary-out `{}` must not be the same path as --out-dir `{}`".format(
                            path_diagnostic_label(summary_out),
                            path_diagnostic_label(out_dir),
                        )
                    )
    return errors


def require_no_unrequired_evidence(
    paths_by_kind: Mapping[str, Sequence[Path]],
    required_kinds: Iterable[str],
    errors: list[str],
    *,
    diagnostic: str,
) -> None:
    """Reject evidence supplied for kinds excluded by --require-kind."""

    error_list = _require_error_list(errors)
    message = _runner_notice_message(diagnostic)
    if not isinstance(paths_by_kind, Mapping):
        raise ValueError("runner evidence paths must be keyed by evidence kind")
    if isinstance(required_kinds, (str, bytes, bytearray, Mapping)) or not isinstance(
        required_kinds,
        Iterable,
    ):
        raise ValueError("required evidence kinds must be an iterable of strings")
    required = frozenset(required_kinds)
    if not all(isinstance(kind, str) and kind for kind in required):
        raise ValueError("required evidence kinds must be non-empty strings")
    for kind, paths in paths_by_kind.items():
        if not isinstance(kind, str) or not kind:
            raise ValueError("runner evidence kind keys must be non-empty strings")
        if kind in required:
            continue
        if isinstance(paths, (str, bytes, bytearray, Mapping)) or not isinstance(
            paths,
            Sequence,
        ):
            if message not in error_list:
                error_list.append(message)
            continue
        if paths and message not in error_list:
            error_list.append(message)


def resolve_runner_input_file(path: Path, errors: list[str]) -> Path | None:
    """Return a canonical runner input path identity, recording resolver failures."""

    return resolve_path_identity(path, errors, label="input file")


InputFileIdentities = dict[Path, tuple[str, Path]]
InputDirIdentities = dict[Path, tuple[str, Path]]
RESERVED_OUTPUT_ARTIFACT_DIAGNOSTIC = "must not be the same path as reserved output"
COMMAND_PLAN_SHAPE_DIAGNOSTIC = "command plan must be a sequence of steps"
INPUT_FILE_PATH_DIAGNOSTIC = "input evidence file must be a path"
INPUT_FILE_INSPECTION_DIAGNOSTIC = "input evidence file cannot be inspected"
INPUT_FILE_SYMLINK_DIAGNOSTIC = "input evidence file must not be a symlink"
INPUT_FILE_PARENT_SYMLINK_DIAGNOSTIC = (
    "input evidence file parent must not be a symlink"
)
INPUT_FILE_PARENT_DIRECTORY_DIAGNOSTIC = (
    "input evidence file parent must be a directory when it exists"
)
INPUT_FILE_MISSING_DIAGNOSTIC = "input evidence file must exist and be a file"
INPUT_FILE_RESOLUTION_DIAGNOSTIC = "input evidence file cannot be resolved"
INPUT_FILE_DUPLICATE_DIAGNOSTIC = "duplicate input evidence file"


def resolve_runner_evidence_input_file(path: Path, errors: list[str]) -> Path | None:
    """Return an evidence input identity without echoing the operator path."""

    error_list = _require_error_list(errors)
    if not isinstance(path, Path):
        error_list.append(INPUT_FILE_PATH_DIAGNOSTIC)
        return None
    resolution_errors: list[str] = []
    resolved = resolve_path_identity(path, resolution_errors, label="input file")
    if resolved is None:
        error_list.append(INPUT_FILE_RESOLUTION_DIAGNOSTIC)
        return None
    return resolved


def validate_runner_evidence_input_parent_chain(
    path: Path,
    errors: list[str],
) -> bool:
    """Validate evidence input parents without echoing filesystem labels."""

    error_list = _require_error_list(errors)
    if not isinstance(path, Path):
        error_list.append(INPUT_FILE_PATH_DIAGNOSTIC)
        return False
    for parent in (path.parent, *path.parent.parents):
        try:
            parent_is_symlink = parent.is_symlink()
        except (OSError, RuntimeError):
            error_list.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
            return False
        if parent_is_symlink:
            error_list.append(INPUT_FILE_PARENT_SYMLINK_DIAGNOSTIC)
            return False
        try:
            parent_exists = parent.exists()
        except (OSError, RuntimeError):
            error_list.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
            return False
        if parent_exists:
            try:
                parent_is_dir = parent.is_dir()
            except (OSError, RuntimeError):
                error_list.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
                return False
            if not parent_is_dir:
                error_list.append(INPUT_FILE_PARENT_DIRECTORY_DIAGNOSTIC)
                return False
    return True


def command_plan_steps(plan: Any) -> Sequence[Any] | None:
    """Return command-plan steps or reject scalar/object containers."""

    if isinstance(plan, (str, bytes, bytearray, Mapping)) or not isinstance(
        plan, Sequence
    ):
        return None
    return plan


def _command_vector_errors(step_label: str, command: Any) -> list[str]:
    if not isinstance(command, list) or not command or not all(
        isinstance(part, str) for part in command
    ):
        return [f"{step_label} command must be a non-empty list of strings"]
    errors: list[str] = []
    executable_is_canonical = diagnostic_text_is_canonical(command[0])
    if not executable_is_canonical:
        errors.append(
            f"{step_label} command executable must be a non-empty canonical string"
        )
    for index, part in enumerate(command):
        if index == 0 and not executable_is_canonical:
            continue
        if "\0" in part:
            errors.append(
                f"{step_label} command argument {index} must not contain NUL bytes"
            )
        elif _contains_control_character(part):
            errors.append(
                f"{step_label} command argument {index} "
                "must not contain control characters"
            )
    return errors


def validate_command_plan_step_shapes(plan: Any) -> list[str]:
    """Reject malformed command-plan step fields before filesystem mutation."""

    errors: list[str] = []
    steps = command_plan_steps(plan)
    if steps is None:
        return [COMMAND_PLAN_SHAPE_DIAGNOSTIC]
    for index, step in enumerate(steps):
        label = getattr(step, "label", None)
        try:
            step_label = _require_label(label)
        except ValueError:
            errors.append(
                f"command-plan step {index} label must be a non-empty canonical string"
            )
            step_label = f"command-plan step {index}"
        artifact = getattr(step, "artifact", None)
        if artifact is not None and not isinstance(artifact, Path):
            if _path_like_value_has_sensitive_component(artifact):
                if PLAN_RENDERED_PATH_ERROR not in errors:
                    errors.append(PLAN_RENDERED_PATH_ERROR)
            else:
                errors.append(
                    f"{step_label} artifact `{path_diagnostic_label(artifact)}` "
                    "must be a path"
                )
        command = getattr(step, "command", None)
        errors.extend(_command_vector_errors(step_label, command))
    return errors


def validate_runner_plan_steps(rendered_plan: Any, command_plan: Any) -> list[str]:
    """Validate rendered collection-plan steps against the command plan."""

    if not isinstance(rendered_plan, Mapping):
        return ["runner plan must be an object"]
    command_steps = command_plan_steps(command_plan)
    if command_steps is None or validate_command_plan_step_shapes(command_plan):
        return [COMMAND_PLAN_SHAPE_DIAGNOSTIC]
    expected_steps: list[dict[str, object]] = []
    for step in command_steps:
        artifact = getattr(step, "artifact", None)
        expected_steps.append(
            {
                "label": getattr(step, "label"),
                "artifact": None if artifact is None else str(artifact),
                "command": getattr(step, "command"),
            }
        )
    if rendered_plan.get("steps") != expected_steps:
        return ["runner plan steps must match command plan"]
    try:
        render_runner_plan(rendered_plan)
    except (TypeError, ValueError) as error:
        return [
            f"failed to render runner plan JSON: {error_diagnostic_label(error)}"
        ]
    return []


def canonical_runner_plan_string(value: Any) -> str | None:
    """Return a non-empty runner-plan string without control characters."""

    if not diagnostic_text_is_canonical(value):
        return None
    return value


def _append_once(errors: list[str], emitted: set[str], diagnostic: str) -> None:
    if diagnostic not in emitted:
        errors.append(diagnostic)
        emitted.add(diagnostic)


def _kind_schema(kind: Any) -> str | None:
    schema = getattr(kind, "schema", None)
    return schema if isinstance(schema, str) else None


def _validate_runner_evidence_contract(
    rendered_contract: Any,
    *,
    prefix: str,
    known_kinds: Mapping[str, Any],
    allowed_kinds: Iterable[str],
    allowed_kinds_label: str,
    evidence_contract: Mapping[str, Mapping[str, Any]],
    evidence_required_fields: Mapping[str, Sequence[str]],
) -> list[str]:
    """Validate a checker-backed evidence contract object."""

    errors: list[str] = []
    allowed_kind_set = set(allowed_kinds)
    if not isinstance(rendered_contract, Mapping):
        errors.append(f"{prefix} evidence_contract must be an object")
    else:
        emitted: set[str] = set()
        for kind_name, contract in rendered_contract.items():
            kind_label = canonical_runner_plan_string(kind_name)
            kind = known_kinds.get(kind_label) if kind_label else None
            if kind_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract keys must be canonical kind names",
                )
            elif kind is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract keys must use known kind names",
                )
            elif kind_label not in allowed_kind_set:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract must contain only {allowed_kinds_label}",
                )
            if not isinstance(contract, Mapping):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract must map each kind to a contract object",
                )
                continue
            if any(
                canonical_runner_plan_string(field_name) is None
                for field_name in contract
            ):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract fields must be canonical strings",
                )
            if set(contract) != {"schema", "required_payload_fields"}:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract fields must be schema and required_payload_fields",
                )
            contract_schema = contract.get("schema")
            if canonical_runner_plan_string(contract_schema) is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract schemas must be canonical strings",
                )
            elif kind is not None and contract_schema != _kind_schema(kind):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract schemas must match evidence kind",
                )
            required_payload_fields = contract.get("required_payload_fields")
            if (
                not isinstance(required_payload_fields, list)
                or not required_payload_fields
            ):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract required_payload_fields must be non-empty lists",
                )
                continue
            seen_fields: set[str] = set()
            for field_name in required_payload_fields:
                field_label = canonical_runner_plan_string(field_name)
                if field_label is None:
                    _append_once(
                        errors,
                        emitted,
                        f"{prefix} evidence_contract required_payload_fields must contain canonical strings",
                    )
                    continue
                if field_label in seen_fields:
                    _append_once(
                        errors,
                        emitted,
                        f"{prefix} evidence_contract required_payload_fields must not contain duplicate fields",
                    )
                else:
                    seen_fields.add(field_label)
            if (
                kind_label is not None
                and kind_label in evidence_required_fields
                and list(required_payload_fields)
                != list(evidence_required_fields[kind_label])
            ):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} evidence_contract required_payload_fields must match checker fields",
                )
    if rendered_contract != dict(evidence_contract):
        errors.append(f"{prefix} evidence_contract must match checker fields")
    return errors


def validate_runner_evidence_plan(
    rendered: Any,
    command_plan: Any,
    *,
    diagnostic_prefix: str,
    plan_schema: str,
    plan_fields: frozenset[str],
    summary_schema: str,
    required_kinds: Sequence[str],
    known_kinds: Mapping[str, Any],
    thresholds: Mapping[str, int],
    required_threshold_fields: frozenset[str],
    positive_threshold_fields: frozenset[str],
    non_negative_threshold_fields: frozenset[str],
    external_evidence: Mapping[str, Sequence[str]],
    evidence_contract: Mapping[str, Mapping[str, Any]],
    evidence_required_fields: Mapping[str, Sequence[str]],
) -> list[str]:
    """Validate a schema-closed rollout evidence collection plan."""

    prefix = _runner_notice_message(diagnostic_prefix)
    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return [f"{prefix} must be an object"]
    if any(canonical_runner_plan_string(field_name) is None for field_name in rendered):
        errors.append(f"{prefix} fields must be canonical strings")
    if set(rendered) != plan_fields:
        errors.append(f"{prefix} fields must match the schema-closed contract")

    rendered_schema = rendered.get("schema")
    if canonical_runner_plan_string(rendered_schema) is None:
        errors.append(f"{prefix} schema must be canonical")
    if rendered_schema != plan_schema:
        errors.append(f"{prefix} schema must match the contract")

    rendered_summary_schema = rendered.get("verifier_summary_schema")
    if canonical_runner_plan_string(rendered_summary_schema) is None:
        errors.append(f"{prefix} verifier schema must be canonical")
    if rendered_summary_schema != summary_schema:
        errors.append(f"{prefix} verifier schema must match checker summary")

    rendered_required_kinds = rendered.get("required_kinds")
    if not isinstance(rendered_required_kinds, list):
        errors.append(f"{prefix} required_kinds must be a list")
    else:
        emitted: set[str] = set()
        seen: set[str] = set()
        for kind_name in rendered_required_kinds:
            kind_label = canonical_runner_plan_string(kind_name)
            if kind_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_kinds must contain canonical strings",
                )
                continue
            if kind_label in seen:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_kinds must not contain duplicate kinds",
                )
            else:
                seen.add(kind_label)
            if kind_label not in known_kinds:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_kinds must use known kind names",
                )
    if rendered_required_kinds != list(required_kinds):
        errors.append(f"{prefix} required_kinds must match args")

    rendered_thresholds = rendered.get("thresholds")
    allowed_threshold_fields = (
        required_threshold_fields
        | positive_threshold_fields
        | non_negative_threshold_fields
    )
    if not isinstance(rendered_thresholds, Mapping):
        errors.append(f"{prefix} thresholds must be an object")
    else:
        emitted: set[str] = set()
        for field_name in rendered_thresholds:
            field_label = canonical_runner_plan_string(field_name)
            if field_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} thresholds keys must be canonical strings",
                )
                continue
            if field_label not in allowed_threshold_fields:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} thresholds must contain only configured threshold fields",
                )
        if not required_threshold_fields <= set(rendered_thresholds):
            errors.append(f"{prefix} thresholds must include all required threshold fields")
        for field_name in sorted(non_negative_threshold_fields):
            value = rendered_thresholds.get(field_name)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                errors.append(
                    f"{prefix} thresholds.{field_name} must be a non-negative integer"
                )
        for field_name in sorted(positive_threshold_fields):
            if field_name not in rendered_thresholds:
                continue
            value = rendered_thresholds.get(field_name)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                errors.append(
                    f"{prefix} thresholds.{field_name} must be a positive integer"
                )
    if rendered_thresholds != dict(thresholds):
        errors.append(f"{prefix} thresholds must match args")

    rendered_external_evidence = rendered.get("external_evidence")
    if not isinstance(rendered_external_evidence, Mapping):
        errors.append(f"{prefix} external_evidence must be an object")
    else:
        emitted: set[str] = set()
        for kind_name, paths in rendered_external_evidence.items():
            kind_label = canonical_runner_plan_string(kind_name)
            if kind_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence keys must be canonical kind names",
                )
                continue
            if kind_label not in known_kinds:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence keys must use known kind names",
                )
            elif kind_label not in required_kinds:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence must contain only required kinds",
                )
            if not isinstance(paths, list) or not paths:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence must map each kind to non-empty path lists",
                )
                continue
            if any(canonical_runner_plan_string(path) is None for path in paths):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence paths must be canonical strings",
                )
    if rendered_external_evidence != dict(external_evidence):
        errors.append(f"{prefix} external_evidence must match args")

    errors.extend(
        _validate_runner_evidence_contract(
            rendered.get("evidence_contract"),
            prefix=prefix,
            known_kinds=known_kinds,
            allowed_kinds=required_kinds,
            allowed_kinds_label="required kinds",
            evidence_contract=evidence_contract,
            evidence_required_fields=evidence_required_fields,
        )
    )

    errors.extend(validate_runner_plan_steps(rendered, command_plan))
    return errors


def validate_runner_fixed_evidence_plan(
    rendered: Any,
    command_plan: Any,
    *,
    diagnostic_prefix: str,
    plan_schema: str,
    plan_fields: frozenset[str],
    summary_schema: str,
    external_evidence: Mapping[str, str],
    external_evidence_fields: frozenset[str],
    known_kinds: Mapping[str, Any],
    evidence_contract: Mapping[str, Mapping[str, Any]],
    evidence_required_fields: Mapping[str, Sequence[str]],
) -> list[str]:
    """Validate a schema-closed plan with fixed scalar external evidence."""

    prefix = _runner_notice_message(diagnostic_prefix)
    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return [f"{prefix} must be an object"]
    if any(canonical_runner_plan_string(field_name) is None for field_name in rendered):
        errors.append(f"{prefix} fields must be canonical strings")
    if set(rendered) != plan_fields:
        errors.append(f"{prefix} fields must match the schema-closed contract")

    rendered_schema = rendered.get("schema")
    if canonical_runner_plan_string(rendered_schema) is None:
        errors.append(f"{prefix} schema must be canonical")
    if rendered_schema != plan_schema:
        errors.append(f"{prefix} schema must match the contract")

    rendered_summary_schema = rendered.get("verifier_summary_schema")
    if canonical_runner_plan_string(rendered_summary_schema) is None:
        errors.append(f"{prefix} verifier schema must be canonical")
    if rendered_summary_schema != summary_schema:
        errors.append(f"{prefix} verifier schema must match checker summary")

    rendered_external_evidence = rendered.get("external_evidence")
    if not isinstance(rendered_external_evidence, Mapping):
        errors.append(f"{prefix} external_evidence must be an object")
    else:
        emitted: set[str] = set()
        for kind_name, path in rendered_external_evidence.items():
            kind_label = canonical_runner_plan_string(kind_name)
            if kind_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence keys must be canonical kind names",
                )
                continue
            if kind_label not in known_kinds:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence keys must use known kind names",
                )
            elif kind_label not in external_evidence_fields:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence must contain only configured evidence fields",
                )
            if canonical_runner_plan_string(path) is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_evidence values must be canonical strings",
                )
        if set(rendered_external_evidence) != external_evidence_fields:
            errors.append(f"{prefix} external_evidence must match configured fields")
    if rendered_external_evidence != dict(external_evidence):
        errors.append(f"{prefix} external_evidence must match args")

    errors.extend(
        _validate_runner_evidence_contract(
            rendered.get("evidence_contract"),
            prefix=prefix,
            known_kinds=known_kinds,
            allowed_kinds=evidence_contract,
            allowed_kinds_label="configured evidence kinds",
            evidence_contract=evidence_contract,
            evidence_required_fields=evidence_required_fields,
        )
    )

    errors.extend(validate_runner_plan_steps(rendered, command_plan))
    return errors


def validate_runner_context_evidence_plan(
    rendered: Any,
    command_plan: Any,
    *,
    diagnostic_prefix: str,
    plan_schema: str,
    plan_fields: frozenset[str],
    summary_schema: str,
    deployment_context: Mapping[str, Any],
    deployment_context_fields: frozenset[str],
    deployment_context_errors: Sequence[str],
    known_kinds: Mapping[str, Any],
    evidence_contract: Mapping[str, Mapping[str, Any]],
    evidence_required_fields: Mapping[str, Sequence[str]],
) -> list[str]:
    """Validate a schema-closed plan with reviewed deployment context."""

    prefix = _runner_notice_message(diagnostic_prefix)
    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return [f"{prefix} must be an object"]
    if any(canonical_runner_plan_string(field_name) is None for field_name in rendered):
        errors.append(f"{prefix} fields must be canonical strings")
    if set(rendered) != plan_fields:
        errors.append(f"{prefix} fields must match the schema-closed contract")

    rendered_schema = rendered.get("schema")
    if canonical_runner_plan_string(rendered_schema) is None:
        errors.append(f"{prefix} schema must be canonical")
    if rendered_schema != plan_schema:
        errors.append(f"{prefix} schema must match the contract")

    rendered_summary_schema = rendered.get("verifier_summary_schema")
    if canonical_runner_plan_string(rendered_summary_schema) is None:
        errors.append(f"{prefix} verifier schema must be canonical")
    if rendered_summary_schema != summary_schema:
        errors.append(f"{prefix} verifier schema must match checker summary")

    rendered_context = rendered.get("deployment_context")
    if not isinstance(rendered_context, Mapping):
        errors.append(f"{prefix} deployment_context must be an object")
    else:
        if any(
            canonical_runner_plan_string(field_name) is None
            for field_name in rendered_context
        ):
            errors.append(f"{prefix} deployment_context keys must be canonical strings")
        if set(rendered_context) != deployment_context_fields:
            errors.append(f"{prefix} deployment_context fields must match configured fields")
        value_error_emitted = False
        for field_name, value in rendered_context.items():
            if field_name == "deployment_context_reviewed":
                if value is not True:
                    errors.append(f"{prefix} deployment_context must be reviewed")
            elif (
                not value_error_emitted
                and canonical_runner_plan_string(value) is None
            ):
                errors.append(
                    f"{prefix} deployment_context values must be canonical strings"
                )
                value_error_emitted = True
        if rendered_context == dict(deployment_context):
            errors.extend(deployment_context_errors)
    if rendered_context != dict(deployment_context):
        errors.append(f"{prefix} deployment_context must match args")

    errors.extend(
        _validate_runner_evidence_contract(
            rendered.get("evidence_contract"),
            prefix=prefix,
            known_kinds=known_kinds,
            allowed_kinds=evidence_contract,
            allowed_kinds_label="configured evidence kinds",
            evidence_contract=evidence_contract,
            evidence_required_fields=evidence_required_fields,
        )
    )

    errors.extend(validate_runner_plan_steps(rendered, command_plan))
    return errors


def _aggregate_summary_contract_required_kinds(gate: Any) -> Sequence[str]:
    required_kinds = getattr(gate, "required_kinds", ())
    return required_kinds if isinstance(required_kinds, Sequence) else ()


def _validate_runner_aggregate_steps(
    rendered_steps: Any,
    command_plan: Any,
    *,
    prefix: str,
) -> list[str]:
    """Validate aggregate collection-plan steps with aggregate diagnostics."""

    errors: list[str] = []
    command_steps = command_plan_steps(command_plan)
    if command_steps is None or validate_command_plan_step_shapes(command_plan):
        expected_steps: list[dict[str, object]] = []
        errors.append(COMMAND_PLAN_SHAPE_DIAGNOSTIC)
    else:
        expected_steps = []
        for step in command_steps:
            artifact = getattr(step, "artifact", None)
            expected_steps.append(
                {
                    "label": getattr(step, "label"),
                    "artifact": None if artifact is None else str(artifact),
                    "command": getattr(step, "command"),
                }
            )

    if not isinstance(rendered_steps, list) or not rendered_steps:
        errors.append(f"{prefix} steps must be a non-empty list")
    else:
        emitted: set[str] = set()
        for step in rendered_steps:
            if not isinstance(step, Mapping):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} steps must contain objects",
                )
                continue
            if set(step) != {"label", "artifact", "command"}:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step fields must be label, artifact, and command",
                )
            if any(canonical_runner_plan_string(field_name) is None for field_name in step):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step fields must be canonical strings",
                )
            if canonical_runner_plan_string(step.get("label")) is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step labels must be canonical strings",
                )
            artifact = step.get("artifact")
            if artifact is not None and canonical_runner_plan_string(artifact) is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step artifacts must be null or canonical strings",
                )
            command = step.get("command")
            if not isinstance(command, list) or not command:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step commands must be non-empty lists",
                )
                continue
            if any(canonical_runner_plan_string(argument) is None for argument in command):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} step commands must contain canonical strings",
                )
    if rendered_steps != expected_steps:
        errors.append(f"{prefix} steps must match command plan")
    return errors


def validate_runner_aggregate_readiness_plan(
    rendered: Any,
    command_plan: Any,
    *,
    diagnostic_prefix: str,
    plan_schema: str,
    plan_fields: frozenset[str],
    summary_schema: str,
    required_gates: Sequence[str],
    known_gates: Mapping[str, Any],
    thresholds: Mapping[str, int],
    required_threshold_fields: frozenset[str],
    positive_threshold_fields: frozenset[str],
    non_negative_threshold_fields: frozenset[str],
    threshold_fields_label: str,
    deployment_context: Mapping[str, Any],
    deployment_context_fields: frozenset[str],
    deployment_context_value_errors: Callable[[Mapping[str, Any]], Sequence[str]],
    external_summaries: Mapping[str, Sequence[str]],
    summary_contract: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate the aggregate SoraFS production-readiness collection plan."""

    prefix = _runner_notice_message(diagnostic_prefix)
    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return [f"{prefix} must be an object"]
    if any(canonical_runner_plan_string(field_name) is None for field_name in rendered):
        errors.append(f"{prefix} fields must be canonical strings")
    if set(rendered) != plan_fields:
        errors.append(f"{prefix} fields must match the schema-closed contract")

    rendered_schema = rendered.get("schema")
    if canonical_runner_plan_string(rendered_schema) is None:
        errors.append(f"{prefix} schema must be canonical")
    if rendered_schema != plan_schema:
        errors.append(f"{prefix} schema must match the contract")

    rendered_summary_schema = rendered.get("verifier_summary_schema")
    if canonical_runner_plan_string(rendered_summary_schema) is None:
        errors.append(f"{prefix} verifier schema must be canonical")
    if rendered_summary_schema != summary_schema:
        errors.append(f"{prefix} verifier schema must match aggregate schema")

    rendered_required_gates = rendered.get("required_gates")
    if not isinstance(rendered_required_gates, list):
        errors.append(f"{prefix} required_gates must be a list")
    else:
        emitted: set[str] = set()
        seen: set[str] = set()
        for gate_name in rendered_required_gates:
            gate_label = canonical_runner_plan_string(gate_name)
            if gate_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_gates must contain canonical strings",
                )
                continue
            if gate_label in seen:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_gates must not contain duplicate gates",
                )
            else:
                seen.add(gate_label)
            if gate_label not in known_gates:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} required_gates must use known gate names",
                )
    if rendered_required_gates != list(required_gates):
        errors.append(f"{prefix} required_gates must match args")

    rendered_thresholds = rendered.get("thresholds")
    allowed_threshold_fields = (
        required_threshold_fields
        | positive_threshold_fields
        | non_negative_threshold_fields
    )
    if not isinstance(rendered_thresholds, Mapping):
        errors.append(f"{prefix} thresholds must be an object")
    else:
        emitted: set[str] = set()
        for field_name in rendered_thresholds:
            field_label = canonical_runner_plan_string(field_name)
            if field_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} thresholds keys must be canonical strings",
                )
                continue
            if field_label not in allowed_threshold_fields:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} thresholds must contain only {threshold_fields_label}",
                )
        for field_name in sorted(required_threshold_fields):
            if field_name not in rendered_thresholds:
                errors.append(f"{prefix} thresholds.{field_name} must be present")
        for field_name in sorted(non_negative_threshold_fields):
            value = rendered_thresholds.get(field_name)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                errors.append(
                    f"{prefix} thresholds.{field_name} must be a non-negative integer"
                )
        for field_name in sorted(positive_threshold_fields):
            if field_name not in rendered_thresholds:
                continue
            value = rendered_thresholds.get(field_name)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                errors.append(
                    f"{prefix} thresholds.{field_name} must be a positive integer"
                )
    if rendered_thresholds != dict(thresholds):
        errors.append(f"{prefix} thresholds must match args")

    rendered_context = rendered.get("deployment_context")
    if not isinstance(rendered_context, Mapping):
        errors.append(f"{prefix} deployment_context must be an object")
    else:
        if set(rendered_context) != deployment_context_fields:
            errors.append(
                f"{prefix} deployment_context fields must be deployment_id and environment"
            )
        if any(
            canonical_runner_plan_string(field_name) is None
            for field_name in rendered_context
        ):
            errors.append(f"{prefix} deployment_context keys must be canonical strings")
        values_are_canonical = all(
            canonical_runner_plan_string(rendered_context.get(field_name)) is not None
            for field_name in deployment_context_fields
        )
        if not values_are_canonical:
            errors.append(f"{prefix} deployment_context must be canonical")
        else:
            errors.extend(deployment_context_value_errors(rendered_context))
    if rendered_context != dict(deployment_context):
        errors.append(f"{prefix} deployment_context must match args")

    rendered_external_summaries = rendered.get("external_summaries")
    if not isinstance(rendered_external_summaries, Mapping):
        errors.append(f"{prefix} external_summaries must be an object")
    else:
        emitted: set[str] = set()
        for gate_name, paths in rendered_external_summaries.items():
            gate_label = canonical_runner_plan_string(gate_name)
            if gate_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries keys must be canonical gate names",
                )
                continue
            if gate_label not in known_gates:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries keys must use known gate names",
                )
            elif gate_label not in required_gates:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries must contain only required gates",
                )
            if not isinstance(paths, list):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries must map each gate to a summary path list",
                )
                continue
            if len(paths) != 1:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries must contain exactly one summary path per gate",
                )
            if any(canonical_runner_plan_string(path) is None for path in paths):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} external_summaries paths must be canonical strings",
                )
    if rendered_external_summaries != dict(external_summaries):
        errors.append(
            f"{prefix} external_summaries must contain exactly one summary per required gate"
        )

    rendered_summary_contract = rendered.get("summary_contract")
    if not isinstance(rendered_summary_contract, Mapping):
        errors.append(f"{prefix} summary_contract must be an object")
    else:
        emitted: set[str] = set()
        for gate_name, contract in rendered_summary_contract.items():
            gate_label = canonical_runner_plan_string(gate_name)
            gate = known_gates.get(gate_label) if gate_label else None
            if gate_label is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract keys must be canonical gate names",
                )
            elif gate is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract keys must use known gate names",
                )
            elif gate_label not in required_gates:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract must contain only required gates",
                )
            if not isinstance(contract, Mapping):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract must map each gate to a contract object",
                )
                continue
            if any(
                canonical_runner_plan_string(field_name) is None
                for field_name in contract
            ):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract gate fields must be canonical strings",
                )
            if set(contract) != {"schema", "required_kinds"}:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract gate fields must be schema and required_kinds",
                )
            contract_schema = contract.get("schema")
            if canonical_runner_plan_string(contract_schema) is None:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract schemas must be canonical strings",
                )
            elif gate is not None and contract_schema != getattr(gate, "schema", None):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract schemas must match gate schema",
                )
            required_kinds = contract.get("required_kinds")
            if not isinstance(required_kinds, list) or not required_kinds:
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract required_kinds must be non-empty lists",
                )
                continue
            seen_kinds: set[str] = set()
            for kind_name in required_kinds:
                kind_label = canonical_runner_plan_string(kind_name)
                if kind_label is None:
                    _append_once(
                        errors,
                        emitted,
                        f"{prefix} summary_contract required_kinds must contain canonical strings",
                    )
                    continue
                if kind_label in seen_kinds:
                    _append_once(
                        errors,
                        emitted,
                        f"{prefix} summary_contract required_kinds must not contain duplicate kinds",
                    )
                else:
                    seen_kinds.add(kind_label)
            if gate is not None and list(required_kinds) != list(
                _aggregate_summary_contract_required_kinds(gate)
            ):
                _append_once(
                    errors,
                    emitted,
                    f"{prefix} summary_contract required_kinds must match gate contract",
                )
    if rendered_summary_contract != dict(summary_contract):
        errors.append(f"{prefix} summary_contract must match required gates")

    errors.extend(
        _validate_runner_aggregate_steps(
            rendered.get("steps"),
            command_plan,
            prefix=prefix,
        )
    )
    try:
        render_runner_plan(rendered)
    except (TypeError, ValueError):
        errors.append(f"{prefix} must be strict JSON renderable")
    return errors


def runner_arg_label(field: str) -> str:
    """Return the CLI option label for an argparse namespace field."""

    field_name = _require_runner_arg_field(field)
    return f"--{field_name.replace('_', '-')}"


def require_runner_positive_int(
    args: argparse.Namespace,
    field: str,
    errors: list[str],
    *,
    allow_none: bool = False,
) -> bool:
    """Require a direct runner namespace value to be a positive integer."""

    error_list = _require_error_list(errors)
    field_name = _require_runner_arg_field(field)
    value = getattr(args, field_name, None)
    if value is None and allow_none:
        return True
    valid = isinstance(value, int) and not isinstance(value, bool) and value > 0
    if not valid:
        suffix = " when supplied" if allow_none else ""
        error_list.append(f"{runner_arg_label(field_name)} must be positive{suffix}")
    return valid


def require_runner_non_negative_int(
    args: argparse.Namespace,
    field: str,
    errors: list[str],
) -> bool:
    """Require a direct runner namespace value to be a non-negative integer."""

    error_list = _require_error_list(errors)
    field_name = _require_runner_arg_field(field)
    value = getattr(args, field_name, None)
    valid = isinstance(value, int) and not isinstance(value, bool) and value >= 0
    if not valid:
        error_list.append(f"{runner_arg_label(field_name)} must be non-negative")
    return valid


def require_existing_files(
    paths: Sequence[Path],
    label: str,
    *,
    seen: InputFileIdentities | None = None,
) -> list[str]:
    """Validate existing runner input files and reject repeated path identities."""

    errors: list[str] = []
    path_label = _require_label(label)
    path_items = _runner_path_sequence(paths, errors, label=path_label)
    if path_items is None:
        return errors
    seen_map = _runner_input_identity_map(seen, errors, label=path_label)
    if seen_map is None:
        return errors
    for path in path_items:
        if not isinstance(path, Path):
            errors.append(INPUT_FILE_PATH_DIAGNOSTIC)
            continue
        if not plan_rendered_path_is_safe(path):
            if PLAN_RENDERED_PATH_ERROR not in errors:
                errors.append(PLAN_RENDERED_PATH_ERROR)
            continue
        try:
            path_exists = path.exists()
        except (OSError, RuntimeError):
            errors.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
            continue
        try:
            path_is_symlink = path.is_symlink()
        except (OSError, RuntimeError):
            errors.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
            continue
        if path_is_symlink:
            errors.append(INPUT_FILE_SYMLINK_DIAGNOSTIC)
            continue
        if not validate_runner_evidence_input_parent_chain(path, errors):
            continue
        if not path_exists:
            errors.append(INPUT_FILE_MISSING_DIAGNOSTIC)
            continue
        resolved = resolve_runner_evidence_input_file(path, errors)
        if resolved is None:
            continue
        try:
            path_is_file = path.is_file()
        except (OSError, RuntimeError):
            errors.append(INPUT_FILE_INSPECTION_DIAGNOSTIC)
            continue
        if not path_is_file:
            errors.append(INPUT_FILE_MISSING_DIAGNOSTIC)
            continue
        previous = seen_map.get(resolved)
        if previous is not None:
            errors.append(INPUT_FILE_DUPLICATE_DIAGNOSTIC)
            continue
        seen_map[resolved] = (path_label, path)
    return errors


def require_existing_dirs(
    paths: Sequence[Path],
    label: str,
    *,
    seen: InputDirIdentities | None = None,
) -> list[str]:
    """Validate existing runner input directories and reject repeated identities."""

    errors: list[str] = []
    path_label = _require_label(label)
    path_items = _runner_path_sequence(paths, errors, label=path_label)
    if path_items is None:
        return errors
    seen_map = _runner_input_identity_map(seen, errors, label=path_label)
    if seen_map is None:
        return errors
    for path in path_items:
        if isinstance(path, Path) and not plan_rendered_path_is_safe(path):
            if PLAN_RENDERED_PATH_ERROR not in errors:
                errors.append(PLAN_RENDERED_PATH_ERROR)
            continue
        path_exists = inspect_runner_path_exists(path, errors, label=path_label)
        if path_exists is None:
            continue
        path_is_symlink = inspect_runner_path_is_symlink(
            path,
            errors,
            label=path_label,
        )
        if path_is_symlink is None:
            continue
        if path_is_symlink:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must not be a symlink"
            )
            continue
        if not validate_runner_input_parent_chain(path, errors, label=path_label):
            continue
        if not path_exists:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a directory"
            )
            continue
        resolved = resolve_runner_input_file(path, errors)
        if resolved is None:
            continue
        path_is_dir = inspect_runner_path_is_dir(path, errors, label=path_label)
        if path_is_dir is None:
            continue
        if not path_is_dir:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a directory"
            )
            continue
        previous = seen_map.get(resolved)
        if previous is not None:
            previous_label, previous_path = previous
            errors.append(
                f"duplicate {path_label} directory `{path_diagnostic_label(path)}` "
                f"matches {previous_label} `{path_diagnostic_label(previous_path)}`"
            )
            continue
        seen_map[resolved] = (path_label, path)
    return errors


def resolve_runner_output_path(path: Path, errors: list[str]) -> Path | None:
    """Return a canonical runner output path identity, recording resolver failures."""

    return resolve_path_identity(path, errors, label="output path")


def validate_command_plan_artifacts(
    plan: Any,
    *,
    reserved_output_paths: Sequence[Path] = (),
) -> list[str]:
    """Reject ambiguous planned artifact outputs before executing commands."""

    errors: list[str] = []
    shape_errors = validate_command_plan_step_shapes(plan)
    if shape_errors:
        return shape_errors
    steps = command_plan_steps(plan)
    assert steps is not None
    seen: dict[Path, tuple[str, Path]] = {}
    reserved: dict[Path, Path] = {}
    reserved_output_items = _reserved_output_path_sequence(
        reserved_output_paths,
        errors,
    )
    if reserved_output_items is None:
        return errors
    if any(_path_like_value_has_sensitive_component(path) for path in reserved_output_items):
        errors.append(PLAN_RENDERED_PATH_ERROR)
        return errors
    validate_plan_rendered_paths(
        (
            *(path for path in reserved_output_items if isinstance(path, Path)),
            *(
                getattr(step, "artifact", None)
                for step in steps
                if isinstance(getattr(step, "artifact", None), Path)
            ),
        ),
        errors,
    )
    if errors:
        return errors
    for path in reserved_output_items:
        if not isinstance(path, Path):
            errors.append(
                f"reserved output path `{path_diagnostic_label(path)}` "
                "must be a path"
            )
            continue
        reserved_is_symlink = inspect_runner_path_is_symlink(
            path,
            errors,
            label="reserved output path",
        )
        if reserved_is_symlink is None:
            continue
        if reserved_is_symlink:
            errors.append(
                f"reserved output path `{path_diagnostic_label(path)}` "
                "must not be a symlink"
            )
            continue
        if not validate_runner_output_parent(
            path,
            errors,
            label="reserved output path",
        ):
            continue
        resolved = resolve_runner_output_path(path, errors)
        if resolved is not None:
            previous = reserved.get(resolved)
            if previous is not None:
                errors.append(
                    "duplicate reserved output path `{}` matches `{}`".format(
                        path_diagnostic_label(path),
                        path_diagnostic_label(previous),
                    )
                )
                continue
            reserved[resolved] = path
    if errors:
        return errors

    for step in steps:
        artifact = getattr(step, "artifact", None)
        if artifact is None:
            continue
        label = getattr(step, "label")
        artifact_is_symlink = inspect_runner_path_is_symlink(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_is_symlink is None:
            continue
        if artifact_is_symlink:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                "must not be a symlink"
            )
            continue
        artifact_exists = inspect_runner_path_exists(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_exists is None:
            continue
        if artifact_exists:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                "must not already exist"
            )
            continue
        if not validate_runner_output_parent(
            artifact,
            errors,
            label=f"{label} artifact",
        ):
            continue
        resolved = resolve_runner_output_path(artifact, errors)
        if resolved is None:
            continue
        reserved_path = reserved.get(resolved)
        if reserved_path is not None:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                f"{RESERVED_OUTPUT_ARTIFACT_DIAGNOSTIC} "
                f"`{path_diagnostic_label(reserved_path)}`"
            )
            continue
        previous = seen.get(resolved)
        if previous is not None:
            previous_label, previous_artifact = previous
            errors.append(
                f"duplicate planned artifact `{path_diagnostic_label(artifact)}` "
                f"for {label} matches {previous_label} "
                f"`{path_diagnostic_label(previous_artifact)}`"
            )
            continue
        seen[resolved] = (label, artifact)
    return errors


def render_runner_plan(plan: Mapping[str, Any]) -> str:
    """Render a SoraFS runner command plan with the shared dry-run JSON shape."""

    if not isinstance(plan, Mapping):
        raise ValueError("runner plan must be an object")
    return json.dumps(plan, indent=2, sort_keys=True, allow_nan=False) + "\n"


def write_runner_plan(plan: Mapping[str, Any]) -> list[str]:
    """Write a SoraFS runner command plan to stdout."""

    try:
        sys.stdout.write(render_runner_plan(plan))
    except (TypeError, ValueError) as error:
        return [
            f"failed to render runner plan JSON: {error_diagnostic_label(error)}"
        ]
    return []


def emit_runner_error_lines(errors: Iterable[str]) -> None:
    """Emit one stderr ERROR line for each runner error."""

    for error in _runner_error_messages(errors):
        print(f"ERROR: {error}", file=sys.stderr)


def emit_runner_exception(error: BaseException) -> None:
    """Emit one sanitized stderr ERROR line for a caught runner exception."""

    emit_runner_error_lines((error_diagnostic_label(error),))


def emit_runner_error_block(title: str, errors: Iterable[str]) -> None:
    """Emit a runner error heading followed by bullet diagnostics."""

    error_messages = _runner_error_messages(errors)
    print(title, file=sys.stderr)
    for error in error_messages:
        print(f"- {error}", file=sys.stderr)


def emit_runner_notice(message: str) -> None:
    """Emit a human runner notice on stderr."""

    print(_runner_notice_message(message), file=sys.stderr)


def run_command_plan(
    plan: Any,
    out_dir: Path,
    *,
    prepare_step: Callable[[Any], Sequence[str]] | None = None,
    notice_command: Callable[[Any], Sequence[str]] | None = None,
) -> int:
    """Run a SoraFS collection command plan with structured launch/output errors.

    ``prepare_step`` is an optional runner-owned callback for creating a
    validated step's narrowly scoped runtime prerequisites immediately before
    launch. It must return a sequence of canonical, payload-free diagnostics.
    ``notice_command`` may return a redacted command vector for human notices;
    the original in-memory vector is always passed to the subprocess.
    """

    errors: list[str] = []
    if prepare_step is not None and not callable(prepare_step):
        emit_runner_error_lines(
            ("command-plan step preparation callback must be callable",)
        )
        return 1
    if notice_command is not None and not callable(notice_command):
        emit_runner_error_lines(
            ("command-plan notice renderer must be callable",)
        )
        return 1
    shape_errors = validate_command_plan_step_shapes(plan)
    if shape_errors:
        emit_runner_error_lines(shape_errors)
        return 1
    steps = command_plan_steps(plan)
    assert steps is not None
    if validate_runner_output_dir(out_dir, errors):
        errors.extend(
            validate_command_plan_artifacts(
                steps,
                reserved_output_paths=(out_dir,),
            )
        )
    if errors:
        emit_runner_error_lines(errors)
        return 1

    try:
        out_dir.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        out_dir_label = path_diagnostic_label(out_dir)
        emit_runner_error_lines(
            (
                f"failed to create --out-dir `{out_dir_label}`: "
                f"{error_diagnostic_label(error, path_label=out_dir_label)}",
            )
        )
        return 1

    for step in steps:
        label = getattr(step, "label")
        artifact = getattr(step, "artifact", None)
        command = getattr(step, "command", None)
        command_errors = _command_vector_errors(label, command)
        if command_errors:
            emit_runner_error_lines(command_errors)
            return 1
        if prepare_step is not None:
            try:
                preparation_errors = _runner_error_messages(prepare_step(step))
            except (OSError, RuntimeError, TypeError, ValueError) as error:
                emit_runner_error_lines(
                    (
                        f"{label} preparation failed: "
                        f"{error_diagnostic_label(error)}",
                    )
                )
                return 1
            if preparation_errors:
                emit_runner_error_lines(preparation_errors)
                return 1
        displayed_command = command
        if notice_command is not None:
            try:
                rendered_notice_command = notice_command(step)
            except (OSError, RuntimeError, TypeError, ValueError) as error:
                emit_runner_error_lines(
                    (
                        f"{label} notice rendering failed: "
                        f"{error_diagnostic_label(error)}",
                    )
                )
                return 1
            if isinstance(
                rendered_notice_command,
                (str, bytes, bytearray, Mapping),
            ) or not isinstance(rendered_notice_command, Sequence):
                displayed_command = rendered_notice_command
            else:
                displayed_command = list(rendered_notice_command)
            display_errors = _command_vector_errors(label, displayed_command)
            if display_errors:
                emit_runner_error_lines(display_errors)
                return 1
        emit_runner_notice(f"RUN {label}: {shlex.join(displayed_command)}")
        try:
            result = subprocess.run(command, check=False)
        except OSError as error:
            emit_runner_error_lines(
                (f"{label} failed to launch: {error_diagnostic_label(error)}",)
            )
            return 1
        if result.returncode != 0:
            emit_runner_error_lines(
                (f"{label} failed with exit code {result.returncode}",)
            )
            return result.returncode
        out_dir_errors: list[str] = []
        if not validate_runner_output_dir(
            out_dir,
            out_dir_errors,
            require_exists=True,
        ):
            emit_runner_error_lines(out_dir_errors)
            return 1
        if artifact is not None:
            artifact_errors: list[str] = []
            if not validate_runner_output_parent(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            ):
                emit_runner_error_lines(artifact_errors)
                return 1
            artifact_is_symlink = inspect_runner_path_is_symlink(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if artifact_is_symlink:
                emit_runner_error_lines(
                    (
                        f"{label} expected artifact "
                        f"`{path_diagnostic_label(artifact)}` "
                        "must not be a symlink",
                    )
                )
                return 1
            artifact_is_file = inspect_runner_path_is_file(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if not artifact_is_file:
                emit_runner_error_lines(
                    (
                        f"{label} did not write expected artifact "
                        f"`{path_diagnostic_label(artifact)}`",
                    )
                )
                return 1
            artifact_size = inspect_runner_path_size(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if artifact_size == 0:
                emit_runner_error_lines(
                    (
                        f"{label} wrote empty expected artifact "
                        f"`{path_diagnostic_label(artifact)}`",
                    )
                )
                return 1
    return 0
