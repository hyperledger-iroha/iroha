#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

ROOT = Path("/Users/takemiyamakoto/devstuff/iroha-build-efficiency")
MANIFEST = Path("/tmp/iroha_raw_literal_assets_v1.tsv")

SPEC_TEXT = r'''
mochi/mochi-core/src/supervisor.rs 4389-4399 559e83c89863
mochi/mochi-core/src/supervisor.rs 4417-4450 27c49db41253
mochi/mochi-core/src/supervisor.rs 4465-4467 275239824e00
mochi/mochi-core/src/supervisor.rs 4525-4528 27507ba750d0
mochi/mochi-core/src/supervisor.rs 4550-4553 1abe6f1599a4
mochi/mochi-core/src/supervisor.rs 4617-4686 484a62e3e49e
mochi/mochi-core/src/supervisor.rs 4731-4796 2a5adcdb41c3
mochi/mochi-core/src/supervisor.rs 7687-7693 3342c3c9198b
crates/iroha_config/tests/autoscale_config.rs 176-185 878e0a787a99
crates/iroha_config/tests/autoscale_config.rs 197-216 80e7e720d91c
crates/iroha_config/tests/autoscale_config.rs 229-253 92dc76a26e34
crates/iroha_config/tests/autoscale_config.rs 278-307 c4925e1fd28f
crates/iroha_config/tests/autoscale_config.rs 332-361 ef770358157e
crates/iroha_config/tests/autoscale_config.rs 373-397 eb53c11159af
crates/iroha_config/tests/autoscale_config.rs 410-434 65c788acef7c
crates/iroha_config/tests/autoscale_config.rs 577-585 c5ec2ae3a3dd
crates/iroha_config/tests/autoscale_config.rs 598-606 806b1a5a2768
crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs 655-662 320e3f55053c
crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs 681-707 0c5fdc176059
crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs 729-746 a1ad4b84998b
crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs 795-834 868f845a210e
crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs 846-912 7e3f91cb4c6b
xtask/src/kagami_profiles.rs 457-467 471935039bd1
xtask/src/kagami_profiles.rs 472-477 56eaa42054b1
xtask/src/kagami_profiles.rs 482-488 5e9c8ebfee4e
xtask/src/kagami_profiles.rs 493-499 3bd23d515289
xtask/src/kagami_profiles.rs 504-509 c43402e72ae0
xtask/src/kagami_profiles.rs 531-578 5a4ac7ec7245
xtask/src/kagami_profiles.rs 611-622 5a99dc6e6fb4
xtask/src/kagami_profiles.rs 665-682 8b9c7dc18338
xtask/src/kagami_profiles.rs 835-842 22525014a1b0
crates/sorafs_car/tests/capacity_cli.rs 301-337 d9498c428ae5
crates/sorafs_car/tests/capacity_cli.rs 525-539 dd54c5210e7b
crates/sorafs_car/tests/capacity_cli.rs 541-571 db3621dc12bb
crates/sorafs_car/tests/capacity_cli.rs 573-589 1298e2e183dd
tools/soradns-resolver/src/config.rs 750-791 1834d58d159b
tools/soradns-resolver/src/config.rs 816-821 3750ed80f1d8
tools/soradns-resolver/src/config.rs 833-838 b422b87c5e6e
tools/soradns-resolver/src/config.rs 855-861 80131f9f2a73
tools/soradns-resolver/src/config.rs 875-880 3553fe90c8ff
tools/soradns-resolver/src/config.rs 897-902 20f88420770b
xtask/src/ministry_agenda.rs 893-902 3e01e9b01673
xtask/src/ministry_agenda.rs 905-961 14be09ba42b8
mochi/mochi-ui-egui/src/config.rs 1016-1039 f9ca49aa26e2
mochi/mochi-ui-egui/src/config.rs 1186-1203 62d757a049d8
mochi/mochi-ui-egui/src/config.rs 1508-1532 6b8f9b7e2b06
xtask/src/soranet_gar_controller.rs 882-914 3ca231d8931b
xtask/src/soranet_gar_controller.rs 992-1024 6108a71c61f9
mochi/mochi-core/src/bootstrap.rs 218-235 3aa19183db48
mochi/mochi-core/src/bootstrap.rs 247-272 2503d7955072
mochi/mochi-core/src/bootstrap.rs 282-300 10ebd66337c7
crates/iroha_config/tests/sorafs_por_replay_archive.rs 79-87 a606e0401df2
crates/iroha_config/tests/sorafs_por_replay_archive.rs 108-146 4555b503bfc4
crates/iroha_config/tests/sorafs_por_replay_archive.rs 373-381 e56770861081
scripts/fastpq/src/bin/metal_profile.rs 643-693 7807218ca09a
xtask/src/main.rs 10289-10294 12c4a22f2c07
xtask/src/main.rs 11813-11851 e7136c6bdae1
xtask/src/main.rs 11853-11860 a0c31d6e4d9d
crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs 79-87 cc7348a667f4
crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs 105-135 0e6b8366e6ff
crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs 263-273 890e088d4343
crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs 53-61 0d0b3d693236
crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs 71-101 39e9aee1636e
crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs 113-123 c3d31dfe8ad8
crates/iroha_swarm/src/schema.rs 744-753 f0bab2d34b59
crates/iroha_swarm/src/schema.rs 831-870 af8369e3d597
crates/build-support/src/bin/sumeragi_da_report.rs 734-755 7b0608366f76
crates/build-support/src/bin/sumeragi_da_report.rs 808-833 cc4804690141
crates/iroha_torii/src/sorafs/evidence_viewer_api.rs 1697-1730 097abd46e146
crates/iroha_torii/src/sorafs/evidence_viewer_api.rs 1732-1743 cbfba33d5d9c
mochi/mochi-ui-egui/src/gui.rs 2866-2869 d89dbf8f5641
mochi/mochi-ui-egui/src/gui.rs 3139-3150 c311636b4b42
mochi/mochi-ui-egui/src/gui.rs 13806-13812 56207812770b
mochi/mochi-ui-egui/src/gui.rs 16047-16068 24794537905d
mochi/mochi-ui-egui/src/gui.rs 16076-16078 306c6ca74075
crates/iroha_config/tests/trusted_peers_pop_validation.rs 43-56 6003859c9fdf
crates/iroha_config/tests/trusted_peers_pop_validation.rs 73-82 9474d89f53c5
crates/iroha_config/tests/trusted_peers_pop_validation.rs 122-130 9851b1199536
crates/iroha_config/tests/trusted_peers_pop_validation.rs 145-157 1cac6b1e5b89
crates/iroha_cli/src/offline.rs 3353-3373 b5af316d2c9f
crates/iroha_cli/src/offline.rs 3606-3626 905f0cc56300
crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs 272-292 4b0677f0c80b
crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs 841-861 b7d46e1cd9f5
mochi/mochi-core/src/torii.rs 8223-8231 0a0c5a315127
mochi/mochi-core/src/torii.rs 8274-8292 1b886b3cfa18
mochi/mochi-core/src/torii.rs 8322-8329 6184db554e44
crates/norito/src/core/simd_crc64.rs 1401-1428 7380a0aafddb
xtask/tests/sm_wycheproof_sync.rs 16-42 05c00d71ff5e
crates/iroha_kagami/src/kagemusha/taira.rs 1207-1232 bbc583778e53
crates/iroha_kagami/src/wizard.rs 197-222 49e0255ae4fa
xtask/src/soranet_gateway_pq.rs 526-550 c319b527497b
crates/sorafs_car/src/trustless.rs 492-516 c319b527497b
crates/iroha_cli/src/commands/connect.rs 526-549 f7f7526a5aaf
crates/iroha_config/examples/sumeragi_v2_context_hash.rs 89-110 85f065548cdd
xtask/src/sns.rs 1822-1834 6f71077a1fa7
xtask/src/sns.rs 1881-1889 5148a736a335
xtask/src/sorafs.rs 9135-9155 4ee8a78c499d
crates/iroha_data_model/tests/query_json_envelope.rs 25-41 5fabdb5f319f
crates/build-support/src/bin/clippy_inventory.rs 260-275 31be0d72b1d4
crates/norito/tests/cross_language.rs 88-101 0c4ea7eeb479
xtask/src/sm.rs 436-449 ff93d41dafd9
tools/norito_codegen_exporter/src/main.rs 1038-1048 4347fd70fffb
crates/iroha_kagami/src/swarm.rs 387-397 6b1299281ab8
crates/iroha_core/src/smartcontracts/ivm.rs 717-727 a68a2ba20e9f
crates/iroha_data_model/tests/ballot_proof_json.rs 55-64 569e518d8b38
xtask/src/ministry_jury.rs 504-512 7bc3349bbf69
xtask/src/ministry.rs 2131-2139 cbd7aaa60e2e
crates/iroha_kagami/src/genesis/validate.rs 287-295 a98f35788fd8
mochi/mochi-core/src/compose.rs 2159-2167 5a86383101ee
'''.strip()

