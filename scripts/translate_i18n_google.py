#!/usr/bin/env python3
"""
Bulk-translate source-identical i18n files using the no-key Google endpoint.

Targets Markdown/MDX/Org docs that:
1) carry i18n metadata (`lang`, `source` / `#+SOURCE`), and
2) still mirror the English source body verbatim.

The script preserves metadata, masks non-prose regions (code, URLs, directives),
applies translation in chunks, performs structural QA checks, and caches
translations per (source-body-hash, locale) to avoid duplicated API work.
"""

from __future__ import annotations

import argparse
import concurrent.futures as futures
import hashlib
import html
import json
import os
import re
import sys
import time
import urllib.parse
import urllib.request
from urllib.error import HTTPError
from dataclasses import dataclass
from datetime import date
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CACHE_PATH = ROOT / "artifacts" / "translation_cache_google.json"
DEFAULT_MAX_CHARS = 1400
TRANSLATION_ENDPOINT = "https://translate.google.com/m"
PLACEHOLDER_RE = re.compile(r"I18N[A-Z]\d{8}X")
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


@dataclass
class Candidate:
    path: Path
    lang: str
    source: Path
    source_body: str
    frontmatter: str
    source_hash: str
    kind: str  # "md" | "org"


@dataclass(frozen=True)
class TranslationTask:
    key: str
    kind: str
    source_body: str
    target_lang: str


def split_frontmatter(text: str) -> tuple[str | None, str]:
    if not text.startswith("---\n"):
        return None, text
    end = text.find("\n---\n", 4)
    if end == -1:
        return None, text
    return text[4:end], text[end + 5 :]


