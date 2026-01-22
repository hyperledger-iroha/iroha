---
lang: ja
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Data Availability 脅威モデル自動化 (DA-1)

ロードマップ項目 DA-1 と `status.md` は、`docs/source/da/threat_model.md` と
Docusaurus のミラーに掲載される Norito PDP/PoTR 脅威モデルの要約を生成する、
決定論的な自動化ループを求めています。このディレクトリは、以下が参照する
成果物を保存します。

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model`（`scripts/docs/render_da_threat_model_tables.py` を実行）
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## フロー

1. **レポート生成**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON サマリには、レプリケーション失敗率のシミュレーション値、chunker の
   しきい値、`integration_tests/src/da/pdp_potr.rs` の PDP/PoTR ハーネスで検出された
   ポリシー違反が記録されます。
2. **Markdown テーブル生成**
   ```bash
   make docs-da-threat-model
   ```
   これにより `scripts/docs/render_da_threat_model_tables.py` が実行され、
   `docs/source/da/threat_model.md` と `docs/portal/docs/da/threat-model.md` が更新されます。
3. **成果物のアーカイブ**：JSON レポート（および必要なら CLI ログ）を
   `docs/automation/da/reports/<timestamp>-threat_model_report.json` にコピーします。
   ガバナンス判断が特定の実行に依存する場合は、コミットハッシュとシミュレーターの
   シードを隣接する `<timestamp>-metadata.md` に記載します。

## 証跡の期待事項

- JSON ファイルは git に収まるよう 100 KiB 未満に保ちます。大きなトレースは外部
  ストレージに置き、必要であればメタデータのメモに署名済みハッシュを記載します。
- 各アーカイブファイルにはシード、設定パス、シミュレーターのバージョンを記載し、
  再実行時に正確に再現できるようにします。
- DA-1 の受け入れ基準が進むたびに、`status.md` またはロードマップのエントリから
  アーカイブファイルにリンクし、ハーネスを再実行せずにベースラインを検証できるようにします。

## コミットメント照合（シーケンサー省略）

`cargo xtask da-commitment-reconcile` を使用し、DA の取り込みレシートと
DA コミットメント記録を突合して、シーケンサーの省略や改ざんを検知します。

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito または JSON のレシート、`SignedBlockWire`、`.norito`、または JSON バンドルの
  コミットメントを受け付けます。
- ブロックログに欠けがある、またはハッシュが一致しない場合は失敗します。`--allow-unexpected`
  はレシート集合を意図的に限定したときにブロック側のみのチケットを無視します。
- 出力された JSON をガバナンス/Alertmanager のパケットに添付して省略アラートに利用します。
  デフォルトは `artifacts/da/commitment_reconciliation.json` です。

## 権限監査（四半期アクセスレビュー）

`cargo xtask da-privilege-audit` を使い、DA の manifest/replay ディレクトリ
（および追加パス）をスキャンして、欠落、ディレクトリでない、または world-writable の
エントリを検出します。

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Torii 設定から DA の取り込みパスを読み取り、利用可能な場合は Unix 権限を検査します。
- 欠落/ディレクトリではない/world-writable なパスを検出し、問題がある場合は非ゼロの
  終了コードを返します。
- 署名済み JSON バンドル（既定は `artifacts/da/privilege_audit.json`）を四半期レビューの
  パケットやダッシュボードに添付します。