# source -> (root kind, semantic topic, extension, category)
META = {
    "mochi/mochi-core/src/supervisor.rs": ("fixtures", "supervisor-scripts", "sh.in", "test-shell-fixture"),
    "crates/iroha_config/tests/autoscale_config.rs": ("fixtures", "autoscale", "toml", "test-toml-fixture"),
    "crates/iroha_data_model/src/soracloud/tests/manifest_validation.rs": ("fixtures", "soracloud-manifests", "json", "test-json-fixture"),
    "xtask/src/kagami_profiles.rs": ("templates", "kagami-profiles", "toml.in", "generated-template"),
    "crates/sorafs_car/tests/capacity_cli.rs": ("fixtures", "capacity-cli", "json", "test-json-fixture"),
    "tools/soradns-resolver/src/config.rs": ("fixtures", "resolver-config", "json.in", "test-json-template"),
    "xtask/src/ministry_agenda.rs": ("fixtures", "ministry-agenda", "json", "test-json-fixture"),
    "mochi/mochi-ui-egui/src/config.rs": ("fixtures", "config", "toml", "test-toml-fixture"),
    "xtask/src/soranet_gar_controller.rs": ("fixtures", "soranet-gar", "json.in", "test-json-template"),
    "mochi/mochi-core/src/bootstrap.rs": ("templates", "bootstrap", "txt.in", "generated-source-template"),
    "crates/iroha_config/tests/sorafs_por_replay_archive.rs": ("fixtures", "sorafs-por-replay", "toml", "test-toml-fixture"),
    "scripts/fastpq/src/bin/metal_profile.rs": ("fixtures", "metal-profile", "json", "test-json-fixture"),
    "xtask/src/main.rs": ("fixtures", "signature-manifests", "json", "test-json-fixture"),
    "crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs": ("fixtures", "sorafs-reputation-archive", "toml", "test-toml-fixture"),
    "crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs": ("fixtures", "sorafs-provider-archive", "toml", "test-toml-fixture"),
    "crates/iroha_swarm/src/schema.rs": ("templates", "compose-scripts", "sh.in", "generated-shell-template"),
    "crates/build-support/src/bin/sumeragi_da_report.rs": ("fixtures", "sumeragi-da-report", "json", "test-json-fixture"),
    "crates/iroha_torii/src/sorafs/evidence_viewer_api.rs": ("assets", "evidence-viewer", "html", "embedded-web-asset"),
    "mochi/mochi-ui-egui/src/gui.rs": ("fixtures", "gui", "txt", "test-fixture"),
    "crates/iroha_config/tests/trusted_peers_pop_validation.rs": ("fixtures", "trusted-peers-pop", "toml", "test-toml-fixture"),
    "crates/iroha_cli/src/offline.rs": ("fixtures", "offline-eval-capture", "json", "test-json-fixture"),
    "crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs": ("fixtures", "soracloud-manifest-bundles", "json", "test-json-fixture"),
    "mochi/mochi-core/src/torii.rs": ("fixtures", "torii-responses", "json", "test-http-fixture"),
    "crates/norito/src/core/simd_crc64.rs": ("fixtures", "crc64", "c", "test-c-source-fixture"),
    "xtask/tests/sm_wycheproof_sync.rs": ("fixtures", "sm-wycheproof", "json.in", "test-json-template"),
    "crates/iroha_kagami/src/kagemusha/taira.rs": ("fixtures", "kagemusha-taira", "json", "test-json-fixture"),
    "crates/iroha_kagami/src/wizard.rs": ("templates", "wizard", "toml.in", "generated-config-template"),
    "xtask/src/soranet_gateway_pq.rs": ("fixtures", "soranet-gateway-pq", "toml", "test-toml-fixture"),
    "crates/sorafs_car/src/trustless.rs": ("fixtures", "trustless-gateway", "toml", "test-toml-fixture"),
    "crates/iroha_cli/src/commands/connect.rs": ("fixtures", "connect", "json", "test-json-fixture"),
    "crates/iroha_config/examples/sumeragi_v2_context_hash.rs": ("templates", "sumeragi-v2-context", "toml.in", "example-config-template"),
    "xtask/src/sns.rs": ("fixtures", "sns-dashboard", "json", "test-json-fixture"),
    "crates/iroha_data_model/tests/query_json_envelope.rs": ("fixtures", "query-envelope", "json", "test-json-fixture"),
    "crates/build-support/src/bin/clippy_inventory.rs": ("fixtures", "clippy-inventory", "jsonl", "test-json-lines-fixture"),
    "crates/norito/tests/cross_language.rs": ("fixtures", "cross-language-schema", "json", "test-json-fixture"),
    "xtask/src/sm.rs": ("templates", "sm2-config", "toml.in", "generated-config-template"),
    "tools/norito_codegen_exporter/src/main.rs": ("fixtures", "rustdoc-index", "json", "test-json-fixture"),
    "crates/iroha_kagami/src/swarm.rs": ("fixtures", "swarm-peers", "toml", "test-toml-fixture"),
    "crates/iroha_core/src/smartcontracts/ivm.rs": ("fixtures", "ivm-contracts", "ko", "test-kotodama-fixture"),
    "crates/iroha_data_model/tests/ballot_proof_json.rs": ("fixtures", "ballot-proof", "json.in", "test-json-template"),
    "xtask/src/ministry_jury.rs": ("fixtures", "ministry-jury", "json", "test-json-fixture"),
    "xtask/src/ministry.rs": ("fixtures", "ministry-reports", "md", "test-markdown-fixture"),
    "crates/iroha_kagami/src/genesis/validate.rs": ("fixtures", "genesis-validation", "json", "test-json-fixture"),
    "mochi/mochi-core/src/compose.rs": ("fixtures", "compose-manifests", "json", "test-json-fixture"),
}

