---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ゲートウェイのキックオフと DNS の SoraFS のランブック

ポータル レポートと Runbook のコピーを作成します。
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
DNS およびゲートウェイ分散化のワークストリームの運用方法を認識する
ネットワーキング、運用ドキュメントをリードするパラケダン エンサヤル ラ ピラ デ
自動化は 2025 年から 2003 年のキックオフまでに開始されます。

## アルカンセ・イ・アントレガブル

- DNS (SF-4) およびゲートウェイ (SF-5) からの派生情報の保存
  ホストの決定、リゾルバーのディレクトリのリリース、TLS/GAR の自動化
  証拠をキャプチャします。
- Mantener los insumos del キックオフ (議題、招待状、追跡記録、スナップショット)
  de telemetría GAR) 所有者の割り当てを確認します。
- 政府の改訂に関する監査対象の成果物をまとめて作成します: ノート
  リゾルバーのディレクトリのリリース、ゲートウェイのソンダのログ、ハーネスのサリダ
  Docs/DevRel に準拠して再開します。

## 役割と責任

|ワークストリーム |責任 | Artefactos requeridos | 写真
|-----------|---------------------|----------------------|
|ネットワークTL (スタックDNS) |ホストの計画決定の管理、RAD ディレクトリの取り出しリリース、リゾルバーのテレメトリ入力の公開。 | `artifacts/soradns_directory/<ts>/`、`docs/source/soradns/deterministic_hosts.md` の差分、メタデータ RAD。 |
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR の自動化のイジェクター ドリル、`sorafs-gateway-probe` の誤り、PagerDuty の実際のフック。 | `artifacts/sorafs_gateway_probe/<ts>/`、JSON のプローブ、`ops/drill-log.md` のエントラダ。 |
| QA ギルドおよびツーリング WG | Ejecutar `ci/check_sorafs_gateway_conformance.sh`、curar フィクスチャ、自己証明書 Norito のアーカイバ バンドル。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel |レジストラの議事録、実際の内容、事前に読んだ内容と付録、およびエステポータルでの証拠の履歴書を公開します。 |実際のアーカイブ `docs/source/sorafs_gateway_dns_design_*.md` はロールアウトされません。 |

## エントリと前提条件

- ホスト決定の詳細 (`docs/source/soradns/deterministic_hosts.md`)
  リゾルバーの認証 (`docs/source/soradns/resolver_attestation_directory.md`)。
- ゲートウェイのアーティファクト: 手動操作、TLS/ECH 自動化ヘルパー、
  ダイレクト モードと自己証明書 `docs/source/sorafs_gateway_*` の設定。
- ツーリング: `cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh`、y CI ヘルパー
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- 秘密: GAR のリリース情報、ACME DNS/TLS の認証情報、PagerDuty のルーティング キー、
  Torii パラオブジェクト リゾルバーの認証トークン。

## 事前チェックリスト

1. アシスタントと実際の議題を確認する
   `docs/source/sorafs_gateway_dns_design_attendance.md` y circulando la agenda
   ビジェンテ (`docs/source/sorafs_gateway_dns_design_agenda.md`)。
2. 加工品の準備をする
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. Refresca フィクスチャ (マニフェスト GAR、プルエバス RAD、ゲートウェイの適合バンドル)
   `git submodule` は、最も重要なタグと一致しています。
4. Verifica Secretos (Ed25519 のリリース、ACME のアーカイブ、PagerDuty のトークン)
   デフォルトのチェックサムは偶然に一致します。
5. 遠隔測定ターゲットの危険有害性煙テスト (プッシュゲートウェイのエンドポイント、テーブルロ GAR Grafana)
   アンテ・デル・ドリル。

## 自動化の手順

### RAD のホストとリリースのディレクトリを決定するためのマップ

1. マニフェストのホスト制御セットの決定を支援するツール
   プロプエスト・イ・コンファームア・ケ・ノ・ハヤ・ドリフト・リスペクト・デ
   `docs/source/soradns/deterministic_hosts.md`。
2. ディレクトリリゾルバーのバンドルの一般:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ディレクトリのレジストラ ID、SHA-256 のアクセス許可
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` キックオフ終了まで。

### テレメトリ DNS のキャプチャ

- 危険信号の尾部の透明な記録の持続時間 ≥10 分
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway のメトリクスとスナップショットのアーカイブを NDJSON からエクスポート
  実行 ID のディレクトリ。

### ゲートウェイの自動化のドリル

1. 安全な TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 適合性ハーネス排出 (`ci/check_sorafs_gateway_conformance.sh`)
   自己証明書ヘルパー (`scripts/sorafs_gateway_self_cert.sh`) の参照
   アテスタシオン Norito のバンドル。
3. 自動化デモ用の PagerDuty/Webhook イベントのキャプチャ
   極端な機能。

### 証拠の証明

- Actualiza `ops/drill-log.md` のタイムスタンプ、参加者およびプローブのハッシュ。
- 実行 ID と公開再開のディレクトリを保護します
  Docs/DevRel の詳細。
- 改訂前のチケットの証拠の束
  デルキックオフ。

## セッションの促進と証拠の収集- **リネア デ ティエンポ デル モデラドール:**
  - T-24 h — プログラム管理の公開記録 + 議題/支援のスナップショット `#nexus-steering`。
  - T-2 h — ネットワーキング TL は、`docs/source/sorafs_gateway_dns_design_gar_telemetry.md` のテレメトリ GAR およびレジストラ ロス デルタのスナップショットを参照します。
  - T-15 m — Ops Automation は、`artifacts/sorafs_gateway_dns/current` の実行 ID を記述したプローブの準備を確認します。
  - Durante la llamada — エル・モード・コンパルテ・エステ・ランブックと、生体内での安全管理を担当。 Docs/DevRel は、アクションでアイテムをキャプチャします。
- **Plantilla de minutas:** Copia el esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (タンビエンエスペジャードとエルバンドル)
  デル ポータル) は、完全なインスタンスを作成するための委員会です。リストを含める
  支援、決定、アクションの項目、証拠のハッシュ、保留中の情報。
- **証拠資料:** Comprime el directoryio `runbook_bundle/` del ensayo、
  補助的な PDF の詳細レンダリング、登録ハッシュ SHA-256 en las minutas +
  議題とルエゴ通知、別名レビュー担当者、ゴベルナンザ クアンド ラス カルガス
  アテリセン en `s3://sora-governance/sorafs/gateway_dns/<date>/`。

## 証拠のスナップショット (2025 年のキックオフ)

ロードマップと最終結果の最終的な成果物や生産の参考資料
ヴィベンエンエルバケット`s3://sora-governance/sorafs/gateway_dns/`。ロス・ハッシュ・アバホ
リフレジャン エル マニフェスト カノニコ (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - バンドルのタールボール: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF ドキュメント: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **生体内ワークショップ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(保留中の情報: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara el SHA-256 cuando el PDF renderizado llegue al Bundle.)_

## 素材関係

- [ゲートウェイのオペレーション プレイブック](./operations-playbook.md)
- [SoraFS の観測計画](./observability-plan.md)
- [DNS 分散ゲートウェイのトラッカー](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)