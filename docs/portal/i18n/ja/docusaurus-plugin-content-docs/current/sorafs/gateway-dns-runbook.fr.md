---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ゲートウェイと DNS のランブック SoraFS

canonique dans の runbook のコピー デュ ポルテイル リフレートを確認します
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
ワークストリームの DNS 分散化とゲートウェイのギャルド オペレーションをキャプチャする
責任を負い、運用とドキュメントを積み重ねる
前自動化の推進 2025-03。

## ポルテとリブルブル

- DNS (SF-4) とゲートウェイ (SF-5) の派生決定版
  ホテル、リゾルバー レパートリーのリリース、TLS/GAR などの自動化
  ド・プルーヴを捕獲する。
- Garder les entrées duキックオフ (議題、招待状、プレゼンテーション、スナップショット)
  Télémétrie GAR) は、所有者の愛情を共有します。
- Produire un Bundle d’artefacts Auditable pour les reviewers de gouvernance : メモ
  リゾルバーのレパートリーのリリース、ゾンデ ゲートウェイのログ、ハーネスの出撃
  Docs/DevRel に準拠して合成します。

## 役割と責任

|ワークストリーム |責任 |アーティファクトが必要 |
|-----------|-------|------|
|ネットワークTL (パイルDNS) | Hôtes の計画決定の保守、RAD レパートリーのリリースの実行、リゾルバーの入力の発行者。 | `artifacts/soradns_directory/<ts>/`、`docs/source/soradns/deterministic_hosts.md` の差分、RAD のメタドン。 |
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR の自動化の訓練、ランサー `sorafs-gateway-probe`、PagerDuty のフックの実行。 | `artifacts/sorafs_gateway_probe/<ts>/`、プローブの JSON、エントリ `ops/drill-log.md`。 |
| QA ギルドおよびツーリング WG | Lancer `ci/check_sorafs_gateway_conformance.sh`、curer les フィクスチャ、archer les バンドル自己証明書 Norito。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel |議事録、デザインの事前読み取りと付属文書の収集、ポータルの統合と証拠の発行。 | Fichiers `docs/source/sorafs_gateway_dns_design_*.md` のロールアウトに関する記録。 |

## 前菜と前菜

- 仕様決定表 (`docs/source/soradns/deterministic_hosts.md`) など
  スキャフォールディング証明書デゾルバー (`docs/source/soradns/resolver_attestation_directory.md`)。
- アーティファクト ゲートウェイ: manuel opérateur、自動化ヘルパー TLS/ECH、
  ガイダンス ダイレクト モード、ワークフロー自己証明書 `docs/source/sorafs_gateway_*`。
- ツーリング：`cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh`、およびヘルパー CI
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- シークレット: リリース GAR、認証情報 ACME DNS/TLS、ルーティング キー PagerDuty、
  トークン認証 Torii は、リゾルバーからファイルを取得します。

## チェックリスト前編

1. 参加者と当面の議題を確認する
   `docs/source/sorafs_gateway_dns_design_attendance.md` と拡散的な議題
   クーラント (`docs/source/sorafs_gateway_dns_design_agenda.md`)。
2. 工芸品の準備
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` など
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. Rafraîchir les fixtures (マニフェスト GAR、preuves RAD、バンドル de conconité ゲートウェイ) など
   s’assurer que l’état `git submodule` はリハーサルのタグに対応します。
4. 秘密の検証 (リリース Ed25519、ACME の認証、トークン PagerDuty)
   ボールトのチェックサムなどの対応情報。
5. 遠隔喫煙テストのフェア (エンドポイント Pushgateway、ボード GAR Grafana)
   アバントルドリル。

## 自動化のためのリハーサルの練習

### カルト ドットの決定とレパートリーのリリース RAD

1. マニフェストの設定を決定するための実行者
   親密な関係の不在を提案し確認する
   `docs/source/soradns/deterministic_hosts.md`。
2. リゾルバーのバンドルの一般:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. レパートリーの ID、SHA-256 および出撃命令の登録者
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` キックオフまでの残り時間。

### DNS のキャプチャ

- ペンダントの透明度のログを調整する平均 10 分以上
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway のメトリクス エクスポータとスナップショットのアーカイバ NDJSON à côté du
  ランIDのレパートリー。

### ゲートウェイの自動化のドリル

1. TLS/ECH の実行者:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. ランサー ル ハーネス ドゥ コンフォーミテ (`ci/check_sorafs_gateway_conformance.sh`) など
   l’helper self-cert (`scripts/sorafs_gateway_self_cert.sh`) rafraîchir le バンドルを注ぐ
   証明書 Norito。
3. PagerDuty/Webhook を自動化するためのキャプチャー
   試合ごとに機能します。

### パッケージング・デ・プリューブ

- 時間ごとの測定 `ops/drill-log.md` の平均タイムスタンプ、参加者、およびプローブのハッシュ。
- 実行 ID および出版者および総合エグゼクティブのレパートリーとアーティファクトのストッカー
  ドキュメント/DevRel の分数。
- キックオフの前に、事前のチケットと事前のチケットを確認します。

## アニメーション デ セッションとレミーズ デ プリューブ- **中程度のタイムライン:**
  - T-24 時間 — プログラム管理の懸垂下降 + スナップショットの議題/プレゼンス `#nexus-steering`。
  - T-2 h — ネットワーキング TL rafraîchit le snapshot télémétrie GAR et consigne les deltas dans `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`。
  - T-15 m — Ops Automation は、プローブの準備とクリティカル実行 ID および `artifacts/sorafs_gateway_dns/current` を検証します。
  - ペンダント l’appel — Le modérateur Partage ce runbook et assigne unscribe en direct ; Docs/DevRel は、アクション全体をキャプチャします。
- **議事録のテンプレート:** squelette のコピー
  `docs/source/sorafs_gateway_dns_design_minutes.md` (バンドルされたミロワールのコレクション
  du portail) と、セッションごとにインスタンスをコミットします。リストを含める
  参加者、決定、行動、事前の危険と危険な行為。
- **デ プリーヴのアップロード :** リハーサル中のジッパー ル レパートリー `runbook_bundle/`、
  PDF で議事録に参加、ハッシュ SHA-256 を登録 + 議事録を作成
  議題、審査員のエイリアス、アップロードの実行
  責任者は `s3://sora-governance/sorafs/gateway_dns/<date>/` です。

## スナップショット デ プリューブ (2025 年 3 月キックオフ)

アーティファクトのリハーサル/ライブ リファレンスとロードマップと議事録
バケット `s3://sora-governance/sorafs/gateway_dns/` を購入してください。レハッシュ
ci-dessous reflètent leマニフェスト canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - バンドル tarball : `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF 分: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **アトリエライブ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(注意してアップロード: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel SHA-256 を使用して PDF をバンドルします。)_

## マテリエル コネックス

- [プレイブック操作ゲートウェイ](./operations-playbook.md)
- [観測計画 SoraFS](./observability-plan.md)
- [トラッカー DNS 分散化およびゲートウェイ](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)