OVERRIDE = {
    ("xtask/src/kagami_profiles.rs", 457): ("governance-overrides", "toml.in"),
    ("xtask/src/kagami_profiles.rs", 472): ("sorafs-site-bindings", "toml"),
    ("xtask/src/kagami_profiles.rs", 482): ("taira-nexus-overrides", "toml"),
    ("xtask/src/kagami_profiles.rs", 493): ("taira-mcp-overrides", "toml"),
    ("xtask/src/kagami_profiles.rs", 504): ("taira-offline-overrides", "toml"),
    ("xtask/src/kagami_profiles.rs", 531): ("node-config", "toml.in"),
    ("xtask/src/kagami_profiles.rs", 611): ("docker-compose", "yml.in"),
    ("xtask/src/kagami_profiles.rs", 665): ("readme", "md.in"),
    ("xtask/src/kagami_profiles.rs", 835): ("stub-genesis", "json"),
    ("mochi/mochi-core/src/bootstrap.rs", 218): ("typescript-config", "ts.in"),
    ("mochi/mochi-core/src/bootstrap.rs", 247): ("rust-config", "rs.in"),
    ("mochi/mochi-core/src/bootstrap.rs", 282): ("kotlin-config", "kt.in"),
    ("crates/iroha_torii/src/sorafs/evidence_viewer_api.rs", 1697): ("viewer", "html"),
    ("crates/iroha_torii/src/sorafs/evidence_viewer_api.rs", 1732): ("viewer", "css"),
    ("mochi/mochi-ui-egui/src/gui.rs", 2866): ("nexus-config", "toml"),
    ("mochi/mochi-ui-egui/src/gui.rs", 3139): ("kagami-override", "sh"),
    ("mochi/mochi-ui-egui/src/gui.rs", 13806): ("multisig-policy", "json.in"),
    ("mochi/mochi-ui-egui/src/gui.rs", 16047): ("kagami", "sh"),
    ("mochi/mochi-ui-egui/src/gui.rs", 16076): ("noop", "sh"),
    ("mochi/mochi-core/src/supervisor.rs", 4617): ("kagami", "sh.in"),
    ("mochi/mochi-core/src/supervisor.rs", 4731): ("kagami-override", "sh.in"),
    ("mochi/mochi-core/src/torii.rs", 8322): ("metrics-response", "prom"),
}

