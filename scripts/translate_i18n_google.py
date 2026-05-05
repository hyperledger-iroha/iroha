#!/usr/bin/env python3
"""
Bulk-translate i18n files using the no-key Google endpoint.

Targets Markdown/MDX/Org docs that:
1) carry i18n metadata (`lang`, `source` / `#+SOURCE`), and
2) match the selected refresh mode (`source-identical`, `stale`, or `all`).

The script preserves metadata, masks non-prose regions (code, URLs, directives),
applies translation in chunks, performs structural QA checks, and caches
translations per (source-body-hash, locale) to avoid duplicated API work.
"""

from __future__ import annotations

import argparse
import concurrent.futures as futures
import fnmatch
import hashlib
import html
import json
import os
import re
import sys
import threading
import time
import urllib.parse
import urllib.request
from urllib.error import HTTPError
from dataclasses import dataclass
from datetime import date, datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CACHE_PATH = ROOT / "artifacts" / "translation_cache_google.json"
DEFAULT_MAX_CHARS = 1400
TRANSLATION_ENDPOINT = "https://translate.google.com/m"
SIMPLYTRANSLATE_ENDPOINT = "https://simplytranslate.org/api/translate"
SIMPLYTRANSLATE_MAX_CHARS = 3000
MYMEMORY_ENDPOINT = "https://api.mymemory.translated.net/get"
MYMEMORY_MAX_CHARS = 450
PLACEHOLDER_RE = re.compile(r"(?i)i18n\w*\s*\d{1,12}\w*")
LANG_CODE_MAP = {
    "zh-hans": "zh-CN",
    "zh-hant": "zh-TW",
}
GLOSSARY_TERMS = (
    "Hyperledger",
    "Iroha",
    "Norito",
    "SoraFS",
    "SORA",
    "Nexus",
    "Torii",
    "Sumeragi",
    "Kotodama",
    "IVM",
    "Kagami",
    "Izanami",
    "OpenAPI",
    "Sigstore",
    "OIDC",
    "Prometheus",
    "Grafana",
    "Docusaurus",
    "Docker",
    "AppImage",
)
EXCLUDED_DIR_NAMES = {
    ".git",
    "target",
    "node_modules",
    "vendor",
    "artifacts",
    "tmp",
    "clean_copy",
    "rust_out",
    "__pycache__",
}
TRANSLATE_SAVE_EVERY = 10
APPLY_PROGRESS_EVERY = 250
_GOOGLE_BLOCKED = False
_GOOGLE_BLOCKED_LOCK = threading.Lock()
_SIMPLY_BLOCK_UNTIL = 0.0
_SIMPLY_BLOCK_LOCK = threading.Lock()


@dataclass
class Candidate:
    path: Path
    lang: str
    source_lang: str
    source: Path
    source_last_modified: str
    source_body: str
    source_frontmatter: str | None
    preamble: str
    frontmatter: str
    source_hash: str
    translation_cache_hash: str
    kind: str  # "md" | "org"


@dataclass(frozen=True)
class TranslationTask:
    key: str
    kind: str
    source_body: str
    source_lang: str
    target_lang: str


def split_frontmatter(text: str) -> tuple[str, str | None, str]:
    """
    Split optional markdown frontmatter while preserving any preamble comments.

    Returns `(preamble, frontmatter_or_none, body)`.
    """
    i = 0
    n = len(text)

    while i < n:
        if text.startswith("<!--", i):
            end = text.find("-->", i + 4)
            if end == -1:
                return "", None, text
            i = end + 3
            while i < n and text[i] in "\r\n":
                i += 1
            continue
        if text.startswith("\n", i) or text.startswith("\r\n", i):
            i += 1
            continue
        break

    if not text.startswith("---\n", i):
        return "", None, text
    end = text.find("\n---\n", i + 4)
    if end == -1:
        return "", None, text
    preamble = text[:i]
    return preamble, text[i + 4 : end], text[end + 5 :]


def parse_frontmatter_map(frontmatter: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in frontmatter.splitlines():
        if ":" not in line or line.startswith((" ", "\t")):
            continue
        key, value = line.split(":", 1)
        out[key.strip()] = value.strip()
    return out


def normalize_frontmatter_value(value: str) -> str:
    trimmed = value.strip()
    if (
        len(trimmed) >= 2
        and ((trimmed[0] == '"' and trimmed[-1] == '"') or (trimmed[0] == "'" and trimmed[-1] == "'"))
    ):
        return trimmed[1:-1]
    return trimmed


def source_last_modified_iso(path: Path) -> str:
    return datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc).isoformat()


def matches_path_filters(path: Path, path_globs: list[str] | None) -> bool:
    if not path_globs:
        return True
    rel = path.relative_to(ROOT).as_posix()
    return any(fnmatch.fnmatch(rel, pattern) for pattern in path_globs)