def parse_frontmatter_map(frontmatter: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in frontmatter.splitlines():
        if ":" not in line or line.startswith((" ", "\t")):
            continue
        key, value = line.split(":", 1)
        out[key.strip()] = value.strip()
    return out


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
    idx_map: dict[str, str] = {}
    for token, value in placeholders.items():
        m = re.match(r"^I18N[A-Z](\d{8})X$", token)
        if m:
            idx_map[m.group(1)] = value

    def restore_variant(match: re.Match[str]) -> str:
        idx = match.group(1)
        return idx_map.get(idx, match.group(0))

    text = re.sub(r"I18N[A-Z]\s*(\d{8})\s*X", restore_variant, text)

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


def translate_chunk(chunk: str, target_lang: str, retries: int = 8) -> str:
    params = {
        "sl": "en",
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
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = response.read().decode("utf-8")
            m = re.search(r'<div class="result-container">(.*?)</div>', payload, flags=re.DOTALL)
            if not m:
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


def translate_markdown(text: str, target_lang: str, max_chars: int) -> str:
    masked, placeholders = mask_markdown(text)
    chunks = split_chunks(masked, max_chars=max_chars)
    translated = "".join(translate_chunk(chunk, target_lang) for chunk in chunks)
    return unmask_text(translated, placeholders)


def translate_org(text: str, target_lang: str, max_chars: int) -> str:
    masked, placeholders = mask_org(text)
    chunks = split_chunks(masked, max_chars=max_chars)
    translated = "".join(translate_chunk(chunk, target_lang) for chunk in chunks)
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
        if task.kind == "md":
            translated = translate_markdown(task.source_body, task.target_lang, max_chars)
        else:
            translated = translate_org(task.source_body, task.target_lang, max_chars)
        ok, reason = validate_translation(task.source_body, translated)
        if not ok:
            return task.key, None, reason
        return task.key, translated, None
    except Exception as exc:  # noqa: BLE001 - network and decode failures
        return task.key, None, f"translation error: {exc}"


def discover_candidates(only_changed: bool) -> list[Candidate]:
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

        text = path.read_text(encoding="utf-8", errors="ignore")
        lang: str | None = None
        source_rel: str | None = None
        source_body: str | None = None
        frontmatter = ""
        kind = "md"

        if path.suffix in {".md", ".mdx"}:
            kind = "md"
            fm, body = split_frontmatter(text)
            if fm is None:
                continue
            frontmatter = fm
            meta = parse_frontmatter_map(fm)
            lang = meta.get("lang")
            source_rel = meta.get("source")
            if not lang or not source_rel or lang == "en":
                continue
            source = (ROOT / source_rel).resolve()
            if not source.exists():
                continue
            source_text = source.read_text(encoding="utf-8", errors="ignore")
            _, source_body = split_frontmatter(source_text)
            if body.lstrip("\n") != source_body.lstrip("\n"):
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
            source = (ROOT / source_rel).resolve()
            if not source.exists():
                continue
            source_text = source.read_text(encoding="utf-8", errors="ignore")
            source_body = source_text
            # Stubs converted earlier are "<meta block>\\n\\n<source text>".
            if not text.endswith(source_text):
                continue
            # frontmatter equivalent = initial metadata block up to first non #+ line.
            lines = text.splitlines()
            meta_lines: list[str] = []
            i = 0
            while i < len(lines) and (lines[i].startswith("#+") or lines[i].strip() == ""):
                meta_lines.append(lines[i])
                i += 1
            frontmatter = "\n".join(meta_lines).rstrip("\n")

        source_hash = hashlib.sha256(source_body.encode("utf-8")).hexdigest()
        candidates.append(
            Candidate(
                path=path.resolve(),
                lang=lang,
                source=source,
                source_body=source_body,
                frontmatter=frontmatter,
                source_hash=source_hash,
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
    cache_path.write_text(
        json.dumps(cache, ensure_ascii=False, indent=2, sort_keys=True),
        encoding="utf-8",
    )


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
    args = parser.parse_args()

    cache_path = Path(args.cache).resolve()
    cache = load_cache(cache_path)
    candidates = discover_candidates(only_changed=args.only_changed)
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
        target_lang = LANG_CODE_MAP.get(candidate.lang, candidate.lang)
        cache_key = f"{candidate.kind}::{candidate.source_hash}::{target_lang}"
        if cache_key in cache or cache_key in unique_tasks:
            continue
        unique_tasks[cache_key] = TranslationTask(
            key=cache_key,
            kind=candidate.kind,
            source_body=candidate.source_body,
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
                if completed % 25 == 0 or completed == len(unique_tasks):
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
                    if completed % 25 == 0 or completed == len(unique_tasks):
                        save_cache(cache_path, cache)
                        print(
                            f"translate_progress={completed}/{len(unique_tasks)} ok={translated_keys} failed={len(failed_keys)}"
                        )
                        sys.stdout.flush()

    # Phase 2: apply translated/cache content to files.
    translated = 0
    failed = 0
    for idx, candidate in enumerate(candidates, start=1):
        target_lang = LANG_CODE_MAP.get(candidate.lang, candidate.lang)
        rel_path = candidate.path.relative_to(ROOT)
        cache_key = f"{candidate.kind}::{candidate.source_hash}::{target_lang}"
        if cache_key in failed_keys:
            failed += 1
            print(f"warning: skipped {rel_path} due to failed translation key", file=sys.stderr)
            continue

        translated_body = cache.get(cache_key)
        if translated_body is None:
            failed += 1
            print(f"warning: missing cache for {rel_path}", file=sys.stderr)
            continue

        if candidate.kind == "md":
            new_frontmatter = update_frontmatter(
                candidate.frontmatter,
                {
                    "status": "complete",
                    "translator": "machine-google-reviewed",
                    "translation_last_reviewed": str(date.today()),
                },
            )
            new_text = f"---\n{new_frontmatter}\n---\n\n{translated_body.lstrip(chr(10))}"
        else:
            lines = candidate.frontmatter.splitlines()
            out_lines: list[str] = []
            seen_translator = False
            for line in lines:
                if line.startswith("#+STATUS:"):
                    out_lines.append("#+STATUS: complete")
                elif line.startswith("#+TRANSLATION_LAST_REVIEWED:"):
                    out_lines.append(f"#+TRANSLATION_LAST_REVIEWED: {date.today()}")
                elif line.startswith("#+TRANSLATOR:"):
                    out_lines.append("#+TRANSLATOR: machine-google-reviewed")
                    seen_translator = True
                else:
                    out_lines.append(line)
            if not seen_translator:
                out_lines.append("#+TRANSLATOR: machine-google-reviewed")
            new_text = "\n".join(out_lines).rstrip("\n") + "\n\n" + translated_body.lstrip("\n")

        candidate.path.write_text(new_text, encoding="utf-8")
        translated += 1
        print(f"file={idx}/{total} ok={rel_path}")
        sys.stdout.flush()

        if idx % 25 == 0 or idx == total:
            save_cache(cache_path, cache)
            print(f"progress={idx}/{total} translated={translated} failed={failed}")
            sys.stdout.flush()

    save_cache(cache_path, cache)
    print(f"done translated={translated} failed={failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
