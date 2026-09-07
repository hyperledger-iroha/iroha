#!/usr/bin/env python3
"""Archive dirty project records losslessly, then replace them with bounded current views.

Archive bodies are stored once per exact section and link-resolution context.
Source occurrence manifests retain byte order, heading context and original hashes.
Relative link rewrites are reversible; verification reconstructs the original inputs.
Requires Python 3.10+ and only its standard library, with no environment variables.
The default root is this script's repository. Capture and reconstruction require
new output paths; only the explicit replace command can replace both root views,
and only while their bytes still match the authenticated historical inputs.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from datetime import date
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import tempfile
import unicodedata
from urllib.parse import quote, unquote, urlsplit, urlunsplit

SOURCES = ("status.md", "roadmap.md")
VIEW_LINE_LIMIT = 300
PAGE_LINE_TARGET = 500
INVENTORY_PATH = "record-inventory.json.gz"
MAX_INVENTORY_BYTES = 128 * 1024 * 1024
ATX = re.compile(rb"^ {0,3}(#{1,6})(?:[ \t]+|$)(.*?)(?:\r?\n)?$")
FENCE = re.compile(rb"^ {0,3}(`{3,}|~{3,})(.*)$")
LIST = re.compile(rb"^(?:[-+*]|[0-9]{1,9}[.)])[ \t]+")
SUBSYSTEMS = (
    ("kagemusha", r"kagemusha|offline|secure.element|peer transport|nearby"),
    ("consensus-network", r"sumeragi|consensus|da/rbc|rbc|p2p|commitqc|quorum|finality|lane.lifecycle|multilane"),
    ("sccp-settlement", r"sccp|atomic.private|private.settlement|tron|ethereum|\bton\b|\bbsc\b"),
    ("sorafs", r"sorafs|governance.dag|car archive"),
    ("soranet", r"soranet|vpn|relay|inrou"),
    ("musubi-taikai", r"musubi|taikai|soracloud|kaigi"),
    ("governance-identity", r"parliament|governance|sns|alias|constitution|dataspace.bootstrap|onboarding"),
    ("crypto-proofs", r"crypto|privacy|zk|fhe|bfv|halo2|signature|mkhe|figure.9|fastpq"),
    ("vm-norito-model", r"\bivm\b|kotodama|norito|data.model|derive|proc.macro|abi|kagami"),
    ("sdks-native", r"sdk|swift|kotlin|java|python|c#|android|ios|native|electron|wallet"),
    ("ledger-state", r"kura|world|wsv|ledger|state|core|transaction|asset|nft"),
    ("architecture-build", r"architecture|build|memory|compile|workspace|repository|dependency|warning|test.target"),
    ("operations-torii", r"torii|telemetry|configuration|mcp|taira|minamoto|iso.?20022|security|release|operator|deployment"),
    ("community-docs", r"community|documentation|contributor|maintainer|lfdt|mochi"),
)


class ArchiveError(ValueError):
    """An archive, source identity, or replacement precondition is invalid."""


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_path(root: Path, relative: str) -> Path:
    parts = PurePosixPath(relative)
    if not relative or parts.is_absolute() or any(p in ("..", ".", "") for p in relative.split("/")):
        raise ArchiveError(f"unsafe archive path: {relative}")
    result = root.joinpath(*parts.parts)
    if any(root.joinpath(*parts.parts[:n]).is_symlink() for n in range(1, len(parts.parts) + 1)):
        raise ArchiveError(f"archive paths must not traverse symlinks: {relative}")
    if not result.resolve().is_relative_to(root.resolve()):
        raise ArchiveError(f"archive path escapes root: {relative}")
    return result


def visible_lines(raw: bytes):
    """Yield line offsets and visibility, respecting CommonMark fenced blocks."""
    fence = None
    offset = 0
    for line in raw.splitlines(keepends=True):
        marker = FENCE.match(line.rstrip(b"\r\n"))
        visible = fence is None
        if marker:
            run, rest = marker.groups()
            if fence is not None:
                if run[:1] == fence[:1] and len(run) >= len(fence) and not rest.strip():
                    fence = None
                visible = False
            elif run[:1] != b"`" or b"`" not in rest:
                fence = run
                visible = False
        yield offset, line, visible
        offset += len(line)


def slug(text: str) -> str:
    text = re.sub(r"<[^>]*>", "", text).lower()
    text = re.sub(r"!?\[([^]]*)\]\([^)]*\)", r"\1", text)
    return "".join(c for c in text if c in " _-" or unicodedata.category(c)[0] in "LN").replace(" ", "-")


def split_sections(raw: bytes) -> list[dict]:
    """Split headings and top-level evidence items; never split a fenced block."""
    lines = list(visible_lines(raw))
    headings = {}
    for index, (offset, line, visible) in enumerate(lines):
        if not visible:
            continue
        match = ATX.match(line)
        if match:
            title = re.sub(rb"[ \t]+#+[ \t]*$", b"", match[2]).decode("utf-8").strip()
            headings[offset] = (len(match[1]), title)
        elif index and re.fullmatch(rb" {0,3}(?:=+|-+)[ \t]*(?:\r?\n)?", line):
            before_offset, before, before_visible = lines[index - 1]
            if before_visible and before.strip() and before_offset not in headings and not LIST.match(before):
                headings[before_offset] = (1 if line.lstrip().startswith(b"=") else 2, before.decode("utf-8").strip())
    boundaries = {0, len(raw), *headings}
    boundaries.update(offset for offset, line, visible in lines if visible and LIST.match(line))
    stack = []
    result = []
    slug_counts = defaultdict(int)
    offsets = sorted(boundaries)
    line_starts = {offset: i + 1 for i, (offset, _, _) in enumerate(lines)}
    for start, end in zip(offsets, offsets[1:]):
        if start == end:
            continue
        heading = headings.get(start)
        anchor = None
        if heading:
            level, title = heading
            while stack and stack[-1][0] >= level:
                stack.pop()
            stack.append(heading)
            base = slug(title)
            number = slug_counts[base]
            anchor = f"{base}-{number}" if number else base
            slug_counts[base] += 1
        context = [title for _, title in stack]
        result.append({"start": start, "end": end, "line": line_starts.get(start, 1),
                       "headings": context, "heading_level": heading[0] if heading else None,
                       "source_anchor": anchor})
    return result


def link_spans(raw: bytes) -> list[tuple[int, int]]:
    """Locate Markdown destinations/reference definitions outside code, plus HTML links.

    Balanced destination parentheses and angle-bracket destinations are retained.
    Code fences, indented code, inline code and escaped link syntax are untouched.
    """
    spans = []
    list_item = bool(LIST.match(raw.lstrip(b"\r\n")))
    code_indent = b"      " if list_item else b"    "
    for offset, line, visible in visible_lines(raw):
        if not visible or line.startswith((code_indent, b"\t")):
            continue
        masked = bytearray(line)
        for match in re.finditer(rb"(`+)(.*?)\1", line):
            masked[match.start():match.end()] = b" " * (match.end() - match.start())
        search = bytes(masked)
        reference = re.match(rb" {0,3}\[[^]\r\n]+\]:[ \t]*(?:<([^>\r\n]*)>|([^ \t\r\n]+))", search)
        if reference:
            group = 1 if reference[1] is not None else 2
            spans.append((offset + reference.start(group), offset + reference.end(group)))
        for match in re.finditer(rb"(?<!\\)\]\([ \t]*", search):
            start = match.end()
            if start < len(line) and line[start:start + 1] == b"<":
                end = line.find(b">", start + 1)
                if end != -1:
                    spans.append((offset + start + 1, offset + end))
                continue
            end = start
            depth = 0
            while end < len(line):
                c = line[end:end + 1]
                if c == b"\\" and end + 1 < len(line):
                    end += 2
                    continue
                if c == b"(":
                    depth += 1
                elif c == b")":
                    if not depth:
                        break
                    depth -= 1
                elif c in b" \t\r\n" and not depth:
                    break
                end += 1
            if end > start:
                spans.append((offset + start, offset + end))
        for match in re.finditer(rb"\b(?:href|src)=[\"']([^\"']+)[\"']", search):
            spans.append((offset + match.start(1), offset + match.end(1)))
    return sorted(set(spans))


def reference_definitions(raw: bytes) -> dict[str, str]:
    """Collect single-line reference definitions before dividing a source into records."""
    result = {}
    for _, line, visible in visible_lines(raw):
        if visible:
            match = re.match(rb" {0,3}\[([^]\r\n]+)\]:[ \t]*(?:<([^>\r\n]*)>|([^ \t\r\n]+))", line)
            if match:
                label = " ".join(match[1].decode().split()).casefold()
                result.setdefault(label, (match[2] if match[2] is not None else match[3]).decode())
    return result


def reference_uses(raw: bytes, definitions: dict[str, str]) -> list[tuple[int, int, bytes, str]]:
    """Expand defined full, collapsed, and shortcut references into portable links."""
    result = []
    for offset, line, visible in visible_lines(raw):
        if not visible or line.startswith((b"    ", b"\t")) or re.match(rb" {0,3}\[[^]]+\]:", line):
            continue
        masked = bytearray(line)
        for code in re.finditer(rb"(`+)(.*?)\1", line):
            masked[code.start():code.end()] = b" " * (code.end() - code.start())
        for match in re.finditer(rb"(?<!\\)(!?)\[([^]\r\n]+)\](?:\[([^]\r\n]*)\])?(?![\[(])", bytes(masked)):
            label = (match[3] or match[2]).decode()
            target = definitions.get(" ".join(label.split()).casefold())
            if target is not None:
                result.append((offset + match.start(), offset + match.end(), match[1] + b"[" + match[2] + b"]", target))
    return result


def category(context: list[str], raw: bytes) -> tuple[str, str]:
    title = " ".join(context[1:] or context) + " " + raw.splitlines()[0].decode("utf-8")[:300]
    dates = re.findall(r"\b\d{4}-\d{2}-\d{2}\b", title)
    event_date = "undated"
    for value in dates:
        try:
            date.fromisoformat(value)
            event_date = value
            break
        except ValueError:
            pass
    subsystem = next((name for name, pattern in SUBSYSTEMS if re.search(pattern, title, re.I)), "general")
    return subsystem, event_date


def remap_link(target: str, source: str, page: str, archive_relative: str, anchors: dict) -> str:
    parsed = urlsplit(target)
    if parsed.scheme or parsed.netloc or target.startswith("/"):
        return target
    source_path = PurePosixPath(source)
    original_path = str(source_path if not parsed.path else source_path.parent / unquote(parsed.path))
    original_path = os.path.normpath(original_path).replace(os.sep, "/")
    if original_path in SOURCES:
        reference = anchors.get((original_path, unquote(parsed.fragment)))
        if reference:
            target_page, record_id = reference
            absolute = str(PurePosixPath(archive_relative) / target_page)
            relative = os.path.relpath(absolute, str(PurePosixPath(archive_relative) / PurePosixPath(page).parent))
            return urlunsplit(("", "", relative, parsed.query, "record-" + record_id))
    page_parent = str(PurePosixPath(archive_relative) / PurePosixPath(page).parent)
    relative = os.path.relpath(original_path, page_parent).replace(os.sep, "/")
    return urlunsplit(("", "", quote(relative, safe="/._-~%"), parsed.query, parsed.fragment))


def render(raw: bytes, source: str, page: str, archive_relative: str, anchors: dict,
           definitions: dict[str, str] | None = None) -> tuple[bytes, list]:
    output = bytearray()
    rewrites = []
    previous = 0
    edits = [(start, end, remap_link(raw[start:end].decode(), source, page, archive_relative, anchors).encode())
             for start, end in link_spans(raw)]
    for start, end, label, target in reference_uses(raw, definitions or {}):
        destination = remap_link(target, source, page, archive_relative, anchors).encode()
        edits.append((start, end, label + b"(<" + destination + b">)"))
    for start, end, value in sorted(edits):
        if start < previous:
            raise ArchiveError("overlapping link destinations")
        original = raw[start:end]
        output.extend(raw[previous:start])
        rendered_start = len(output)
        output.extend(value)
        if value != original:
            rewrites.append({"original_start": start, "original_end": end,
                             "rendered_start": rendered_start, "rendered_end": len(output),
                             "original": original.decode("utf-8"), "replacement": value.decode("utf-8")})
        previous = end
    output.extend(raw[previous:])
    return bytes(output), rewrites


def restore(body: bytes, rewrites: list) -> bytes:
    result = body
    previous = len(body)
    for change in reversed(rewrites):
        start, end = change["rendered_start"], change["rendered_end"]
        if not 0 <= start <= end <= previous or result[start:end] != change["replacement"].encode("utf-8"):
            raise ArchiveError("link rewrite identity mismatch")
        result = result[:start] + change["original"].encode("utf-8") + result[end:]
        previous = start
    for change in rewrites:
        if result[change["original_start"]:change["original_end"]] != change["original"].encode("utf-8"):
            raise ArchiveError("original link rewrite offsets mismatch")
    return result


def json_bytes(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def seal_manifest(archive: Path, manifest: dict):
    """Store repetitive occurrence metadata compressed, with a readable hash-bound manifest."""
    inventory = json_bytes({"records": manifest["records"], "sources": manifest["sources"]})
    if len(inventory) > MAX_INVENTORY_BYTES:
        raise ArchiveError("record inventory exceeds the bounded verifier input")
    compressed = gzip.compress(inventory, mtime=0)
    safe_path(archive, INVENTORY_PATH).write_bytes(compressed)
    disk = {k: v for k, v in manifest.items() if k not in ("records", "sources", "inventory")}
    disk["sources"] = [{k: v for k, v in source.items() if k != "occurrences"} for source in manifest["sources"]]
    disk["inventory"] = {"path": INVENTORY_PATH, "sha256": digest(compressed),
                         "bytes": len(compressed), "expanded_bytes": len(inventory),
                         "records": len(manifest["records"]),
                         "occurrences": sum(len(s["occurrences"]) for s in manifest["sources"])}
    safe_path(archive, "manifest.json").write_bytes(json_bytes(disk))


def source_bytes(root: Path) -> dict[str, bytes]:
    result = {}
    for name in SOURCES:
        path = safe_path(root, name)
        before = path.stat()
        raw = path.read_bytes()
        after = path.stat()
        if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns):
            raise ArchiveError(f"source changed while reading: {name}")
        raw.decode("utf-8")
        result[name] = raw
    return result


def link_audit(root: Path, records: dict, anchors: dict, definitions: dict) -> list[dict]:
    """Record pre-existing missing paths/fragments rather than inventing replacements."""
    missing = {}
    for record in records.values():
        raw, source = record["_raw"], record["source"]
        targets = [raw[a:b].decode() for a, b in link_spans(raw)]
        targets += [target for _, _, _, target in reference_uses(raw, definitions[source])]
        for target in targets:
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc or target.startswith("/"):
                continue
            path = os.path.normpath(str(PurePosixPath(source).parent / unquote(parsed.path))) if parsed.path else source
            reason = None
            if path in SOURCES and (path, unquote(parsed.fragment)) not in anchors:
                reason = "original-source-fragment-not-found"
            elif not (root / path).exists():
                reason = "original-path-not-found"
            if reason:
                key = (source, target, reason)
                missing.setdefault(key, {"source": source, "target": target, "reason": reason, "records": []})["records"].append(record["id"])
    return [dict(item, records=sorted(set(item["records"]))) for _, item in sorted(missing.items())]


def capture(root: Path, archive_relative: str, captured: str) -> dict:
    date.fromisoformat(captured)
    root = root.resolve()
    destination = safe_path(root, archive_relative)
    if destination.exists():
        raise ArchiveError("archive already exists; existing evidence is immutable")
    originals = source_bytes(root)
    definitions = {name: reference_definitions(raw) for name, raw in originals.items()}
    records = {}
    sources = []
    for name, raw in originals.items():
        occurrences = []
        for section in split_sections(raw):
            body = raw[section["start"]:section["end"]]
            # Equal fragment-only text in two documents has different link meaning.
            context = name if any(body[a:b].startswith(b"#") for a, b in link_spans(body)) or reference_uses(body, definitions[name]) else ""
            original_hash = digest(body)
            record_id = digest(body + b"\0source-fragment-context\0" + context.encode()) if context else original_hash
            if record_id not in records:
                subsystem, event_date = category(section["headings"], body)
                records[record_id] = {"id": record_id, "original_sha256": original_hash,
                    "original_bytes": len(body), "identity_context": context, "source": name,
                    "subsystem": subsystem, "event_date": event_date,
                    "headings": section["headings"], "_raw": body}
            elif records[record_id]["_raw"] != body:
                raise ArchiveError("record hash collision")
            occurrences.append({**section, "record": record_id})
        sources.append({"path": name, "sha256": digest(raw), "bytes": len(raw),
                        "lines": len(raw.splitlines()), "occurrences": occurrences})
    groups = defaultdict(list)
    for record in records.values():
        groups[(record["subsystem"], record["event_date"])].append(record)
    pages = {}
    for (subsystem, event_date), values in sorted(groups.items()):
        part, lines = 1, 0
        for record in values:
            length = len(record["_raw"].splitlines()) + 5
            if lines and lines + length > PAGE_LINE_TARGET:
                part += 1
                lines = 0
            page = f"records/{subsystem}/{event_date}-{part:03d}.md"
            record["page"] = page
            pages.setdefault(page, []).append(record)
            lines += length
    anchors = {}
    for source in sources:
        for item in source["occurrences"]:
            record = records[item["record"]]
            anchors.setdefault((source["path"], ""), (record["page"], record["id"]))
            if item["source_anchor"] is not None:
                anchors[(source["path"], item["source_anchor"])] = (record["page"], record["id"])
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix=".history-stage-", dir=destination.parent))
    try:
        files = []
        def write(relative: str, raw: bytes):
            path = safe_path(stage, relative)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            files.append({"path": relative, "sha256": digest(raw), "bytes": len(raw)})
        for page, values in pages.items():
            content = bytearray(b"# Historical project evidence\n\nThis page is historical evidence, not current release readiness.\nSee [the archive index](../../index.md) for provenance and reconstruction.\n\n")
            previous_context = None
            for record in values:
                title = " / ".join(record["headings"])
                content.extend(f'\n<a id="record-{record["id"]}"></a>\n\n'.encode())
                if title != previous_context:
                    content.extend(f'<!-- Original context: {title.replace("--", "—")} -->\n'.encode())
                    previous_context = title
                rendered, rewrites = render(record["_raw"], record["source"], page, archive_relative, anchors, definitions[record["source"]])
                record["body_start"] = len(content)
                content.extend(rendered)
                record["body_end"] = len(content)
                record["rendered_sha256"] = digest(rendered)
                record["rewrites"] = rewrites
                content.extend(b"\n")
            write(page, bytes(content))
        missing_links = link_audit(root, records, anchors, definitions)
        write("unresolved-original-links.json", json_bytes({"historical": True, "captured_date": captured, "links": missing_links}))
        index = ["# Historical project evidence", "", f"Captured from the dirty working copies on {captured}. These records are historical,",
                 "including statements once labelled active; they are not current release readiness.", "",
                 "The manifest records every original occurrence, byte order, heading context and hash.",
                 "Its hash-bound `record-inventory.json.gz` contains the complete JSON occurrence",
                 "inventory; compression removes metadata repetition without dropping source evidence.",
                 "Only exact sections with the same relative-link context are deduplicated. Link",
                 "rewrites are reversible; code and pre-existing unresolved targets remain evidence.", "", "| Original source | Bytes | Lines | SHA-256 |", "| --- | ---: | ---: | --- |"]
        for source in sources:
            index.append(f'| `{source["path"]}` | {source["bytes"]} | {source["lines"]} | `{source["sha256"]}` |')
        index += ["", f'{len(records)} unique records preserve {sum(len(s["occurrences"]) for s in sources)} source occurrences.',
                  "", f'{len(missing_links)} distinct source/target pairs were already unresolved at capture; see',
                  "[the original-link audit](unresolved-original-links.json). Their original destinations",
                  "are retained, not silently redirected. External URLs and fragments in other files",
                  "are not network-checked. Defined reference links are expanded reversibly so they",
                  "continue to work when their definition and use land on different archive pages.",
                  "", "## Subsystem navigation", "", "| Subsystem | Records | Pages |", "| --- | ---: | ---: |"]
        for subsystem in sorted({r["subsystem"] for r in records.values()}):
            selected = [p for p in pages if p.startswith(f"records/{subsystem}/")]
            count = sum(len(pages[p]) for p in selected)
            index.append(f"| [{subsystem}](records/{subsystem}/index.md) | {count} | {len(selected)} |")
            navigation = [f"# Historical {subsystem} evidence", "", "These dated records are not current release readiness.", "", "[Archive provenance](../../index.md)", ""]
            navigation += [f'- [{PurePosixPath(p).stem}]({PurePosixPath(p).name}) — {len(pages[p])} records' for p in selected]
            write(f"records/{subsystem}/index.md", ("\n".join(navigation) + "\n").encode())
        index += ["", "## Original roadmap areas", "", "Every original heading remains traceable; the current roadmap consolidates outcomes.", ""]
        roadmap = next(s for s in sources if s["path"] == "roadmap.md")
        for item in roadmap["occurrences"]:
            if item["heading_level"] == 2:
                record = records[item["record"]]
                index.append(f'- [{item["headings"][-1]}]({record["page"]}#record-{record["id"]})')
        index += ["", "## Integrity and reconstruction", "", "From the repository root:", "", "```sh",
                  f"python3 scripts/archive_project_history.py verify --archive {archive_relative}",
                  f"python3 scripts/archive_project_history.py reconstruct --archive {archive_relative} --output-dir /tmp/iroha-original-records",
                  "```", "", "Reconstruction writes the exact original bytes, including original relative links.",
                  "The original roots are replaced only after their hashes still match this manifest."]
        write("index.md", ("\n".join(index) + "\n").encode())
        manifest = {"schema_version": 1, "historical": True, "captured_date": captured,
                    "archive_relative": archive_relative, "sources": sources,
                    "records": [{k: v for k, v in r.items() if k != "_raw"} for r in records.values()], "files": files}
        seal_manifest(stage, manifest)
        verify(stage)
        if source_bytes(root) != originals:
            raise ArchiveError("source changed before archive publication")
        os.replace(stage, destination)
        return manifest
    finally:
        if stage.exists():
            shutil.rmtree(stage)


def verify(archive: Path) -> tuple[dict, dict[str, bytes]]:
    manifest = json.loads(safe_path(archive, "manifest.json").read_bytes())
    if manifest.get("schema_version") != 1 or manifest.get("historical") is not True:
        raise ArchiveError("unsupported or non-historical archive schema")
    identity = manifest["inventory"]
    if identity["path"] != INVENTORY_PATH or not 0 <= identity["expanded_bytes"] <= MAX_INVENTORY_BYTES:
        raise ArchiveError("invalid record inventory bounds or path")
    compressed = safe_path(archive, INVENTORY_PATH).read_bytes()
    if len(compressed) != identity["bytes"] or digest(compressed) != identity["sha256"]:
        raise ArchiveError("record inventory identity mismatch")
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
        expanded = stream.read(identity["expanded_bytes"] + 1)
    if len(expanded) != identity["expanded_bytes"]:
        raise ArchiveError("record inventory expanded extent mismatch")
    inventory = json.loads(expanded)
    summary = [{k: v for k, v in source.items() if k != "occurrences"} for source in inventory["sources"]]
    if summary != manifest["sources"] or len(inventory["records"]) != identity["records"] or sum(len(s["occurrences"]) for s in inventory["sources"]) != identity["occurrences"]:
        raise ArchiveError("record inventory summary mismatch")
    manifest.update(inventory)
    del manifest["inventory"]
    files = {}
    for item in manifest["files"]:
        if item["path"] in files:
            raise ArchiveError("duplicate file inventory entry")
        raw = safe_path(archive, item["path"]).read_bytes()
        if len(raw) != item["bytes"] or digest(raw) != item["sha256"]:
            raise ArchiveError(f'archive file drift: {item["path"]}')
        files[item["path"]] = raw
    expected_files = {"manifest.json", INVENTORY_PATH, *files}
    actual_files = {p.relative_to(archive).as_posix() for p in archive.rglob("*") if p.is_file() or p.is_symlink()}
    if actual_files != expected_files:
        raise ArchiveError("archive contains unmanifested or missing files")
    records = {}
    metadata = {}
    spans = defaultdict(list)
    for item in manifest["records"]:
        if item["id"] in records:
            raise ArchiveError("duplicate record identity")
        page = files[item["page"]]
        start, end = item["body_start"], item["body_end"]
        if not 0 <= start <= end <= len(page):
            raise ArchiveError("record body is out of range")
        spans[item["page"]].append((start, end))
        body = page[start:end]
        if digest(body) != item["rendered_sha256"]:
            raise ArchiveError("rendered record identity mismatch")
        original = restore(body, item["rewrites"])
        if len(original) != item["original_bytes"] or digest(original) != item["original_sha256"]:
            raise ArchiveError("original record identity mismatch")
        context = item["identity_context"]
        expected_id = digest(original + b"\0source-fragment-context\0" + context.encode()) if context else digest(original)
        if expected_id != item["id"]:
            raise ArchiveError("record key mismatch")
        records[item["id"]] = original
        metadata[item["id"]] = item
    for values in spans.values():
        ordered = sorted(values)
        if any(end > start for (_, end), (start, _) in zip(ordered, ordered[1:])):
            raise ArchiveError("overlapping archive records")
    originals = {}
    used = set()
    first_occurrence = {}
    for source in manifest["sources"]:
        if source["path"] not in SOURCES or source["path"] in originals:
            raise ArchiveError("invalid source inventory")
        assembled = bytearray()
        for item in source["occurrences"]:
            original = records[item["record"]]
            used.add(item["record"])
            first_occurrence.setdefault(item["record"], (source["path"], item["headings"]))
            if item["start"] != len(assembled) or item["end"] != len(assembled) + len(original):
                raise ArchiveError("source occurrence order or extent mismatch")
            assembled.extend(original)
        raw = bytes(assembled)
        if len(raw) != source["bytes"] or digest(raw) != source["sha256"] or len(raw.splitlines()) != source["lines"]:
            raise ArchiveError(f'original source reconstruction mismatch: {source["path"]}')
        parsed = split_sections(raw)
        expected = [{k: v for k, v in item.items() if k != "record"} for item in source["occurrences"]]
        if parsed != expected:
            raise ArchiveError("original heading/section occurrence metadata drift")
        originals[source["path"]] = raw
    if set(originals) != set(SOURCES) or used != set(records):
        raise ArchiveError("source inventory incomplete or orphan archive records")
    anchors = {}
    definitions = {name: reference_definitions(raw) for name, raw in originals.items()}
    for source in manifest["sources"]:
        for occurrence in source["occurrences"]:
            item = metadata[occurrence["record"]]
            anchors.setdefault((source["path"], ""), (item["page"], item["id"]))
            if occurrence["source_anchor"] is not None:
                anchors[(source["path"], occurrence["source_anchor"])] = (item["page"], item["id"])
    for record_id, original in records.items():
        item = metadata[record_id]
        if (item["source"], item["headings"]) != first_occurrence[record_id]:
            raise ArchiveError("invalid record source context")
        if (item["subsystem"], item["event_date"]) != category(item["headings"], original):
            raise ArchiveError("invalid historical subsystem or date projection")
        context = item["source"] if any(original[a:b].startswith(b"#") for a, b in link_spans(original)) or reference_uses(original, definitions[item["source"]]) else ""
        if item["identity_context"] != context:
            raise ArchiveError("invalid relative-fragment identity context")
        rendered, rewrites = render(original, item["source"], item["page"], manifest["archive_relative"], anchors, definitions[item["source"]])
        if rendered != files[item["page"]][item["body_start"]:item["body_end"]] or rewrites != item["rewrites"]:
            raise ArchiveError("noncanonical relative-link rewrite")
    return manifest, originals


def validate_view(name: str, raw: bytes, archive_relative: str):
    text = raw.decode("utf-8")
    if len(text.splitlines()) > VIEW_LINE_LIMIT:
        raise ArchiveError(f"{name} exceeds the {VIEW_LINE_LIMIT}-line current-view limit")
    if not text.startswith("# " + Path(name).stem.capitalize() + "\n"):
        raise ArchiveError(f"{name} must start with its canonical document title")
    if archive_relative + "/index.md" not in text:
        raise ArchiveError(f"{name} must link its historical source archive")


def verify_current(root: Path, manifest: dict):
    """Check bounded current roots and complete historical-area outcome coverage."""
    views = source_bytes(root)
    for name, raw in views.items():
        validate_view(name, raw, manifest["archive_relative"])
        for start, end in link_spans(raw):
            target = urlsplit(raw[start:end].decode())
            if not target.scheme and not target.netloc and target.path and not (root / unquote(target.path)).exists():
                raise ArchiveError(f"current view has a missing relative link: {name}: {target.path}")
    coverage = json.loads(safe_path(root, "docs/history/current-roadmap-coverage.json").read_bytes())
    original = next(s for s in manifest["sources"] if s["path"] == "roadmap.md")
    if (coverage.get("schema_version") != 1 or coverage.get("archive") != manifest["archive_relative"]
            or coverage.get("original_source") != "roadmap.md" or coverage.get("current_source") != "roadmap.md"
            or coverage.get("original_sha256") != original["sha256"]):
        raise ArchiveError("current roadmap coverage source identity mismatch")
    expected = [{"heading": o["headings"][-1], "original_anchor": o["source_anchor"], "record": o["record"]}
                for o in original["occurrences"] if o["heading_level"] == 2]
    actual = [{k: v for k, v in area.items() if k != "outcomes"} for area in coverage["areas"]]
    if actual != expected:
        raise ArchiveError("current roadmap coverage omits or changes an original area")
    identifiers = re.findall(rb"^\| ([A-Z][0-9]+) \|", views["roadmap.md"], re.M)
    if len(identifiers) != len(set(identifiers)):
        raise ArchiveError("duplicate current roadmap outcome identifier")
    identifiers = {value.decode() for value in identifiers}
    mapped = set()
    for area in coverage["areas"]:
        outcomes = area["outcomes"]
        if not outcomes or len(outcomes) != len(set(outcomes)) or not set(outcomes) <= identifiers:
            raise ArchiveError("current roadmap area has invalid outcome coverage")
        mapped.update(outcomes)
    if mapped != identifiers:
        raise ArchiveError("current roadmap has outcomes missing from its coverage map")


def replace_views(root: Path, archive: Path, views: dict[str, bytes]):
    manifest, originals = verify(archive)
    if set(views) != set(SOURCES):
        raise ArchiveError("both current views are required")
    for name, raw in views.items():
        validate_view(name, raw, manifest["archive_relative"])
    if source_bytes(root) != originals:
        raise ArchiveError("source changed since capture; refusing root replacement")
    staged = {}
    try:
        for name, raw in views.items():
            fd, path = tempfile.mkstemp(prefix=".current-view-", dir=root)
            staged[name] = Path(path)
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        # A second full-source check catches writes made while preparing replacement files.
        if source_bytes(root) != originals:
            raise ArchiveError("source changed while staging current views")
        for name in SOURCES:
            if safe_path(root, name).read_bytes() != originals[name]:
                raise ArchiveError(f"source changed immediately before replacement: {name}")
            os.replace(staged[name], root / name)
    finally:
        for path in staged.values():
            path.unlink(missing_ok=True)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("capture", "verify", "reconstruct", "replace"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--archive", required=True)
    parser.add_argument("--date")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--status-view", type=Path)
    parser.add_argument("--roadmap-view", type=Path)
    parser.add_argument("--check-current", action="store_true")
    args = parser.parse_args(argv)
    try:
        root = args.root.resolve()
        archive = safe_path(root, args.archive)
        if args.command == "capture":
            if not args.date:
                raise ArchiveError("capture requires an explicit --date")
            manifest = capture(root, args.archive, args.date)
        else:
            manifest, originals = verify(archive)
            if args.command == "reconstruct":
                if not args.output_dir or args.output_dir.exists():
                    raise ArchiveError("reconstruction requires a new --output-dir")
                args.output_dir.mkdir(parents=True)
                for name, raw in originals.items():
                    (args.output_dir / name).write_bytes(raw)
            elif args.command == "replace":
                if not args.status_view or not args.roadmap_view:
                    raise ArchiveError("replace requires both prepared current views")
                replace_views(root, archive, {"status.md": args.status_view.read_bytes(), "roadmap.md": args.roadmap_view.read_bytes()})
            if args.check_current:
                verify_current(root, manifest)
        print(f'Historical archive verified: {len(manifest["records"])} records, '
              f'{sum(len(s["occurrences"]) for s in manifest["sources"])} occurrences.')
        return 0
    except (ArchiveError, OSError, KeyError, ValueError, TypeError, EOFError) as error:
        parser.exit(1, f"history archive: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