def update_frontmatter(frontmatter: str, updates: dict[str, str]) -> str:
    lines = frontmatter.splitlines()
    index: dict[str, int] = {}
    key_re = re.compile(r"^([A-Za-z0-9_-]+):(.*)$")
    for i, line in enumerate(lines):
        m = key_re.match(line)
        if m and not line.startswith((" ", "\t")):
            index[m.group(1)] = i

    for key, value in updates.items():
        if key in index:
            lines[index[key]] = f"{key}: {value}"
        else:
            lines.append(f"{key}: {value}")

    return "\n".join(lines).rstrip("\n")


def frontmatter_lines(frontmatter: str | None) -> list[str]:
    if not frontmatter:
        return []
    return [line for line in frontmatter.splitlines() if line.strip()]


def render_frontmatter(frontmatter: str | None) -> str:
    lines = frontmatter_lines(frontmatter)
    if not lines:
        return ""
    return f"---\n{chr(10).join(lines)}\n---"


def merge_frontmatter_blocks(primary_frontmatter: str, extra_frontmatter: str | None) -> str:
    primary_lines = frontmatter_lines(primary_frontmatter)
    extra_lines = frontmatter_lines(extra_frontmatter)
    lines = primary_lines + extra_lines
    return f"---\n{chr(10).join(lines)}\n---"


def should_translate_frontmatter_key(key: str) -> bool:
    return key in {"title", "description", "sidebar_label"}


def translate_frontmatter_value(
    value: str,
    source_lang: str,
    target_lang: str,
    max_chars: int,
    cache: dict[str, str],
) -> str:
    normalized = normalize_frontmatter_value(value)
    if not normalized or source_lang == target_lang:
        return normalized
    digest = hashlib.sha256(normalized.encode("utf-8")).hexdigest()
    cache_key = f"fm::{source_lang}::{digest}::{target_lang}"
    cached = cache.get(cache_key)
    if cached is not None:
        return cached
    translated = translate_markdown(normalized, source_lang, target_lang, max_chars)
    ok, reason = validate_translation(normalized, translated)
    if not ok:
        raise ValueError(f"frontmatter validation failed: {reason}")
    cache[cache_key] = translated
    return translated


def translate_source_frontmatter(
    frontmatter: str | None,
    source_lang: str,
    target_lang: str,
    max_chars: int,
    cache: dict[str, str],
) -> str | None:
    if not frontmatter:
        return frontmatter
    out_lines: list[str] = []
    for line in frontmatter.splitlines():
        if ":" not in line or line.startswith((" ", "\t")):
            out_lines.append(line)
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        if should_translate_frontmatter_key(key):
            translated = translate_frontmatter_value(
                value.strip(), source_lang, target_lang, max_chars, cache
            )
            out_lines.append(f"{key}: {translated}")
        else:
            out_lines.append(line)
    return "\n".join(out_lines).rstrip("\n")


def is_portal_docs_sibling_translation(path: Path) -> bool:
    rel = path.relative_to(ROOT).as_posix()
    return rel.startswith("docs/portal/docs/")


def is_portal_i18n_translation(path: Path) -> bool:
    rel = path.relative_to(ROOT).as_posix()
    return rel.startswith("docs/portal/i18n/")


def mask_markdown(text: str) -> tuple[str, dict[str, str]]:
    placeholders: dict[str, str] = {}
    counter = 0

    def reserve(value: str, tag: str) -> str:
        nonlocal counter
        token = f"I18N{tag[0]}{counter:08d}X"
        counter += 1
        placeholders[token] = value
        return token

    # Keep project-specific terms untouched.
    for term in sorted(GLOSSARY_TERMS, key=len, reverse=True):
        text = re.sub(
            rf"(?<![A-Za-z0-9_]){re.escape(term)}(?![A-Za-z0-9_])",
            lambda m: reserve(m.group(0), "TERM"),
            text,
        )

    # Fenced code blocks first.
    text = re.sub(
        r"```[\s\S]*?```",
        lambda m: reserve(m.group(0), "FENCE"),
        text,
        flags=re.MULTILINE,
    )
    text = re.sub(
        r"~~~[\s\S]*?~~~",
        lambda m: reserve(m.group(0), "FENCE"),
        text,
        flags=re.MULTILINE,
    )
    # HTML comments.
    text = re.sub(
        r"<!--[\s\S]*?-->",
        lambda m: reserve(m.group(0), "HTML"),
        text,
        flags=re.MULTILINE,
    )
    # Markdown link-style definitions.
    text = re.sub(
        r"^[ \t]{0,3}\[[^\]]+\]:[^\n]*$",
        lambda m: reserve(m.group(0), "LINKDEF"),
        text,
        flags=re.MULTILINE,
    )
    # Markdown links and images.
    text = re.sub(
        r"(!?\[[^\]]*\])\(([^)\n]+)\)",
        lambda m: f"{m.group(1)}({reserve(m.group(2), 'URL')})",
        text,
    )
    # Raw URLs.
    text = re.sub(
        r"https?://[^\s<>\]`\"')]+",
        lambda m: reserve(m.group(0), "URL"),
        text,
    )
    # Inline code.
    text = re.sub(
        r"`[^`\n]+`",
        lambda m: reserve(m.group(0), "INLINE"),
        text,
        flags=re.MULTILINE,
    )
    return text, placeholders


