#!/usr/bin/env python3
import re
import sys
from pathlib import Path, PurePosixPath
import unicodedata

ROOT = Path(__file__).resolve().parents[1]

LINK_RE = re.compile(r"\[(?:[^\]]+)\]\(([^)]+)\)")
CODE_ANCHOR_RE = re.compile(r"^L\d+(?:C\d+)?(?:-L\d+(?:C\d+)?)?$")
DOC_HEADING_ID_RE = re.compile(r"\s*\{#([A-Za-z0-9_-]+)\}\s*$")
DOC_EXTS = (".md", ".mdx")

PORTAL_ROOT = ROOT / "docs" / "portal"
PORTAL_DOCS_ROOT = PORTAL_ROOT / "docs"
PORTAL_VERSIONED_ROOT = PORTAL_ROOT / "versioned_docs"
PORTAL_I18N_ROOT = PORTAL_ROOT / "i18n"
PORTAL_STATIC_ROOT = PORTAL_ROOT / "static"
ALLOW_PATHS = {ROOT / "artifacts" / "docs_portal_preview"}

SKIP_DIRS = {
    ".cargo",
    ".git",
    ".idea",
    ".vscode",
    "artifacts",
    "build",
    "dist",
    "node_modules",
    "target",
    "target-codex",
    "target_codex",
    "target-snapshot",
    "target-wallet",
    "target_tmp_clippy",
    "target_tmp_sumeragi",
    "tmp",
    "vendor",
}

def is_allowed_path(path: Path) -> bool:
    for allowed in ALLOW_PATHS:
        try:
            path.relative_to(allowed)
            return True
        except ValueError:
            continue
    return False

def should_skip_path(path: Path) -> bool:
    if is_allowed_path(path):
        return False
    return any(part in SKIP_DIRS for part in path.parts)

def slugify(title: str) -> str:
    # GitHub-like slug: lowercase, remove diacritics, whitespace -> '-', then drop punctuation.
    title = title.strip().lower()
    title = unicodedata.normalize("NFKD", title)
    title = "".join(c for c in title if not unicodedata.combining(c))
    title = re.sub(r"\s+", "-", title)
    cleaned = "".join(c for c in title if c.isalnum() or c == "-")
    return cleaned.strip("-")

