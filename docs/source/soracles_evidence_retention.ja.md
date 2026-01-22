---
lang: ja
direction: ltr
source: docs/source/soracles_evidence_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3121ec15db54ca27fab0f0e11a5780839b69eb46c7fa70994d97a6116cc28cf1
source_last_modified: "2026-01-04T10:50:53.653513+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soracles_evidence_retention.md -->

# Soracles Evidence Retention & GC

ロードマップ OR-14 は、oracle 証跡アーティファクトに対する監査可能な
retention ポリシーと、オンチェーン hash を変更せずに古いバンドルを
剪定するツールを求めている。本メモはデフォルトと、`bundle.json`
メタデータ（バンドル生成時刻）と共に追加された
`iroha soracles evidence-gc` ヘルパーを記録する。

## Retention ポリシー

- **Observations, reports, connector responses, telemetry:** デフォルトで
  **180 日**保持。2 四半期分の ingestion/audit トレイルを保持しつつ
  オペレーターのストレージを制限する。ガバナンスポリシーに応じて
  GC フラグで期間を調整する。
- **Dispute evidence:** **365 日**保持。再開された争議や遅い attestation
  でも検証できる。GC ヘルパーは dispute アーティファクトを含むバンドルを
  デフォルトで **`--dispute-retention-days`** の長いウィンドウで保持する。
- **オンチェーン hash は不変。** `FeedEventRecord` は `evidence_hashes`
  のみ保持するため、アーティファクト剪定は台帳状態に影響しない。
  ただし削除時には GC レポートをガバナンスバンドルへ添付し、
  監査トレイルを維持する。
- **バンドル時刻:** `bundle.json` に `generated_at_unix`（秒）を追加。
  GC はこのタイムスタンプを優先し、無い場合は `mtime` にフォールバックする。

## Garbage collection の実行

新しい CLI ヘルパーを使い、保持ウィンドウを超えたバンドルを剪定し、
必要なら参照されないファイルを削除する:

```bash
iroha soracles evidence-gc \
  --root artifacts/soracles \
  --retention-days 180 \
  --dispute-retention-days 365 \
  --prune-unreferenced \
  --report artifacts/soracles/gc_report.json
```

フラグ:
- `--root`: バンドルフォルダ（`bundle.json` を含む）のルートディレクトリ。
- `--retention-days`: `generated_at_unix`（無い場合は `mtime`）がウィンドウを
  超えたバンドルを削除する。
- `--dispute-retention-days`: dispute 証拠を含むバンドルの最小保持期間
  （デフォルト **365**）。観測/レポートより長く保持するために使う。
- `--prune-unreferenced`: `bundle.json` に参照されない `artifact_root` 配下の
  ファイルを削除する。
- `--dry-run`: 削除せずに削除対象を報告する。
- `--report`: JSON サマリの出力先（デフォルト `<root>/gc_report.json`）。

レポートは `removed_bundles`, `pruned_files`, `skipped_bundles`,
`retained_bundles`, `bytes_freed`, retention ウィンドウ、dry-run の有無を
記録する。SoraFS へアップロードする際、残存証拠と併せてレポートを添付し、
ガバナンスレビューが剪定理由を追跡できるようにする。

## オンチェーンのクリーンアップルール

- オンチェーンの evidence hashes は永久。GC は off-chain コピーのみ削除する。
- 剪定時は元の `evidence_hashes` を変更せず、GC レポートを関連する
  ガバナンスパケットまたは SoraFS バンドルに添付する。これにより
  イミュータビリティ要件を満たしつつストレージを節約する。
- 剪定後にバンドルを再公開する場合は、同じ hash 名のアーティファクトを
  維持する（または削除済み hash のマニフェストを含める）ことで、
  検証者が意図的な欠落を理解できるようにする。
