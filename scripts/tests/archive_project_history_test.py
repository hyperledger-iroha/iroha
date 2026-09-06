"""Lossless dirty-source history, link identity, and bounded current-view contracts."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

SPEC = importlib.util.spec_from_file_location("history", Path(__file__).parents[1] / "archive_project_history.py")
history = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(history)
ARCHIVE = "docs/history/2026-09-06"


@pytest.fixture
def repo(tmp_path):
    root = tmp_path.resolve()
    (root / "docs").mkdir()
    (root / "docs/guide (one).md").write_text("# Guide\n")
    (root / "status.md").write_bytes(b"# Status\r\n\r\n## SDK 2026-09-05\r\n\r\n- Identical evidence.\r\n\r\n- [Guide](<docs/guide (one).md>) and [Roadmap](roadmap.md#sdk).\r\n\r\n## SDK 2026-09-05\r\n\r\n- Identical evidence.\r\n\r\n- Local [anchor](#sdk-2026-09-05-1).\r\n")
    (root / "roadmap.md").write_bytes(b"# Roadmap\n\n## SDK\n\n- Follow up [status](status.md#sdk-2026-09-05).\n\n- Preserve [missing](docs/missing.md).\n")
    return root


def make_archive(repo):
    manifest = history.capture(repo, ARCHIVE, "2026-09-06")
    return repo / ARCHIVE, manifest


def views():
    return {name: ("# " + Path(name).stem.capitalize() + "\n\n[Historical evidence](" + ARCHIVE + "/index.md)\n").encode() for name in history.SOURCES}


def write_manifest(archive, manifest):
    history.seal_manifest(archive, manifest)


def test_exact_dirty_sources_reconstruct_with_duplicates_crlf_and_links(repo):
    originals = history.source_bytes(repo)
    archive, manifest = make_archive(repo)
    verified, actual = history.verify(archive)
    assert actual == originals
    assert verified == manifest
    assert len(manifest["records"]) < sum(len(s["occurrences"]) for s in manifest["sources"])
    assert any(r["rewrites"] for r in manifest["records"])
    for record in manifest["records"]:
        for rewrite in record["rewrites"]:
            if rewrite["original"].endswith(".md#sdk"):
                assert "#record-" in rewrite["replacement"]
    audit = json.loads((archive / "unresolved-original-links.json").read_bytes())
    assert [(x["source"], x["target"], x["reason"]) for x in audit["links"]] == [("roadmap.md", "docs/missing.md", "original-path-not-found")]


def test_only_exact_records_deduplicate(repo):
    (repo / "status.md").write_bytes(b"# Status\n\n- Same.\n\n- Same.\n\n- Same!\n\n")
    _, manifest = make_archive(repo)
    items = manifest["sources"][0]["occurrences"]
    assert items[1]["record"] == items[2]["record"]
    assert items[1]["record"] != items[3]["record"]


def test_headings_and_lists_outside_fences_keep_order():
    raw = b"# Root\n\n## Area\n\n```md\n## Fake\n- fake item\n~~~~\n```\n\n- real\n  - child\n\nNext area\n---------\n\n~~~\n# Also fake\n~~~~\n\n## Area\n"
    sections = history.split_sections(raw)
    assert [s["headings"][-1] for s in sections if s["heading_level"]] == ["Root", "Area", "Next area", "Area"]
    assert [s["source_anchor"] for s in sections if s["heading_level"]] == ["root", "area", "next-area", "area-1"]
    assert b"".join(raw[s["start"]:s["end"]] for s in sections) == raw
    assert len(sections) == 5


def test_links_skip_code_and_preserve_balanced_and_html_destinations():
    raw = b"[a](docs/a(b).md) [b](<docs/a b.md>)\n[ref]: docs/file.md\n<img src='docs/img.svg'>\n`[code](untouched.md)`\n    [code](also.md)\n```\n[fenced](never.md)\n```\n\\](escaped.md)\n"
    values = [raw[a:b] for a, b in history.link_spans(raw)]
    assert values == [b"docs/a(b).md", b"docs/a b.md", b"docs/file.md", b"docs/img.svg"]
    rendered, rewrites = history.render(raw, "status.md", "records/x/page.md", ARCHIVE, {})
    assert b"../../../../a%28b%29.md" in rendered
    assert history.restore(rendered, rewrites) == raw


def test_reference_uses_remain_portable_across_pages(repo, monkeypatch):
    monkeypatch.setattr(history, "PAGE_LINE_TARGET", 8)
    (repo / "status.md").write_bytes(b"# Status\n\n## First\n[Guide], [custom][guide], and [guide][].\n`[guide]`\n\n## Second\n[guide]: <docs/guide (one).md>\n")
    archive, manifest = make_archive(repo)
    history.verify(archive)
    usage = next(r for r in manifest["records"] if len(r["rewrites"]) == 3)
    page = (archive / usage["page"]).read_bytes()
    body = page[usage["body_start"]:usage["body_end"]]
    assert b"[Guide](<../../../../guide%20%28one%29.md>)" in body
    assert b"`[guide]`" in body


def test_identical_fragment_text_in_two_sources_keeps_source_context(repo):
    for name in history.SOURCES:
        (repo / name).write_bytes(b"# " + name.encode() + b"\n\n## Target\n\n- [Go](#target)\n")
    archive, manifest = make_archive(repo)
    records = [r for r in manifest["records"] if r["identity_context"]]
    assert len(records) == 2
    assert records[0]["original_sha256"] == records[1]["original_sha256"]
    assert records[0]["id"] != records[1]["id"]
    history.verify(archive)


@pytest.mark.parametrize("target", ["../escape", "/absolute", "docs/../escape", "./escape", "docs//bad"])
def test_unsafe_archive_paths_rejected(repo, target):
    with pytest.raises(history.ArchiveError, match="unsafe"):
        history.capture(repo, target, "2026-09-06")


def test_archive_paths_cannot_traverse_symlinks(repo):
    (repo / "linked").symlink_to(repo / "docs", target_is_directory=True)
    with pytest.raises(history.ArchiveError, match="symlink"):
        history.capture(repo, "linked/archive", "2026-09-06")


def test_existing_archive_is_immutable(repo):
    make_archive(repo)
    with pytest.raises(history.ArchiveError, match="immutable"):
        make_archive(repo)


@pytest.mark.parametrize("mutation", ["source_hash", "occurrence_order", "heading", "record_extent", "rewrite", "schema", "record_source", "record_heading", "record_date"])
def test_manifest_drift_fails_closed(repo, mutation):
    archive, manifest = make_archive(repo)
    if mutation == "source_hash":
        manifest["sources"][0]["sha256"] = "0" * 64
    elif mutation == "occurrence_order":
        manifest["sources"][0]["occurrences"].reverse()
    elif mutation == "heading":
        manifest["sources"][0]["occurrences"][1]["headings"] = ["Fake"]
    elif mutation == "record_extent":
        manifest["records"][0]["body_end"] += 1
    elif mutation == "rewrite":
        next(r for r in manifest["records"] if r["rewrites"])["rewrites"][0]["original"] = "invented.md"
    elif mutation == "record_source":
        manifest["records"][0]["source"] = "roadmap.md"
    elif mutation == "record_heading":
        manifest["records"][0]["headings"] = ["Invented"]
    elif mutation == "record_date":
        manifest["records"][0]["event_date"] = "2000-01-01"
    else:
        manifest["schema_version"] = 0
    write_manifest(archive, manifest)
    with pytest.raises(history.ArchiveError):
        history.verify(archive)


@pytest.mark.parametrize("mutation", ["body", "extra_file", "missing_file", "symlink"])
def test_archive_file_drift_fails_closed(repo, mutation):
    archive, manifest = make_archive(repo)
    path = archive / manifest["records"][0]["page"]
    if mutation == "body":
        path.write_bytes(path.read_bytes() + b"invented\n")
    elif mutation == "extra_file":
        (archive / "extra.md").write_text("extra")
    elif mutation == "missing_file":
        path.unlink()
    else:
        path.unlink()
        path.symlink_to(repo / "status.md")
    with pytest.raises((history.ArchiveError, OSError)):
        history.verify(archive)


def test_capture_refuses_concurrent_source_update(repo, monkeypatch):
    original_verify = history.verify
    def change(archive):
        result = original_verify(archive)
        (repo / "status.md").write_bytes(b"# Concurrent update\n")
        return result
    monkeypatch.setattr(history, "verify", change)
    with pytest.raises(history.ArchiveError, match="changed before archive"):
        make_archive(repo)
    assert not (repo / ARCHIVE).exists()
    assert (repo / "status.md").read_bytes() == b"# Concurrent update\n"


def test_replace_preserves_concurrent_update(repo):
    archive, _ = make_archive(repo)
    before = (repo / "roadmap.md").read_bytes()
    (repo / "status.md").write_bytes(b"# Concurrent update\n")
    with pytest.raises(history.ArchiveError, match="changed since capture"):
        history.replace_views(repo, archive, views())
    assert (repo / "status.md").read_bytes() == b"# Concurrent update\n"
    assert (repo / "roadmap.md").read_bytes() == before


def test_replace_detects_update_during_staging(repo, monkeypatch):
    archive, _ = make_archive(repo)
    before = (repo / "status.md").read_bytes()
    mkstemp = history.tempfile.mkstemp
    def change(*args, **kwargs):
        result = mkstemp(*args, **kwargs)
        (repo / "roadmap.md").write_bytes(b"# Concurrent update\n")
        return result
    monkeypatch.setattr(history.tempfile, "mkstemp", change)
    with pytest.raises(history.ArchiveError, match="changed while staging"):
        history.replace_views(repo, archive, views())
    assert (repo / "status.md").read_bytes() == before
    assert (repo / "roadmap.md").read_bytes() == b"# Concurrent update\n"
    assert not list(repo.glob(".current-view-*"))


def test_replace_checks_views_then_preserves_original_reconstruction(repo):
    archive, _ = make_archive(repo)
    original = history.source_bytes(repo)
    history.replace_views(repo, archive, views())
    assert history.source_bytes(repo) == views()
    assert history.verify(archive)[1] == original


@pytest.mark.parametrize("bad", [b"# Wrong\n", b"# Status\n", views()["status.md"] + b"line\n" * 300])
def test_current_view_title_archive_and_line_limit_are_mandatory(bad):
    with pytest.raises(history.ArchiveError):
        history.validate_view("status.md", bad, ARCHIVE)


def test_reconstruction_cli_never_overwrites_source(repo):
    archive, _ = make_archive(repo)
    output = repo / "reconstructed"
    assert history.main(["reconstruct", "--root", str(repo), "--archive", ARCHIVE, "--output-dir", str(output)]) == 0
    assert {n: (output / n).read_bytes() for n in history.SOURCES} == history.source_bytes(repo)
    with pytest.raises(SystemExit):
        history.main(["reconstruct", "--root", str(repo), "--archive", ARCHIVE, "--output-dir", str(repo)])


def current_fixture(repo, manifest):
    current = views()
    current['roadmap.md'] += b'\n| ID | Outcome | Owner | Completion |\n| --- | --- | --- | --- |\n| A1 | SDK outcome | SDK | All consumers tested |\n'
    for name, raw in current.items():
        (repo / name).write_bytes(raw)
    source = next(s for s in manifest['sources'] if s['path'] == 'roadmap.md')
    coverage = {'schema_version': 1, 'archive': ARCHIVE, 'original_source': 'roadmap.md',
                'original_sha256': source['sha256'], 'current_source': 'roadmap.md',
                'areas': [{'heading': o['headings'][-1], 'original_anchor': o['source_anchor'],
                           'record': o['record'], 'outcomes': ['A1']} for o in source['occurrences'] if o['heading_level'] == 2]}
    path = repo / 'docs/history/current-roadmap-coverage.json'
    path.write_bytes(history.json_bytes(coverage))
    return path, coverage


def test_complete_current_views_and_area_coverage(repo):
    _, manifest = make_archive(repo)
    current_fixture(repo, manifest)
    history.verify_current(repo, manifest)


@pytest.mark.parametrize('mutation', ['omitted_area', 'old_hash', 'unknown_outcome', 'duplicate_outcome', 'missing_link', 'oversized'])
def test_current_views_reject_coverage_and_navigation_drift(repo, mutation):
    _, manifest = make_archive(repo)
    path, coverage = current_fixture(repo, manifest)
    if mutation == 'omitted_area':
        coverage['areas'] = []
    elif mutation == 'old_hash':
        coverage['original_sha256'] = '0' * 64
    elif mutation == 'unknown_outcome':
        coverage['areas'][0]['outcomes'] = ['A2']
    elif mutation == 'duplicate_outcome':
        with (repo / 'roadmap.md').open('ab') as stream:
            stream.write(b'| A1 | Duplicate | SDK | Invalid |\n')
    elif mutation == 'missing_link':
        with (repo / 'status.md').open('ab') as stream:
            stream.write(b'[Missing](missing.md)\n')
    else:
        with (repo / 'status.md').open('ab') as stream:
            stream.write(b'line\n' * 300)
    path.write_bytes(history.json_bytes(coverage))
    with pytest.raises(history.ArchiveError):
        history.verify_current(repo, manifest)


@pytest.mark.parametrize('mutation', ['gzip_drift', 'expanded_size', 'source_summary', 'uncompressed_format'])
def test_compressed_inventory_has_one_hash_bound_format(repo, mutation):
    archive, manifest = make_archive(repo)
    disk = json.loads((archive / 'manifest.json').read_bytes())
    if mutation == 'gzip_drift':
        path = archive / history.INVENTORY_PATH
        path.write_bytes(path.read_bytes() + b'extra')
    elif mutation == 'expanded_size':
        disk['inventory']['expanded_bytes'] -= 1
    elif mutation == 'source_summary':
        disk['sources'][0]['lines'] += 1
    else:
        disk = manifest
    (archive / 'manifest.json').write_bytes(history.json_bytes(disk))
    with pytest.raises((history.ArchiveError, KeyError)):
        history.verify(archive)


def test_repository_current_views_cover_every_archived_roadmap_area():
    root = Path(__file__).resolve().parents[2]
    archive = root / ARCHIVE
    disk = json.loads((archive / 'manifest.json').read_bytes())
    import gzip
    inventory = json.loads(gzip.decompress((archive / history.INVENTORY_PATH).read_bytes()))
    manifest = {**disk, **inventory}
    history.verify_current(root, manifest)
    assert len(json.loads((root / 'docs/history/current-roadmap-coverage.json').read_bytes())['areas']) == 66
    assert [(s['path'], s['sha256']) for s in manifest['sources']] == [
        ('status.md', 'ebafd062901d863418432e4a27eca2049b4fd57470b1a1c9d7d1499cdb2ebb2d'),
        ('roadmap.md', 'ba0930ff8fa9cb9ee45b8c6d31ac67c571452e78d9d7696b568a808cb3509835'),
    ]