def mask_org(text: str) -> tuple[str, dict[str, str]]:
    placeholders: dict[str, str] = {}
    counter = 0

    def reserve(value: str, tag: str) -> str:
        nonlocal counter
        token = f"I18N{tag[0]}{counter:08d}X"
        counter += 1
        placeholders[token] = value
        return token

    for term in sorted(GLOSSARY_TERMS, key=len, reverse=True):
        text = re.sub(
            rf"(?<![A-Za-z0-9_]){re.escape(term)}(?![A-Za-z0-9_])",
            lambda m: reserve(m.group(0), "TERM"),
            text,
        )

    # Source/example blocks.
    text = re.sub(
        r"(?is)^#\+BEGIN_[A-Z_]+[\s\S]*?^#\+END_[A-Z_]+\s*$",
        lambda m: reserve(m.group(0), "BLOCK"),
        text,
        flags=re.MULTILINE,
    )
    # Directive lines.
    text = re.sub(
        r"^#\+[A-Z0-9_]+:[^\n]*$",
        lambda m: reserve(m.group(0), "DIRECTIVE"),
        text,
        flags=re.MULTILINE,
    )
    # Org links.
    text = re.sub(
        r"\[\[[^\]]+\](?:\[[^\]]*\])?\]",
        lambda m: reserve(m.group(0), "LINK"),
        text,
    )
    # Inline verbatim/code.
    text = re.sub(r"~[^~\n]+~", lambda m: reserve(m.group(0), "INLINE"), text)
    text = re.sub(r"=[^=\n]+=", lambda m: reserve(m.group(0), "INLINE"), text)
    # Raw URLs.
    text = re.sub(
        r"https?://[^\s<>\]`\"')]+",
        lambda m: reserve(m.group(0), "URL"),
        text,
    )
    return text, placeholders


def unmask_text(text: str, placeholders: dict[str, str]) -> str:
    # Recover placeholder variants where the translator altered the tag letter
    # or inserted spaces around the numeric index.
    idx_map: dict[int, str] = {}
    for token, value in placeholders.items():
        m = re.match(r"^I18N[A-Z](\d{8})X$", token)
        if m:
            idx_map[int(m.group(1))] = value

    def resolve_idx_value(idx_digits: str) -> str | None:
        try:
            idx = int(idx_digits)
        except ValueError:
            return None
        value = idx_map.get(idx)
        if value is not None:
            return value
        if not idx_map:
            return None

        norm = idx_digits.lstrip("0") or "0"

        # Some providers inject extra zeros into placeholder indices.
        max_len = len(str(max(idx_map)))
        for width in range(max_len, min(max_len + 3, len(norm)) + 1):
            tail = int(norm[-width:])
            value = idx_map.get(tail)
            if value is not None:
                return value

        # Fallback: try removing a single zero from the index.
        for i, ch in enumerate(norm):
            if ch != "0":
                continue
            candidate_digits = norm[:i] + norm[i + 1 :]
            if not candidate_digits:
                continue
            candidate = int(candidate_digits)
            value = idx_map.get(candidate)
            if value is not None:
                return value

        return None

    def restore_variant(match: re.Match[str]) -> str:
        idx_digits = match.group(1)
        value = resolve_idx_value(idx_digits)
        return value if value is not None else match.group(0)

    text = re.sub(r"I18N[A-Z]\s*(\d{1,12})\s*X", restore_variant, text)
    text = re.sub(r"(?i)i18n[A-Z]?\s*(\d{1,12})\s*x", restore_variant, text)
    text = re.sub(r"(?i)i18n\s*(\d{1,12})", restore_variant, text)
    text = re.sub(r"(?i)i18n\w*\s*(\d{1,12})\w*", restore_variant, text)

    # Case-insensitive exact-token recovery for tags mutated to lowercase.
    for token, value in placeholders.items():
        text = re.sub(re.escape(token), lambda _m, v=value: v, text, flags=re.IGNORECASE)

    for token, value in placeholders.items():
        text = text.replace(token, value)
    return text


def split_chunks(text: str, max_chars: int) -> list[str]:
    if len(text) <= max_chars:
        return [text]

    parts = re.split(r"(\n{2,})", text)
    chunks: list[str] = []
    current = ""
    for part in parts:
        if len(current) + len(part) <= max_chars:
            current += part
            continue
        if current:
            chunks.append(current)
            current = ""
        if len(part) <= max_chars:
            current = part
            continue

        # Oversized part: split by line, then fallback hard split.
        line_buf = ""
        for line in part.splitlines(keepends=True):
            if len(line_buf) + len(line) <= max_chars:
                line_buf += line
            else:
                if line_buf:
                    chunks.append(line_buf)
                    line_buf = ""
                if len(line) <= max_chars:
                    line_buf = line
                else:
                    for i in range(0, len(line), max_chars):
                        chunks.append(line[i : i + max_chars])
        if line_buf:
            current = line_buf

    if current:
        chunks.append(current)
    return chunks


