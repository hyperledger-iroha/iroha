"""Portable archive path-component helpers for SoraFS release tooling."""

from __future__ import annotations

import re
from html import unescape
from urllib.parse import unquote


def decoded_text_variants(value: str) -> tuple[str, ...]:
    """Return raw plus repeatedly percent/HTML-decoded text variants."""

    variants = [value]
    seen = {value}
    current = value
    for _ in range(4):
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        variants.append(decoded)
        seen.add(decoded)
        current = decoded
    return tuple(variants)


def path_component_has_windows_drive_prefix(component: str) -> bool:
    """Return whether a path component starts with a Windows drive prefix."""

    return len(component) >= 2 and component[1] == ":" and component[0].isalpha()


def path_component_has_uri_scheme_prefix(component: str) -> bool:
    """Return whether a path component starts with a URI-like scheme."""

    head, separator, _tail = component.partition(":")
    if not separator:
        return False
    return re.fullmatch(r"[A-Za-z][A-Za-z0-9+.-]*", head) is not None
