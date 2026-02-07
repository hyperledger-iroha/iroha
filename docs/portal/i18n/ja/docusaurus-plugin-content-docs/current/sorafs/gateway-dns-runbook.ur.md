---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS ٯیٹ وے اور DNS کک آف رن بک

یہ پورٹل کاپی کینونیکل رنبک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) میں ہے۔
分散型 DNS およびゲートウェイ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ
نیٹ ورکنگ، آپس، اور ڈاکیومنٹیشن لیڈز 2025-03 کے کک آف سے پہلے آٹومیشن اسٹیکやあ
شقیق کر سکیں۔

## ありがとうございます

- DNS (SF-4) とゲートウェイ (SF-5) のマイルストーンと決定的なホストの導出
  リゾルバー ディレクトリのリリース、TLS/GAR 自動化、証拠のキャプチャ、セキュリティ
- 会議 (議題、招待、出席追跡、GAR テレメトリ スナップショット)
  オーナーの割り当てを確認する
- アーティファクト バンドルの詳細: リゾルバー ディレクトリ
  リリース ノート、ゲートウェイ プローブ ログ、適合ハーネス出力、ドキュメント/DevRel の概要

## زمہ داریاں

| और देखें और देखेंアーティファクト |
|-----------|-----------|---------------------|
|ネットワーキング TL (DNS スタック) |確定的ホスト計画 RAD ディレクトリのリリース リゾルバー テレメトリ入力| `artifacts/soradns_directory/<ts>/`、`docs/source/soradns/deterministic_hosts.md` 差分、RAD メタデータ|
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR 自動化ドリル `sorafs-gateway-probe` PagerDuty フック| `artifacts/sorafs_gateway_probe/<ts>/`、プローブ JSON、`ops/drill-log.md` エントリ|
| QA ギルドおよびツーリング WG | `ci/check_sorafs_gateway_conformance.sh` フィクスチャ Norito 自己証明書バンドル| `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel |分数 デザインの事前読み + 付録 説明 資料 証拠の概要 説明| `docs/source/sorafs_gateway_dns_design_*.md` ロールアウト ノート|

## ان پٹس اور پری ریکوائرمنٹس

- 確定的なホスト仕様 (`docs/source/soradns/deterministic_hosts.md`) およびリゾルバー
  認証スキャフォールディング (`docs/source/soradns/resolver_attestation_directory.md`)。
- ゲートウェイ アーティファクト: オペレータ ハンドブック、TLS/ECH 自動化ヘルパー、ダイレクト モード ガイダンス
  自己証明書ワークフロー `docs/source/sorafs_gateway_*` の説明
- ツーリング: `cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh`、CI ヘルパー
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- シークレット: GAR リリース キー、DNS/TLS ACME 認証情報、PagerDuty ルーティング キー
  リゾルバーが Torii 認証トークンを取得します

## پری فلائٹ چیک لسٹ

1. `docs/source/sorafs_gateway_dns_design_attendance.md` 会議の議題
   議題 (`docs/source/sorafs_gateway_dns_design_agenda.md`) の議題 (`docs/source/sorafs_gateway_dns_design_agenda.md`) の議題
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/`
   `artifacts/soradns_directory/<YYYYMMDD>/` アーティファクト ルーツ تیار کریں۔
3. フィクスチャ (GAR マニフェスト、RAD プルーフ、ゲートウェイ適合バンドル)
   یقینی بنائیں کہ `git submodule` کی حالت تازہ ترین リハーサル タグ سے میچ کرتی ہے۔
4. シークレット (Ed25519 リリース キー、ACME アカウント ファイル、PagerDuty トークン)
   ボールトのチェックサム
5. テレメトリ ターゲット (プッシュゲートウェイ エンドポイント、GAR Grafana ボード) の訓練と煙テスト

## آٹومیشن リハーサルの手順

### 決定論的ホスト マップと RAD ディレクトリのリリース

1. マニフェストは、決定論的ホスト導出ヘルパーをマニフェストします。
   تصدیق کریں کہ `docs/source/soradns/deterministic_hosts.md` کے مقابلے میں کوئی ドリフト نہیں۔
2. リゾルバー ディレクトリ バンドルの例:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ディレクトリ ID、SHA-256、出力パス
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` キックオフ時間 میں ریکارڈ کریں۔

### DNS テレメトリのキャプチャ

- リゾルバー透明度ログ ≥ 10 の末尾:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway メトリクスのエクスポート、NDJSON スナップショット、実行 ID ディレクトリ、およびエクスポート

### ゲートウェイ自動化訓練

1. TLS/ECH プローブ:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 適合ハーネス (`ci/check_sorafs_gateway_conformance.sh`) 自己証明書ヘルパー
   (`scripts/sorafs_gateway_self_cert.sh`) چلائیں تاکہ Norito 構成証明バンドル ہو۔
3. PagerDuty/Webhook イベントをエンドツーエンドでキャプチャする

### 証拠の梱包

- `ops/drill-log.md` タイムスタンプ、参加者、プローブ ハッシュ、およびデータ
- 実行 ID ディレクトリ アーティファクト ドキュメント/DevRel 議事録 概要概要
- キックオフレビュー - ガバナンスチケット - 証拠バンドル - キックオフレビュー

## 証拠の引き渡し- **モデレーターのタイムライン:**
  - T-24 h — プログラム管理 `#nexus-steering` リマインダー + 議題/出席スナップショット پوسٹ کرے۔
  - T-2 h — ネットワーキング TL GAR テレメトリ スナップショット ریفریش کر کے `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں デルタ ریکارڈ کرے۔
  - T-15 m — Ops Automation プローブの準備状況
  - 司会者 — 司会者 司会者 司会者 ライブ スクライブ 割り当てDocs/DevRel インライン アクション アイテム
- **議事録テンプレート:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` スケルトン کاپی کریں (ポータル バンドル میں بھی ہے) اور ہر سیشن کے لیے ایک مکمل インスタンス コミット کریں۔出席者名簿、意思決定、アクションアイテム、証拠ハッシュ、未解決のリスク、リスク
- **証拠のアップロード:** リハーサル `runbook_bundle/` ディレクトリ レンダリングされた議事録 PDF 添付 議事録 + 議題 SHA-256 ハッシュ アップロード ガバナンス審査員別名 کو ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## 証拠のスナップショット (2025 年 3 月キックオフ)

ロードマップ 分数 時間 リハーサル/ライブ アーティファクト
`s3://sora-governance/sorafs/gateway_dns/` バケット میں ہیں۔ハッシュ
正規マニフェスト (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) を反映します。

- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - バンドル tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - 議事録 PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ライブ ワークショップ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(アップロード保留中: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF آنے پر SHA-256 شامل کرے گا۔)_

## 関連資料

- [ゲートウェイ操作プレイブック](./operations-playbook.md)
- [SoraFS 可観測性計画](./observability-plan.md)
- [分散型 DNS およびゲートウェイ トラッカー](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)