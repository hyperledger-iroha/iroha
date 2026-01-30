---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: daea1194d781913c3482625652da78341c6072fbf4983baab4a868cc8877e389
source_last_modified: "2025-11-14T04:43:21.733324+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Gateway & DNS キックオフ・ランブック

このポータル版は、以下の正規ランブックを反映しています。
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)
Decentralized DNS & Gateway ワークストリームの運用ガードレールをまとめ、
ネットワーク、Ops、ドキュメントのリードが 2025-03 のキックオフ前に
自動化スタックをリハーサルできるようにします。

## スコープと成果物

- DNS (SF-4) と gateway (SF-5) のマイルストーンを結び付けるため、
  ホストの決定的導出、resolver ディレクトリのリリース、TLS/GAR 自動化、
  証跡の収集をリハーサルします。
- キックオフの入力 (アジェンダ、招待、出席トラッカー、GAR テレメトリ
  スナップショット) を最新の担当割り当てと同期させます。
- ガバナンスレビュー向けに監査可能なアーティファクトバンドルを作成します:
  resolver ディレクトリのリリースノート、gateway プローブのログ、
  適合性ハーネスの出力、Docs/DevRel のサマリー。

## 役割と責任

| ワークストリーム | 責任 | 必要なアーティファクト |
|------------------|------|--------------------------|
| Networking TL (DNS スタック) | 決定的ホスト計画の維持、RAD ディレクトリリリースの実行、resolver テレメトリ入力の公開。 | `artifacts/soradns_directory/<ts>/`、`docs/source/soradns/deterministic_hosts.md` の差分、RAD メタデータ。 |
| Ops Automation Lead (gateway) | TLS/ECH/GAR 自動化ドリルの実行、`sorafs-gateway-probe` の実行、PagerDuty フックの更新。 | `artifacts/sorafs_gateway_probe/<ts>/`、プローブ JSON、`ops/drill-log.md` のエントリ。 |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh` の実行、フィクスチャのキュレーション、Norito self-cert バンドルのアーカイブ。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | 議事録の記録、設計プリリード + 付録の更新、ポータルでの証跡サマリー公開。 | 更新済み `docs/source/sorafs_gateway_dns_design_*.md` とロールアウトノート。 |

## 入力と前提条件

- 決定的ホスト仕様 (`docs/source/soradns/deterministic_hosts.md`) と resolver
  アテステーションの足場 (`docs/source/soradns/resolver_attestation_directory.md`)。
- gateway アーティファクト: オペレーター手順書、TLS/ECH 自動化ヘルパー、
  direct-mode ガイダンス、`docs/source/sorafs_gateway_*` 配下の self-cert workflow。
- Tooling: `cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh`、および CI ヘルパー
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- 秘密情報: GAR リリースキー、DNS/TLS の ACME 資格情報、PagerDuty の routing key、
  resolver 取得用の Torii 認証トークン。

## 事前チェックリスト

1. 参加者とアジェンダを確認し、
   `docs/source/sorafs_gateway_dns_design_attendance.md` を更新して
   現行アジェンダ (`docs/source/sorafs_gateway_dns_design_agenda.md`) を配布します。
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` や
   `artifacts/soradns_directory/<YYYYMMDD>/` などのアーティファクトルートを準備します。
3. フィクスチャ (GAR manifests、RAD proofs、gateway conformance bundles) を更新し、
   `git submodule` の状態が最新リハーサルタグに一致することを確認します。
4. シークレット (Ed25519 リリースキー、ACME アカウントファイル、PagerDuty トークン)
   の有無と vault の checksums 一致を確認します。
5. テレメトリ対象 (Pushgateway エンドポイント、GAR Grafana ボード) を
   ドリル前にスモークテストします。

## 自動化リハーサル手順

### 決定的ホストマップと RAD ディレクトリリリース

1. 提案された manifests に対して決定的ホスト導出ヘルパーを実行し、
   `docs/source/soradns/deterministic_hosts.md` とのドリフトが無いことを確認します。
2. resolver ディレクトリバンドルを生成します:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. 出力されたディレクトリ ID、SHA-256、出力パスを
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` とキックオフ議事録に記録します。

### DNS テレメトリの取得

- `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging` を使って
  resolver 透明性ログを ≥10 分間 tail します。
- Pushgateway メトリクスをエクスポートし、NDJSON スナップショットを
  run ID ディレクトリにアーカイブします。

### Gateway 自動化ドリル

1. TLS/ECH プローブを実行します:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 適合性ハーネス (`ci/check_sorafs_gateway_conformance.sh`) と self-cert ヘルパー
   (`scripts/sorafs_gateway_self_cert.sh`) を実行し、Norito アテステーションバンドルを更新します。
3. PagerDuty/Webhook イベントを収集し、自動化パスが end-to-end で動くことを証明します。

### 証跡パッケージング

- `ops/drill-log.md` にタイムスタンプ、参加者、プローブハッシュを記録します。
- run ID ディレクトリにアーティファクトを保存し、Docs/DevRel 議事録に
  エグゼクティブサマリーを掲載します。
- キックオフレビュー前にガバナンスチケットへ証跡バンドルをリンクします。

## セッション進行と証跡引き渡し

- **モデレーターのタイムライン:**
  - T-24 h — Program Management が `#nexus-steering` にリマインダーとアジェンダ/出席スナップショットを投稿。
  - T-2 h — Networking TL が GAR テレメトリスナップショットを更新し、`docs/source/sorafs_gateway_dns_design_gar_telemetry.md` に差分を記録。
  - T-15 m — Ops Automation がプローブ準備を確認し、`artifacts/sorafs_gateway_dns/current` にアクティブ run ID を記録。
  - 通話中 — モデレーターが本ランブックを共有し、ライブスクリブを割り当て; Docs/DevRel がアクションアイテムを記録します。
- **議事録テンプレート:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` の骨子 (ポータル bundle にもミラー) を
  コピーし、各セッションで埋めたものをコミットします。参加者、決定事項、
  アクションアイテム、証跡ハッシュ、未解決リスクを含めてください。
- **証跡アップロード:** リハーサルの `runbook_bundle/` ディレクトリを zip 化し、
  レンダリング済み議事録 PDF を添付し、議事録 + アジェンダに SHA-256 ハッシュを記録。
  その後 `s3://sora-governance/sorafs/gateway_dns/<date>/` にアップロード完了したら
  ガバナンス reviewers の alias に通知します。

## 証跡スナップショット (2025 年 3 月キックオフ)

ロードマップと議事録で参照される最新の rehearsal/live アーティファクトは
`s3://sora-governance/sorafs/gateway_dns/` バケットにあります。以下のハッシュは
カノニカル manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) を反映します。

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - バンドル tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - 議事録 PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ライブワークショップ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(アップロード待ち: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel が PDF 反映後に SHA-256 を追記します。)_

## 関連資料

- [Gateway operations playbook](./operations-playbook.md)
- [SoraFS 観測性計画](./observability-plan.md)
- [分散 DNS & Gateway トラッカー](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
