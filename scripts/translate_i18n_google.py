#!/usr/bin/env python3
"""
Bulk-translate source-identical i18n files using the no-key Google endpoint.

This script targets Markdown/MDX docs that:
1) have i18n front matter (`lang`, `source`), and
2) still mirror the English source body verbatim.

It preserves front matter, masks code-like regions before translation, and
caches translated source bodies per (source-body-hash, locale) so duplicated
copies (for example portal docs + portal i18n mirrors) do not retranslate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import date
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CACHE_PATH = ROOT / "artifacts" / "translation_cache_google.json"
DEFAULT_MAX_CHARS = 3500
TRANSLATION_ENDPOINT = "https://translate.googleapis.com/translate_a/single"
LANG_CODE_MAP = {
    "zh-hans": "zh-CN",
    "zh-hant": "zh-TW",
}


@dataclass
class Candidate:
    path: Path
    lang: str
    source: Path
    source_body: str
    frontmatter: str
    source_hash: str


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
        token = f"__I18N_{tag}_{counter:08d}__"
        counter += 1
        placeholders[token] = value
        return token

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
    # Inline code.
    text = re.sub(
        r"`[^`\n]+`",
        lambda m: reserve(m.group(0), "INLINE"),
        text,
        flags=re.MULTILINE,
    )
    return text, placeholders


def unmask_text(text: str, placeholders: dict[str, str]) -> str:
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


def translate_chunk(chunk: str, target_lang: str, retries: int = 5) -> str:
    params = {
        "client": "gtx",
        "sl": "en",
        "tl": target_lang,
        "dt": "t",
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
                    "Accept": "application/json",
                },
            )
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = response.read().decode("utf-8")
            data = json.loads(payload)
            return "".join(item[0] for item in data[0])
        except Exception as exc:  # noqa: BLE001 - network retries
            last_exc = exc
            time.sleep(backoff)
            backoff = min(backoff * 2, 10.0)

    assert last_exc is not None
    raise last_exc


def translate_markdown(text: str, target_lang: str, max_chars: int) -> str:
    masked, placeholders = mask_markdown(text)
    chunks = split_chunks(masked, max_chars=max_chars)
    translated = "".join(translate_chunk(chunk, target_lang) for chunk in chunks)
    return unmask_text(translated, placeholders)


def discover_candidates(only_changed: bool) -> list[Candidate]:
    candidates: list[Candidate] = []
    paths = ROOT.rglob("*.md")
    mdx_paths = ROOT.rglob("*.mdx")
    all_paths = list(paths) + list(mdx_paths)

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
        frontmatter, body = split_frontmatter(text)
        if frontmatter is None:
            continue
        meta = parse_frontmatter_map(frontmatter)
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

        source_hash = hashlib.sha256(source_body.encode("utf-8")).hexdigest()
        candidates.append(
            Candidate(
                path=path.resolve(),
                lang=lang,
                source=source,
                source_body=source_body,
                frontmatter=frontmatter,
                source_hash=source_hash,
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
    args = parser.parse_args()

    cache_path = Path(args.cache).resolve()
    cache = load_cache(cache_path)
    candidates = discover_candidates(only_changed=args.only_changed)
    if args.limit > 0:
        candidates = candidates[: args.limit]

    total = len(candidates)
    print(f"candidates={total}")
    if total == 0:
        return 0

    translated = 0
    for idx, candidate in enumerate(candidates, start=1):
        target_lang = LANG_CODE_MAP.get(candidate.lang, candidate.lang)
        cache_key = f"{candidate.source_hash}::{target_lang}"
        translated_body = cache.get(cache_key)
        if translated_body is None:
            translated_body = translate_markdown(
                candidate.source_body,
                target_lang=target_lang,
                max_chars=args.max_chars,
            )
            cache[cache_key] = translated_body

        new_frontmatter = update_frontmatter(
            candidate.frontmatter,
            {
                "status": "complete",
                "translator": "machine-google",
                "translation_last_reviewed": str(date.today()),
            },
        )
        new_text = f"---\n{new_frontmatter}\n---\n\n{translated_body.lstrip(chr(10))}"
        candidate.path.write_text(new_text, encoding="utf-8")
        translated += 1

        if idx % 25 == 0 or idx == total:
            save_cache(cache_path, cache)
            print(f"progress={idx}/{total} translated={translated}")
            sys.stdout.flush()

    save_cache(cache_path, cache)
    print(f"done translated={translated}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