def is_google_blocked() -> bool:
    with _GOOGLE_BLOCKED_LOCK:
        return _GOOGLE_BLOCKED


def set_google_blocked() -> None:
    global _GOOGLE_BLOCKED
    with _GOOGLE_BLOCKED_LOCK:
        _GOOGLE_BLOCKED = True


def simply_rate_limit_wait_seconds() -> float:
    with _SIMPLY_BLOCK_LOCK:
        return max(0.0, _SIMPLY_BLOCK_UNTIL - time.time())


def extend_simply_rate_limit(seconds: float) -> None:
    global _SIMPLY_BLOCK_UNTIL
    with _SIMPLY_BLOCK_LOCK:
        _SIMPLY_BLOCK_UNTIL = max(_SIMPLY_BLOCK_UNTIL, time.time() + max(0.0, seconds))


def translate_chunk_google(chunk: str, source_lang: str, target_lang: str, retries: int = 2) -> str:
    params = {
        "sl": source_lang,
        "tl": target_lang,
        "q": chunk,
    }
    url = f"{TRANSLATION_ENDPOINT}?{urllib.parse.urlencode(params)}"
    backoff = 1.0
    last_exc: Exception | None = None

    for _ in range(retries):
        try:
            request = urllib.request.Request(
                url,
                headers={
                    "User-Agent": "Mozilla/5.0",
                    "Accept": "text/html",
                },
            )
            with urllib.request.urlopen(request, timeout=12) as response:
                payload = response.read().decode("utf-8")
                final_url = response.geturl()
            if "sorry/index" in final_url:
                set_google_blocked()
                raise RuntimeError("google blocked with /sorry")
            m = re.search(r'<div class="result-container">(.*?)</div>', payload, flags=re.DOTALL)
            if not m:
                if "unusual traffic" in payload.lower() or "detected unusual traffic" in payload.lower():
                    set_google_blocked()
                    raise RuntimeError("google blocked with traffic challenge")
                raise ValueError("missing result-container")
            translated = html.unescape(m.group(1))
            translated = re.sub(r"<[^>]+>", "", translated)
            if not translated:
                raise ValueError("empty translation")
            return translated
        except HTTPError as exc:
            last_exc = exc
            # Back off aggressively on provider throttling.
            if exc.code == 429:
                time.sleep(min(backoff * 3, 90.0))
            else:
                time.sleep(backoff)
            backoff = min(backoff * 2, 30.0)
        except Exception as exc:  # noqa: BLE001 - network retries
            last_exc = exc
            time.sleep(backoff)
            backoff = min(backoff * 2, 30.0)

    assert last_exc is not None
    raise last_exc


def translate_chunk_mymemory(chunk: str, source_lang: str, target_lang: str, retries: int = 2) -> str:
    params = {
        "q": chunk,
        "langpair": f"{source_lang}|{target_lang}",
    }
    url = f"{MYMEMORY_ENDPOINT}?{urllib.parse.urlencode(params)}"
    backoff = 1.0
    last_exc: Exception | None = None

    for _ in range(retries):
        try:
            request = urllib.request.Request(
                url,
                headers={
                    "User-Agent": "Mozilla/5.0",
                    "Accept": "application/json",
                },
            )
            with urllib.request.urlopen(request, timeout=12) as response:
                payload = response.read().decode("utf-8")
            data = json.loads(payload)
            status_raw = data.get("responseStatus")
            status = int(status_raw) if status_raw is not None else 0
            if status != 200:
                details = data.get("responseDetails") or f"status={status_raw}"
                raise RuntimeError(f"mymemory: {details}")
            translated = (data.get("responseData") or {}).get("translatedText") or ""
            translated = html.unescape(translated)
            if not translated:
                raise ValueError("mymemory empty translation")
            return translated
        except Exception as exc:  # noqa: BLE001 - network retries
            last_exc = exc
            time.sleep(backoff)
            backoff = min(backoff * 2, 30.0)

    assert last_exc is not None
    raise last_exc