def extract_anchors(md: str):
    anchors = set()
    for line in md.splitlines():
        if line.lstrip().startswith('#'):
            # strip leading #'s and whitespace
            stripped = line.lstrip()
            h = stripped[stripped.find(' ') + 1:] if ' ' in stripped else ''
            h = h.strip()
            if h:
                explicit = DOC_HEADING_ID_RE.search(h)
                if explicit:
                    anchors.add(slugify(explicit.group(1)))
                else:
                    anchors.add(slugify(h))
        for match in re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)["\']', line):
            anchors.add(slugify(match.strip()))
    return anchors

def is_external(url: str) -> bool:
    return url.startswith(("http://", "https://", "mailto:", "ftp:"))


def normalize_link(raw: str) -> str:
    raw = raw.strip()
    if raw.startswith("<") and raw.endswith(">"):
        raw = raw[1:-1].strip()
    return raw

def strip_quotes(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
        return value[1:-1].strip()
    return value

def extract_front_matter(text: str):
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        if not line:
            i += 1
            continue
        if line.startswith("<!--"):
            if "-->" in line:
                i += 1
                continue
            i += 1
            while i < len(lines) and "-->" not in lines[i]:
                i += 1
            if i < len(lines):
                i += 1
            continue
        break
    if i >= len(lines) or lines[i].strip() != "---":
        return {}
    i += 1
    fm_lines = []
    while i < len(lines) and lines[i].strip() != "---":
        fm_lines.append(lines[i])
        i += 1
    if i >= len(lines):
        return {}
    data = {}
    for line in fm_lines:
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        if key not in {"id", "slug"}:
            continue
        data[key] = strip_quotes(value)
    return data

def normalize_doc_id(value: str) -> str:
    value = strip_quotes(value)
    if not value:
        return ""
    value = value.replace("\\", "/").strip()
    path = PurePosixPath(value)
    parts = []
    for part in path.parts:
        if part in ("", ".", "/"):
            continue
        if part == "..":
            if parts:
                parts.pop()
            continue
        parts.append(part)
    return "/".join(parts)

def resolve_relative_doc_id(path_part: str, base_dir: str) -> str:
    if not path_part:
        return ""
    base = PurePosixPath(base_dir) if base_dir else PurePosixPath(".")
    combined = base / path_part
    parts = []
    for part in combined.parts:
        if part in ("", ".", "/"):
            continue
        if part == "..":
            if parts:
                parts.pop()
            continue
        parts.append(part)
    return "/".join(parts)

def iter_md_files(root: Path):
    seen = set()
    for ext in DOC_EXTS:
        for path in root.rglob(f"*{ext}"):
            if should_skip_path(path):
                continue
            if path not in seen:
                seen.add(path)
                yield path

def find_portal_doc_roots():
    roots = []
    if PORTAL_DOCS_ROOT.is_dir():
        roots.append(PORTAL_DOCS_ROOT)
    if PORTAL_VERSIONED_ROOT.is_dir():
        roots.extend(sorted(p for p in PORTAL_VERSIONED_ROOT.iterdir() if p.is_dir()))
    if PORTAL_I18N_ROOT.is_dir():
        for lang_dir in sorted(PORTAL_I18N_ROOT.iterdir()):
            docs_dir = lang_dir / "docusaurus-plugin-content-docs"
            if not docs_dir.is_dir():
                continue
            for version_dir in sorted(docs_dir.iterdir()):
                if version_dir.is_dir():
                    roots.append(version_dir)
    return roots

def build_doc_id_map(doc_root: Path):
    mapping = {}
    for ext in DOC_EXTS:
        for doc_path in doc_root.rglob(f"*{ext}"):
            if should_skip_path(doc_path):
                continue
            text = doc_path.read_text(encoding="utf-8", errors="ignore")
            front = extract_front_matter(text)
            doc_ids = set()
            relative = doc_path.relative_to(doc_root).with_suffix("")
            default_id = relative.as_posix()
            doc_ids.add(default_id)
            lower_default = default_id.lower()
            if lower_default.endswith("/index"):
                doc_ids.add(default_id[: -len("/index")])
            if lower_default.endswith("/readme"):
                doc_ids.add(default_id[: -len("/readme")])
            parent = relative.parent.as_posix()
            if parent == ".":
                parent = ""
            id_value = front.get("id")
            if id_value:
                normalized = normalize_doc_id(id_value)
                if normalized:
                    if parent:
                        doc_ids.add(f"{parent}/{normalized}")
                    doc_ids.add(normalized)
            slug_value = front.get("slug")
            if slug_value:
                normalized = normalize_doc_id(slug_value)
                if normalized:
                    doc_ids.add(normalized)
            for doc_id in doc_ids:
                if doc_id and doc_id not in mapping:
                    mapping[doc_id] = doc_path
    return mapping

def find_doc_root(path: Path, doc_roots):
    ordered_roots = sorted(doc_roots, key=lambda p: len(p.parts), reverse=True)
    for root in ordered_roots:
        try:
            path.relative_to(root)
            return root
        except ValueError:
            continue
    return None

def resolve_path_target(md_path: Path, path_part: str):
    base = (ROOT / path_part.lstrip("/")) if path_part.startswith("/") else (md_path.parent / path_part)
    base = base.resolve()
    if base.exists():
        return base, base
    if path_part.endswith("/"):
        for ext in DOC_EXTS:
            for name in ("index", "README"):
                candidate = base / f"{name}{ext}"
                if candidate.exists():
                    return candidate, base
    if Path(path_part).suffix == "":
        for ext in DOC_EXTS:
            candidate = base.with_suffix(ext)
            if candidate.exists():
                return candidate, base
        for ext in DOC_EXTS:
            for name in ("index", "README"):
                candidate = base / f"{name}{ext}"
                if candidate.exists():
                    return candidate, base
    if path_part.startswith("/"):
        static_candidate = PORTAL_STATIC_ROOT / path_part.lstrip("/")
        if static_candidate.exists():
            return static_candidate, base
    return None, base

def resolve_doc_id_target(md_path: Path, path_part: str, doc_roots, doc_id_maps):
    normalized = normalize_doc_id(path_part)
    if not normalized:
        return None
    if Path(normalized).suffix:
        return None
    candidates = []
    if "/" in normalized:
        last_segment = normalized.rsplit("/", 1)[-1]
    else:
        last_segment = None
    roots = []
    doc_root = find_doc_root(md_path, doc_roots)
    if doc_root:
        roots.append(doc_root)
        if not path_part.startswith("/"):
            rel_dir = md_path.relative_to(doc_root).parent.as_posix()
            rel_candidate = resolve_relative_doc_id(path_part, rel_dir)
            if rel_candidate:
                candidates.append(rel_candidate)
    if normalized not in candidates:
        candidates.append(normalized)
    if last_segment and last_segment not in candidates:
        candidates.append(last_segment)
    roots.extend(root for root in doc_roots if root not in roots)
    for root in roots:
        doc_map = doc_id_maps.get(root)
        if not doc_map:
            continue
        for candidate in candidates:
            target = doc_map.get(candidate)
            if target:
                return target
    return None

def main():
    md_files = list(iter_md_files(ROOT))
    doc_roots = find_portal_doc_roots()
    doc_id_maps = {root: build_doc_id_map(root) for root in doc_roots}
    broken = []
    for md_path in md_files:
        text = md_path.read_text(encoding='utf-8', errors='ignore')
        # strip HTML comments to avoid checking commented-out links
        text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
        # precompute anchors for same-file anchor-only links
        self_anchors = extract_anchors(text)
        for m in LINK_RE.finditer(text):
            raw = normalize_link(m.group(1))
            if is_external(raw):
                continue
            # strip query segment
            path_part, hash_part = (raw.split('#', 1) + [''])[:2] if '#' in raw else (raw, '')
            path_part = path_part.split('?', 1)[0]
            if path_part == '' and hash_part:
                # anchor-only
                if slugify(hash_part) not in self_anchors:
                    broken.append((md_path, raw, 'missing-anchor', f"#{hash_part}"))
                continue
            # normalize path
            target, base_target = resolve_path_target(md_path, path_part)
            if target is None:
                target = resolve_doc_id_target(md_path, path_part, doc_roots, doc_id_maps)
            if target is None:
                broken.append((md_path, raw, 'missing-file', str(base_target.relative_to(ROOT) if str(base_target).startswith(str(ROOT)) else base_target)))
                continue
            if hash_part:
                # GitHub-style line anchors (e.g., #L123, #L10-L42, #L10C5) are not present in
                # the raw file contents, so treat them as always valid.
                if CODE_ANCHOR_RE.fullmatch(hash_part):
                    continue
                try:
                    ttext = target.read_text(encoding='utf-8', errors='ignore')
                except Exception:
                    broken.append((md_path, raw, 'unreadable-file', str(target)))
                    continue
                anchors = extract_anchors(ttext)
                if slugify(hash_part) not in anchors:
                    broken.append((md_path, raw, 'missing-anchor', f"{target.relative_to(ROOT)}#{hash_part}"))
    if broken:
        print('Broken links found:')
        for src, raw, kind, info in broken:
            print(f"- {kind}: {src}:{raw} -> {info}")
        sys.exit(1)
    else:
        print('No broken local links found.')

if __name__ == '__main__':
    main()