@dataclass
class Entry:
    source: str
    start: int
    end: int
    expected: str
    token: bytes
    payload: bytes
    before: bytes
    after: bytes
    category: str
    asset: str
    asset_bytes: bytes
    final: bytes | None
    replacement: bytes

    @property
    def sha(self) -> str:
        return hashlib.sha256(self.payload).hexdigest()


def slug(value: str) -> str:
    value = value.lower().replace("_", "-")
    value = re.sub(r"[^a-z0-9]+", "-", value).strip("-")
    parts = value.split("-")
    while len("-".join(parts)) > 44 and len(parts) > 3:
        parts.pop(0)
    return "-".join(parts)


def load_entries() -> list[Entry]:
    prelim = []
    for row in SPEC_TEXT.splitlines():
        source, line_range, expected = row.split()
        start, end = (int(v) for v in line_range.split("-"))
        raw = (ROOT / source).read_bytes()
        lines = raw.splitlines(keepends=True)
        token = b"".join(lines[start - 1:end])
        begin = token.find(b'r#"')
        finish = token.rfind(b'"#')
        if begin < 0 or finish <= begin:
            raise SystemExit(f"raw literal boundary shifted: {source}:{start}-{end}")
        payload = token[begin + 3:finish]
        actual = hashlib.sha256(payload).hexdigest()
        if not actual.startswith(expected):
            raise SystemExit(
                f"raw literal hash shifted: {source}:{start}-{end}: {expected} != {actual}"
            )
        kind, topic, extension, category = META[source]
        text_before = b"".join(lines[:start - 1]).decode("utf-8")
        function = "module"
        matches = list(re.finditer(r"\b(?:async\s+)?fn\s+([A-Za-z0-9_]+)", text_before))
        if matches:
            function = matches[-1].group(1)
        base, extension = OVERRIDE.get((source, start), (slug(function), extension))
        prelim.append((source, start, end, expected, token, payload, token[:begin], token[finish + 2:], category, kind, topic, base, extension))

    counts = Counter((p[0], p[9], p[10], p[11], p[12]) for p in prelim)
    seen = Counter()
    result = []
    for source, start, end, expected, token, payload, before, after, category, kind, topic, base, extension in prelim:
        key = (source, kind, topic, base, extension)
        seen[key] += 1
        if counts[key] > 1:
            base = f"{base}-{seen[key]:02d}"
        trailing_lf = payload.endswith(b"\n")
        if trailing_lf:
            asset_bytes = payload
            final = None
            asset_name = f"{base}.{extension}"
        else:
            prefix, separator, final = payload.rpartition(b"\n")
            if not separator:
                raise SystemExit(f"single-line no-LF literal must be excluded: {source}:{start}")
            asset_bytes = prefix + separator
            asset_name = f"{base}-prefix.{extension}"
        asset_path = (Path(source).parent / kind / topic / "v1" / asset_name).as_posix()
        relative = os.path.relpath(asset_path, Path(source).parent).replace(os.sep, "/")
        if final is None:
            expression = f'include_str!("{relative}")'.encode()
        else:
            if b'"#' in final or b"\n" in final:
                raise SystemExit(f"unsafe final line delimiter: {source}:{start}")
            expression = b'concat!(include_str!("' + relative.encode() + b'"), r#"' + final + b'"#)'
        replacement = before + expression + after
        if replacement.count(b"\n") != 1 or not replacement.endswith(b"\n"):
            raise SystemExit(f"replacement is not exactly one Rust line: {source}:{start}")
        if asset_bytes + (final or b"") != payload:
            raise SystemExit(f"decoded byte parity failed in plan: {source}:{start}")
        result.append(Entry(source, start, end, expected, token, payload, before, after, category, asset_path, asset_bytes, final, replacement))

    assets = [entry.asset for entry in result]
    if len(assets) != len(set(assets)):
        raise SystemExit("planned asset path collision")
    return result


