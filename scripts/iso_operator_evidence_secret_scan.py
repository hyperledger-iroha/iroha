"""Secret-scanning helpers for ISO operator evidence validation."""

import re
import unicodedata
import urllib.parse


SECRET_VALUE_PATTERNS = [
    re.compile(r"\bauthorization\s*:", re.IGNORECASE),
    re.compile(r"\bbearer\s+[A-Za-z0-9._~+/=-]+", re.IGNORECASE),
    re.compile(
        r"\b(?:token|secret|private[\s_./\\-]*key|password|passphrase|api[\s_./\\-]*key|access[\s_./\\-]*key|session[\s_./\\-]*key|client[\s_./\\-]*secret|cookie|set[\s_./\\-]*cookie)\s*[:=]\s*\S+",
        re.IGNORECASE,
    ),
    re.compile(r"\bx[\s_./\\-]*iroha[\s_./\\-]*signature\s*:", re.IGNORECASE),
]


def _secret_scan_values(raw: str) -> tuple[str, ...]:
    values = [raw]
    decoded = raw
    for _ in range(4):
        if "%" not in decoded:
            break
        next_decoded = urllib.parse.unquote(decoded)
        if next_decoded == decoded:
            break
        values.append(next_decoded)
        decoded = next_decoded
    return tuple(values)


def _contains_secret_material(value: str) -> bool:
    return any(
        pattern.search(candidate)
        for raw_candidate in _secret_scan_values(value)
        for candidate in _secret_value_forms(raw_candidate)
        for pattern in SECRET_VALUE_PATTERNS
    )


def _secret_value_forms(value: str) -> tuple[str, ...]:
    return _secret_base_forms(value)


def _secret_base_forms(value: str) -> tuple[str, ...]:
    folded = value.casefold()
    forms: list[str] = []
    for candidate in (
        folded,
        unicodedata.normalize("NFKC", folded).casefold(),
        unicodedata.normalize("NFKD", folded).casefold(),
    ):
        without_obfuscation = "".join(
            ch for ch in candidate if not _is_secret_obfuscation_char(ch)
        )
        obfuscation_spaced = "".join(
            " " if _is_secret_obfuscation_char(ch) else ch for ch in candidate
        )
        forms.extend((candidate, without_obfuscation, obfuscation_spaced))
    return tuple(dict.fromkeys(forms))


def _is_secret_obfuscation_char(ch: str) -> bool:
    category = unicodedata.category(ch)
    return category == "Cf" or category.startswith("M")


def _secret_identifier_forms(value: str) -> tuple[str, ...]:
    forms: list[str] = []
    for candidate in _secret_base_forms(value):
        forms.extend(
            (
                candidate,
                re.sub(r"[\s_./\\-]+", " ", candidate).strip(),
                re.sub(r"[\s_./\\-]+", "", candidate),
            )
        )
    return tuple(dict.fromkeys(forms))


def _contains_secret_marker(value: str, markers: tuple[str, ...]) -> bool:
    candidate_forms = _secret_identifier_forms(value)
    return any(
        marker_form in candidate_form
        for marker in markers
        for marker_form in _secret_identifier_forms(marker)
        for candidate_form in candidate_forms
    )


def _contains_secret_identifier_material(value: str) -> bool:
    strong_markers = (
        "private_key",
        "private-key",
        "private key",
        "private.key",
        "privatekey",
        "password",
        "passphrase",
        "api_key",
        "api-key",
        "api key",
        "api.key",
        "apikey",
        "access_key",
        "access-key",
        "access key",
        "access.key",
        "accesskey",
        "session_key",
        "session-key",
        "session key",
        "session.key",
        "sessionkey",
        "client_secret",
        "client-secret",
        "client secret",
        "client.secret",
        "clientsecret",
        "set-cookie",
        "set cookie",
        "set.cookie",
        "setcookie",
        "x-iroha-signature",
        "x_iroha_signature",
        "x iroha signature",
        "x.iroha.signature",
        "xirohasignature",
    )
    paired_markers = ("authorization", "bearer", "token", "cookie")
    return any(
        _contains_secret_marker(candidate, strong_markers)
        or (
            _contains_secret_marker(candidate, ("secret",))
            and _contains_secret_marker(candidate, paired_markers)
        )
        for candidate in _secret_scan_values(value)
    )