def translate_chunk_simplytranslate(
    chunk: str, source_lang: str, target_lang: str, retries: int = 2
) -> str:
    body = urllib.parse.urlencode(
        {
            "from": source_lang,
            "to": target_lang,
            "text": chunk,
        }
    ).encode("utf-8")
    backoff = 1.0
    last_exc: Exception | None = None

    for _ in range(retries):
        wait = simply_rate_limit_wait_seconds()
        if wait > 0:
            time.sleep(min(wait, 30.0))
        try:
            request = urllib.request.Request(
                SIMPLYTRANSLATE_ENDPOINT,
                data=body,
                headers={
                    "User-Agent": "Mozilla/5.0",
                    "Content-Type": "application/x-www-form-urlencoded",
                    "Accept": "application/json",
                },
            )
            with urllib.request.urlopen(request, timeout=12) as response:
                payload = response.read().decode("utf-8")
            data = json.loads(payload)
            translated = data.get("translated_text") or ""
            translated = html.unescape(translated)
            if not translated:
                raise RuntimeError("simplytranslate empty translation")
            return translated
        except HTTPError as exc:
            last_exc = exc
            if exc.code == 429:
                retry_after = exc.headers.get("Retry-After") if exc.headers else None
                try:
                    delay = float(retry_after) if retry_after else 15.0
                except ValueError:
                    delay = 15.0
                delay = max(3.0, min(delay, 30.0))
                extend_simply_rate_limit(delay)
                time.sleep(min(max(delay, backoff), 30.0))
            else:
                time.sleep(backoff)
            backoff = min(backoff * 2, 30.0)
        except Exception as exc:  # noqa: BLE001 - network retries
            last_exc = exc
            time.sleep(backoff)
            backoff = min(backoff * 2, 30.0)

    assert last_exc is not None
    raise last_exc


def translate_chunk(chunk: str, source_lang: str, target_lang: str, retries: int = 2) -> str:
    if not chunk:
        return chunk

    provider_errors: list[str] = []

    if not is_google_blocked():
        try:
            return translate_chunk_google(chunk, source_lang, target_lang, retries=1)
        except Exception as exc:  # noqa: BLE001 - fallback to secondary provider
            set_google_blocked()
            provider_errors.append(f"google={exc}")

    simply_chunk = chunk
    try:
        if len(simply_chunk) > SIMPLYTRANSLATE_MAX_CHARS:
            return "".join(
                translate_chunk_simplytranslate(piece, source_lang, target_lang, retries=retries)
                for piece in split_chunks(simply_chunk, SIMPLYTRANSLATE_MAX_CHARS)
            )
        return translate_chunk_simplytranslate(
            simply_chunk, source_lang, target_lang, retries=retries
        )
    except Exception as exc:  # noqa: BLE001 - fallback to tertiary provider
        provider_errors.append(f"simplytranslate={exc}")

    mymemory_chunk = chunk
    if len(mymemory_chunk) > MYMEMORY_MAX_CHARS:
        return "".join(
            translate_chunk_mymemory(piece, source_lang, target_lang, retries=retries)
            for piece in split_chunks(mymemory_chunk, MYMEMORY_MAX_CHARS)
        )
    try:
        return translate_chunk_mymemory(
            mymemory_chunk, source_lang, target_lang, retries=retries
        )
    except Exception as exc:  # noqa: BLE001 - no more providers
        provider_errors.append(f"mymemory={exc}")
        raise RuntimeError("; ".join(provider_errors))


def translate_markdown(text: str, source_lang: str, target_lang: str, max_chars: int) -> str:
    masked, placeholders = mask_markdown(text)
    chunks = split_chunks(masked, max_chars=max_chars)
    translated = "".join(translate_chunk(chunk, source_lang, target_lang) for chunk in chunks)
    return unmask_text(translated, placeholders)


def translate_org(text: str, source_lang: str, target_lang: str, max_chars: int) -> str:
    masked, placeholders = mask_org(text)
    chunks = split_chunks(masked, max_chars=max_chars)
    translated = "".join(translate_chunk(chunk, source_lang, target_lang) for chunk in chunks)
    return unmask_text(translated, placeholders)


def extract_urls(text: str) -> list[str]:
    return re.findall(r"https?://[^\s<>\]`\"')]+", text)


def validate_translation(source: str, translated: str) -> tuple[bool, str]:
    if PLACEHOLDER_RE.search(translated):
        return False, "unresolved placeholders"
    if source.strip() == translated.strip():
        # Allow highly-structured/code-like docs that are effectively language-neutral.
        letters = sum(ch.isalpha() and ch.isascii() for ch in source)
        if letters >= 120:
            return False, "output identical to source"
    return True, ""


def translate_task(task: TranslationTask, max_chars: int) -> tuple[str, str | None, str | None]:
    """Return (cache_key, translated_text|None, error_reason|None)."""
    try:
        if not task.source_body:
            return task.key, "", None
        if task.source_lang == task.target_lang:
            return task.key, task.source_body, None
        if task.kind == "md":
            translated = translate_markdown(
                task.source_body, task.source_lang, task.target_lang, max_chars
            )
        else:
            translated = translate_org(task.source_body, task.source_lang, task.target_lang, max_chars)
        ok, reason = validate_translation(task.source_body, translated)
        if not ok:
            return task.key, None, reason
        return task.key, translated, None
    except Exception as exc:  # noqa: BLE001 - network and decode failures
        return task.key, None, f"translation error: {exc}"


