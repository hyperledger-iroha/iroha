---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска ゲートウェイと DNS SoraFS

Эта копия в портале отражает канонический ранбук в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
分散型 DNS およびゲートウェイのガードレールを使用します。
ネットワーキング、オペレーション、およびその他の機能を備えたソリューションを提供します。
2025-03 年のキックオフです。

## 成果物

- DNS (SF-4) とゲートウェイ (SF-5) の接続を確立します。
  ビデオ、ビデオ リゾルバー、TLS/GAR および
  ありがとうございます。
- キックオフ - 日程 (議題、招待状、出席追跡、スナップショット)
  телеметрии GAR) синхронизированными с последними назначениями の所有者。
- ガバナンスレビュー担当者によるバンドルのリリース: リリースノート
  リゾルバ、ゲートウェイ プローブ、適合ハーネスなど
  ドキュメント/開発リリース。

## Роли и ответственности

|ワークストリーム | Ответственности | Требуемые артефакты |
|-----------|------|----------|
|ネットワーキング TL (DNS スタック) | RAD ディレクトリのリリース、リゾルバーのサポート。 | `artifacts/soradns_directory/<ts>/`、差分 `docs/source/soradns/deterministic_hosts.md`、RAD メタデータ。 |
|運用自動化リード (ゲートウェイ) | TLS/ECH/GAR のドリル、`sorafs-gateway-probe`、PagerDuty のフック。 | `artifacts/sorafs_gateway_probe/<ts>/`、プローブ JSON、`ops/drill-log.md` の詳細。 |
| QA ギルドおよびツーリング WG | `ci/check_sorafs_gateway_conformance.sh`、フィクスチャ、Norito 自己証明書バンドルを参照してください。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|ドキュメント / DevRel |議事録、デザインの既読 + 付録、証拠の概要。 | `docs/source/sorafs_gateway_dns_design_*.md` およびロールアウト ノート。 |

## 前提条件

- Спецификация детерминированных хостов (`docs/source/soradns/deterministic_hosts.md`) и
  証明書リゾルバ (`docs/source/soradns/resolver_attestation_directory.md`)。
- ゲートウェイ: オペレーター ハンドブック、TLS/ECH 自動化ヘルパー、
  ガイダンス、ダイレクト モード、自己証明書ワークフロー、`docs/source/sorafs_gateway_*`。
- ツーリング: `cargo xtask soradns-directory-release`、
  `cargo xtask sorafs-gateway-probe`、`scripts/telemetry/run_soradns_transparency_tail.sh`、
  `scripts/sorafs_gateway_self_cert.sh` および CI ヘルパー
  (`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`)。
- シークレット: GAR、DNS/TLS ACME 認証情報、ルーティング キー PagerDuty、
  Torii 認証トークンがリゾルバーをフェッチします。

## 飛行前チェックリスト

1. 議題、議題
   `docs/source/sorafs_gateway_dns_design_attendance.md` と разослав текущую
   アジェンダ (`docs/source/sorafs_gateway_dns_design_agenda.md`)。
2. Подготовьте корни артефактов, например
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` および
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. フィクスチャ (GAR マニフェスト、RAD プルーフ、バンドル適合ゲートウェイ)
   убедитесь、что состояние `git submodule` соответствует последнему リハーサル タグ。
4. 秘密情報 (Ed25519 リリース キー、ACME アカウント ファイル、PagerDuty トークン)
   チェックサムとボールトの両方を実行します。
5. スモーク テスト テレメトリ ターゲット (プッシュゲートウェイ エンドポイント、GAR Grafana ボード)
   ドリル。

## Шаги репетиции автоматизации

### Детерминированная карта хостов и release каталога RAD

1. ヘルパー детерминированной деривации хостов на предложенном наборе
   マニフェストとドリフト отсутствите относительно
   `docs/source/soradns/deterministic_hosts.md`。
2. バンドル クラスのリゾルバー:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зафиксируйте напечатанный ID каталога、SHA-256 и выходные пути внутри
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` と в 分のキックオフ。

### DNS を使用する

- テールリゾルバーの透明性ログは 10 分以上かかります
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- Pushgateway と NDJSON スナップショットを使用して、最高のパフォーマンスを実現します。
  実行ID。

### ゲートウェイのドリル

1. TLS/ECH プローブ:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 適合ハーネス (`ci/check_sorafs_gateway_conformance.sh`) および
   自己証明書ヘルパー (`scripts/sorafs_gateway_self_cert.sh`) の説明
   Norito 証明書バンドル。
3. PagerDuty/Webhook のエンドツーエンドの接続
   そうですね。

### Упаковка доказательств

- `ops/drill-log.md` は、タイムスタンプ、ハッシュ プローブをサポートします。
- 実行 ID とエグゼクティブ サマリーを確認する
  ドキュメント/DevRel 分。
- キックオフ レビューで証拠バンドルとガバナンスを統合します。

## Модерация сессии и передача доказательств- **タイムライン日時:**
  - T-24 h — プログラム管理とスナップショットの議題/出席、`#nexus-steering`。
  - T-2 h — ネットワーキング TL のスナップショット、GAR とデルタ、`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`。
  - T-15 m — Ops Automation のプローブと実行 ID は `artifacts/sorafs_gateway_dns/current` です。
  - Во время звонка — Модератор делится этим ранбуком и назначает live scribe; Docs/DevRel のアクション アイテムが表示されます。
- **所要時間:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (ポータル バンドルのポータル)
  и коммитьте заполненный экземпляр на каждую сессию. Включите список участников,
  アクションアイテム、ハッシュ証拠、открытые риски。
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из リハーサル、
  PDF 議事録、議事録 + 議題の SHA-256 ハッシュ、
  ガバナンスレビューアーのエイリアス после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`。

## Снимок доказательств (キックオフ марта 2025)

リハーサル/ライブのスケジュール、ロードマップ、分、バケットの確認
`s3://sora-governance/sorafs/gateway_dns/`。 Хэли ниже отражают канонический
マニフェスト (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **予行演習 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball バンドル: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - 議事録 PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **ライブ ワークショップ — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(アップロード保留中: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавит SHA-256 PDF とバンドル。)_

## Связанные материалы

- [オペレーション プレイブックとゲートウェイ](./operations-playbook.md)
- [План наблюдаемости SoraFS](./observability-plan.md)
- [DNS およびゲートウェイ](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)