def manifest_patch(entries: list[Entry]) -> bytes:
    header = "source\tstart\tend\tcategory\tliteral_sha256\tasset_path\toriginal_trailing_lf\tasset_mode\tasset_sha256\tinline_final_sha256\n"
    rows = [header]
    for e in entries:
        rows.append("\t".join([
            e.source, str(e.start), str(e.end), e.category, e.sha, e.asset,
            "1" if e.payload.endswith(b"\n") else "0",
            "full" if e.final is None else "prefix-plus-inline-final",
            hashlib.sha256(e.asset_bytes).hexdigest(),
            "-" if e.final is None else hashlib.sha256(e.final).hexdigest(),
        ]) + "\n")
    content = "".join(rows)
    out = ["*** Begin Patch\n", f"*** Add File: {MANIFEST.name}\n"]
    out.extend("+" + line for line in content.splitlines(keepends=True))
    out.append("*** End Patch\n")
    return "".join(out).encode()


def repo_patch(entries: list[Entry]) -> bytes:
    for e in entries:
        if (ROOT / e.asset).exists():
            raise SystemExit(f"planned asset already exists: {e.asset}")
    if (ROOT / ".gitattributes").exists():
        raise SystemExit(".gitattributes collision")
    out = bytearray(b"*** Begin Patch\n")
    grouped: dict[str, list[Entry]] = {}
    for entry in entries:
        grouped.setdefault(entry.source, []).append(entry)
    for source, source_entries in grouped.items():
        out += f"*** Update File: {source}\n".encode()
        for e in source_entries:
            out += b"@@\n"
            for line in e.token.splitlines(keepends=True):
                out += b"-" + line
            out += b"+" + e.replacement
    for e in entries:
        if not e.asset_bytes.endswith(b"\n"):
            raise SystemExit(f"apply_patch asset does not end LF: {e.asset}")
        out += f"*** Add File: {e.asset}\n".encode()
        for line in e.asset_bytes.splitlines(keepends=True):
            out += b"+" + line
    out += b"*** Add File: .gitattributes\n"
    roots = sorted({str(Path(e.asset).parent) for e in entries})
    for root in roots:
        out += f"+/{root}/** text eol=lf\n".encode()
    out += b"*** End Patch\n"
    return bytes(out)


def report(entries: list[Entry]) -> None:
    gross = sum(e.end - e.start + 1 for e in entries)
    net = gross - len(entries)
    full = sum(e.final is None for e in entries)
    prefix = len(entries) - full
    max_replacement = max((len(e.replacement.rstrip(b"\n")), e.source, e.start) for e in entries)
    print(f"sites={len(entries)} gross={gross} replacement={len(entries)} net={net} full={full} prefix={prefix}")
    print(f"asset_bytes={sum(len(e.asset_bytes) for e in entries)} asset_lf={sum(e.asset_bytes.count(b'\n') for e in entries)}")
    print(f"literal_bytes={sum(len(e.payload) for e in entries)} literal_lf={sum(e.payload.count(b'\n') for e in entries)}")
    print(f"max_replacement_line={max_replacement}")


def main() -> None:
    entries = load_entries()
    command = sys.argv[1] if len(sys.argv) > 1 else "report"
    if command == "manifest-patch":
        sys.stdout.buffer.write(manifest_patch(entries))
    elif command == "repo-patch":
        sys.stdout.buffer.write(repo_patch(entries))
    elif command == "report":
        report(entries)
    else:
        raise SystemExit(f"unknown command: {command}")


if __name__ == "__main__":
    main()
