---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ゲートウェイのキックオフと DNS のランブック SoraFS

ポータル エスペリャまたはランブック カノニコ エムのコピーを作成します
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
DNS 分散およびゲートウェイのワークストリームをキャプチャするガードレールの操作
ネットワーキング、運用、ドキュメントのパラケ リデレス、ポッサム エンサイア ピリャ デ
自動化の準備は 2025-03 年にキックオフします。

## エスコポとエントレゲイス

- DNS (SF-4) とゲートウェイ (SF-5) の接続
  ホストの決定、リゾルバーのディレクトリのリリース、TLS/GAR の自動化
  証拠のキャプチャ。
- キックオフを開始するマンター オスモス (議題、招集、発表のトラッカー、スナップショット)
  telemetria GAR) 所有者としての究極のアクセス権を取得します。
- 政府の改訂に関する監査資料の束ね: ノート
  リゾルバーのディレクトリのリリース、ゲートウェイのプローブ、ハーネスのログの作成
  Docs/DevRel の適合性を確認します。

## パペイスと責任

|ワークストリーム |責任 |アルテファトス・レケリドス | 写真
|-----------|---------------------|-----------|
|ネットワークTL (スタックDNS) |ホストの計画決定、RAD ディレクトリの実行プログラムのリリース、テレメトリのリゾルバーの公開入力。 | `artifacts/soradns_directory/<ts>/`、`docs/source/soradns/deterministic_hosts.md` の差分、メタデータ RAD。 |
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR を自動化する Executar ドリル、rodar `sorafs-gateway-probe`、PagerDuty の自動フック。 | `artifacts/sorafs_gateway_probe/<ts>/`、プローブ JSON、エントラダス `ops/drill-log.md`。 |
| QA ギルドおよびツーリング WG | Rodar `ci/check_sorafs_gateway_conformance.sh`、curar フィクスチャ、arquivar バンドルの自己証明書 Norito。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel |議事録の取得、設計の事前確認と付録の取得、公開、証拠の公開、ネスト ポータル。 | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` はロールアウトされていません。 |

## エントリーと前提条件

- ホストの決定に関する特定の情報 (`docs/source/soradns/deterministic_hosts.md`)
  o リゾルバーの足場の確立 (`docs/source/soradns/resolver_attestation_directory.md`)。
- ゲートウェイの技術: 手動操作、TLS/ECH 自動化ヘルパー、
  ダイレクト モードおよび自己証明書のワークフローに関するガイダンス `docs/source/sorafs_gateway_*`。
- ツーリング: `cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh`、CI ヘルパー
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- セグレド: GAR のリリース、認証情報 ACME DNS/TLS、PagerDuty のルーティング キー、
  認証トークンは、Torii パラフェッチのリゾルバーを実行します。

## 飛行前のチェックリスト

1. 参加者と議題を確認する
   `docs/source/sorafs_gateway_dns_design_attendance.md` 議題を巡る
   実 (`docs/source/sorafs_gateway_dns_design_agenda.md`)。
2. raizes de artefatos comoを準備する
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. フィクスチャのアチュアライズ (マニフェスト GAR、プロバス RAD、ゲートウェイの適合バンドル)
   `git submodule` は最高のパフォーマンスを発揮します。
4. Verifique segredos (リリース Ed25519、ACME のアーカイブ、PagerDuty のトークン)
   e se batem com チェックサムはボールトを実行します。
5. テレメトリアのファサードスモークテスト番号ターゲット (エンドポイント Pushgateway、ボード GAR Grafana)
   アンテはドリルをします。

## Etapas de ensaio de automatizacao

### ホストとリリースの決定を決定するためのマップ、RAD の管理

1. マニフェストのホスト制御の決定性を決定するための支援者
   Proposto econfirme que nao ha ドリフトエムリラカオ
   `docs/source/soradns/deterministic_hosts.md`。
2. リゾルバーのディレクトリーのバンドルを入手します。

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ID、ディレクトリ、SHA-256 を登録します。
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` キックオフが始まります。

### テレメトリア DNS のキャプチャ

- 顔面尾部は、10 分以上の透明なリゾルバーのログを記録します
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway の OS スナップショットの NDJSON を実行してメトリクスをエクスポートします。
  diretorio do run ID。

### ゲートウェイを自動化するドリル

1. TLS/ECH のプローブを実行します。

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 適合性の高いハーネス (`ci/check_sorafs_gateway_conformance.sh`) e
   o 自己証明書 (`scripts/sorafs_gateway_self_cert.sh`) パラメータのヘルパー
   o バンドルデアスタコス Norito。
3. 自動化機能を使用して PagerDuty/Webhook のイベントをキャプチャする
   ポンタ機能。

### 証拠の証明

- `ops/drill-log.md` com タイムスタンプ、参加者によるプローブのハッシュを取得します。
- 実行 ID および公開および実行の管理者のアルマゼン アーティファト
  Docs/DevRel を行うのに時間がかかります。
- キックオフを行う前に、政府のチケットなしで証拠をまとめてリンクします。

## セッションと証拠の受け渡しの促進- **リーンハ・ド・テンポ・ド・モデレーター:**
  - T-24 時間 — 終了後のプログラム管理と議題/発表のスナップショット `#nexus-steering`。
  - T-2 h — ネットワーキング TL は、`docs/source/sorafs_gateway_dns_design_gar_telemetry.md` のテレメトリ GAR およびレジストラ オス デルタのスナップショットを取得します。
  - T-15 m — Ops Automation は、`artifacts/sorafs_gateway_dns/current` の実行 ID をプローブし、実行を検証します。
  - デュランテ・ア・シャマダ — ああ、モデラードール・コンパルティーリャ・エステ・ランブックの設計者、生の命を守り続けてください。 Docs/DevRel はインラインでキャプチャーします。
- **議事録のテンプレート:** エスケレートのコピー
  `docs/source/sorafs_gateway_dns_design_minutes.md` (バンドルなし
  do portal) e comite uma instancia preenchida por sessao.リストを含める
  参加者、決定、決定、証拠のハッシュ、および保留。
- **証拠のアップロード:** 管理ファイル `runbook_bundle/` を実行します。
  PDF の詳細レンダリング、レジストリ ハッシュ SHA-256 の詳細 +
  アジェンダ、電子デポワ、アヴィス、ガバナンカの査読者別名、Quando OS アップロード
  チェガレム エム `s3://sora-governance/sorafs/gateway_dns/<date>/`。

## 証拠のスナップショット (マルコ 2025 のキックオフ)

究極の技術/ライブ参照にはロードマップがなく、分もありません
ficam バケット `s3://sora-governance/sorafs/gateway_dns/` はありません。 Os ハッシュ アバイショ
エスペルハムまたはマニフェスト カノニコ (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball バンドル: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF 分: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ワークショップ ao vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(保留中のアップロード: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara、SHA-256 quando、PDF renderizado chegar ao バンドル。)_

## 素材関係

- [ゲートウェイを実行するオペレーション プレイブック](./operations-playbook.md)
- [SoraFS](./observability-plan.md) の観測計画
- [DNS 分散型ゲートウェイのトラッカー](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)