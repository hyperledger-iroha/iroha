---
lang: ja
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 影響評価ツール (MINFO-4b)

ロードマップ参照: **MINFO‑4b — 影響評価ツール。**  
所有者: ガバナンス評議会 / 分析

このメモでは、現在使用されている `cargo xtask ministry-agenda impact` コマンドについて説明します。
住民投票パケットに必要な自動化されたハッシュファミリーの差分を生成します。の
このツールは、検証されたアジェンダ評議会の提案、重複したレジストリ、および
オプションの拒否リスト/ポリシーのスナップショットにより、レビュー担当者がどの拒否リスト/ポリシーを正確に確認できるようになります。
既存のポリシーと衝突する新しいフィンガープリント、およびエントリの数
各ハッシュ ファミリが貢献します。

## 入力

1. **議題提案。** 続く 1 つ以上のファイル
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md)。
   `--proposal <path>` を使用して明示的に渡すか、コマンドを指定します。
   `--proposal-dir <dir>` 経由のディレクトリとそのパス下のすべての `*.json` ファイル
   が含まれています。
2. **レジストリの重複 (オプション)** 一致する JSON ファイル
   `docs/examples/ministry/agenda_duplicate_registry.json`。紛争とは、
   `source = "duplicate_registry"` で報告されています。
3. **ポリシー スナップショット (オプション)。** すべてのポリシーをリストする軽量のマニフェスト
   指紋は GAR/省庁の方針によってすでに施行されています。ローダーは次のことを期待しています
   以下に示すスキーマ (「
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   完全なサンプルについては):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

`hash_family:hash_hex` フィンガープリントがプロポーザル ターゲットと一致するエントリはすべて、
`source = "policy_snapshot"` で報告され、`policy_id` が参照されます。

## 使用法

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

追加の提案は、`--proposal` フラグを繰り返すか、
住民投票バッチ全体を含むディレクトリを指定します。

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out` が省略された場合、このコマンドは生成された JSON を標準出力に出力します。

## 出力

この報告書は署名済みの成果物です (国民投票パケットの下に記録してください)
`artifacts/ministry/impact/` ディレクトリ) は次の構造になります。

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

この JSON を中立的な概要と一緒にすべての国民投票関係書類に添付します。
パネリスト、陪審員、ガバナンス監視員は、爆発の正確な半径を確認できます。
それぞれの提案。出力は確定的 (ハッシュ ファミリによってソート) であり、安全に使用できます。
CI/ランブックに含める。重複したレジストリまたはポリシーのスナップショットが変更された場合、
投票が開始される前にコマンドを再実行し、更新されたアーティファクトを添付します。

> **次のステップ:** 生成された影響レポートを
> [`cargo xtask ministry-panel packet`](referendum_packet.md) したがって、
> `ReferendumPacketV1` ドシエには、ハッシュファミリーの内訳と
> 検討中の提案の詳細な競合リスト。