def discover_candidates(
    only_changed: bool,
    refresh_mode: str,
    path_globs: list[str] | None,
    status_filter: str,
) -> list[Candidate]:
    candidates: list[Candidate] = []
    all_paths: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(ROOT, topdown=True):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIR_NAMES]
        for filename in filenames:
            if not filename.endswith((".md", ".mdx", ".org")):
                continue
            all_paths.append(Path(dirpath) / filename)
    all_paths.sort(key=lambda p: str(p))

    changed_paths: set[Path] = set()
    if only_changed:
        # Limit to currently modified files for incremental runs.
        import subprocess

        out = subprocess.check_output(
            ["git", "status", "--short"],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
        for line in out.splitlines():
            if not line:
                continue
            path = line[3:].strip()
            changed_paths.add((ROOT / path).resolve())

    for path in all_paths:
        resolved = path.resolve()
        if only_changed and resolved not in changed_paths:
            continue
        if not matches_path_filters(resolved, path_globs):
            continue

        text = path.read_text(encoding="utf-8", errors="ignore")
        lang: str | None = None
        source_lang: str = "en"
        source_rel: str | None = None
        source_body: str | None = None
        source_last_modified: str = ""
        preamble = ""
        frontmatter = ""
        kind = "md"
        source: Path

        if path.suffix in {".md", ".mdx"}:
            kind = "md"
            preamble, fm, body = split_frontmatter(text)
            if fm is None:
                continue
            frontmatter = fm
            meta = parse_frontmatter_map(fm)
            lang = meta.get("lang")
            source_rel = meta.get("source")
            status = normalize_frontmatter_value(meta.get("status", ""))
            if not lang or not source_rel or lang == "en":
                continue
            if status_filter != "any" and status != status_filter:
                continue
            source = (ROOT / source_rel).resolve()
            if not source.exists():
                continue
            source_text = source.read_text(encoding="utf-8", errors="ignore")
            _, source_fm, source_body = split_frontmatter(source_text)
            if source_fm is not None:
                source_meta = parse_frontmatter_map(source_fm)
                source_lang = source_meta.get("lang", "en")
            source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
            translation_cache_hash = source_hash
            source_last_modified = source_last_modified_iso(source)
            source_identical = body.lstrip("\n") == source_body.lstrip("\n")
            stale = (
                normalize_frontmatter_value(meta.get("source_hash", "")) != source_hash
                or normalize_frontmatter_value(meta.get("source_last_modified", ""))
                != source_last_modified
            )

            if refresh_mode == "source-identical" and not source_identical:
                continue
            if refresh_mode == "stale" and not stale:
                continue
        else:
            kind = "org"
            m_lang = re.search(r"^#\+LANGUAGE:\s*(.*)$", text, flags=re.MULTILINE)
            m_source = re.search(r"^#\+SOURCE:\s*(.*)$", text, flags=re.MULTILINE)
            m_status = re.search(r"^#\+STATUS:\s*(.*)$", text, flags=re.MULTILINE)
            if not m_lang or not m_source or not m_status:
                continue
            lang = m_lang.group(1).strip()
            source_rel = m_source.group(1).strip()
            status = normalize_frontmatter_value(m_status.group(1))
            if status_filter != "any" and status != status_filter:
                continue
            source = (ROOT / source_rel).resolve()
            if not source.exists():
                continue
            source_text = source.read_text(encoding="utf-8", errors="ignore")
            source_body = source_text
            source_hash = hashlib.sha256(source_body.encode("utf-8")).hexdigest()
            translation_cache_hash = source_hash
            source_last_modified = source_last_modified_iso(source)
            m_source_lang = re.search(r"^#\+LANGUAGE:\s*(.*)$", source_text, flags=re.MULTILINE)
            if m_source_lang:
                source_lang = m_source_lang.group(1).strip()
            source_identical = text.endswith(source_text)
            m_source_hash = re.search(r"^#\+SOURCE_HASH:\s*(.*)$", text, flags=re.MULTILINE)
            m_source_mtime = re.search(
                r"^#\+SOURCE_LAST_MODIFIED:\s*(.*)$", text, flags=re.MULTILINE
            )
            stale = (
                normalize_frontmatter_value(m_source_hash.group(1))
                if m_source_hash
                else ""
            ) != source_hash or (
                normalize_frontmatter_value(m_source_mtime.group(1))
                if m_source_mtime
                else ""
            ) != source_last_modified

            if refresh_mode == "source-identical" and not source_identical:
                continue
            if refresh_mode == "stale" and not stale:
                continue
            # frontmatter equivalent = initial metadata block up to first non #+ line.
            lines = text.splitlines()
            meta_lines: list[str] = []
            i = 0
            while i < len(lines) and (lines[i].startswith("#+") or lines[i].strip() == ""):
                meta_lines.append(lines[i])
                i += 1
            frontmatter = "\n".join(meta_lines).rstrip("\n")

        assert source_body is not None
        if not source_last_modified:
            source_last_modified = source_last_modified_iso(source)
        candidates.append(
            Candidate(
                path=path.resolve(),
                lang=lang,
                source_lang=source_lang,
                source=source,
                source_last_modified=source_last_modified,
                source_body=source_body,
                source_frontmatter=source_fm if kind == "md" else None,
                preamble=preamble,
                frontmatter=frontmatter,
                source_hash=source_hash,
                translation_cache_hash=translation_cache_hash,
                kind=kind,
            )
        )

    return candidates


def load_cache(cache_path: Path) -> dict[str, str]:
    if not cache_path.exists():
        return {}
    try:
        return json.loads(cache_path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001 - tolerate malformed cache
        return {}


def save_cache(cache_path: Path, cache: dict[str, str]) -> None:
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = cache_path.with_suffix(cache_path.suffix + ".tmp")
    tmp_path.write_text(
        json.dumps(cache, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )
    tmp_path.replace(cache_path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--cache",
        default=str(DEFAULT_CACHE_PATH),
        help="Path to JSON cache keyed by <source-hash>::<lang>.",
    )
    parser.add_argument(
        "--max-chars",
        type=int,
        default=DEFAULT_MAX_CHARS,
        help="Max characters per translation API request chunk.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=0,
        help="Process at most N files (0 = no limit).",
    )
    parser.add_argument(
        "--only-changed",
        action="store_true",
        help="Only process files currently marked modified by git status.",
    )
    parser.add_argument(
        "--from-index",
        type=int,
        default=0,
        help="Start processing at this 0-based candidate index.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Parallel translation workers for unique cache-miss tasks.",
    )
    parser.add_argument(
        "--verbose-files",
        action="store_true",
        help="Print every file path during apply phase.",
    )
    parser.add_argument(
        "--refresh-mode",
        choices=("source-identical", "stale", "all"),
        default="source-identical",
        help=(
            "Candidate selection mode: source-identical (default), "
            "stale metadata refresh, or all translations."
        ),
    )
    parser.add_argument(
        "--path-glob",
        action="append",
        default=[],
        help=(
            "Restrict candidate files by repo-relative glob (repeatable), e.g. "
            "'docs/source/data_model.*.md'."
        ),
    )
    parser.add_argument(
        "--status-filter",
        choices=("any", "needs-translation", "complete"),
        default="any",
        help="Restrict markdown/org candidates by translation status metadata.",
    )
    args = parser.parse_args()

    cache_path = Path(args.cache).resolve()
    cache = load_cache(cache_path)
    candidates = discover_candidates(
        only_changed=args.only_changed,
        refresh_mode=args.refresh_mode,
        path_globs=args.path_glob or None,
        status_filter=args.status_filter,
    )
    if args.from_index > 0:
        candidates = candidates[args.from_index :]
    if args.limit > 0:
        candidates = candidates[: args.limit]

    total = len(candidates)
    print(f"candidates={total}")
    if total == 0:
        return 0

    # Phase 1: translate unique cache-miss bodies in parallel.
    unique_tasks: dict[str, TranslationTask] = {}
    for idx, candidate in enumerate(candidates, start=1):
        source_lang = LANG_CODE_MAP.get(candidate.source_lang, candidate.source_lang)
        target_lang = LANG_CODE_MAP.get(candidate.lang, candidate.lang)
        cache_key = (
            f"{candidate.kind}::{source_lang}::{candidate.translation_cache_hash}::{target_lang}"
        )
        legacy_key = f"{candidate.kind}::{candidate.source_hash}::{target_lang}"
        if source_lang == "en" and cache_key not in cache and legacy_key in cache:
            cache[cache_key] = cache[legacy_key]
        if cache_key in cache or cache_key in unique_tasks:
            continue
        unique_tasks[cache_key] = TranslationTask(
            key=cache_key,
            kind=candidate.kind,
            source_body=candidate.source_body,
            source_lang=source_lang,
            target_lang=target_lang,
        )

    print(f"unique_misses={len(unique_tasks)} workers={args.workers}")

    failed_keys: set[str] = set()
    if unique_tasks:
        completed = 0
        translated_keys = 0

        if args.workers <= 1:
            for task in unique_tasks.values():
                key, translated_text, error = translate_task(task, args.max_chars)
                completed += 1
                if translated_text is None:
                    failed_keys.add(key)
                    print(f"warning: failed key {key}: {error}", file=sys.stderr)
                else:
                    cache[key] = translated_text
                    translated_keys += 1
                if completed % TRANSLATE_SAVE_EVERY == 0 or completed == len(unique_tasks):
                    save_cache(cache_path, cache)
                    print(
                        f"translate_progress={completed}/{len(unique_tasks)} ok={translated_keys} failed={len(failed_keys)}"
                    )
                    sys.stdout.flush()
        else:
            with futures.ThreadPoolExecutor(max_workers=args.workers) as executor:
                fut_map = {
                    executor.submit(translate_task, task, args.max_chars): task.key
                    for task in unique_tasks.values()
                }
                for fut in futures.as_completed(fut_map):
                    key, translated_text, error = fut.result()
                    completed += 1
                    if translated_text is None:
                        failed_keys.add(key)
                        print(f"warning: failed key {key}: {error}", file=sys.stderr)
                    else:
                        cache[key] = translated_text
                        translated_keys += 1
                    if completed % TRANSLATE_SAVE_EVERY == 0 or completed == len(unique_tasks):
                        save_cache(cache_path, cache)
                        print(
                            f"translate_progress={completed}/{len(unique_tasks)} ok={translated_keys} failed={len(failed_keys)}"
                        )
                        sys.stdout.flush()

    # Phase 2: apply translated/cache content to files.
    translated = 0
    failed = 0
    for idx, candidate in enumerate(candidates, start=1):
        source_lang = LANG_CODE_MAP.get(candidate.source_lang, candidate.source_lang)
        target_lang = LANG_CODE_MAP.get(candidate.lang, candidate.lang)
        rel_path = candidate.path.relative_to(ROOT)
        cache_key = (
            f"{candidate.kind}::{source_lang}::{candidate.translation_cache_hash}::{target_lang}"
        )
        legacy_key = f"{candidate.kind}::{candidate.source_hash}::{target_lang}"
        if cache_key in failed_keys:
            failed += 1
            print(f"warning: skipped {rel_path} due to failed translation key", file=sys.stderr)
            continue

        translated_body = cache.get(cache_key)
        if translated_body is None and source_lang == "en":
            translated_body = cache.get(legacy_key)
            if translated_body is not None:
                cache[cache_key] = translated_body
        if translated_body is None:
            failed += 1
            print(f"warning: missing cache for {rel_path}", file=sys.stderr)
            continue

        if candidate.kind == "md":
            translated_source_frontmatter = translate_source_frontmatter(
                candidate.source_frontmatter,
                source_lang,
                target_lang,
                args.max_chars,
                cache,
            )
            new_frontmatter = update_frontmatter(
                candidate.frontmatter,
                {
                    "status": "complete",
                    "translator": "machine-google-reviewed",
                    "source_hash": candidate.source_hash,
                    "source_last_modified": f"\"{candidate.source_last_modified}\"",
                    "translation_last_reviewed": str(date.today()),
                },
            )
            translated_body_text = translated_body.lstrip(chr(10))
            if is_portal_i18n_translation(candidate.path):
                merged_frontmatter = merge_frontmatter_blocks(
                    new_frontmatter,
                    translated_source_frontmatter,
                )
                new_text = f"{candidate.preamble}{merged_frontmatter}\n\n{translated_body_text}"
            elif is_portal_docs_sibling_translation(candidate.path) and translated_source_frontmatter:
                new_text = (
                    f"{candidate.preamble}---\n{new_frontmatter}\n---\n\n"
                    f"{render_frontmatter(translated_source_frontmatter)}\n\n"
                    f"{translated_body_text}"
                )
            else:
                new_text = (
                    f"{candidate.preamble}---\n{new_frontmatter}\n---\n\n"
                    f"{translated_body_text}"
                )
        else:
            lines = candidate.frontmatter.splitlines()
            out_lines: list[str] = []
            seen_translator = False
            seen_source_hash = False
            seen_source_last_modified = False
            seen_translation_last_reviewed = False
            for line in lines:
                if line.startswith("#+STATUS:"):
                    out_lines.append("#+STATUS: complete")
                elif line.startswith("#+TRANSLATION_LAST_REVIEWED:"):
                    out_lines.append(f"#+TRANSLATION_LAST_REVIEWED: {date.today()}")
                    seen_translation_last_reviewed = True
                elif line.startswith("#+TRANSLATOR:"):
                    out_lines.append("#+TRANSLATOR: machine-google-reviewed")
                    seen_translator = True
                elif line.startswith("#+SOURCE_HASH:"):
                    out_lines.append(f"#+SOURCE_HASH: {candidate.source_hash}")
                    seen_source_hash = True
                elif line.startswith("#+SOURCE_LAST_MODIFIED:"):
                    out_lines.append(
                        f"#+SOURCE_LAST_MODIFIED: {candidate.source_last_modified}"
                    )
                    seen_source_last_modified = True
                else:
                    out_lines.append(line)
            if not seen_translator:
                out_lines.append("#+TRANSLATOR: machine-google-reviewed")
            if not seen_source_hash:
                out_lines.append(f"#+SOURCE_HASH: {candidate.source_hash}")
            if not seen_source_last_modified:
                out_lines.append(
                    f"#+SOURCE_LAST_MODIFIED: {candidate.source_last_modified}"
                )
            if not seen_translation_last_reviewed:
                out_lines.append(f"#+TRANSLATION_LAST_REVIEWED: {date.today()}")
            new_text = "\n".join(out_lines).rstrip("\n") + "\n\n" + translated_body.lstrip("\n")

        candidate.path.write_text(new_text, encoding="utf-8")
        translated += 1
        if args.verbose_files:
            print(f"file={idx}/{total} ok={rel_path}")
            sys.stdout.flush()

        if idx % APPLY_PROGRESS_EVERY == 0 or idx == total:
            print(f"progress={idx}/{total} translated={translated} failed={failed}")
            sys.stdout.flush()

    save_cache(cache_path, cache)
    print(f"done translated={translated} failed={failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
