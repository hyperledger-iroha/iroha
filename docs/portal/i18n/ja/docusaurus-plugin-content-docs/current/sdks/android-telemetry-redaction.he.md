---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4686e3bddcd6e78c3d61898ce3993f3d1635ce27170ed66d4d74096507d4b0e7
source_last_modified: "2026-01-30T16:38:43+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /sdks/android-telemetry
lang: ja
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Android テレメトリ・レダクション計画
sidebar_label: Android テレメトリ
slug: /sdks/android-telemetry
---

:::note 正式な情報源
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android テレメトリ・レダクション計画 (AND7)

## 適用範囲

本ドキュメントは、ロードマップ項目 **AND7** で要求される Android SDK 向けテレメトリ・レダクション方針と有効化アーティファクトをまとめる。モバイル計測を Rust ノードのベースラインに整合させつつ、デバイス固有のプライバシー保証を考慮する。結果は 2026 年 2 月の SRE ガバナンスレビュー向けの事前資料となる。

目的:

- Android が発行するすべてのシグナルを整理し、共有オブザーバビリティ基盤 (OpenTelemetry トレース、Norito エンコードログ、メトリクス出力) に到達するものを把握する。
- Rust ベースラインと差分のあるフィールドを分類し、レダクション/保持の制御を文書化する。
- サポートチームがレダクション関連アラートに決定論的に対応できるよう、有効化とテスト作業を明確化する。

## シグナル一覧 (ドラフト)

計画中の計測をチャネル別に整理した。フィールド名は Android SDK テレメトリスキーマ (`org.hyperledger.iroha.android.telemetry.*`) に従う。オプションフィールドは `?` で示す。

| シグナルID | チャネル | 主なフィールド | PII/PHI 分類 | レダクション / 保持 | 備考 |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | トレース span | `authority_hash`, `route`, `status_code`, `latency_ms` | authority は公開情報; route に秘匿なし | エクスポート前に authority をハッシュ (`blake2b_256`) し、7日保持 | Rust の `torii.http.request` を反映; ハッシュによりモバイル alias のプライバシーを確保。 |
| `android.torii.http.retry` | イベント | `route`, `retry_count`, `error_code`, `backoff_ms` | なし | レダクションなし; 30日保持 | 決定論的 retry 監査に使用; Rust と同一フィールド。 |
| `android.pending_queue.depth` | Gauge メトリクス | `queue_type`, `depth` | なし | レダクションなし; 90日保持 | Rust の `pipeline.pending_queue_depth` に一致。 |
| `android.keystore.attestation.result` | イベント | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | alias (派生), デバイスメタデータ | alias を決定論的ラベルに置換し、ブランドを enum bucket 化 | AND2 のアテステーション準備に必須; Rust ノードはデバイスメタデータを出力しない。 |
| `android.keystore.attestation.failure` | カウンタ | `alias_label`, `failure_reason` | alias レダクション後はなし | レダクションなし; 90日保持 | chaos ドリルを支援; `alias_label` はハッシュ済み alias から派生。 |
| `android.telemetry.redaction.override` | イベント | `override_id`, `actor_role_masked`, `reason`, `expires_at` | 役割は運用 PII | マスク済み役割カテゴリを出力; 365日保持 + 監査ログ | Rust には存在せず、override はサポート経由で申請。 |
| `android.telemetry.export.status` | カウンタ | `backend`, `status` | なし | レダクションなし; 30日保持 | Rust エクスポータのステータスカウンタとパリティ。 |
| `android.telemetry.redaction.failure` | カウンタ | `signal_id`, `reason` | なし | レダクションなし; 30日保持 | Rust の `streaming_privacy_redaction_fail_total` を反映。 |
| `android.telemetry.device_profile` | Gauge メトリクス | `profile_id`, `sdk_level`, `hardware_tier` | デバイスメタデータ | 粗い bucket (SDK major, hardware tier) を出力; 30日保持 | OEM 詳細を出さずにパリティダッシュボードを構築。 |
| `android.telemetry.network_context` | イベント | `network_type`, `roaming` | キャリアは PII になり得る | `carrier_name` を完全に削除; 他のフィールドは 7日保持 | `ClientConfig.networkContextProvider` がサニタイズ済みスナップショットを提供し、ネットワーク種別 + roaming を出力; ダッシュボードは Rust の `peer_host` のモバイル版として扱う。 |
| `android.telemetry.config.reload` | イベント | `source`, `result`, `duration_ms` | なし | レダクションなし; 30日保持 | Rust の config reload span を反映。 |
| `android.telemetry.chaos.scenario` | イベント | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | デバイスプロファイルは bucket 化 | `device_profile` と同様; 30日保持 | AND7 の準備に必要な chaos リハーサルで記録。 |
| `android.telemetry.redaction.salt_version` | Gauge メトリクス | `salt_epoch`, `rotation_id` | なし | レダクションなし; 365日保持 | Blake2b salt のローテーションを追跡; Android の hash epoch が Rust とずれるとアラート。 |
| `android.crash.report.capture` | イベント | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | クラッシュ指紋 + プロセスメタデータ | `crash_id` を共有 salt でハッシュし、watchdog 状態を bucket 化、stack frames を削除; 30日保持 | `ClientConfig.Builder.enableCrashTelemetryHandler()` 呼び出しで自動有効化; デバイス識別情報を出さずにパリティダッシュボードを提供。 |
| `android.crash.report.upload` | カウンタ | `crash_id`, `backend`, `status`, `retry_count` | クラッシュ指紋 | ハッシュ済み `crash_id` を再利用しステータスのみ出力; 30日保持 | `ClientConfig.crashTelemetryReporter()` または `CrashTelemetryHandler.recordUpload` から送信し、他のテレメトリと同じ Sigstore/OLTP 保証を共有。 |

### 実装フック

- `ClientConfig` は `setTelemetryOptions(...)`/`setTelemetrySink(...)` 経由で manifest 派生のテレメトリを流し、`TelemetryObserver` を自動登録することで authority ハッシュと salt メトリクスを特別な observer なしで送信する。`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` と `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/` 配下のクラスを参照。
- アプリは `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` を呼び出し、反射ベースの `AndroidNetworkContextProvider` を登録できる。これは実行時に `ConnectivityManager` を参照し、`android.telemetry.network_context` を発行しつつコンパイル時の Android 依存を増やさない。
- ユニットテスト `TelemetryOptionsTests` と `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) がハッシュ処理と ClientConfig 統合フックを保護し、manifest のリグレッションを即検出する。
- enablement kit/labs は擬似コードではなく具体 API を参照するよう更新され、本書と runbook を出荷 SDK と整合させる。

> **運用メモ:** owner/status のワークシートは `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` にあり、AND7 の各 checkpoint で本表と合わせて更新する。

## パリティ allowlist とスキーマ diff ワークフロー

ガバナンスは、Android のエクスポートが Rust サービスが意図的に出力する識別子を漏らさないよう二重 allowlist を要求する。本セクションは runbook (`docs/source/android_runbook.md` §2.3) を反映しつつ、AND7 計画を自己完結させる。

| 区分 | Android エクスポータ | Rust サービス | 検証フック |
|----------|-------------------|---------------|-----------------|
| authority/route コンテキスト | `authority`/`alias` を Blake2b-256 でハッシュし、Torii の生 hostname を削除してから出力; `android.telemetry.redaction.salt_version` を出して salt ローテーションを証明。 | Torii hostnames と peer IDs を完全出力して相関。 | `docs/source/sdk/android/readiness/schema_diffs/` の最新 diff で `android.torii.http.request` と `torii.http.request` を比較し、`scripts/telemetry/check_redaction_status.py` で salt epoch を確認。 |
| デバイス/署名者の識別 | `hardware_tier`/`device_profile` を bucket 化し、controller alias をハッシュ、シリアル番号は出力しない。 | validator `peer_id`、controller `public_key`、queue hashes をそのまま出力。 | `docs/source/sdk/mobile_device_profile_alignment.md` と整合し、`java/iroha_android/run_tests.sh` 内で alias hashing テストを実行し、labs で queue‑inspector 出力を保存。 |
| ネットワークメタデータ | `network_type` + `roaming` のみ出力し、`carrier_name` を削除。 | peer hostname/TLS メタデータを保持。 | `readiness/schema_diffs/` に diff を保存し、Grafana の “Network Context” ウィジェットに carrier 文字列が出たらアラート。 |
| override/chaos エビデンス | `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` を役割マスク付きで出力。 | override approval はマスクなし、chaos 専用 span はなし。 | drill 後に `docs/source/sdk/android/readiness/and7_operator_enablement.md` を確認し、override token と chaos artefact が Rust イベントと並ぶことを保証。 |

ワークフロー:

1. manifest/exporter 変更後に `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` を実行し、JSON を `docs/source/sdk/android/readiness/schema_diffs/` に配置する。
2. diff を上表に照らして確認。Android が Rust 専用フィールドを出す (またはその逆) 場合は AND7 readiness bug を起票し、本計画と runbook を更新する。
3. 週次 ops レビューで `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` を実行し、salt epoch と diff のタイムスタンプを readiness ワークシートへ記録する。
4. 逸脱は `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` に記録し、ガバナンスパケットがパリティ判断を反映できるようにする。

> **スキーマ参照:** 正準フィールド ID は `android_telemetry_redaction.proto` 由来 (Android SDK ビルド時に Norito descriptor とともに生成)。スキーマは `authority_hash`、`alias_label`、`attestation_digest`、`device_brand_bucket`、`actor_role_masked` を公開している。

`authority_hash` は Torii authority の 32 バイト digest。`attestation_digest` はアテステーション文の指紋を保持し、`device_brand_bucket` は Android 生ブランド文字列を承認済み enum (`generic`, `oem`, `enterprise`) にマップする。`actor_role_masked` は生ユーザ ID ではなく役割カテゴリ (`support`, `sre`, `audit`) を持つ。

### クラッシュテレメトリ輸出の整合

クラッシュテレメトリは Torii ネットワーク信号と同じ OpenTelemetry エクスポータ/プロベナンスパイプラインを共有し、重複エクスポータに関するガバナンス follow‑up を解消した。クラッシュハンドラは `android.crash.report.capture` にハッシュ済み `crash_id` (Blake2b-256、`android.telemetry.redaction.salt_version` で追跡される salt を使用)、プロセス状態 bucket、ANR watchdog のサニタイズ済みメタデータを格納する。スタックトレースは端末内に留まり、`has_native_trace` と `anr_watchdog_bucket` に要約してから送信するため、PII や OEM 文字列は流出しない。

クラッシュのアップロードは `android.crash.report.upload` カウンタを生成し、SRE がユーザやスタックを知ることなく backend 信頼性を監査できる。両シグナルが Torii エクスポータを再利用するため、Sigstore 署名、保持ポリシー、アラートフックが AND7 と同一になる。サポート runbook は Android と Rust の evidence bundle 間でハッシュ済み crash id を相関でき、専用パイプラインは不要。

`ClientConfig.Builder.enableCrashTelemetryHandler()` でハンドラを有効化し、テレメトリ options/sinks を設定後に `ClientConfig.crashTelemetryReporter()` (または `CrashTelemetryHandler.recordUpload`) を再利用して同一署名パイプラインで backend 結果を出力する。

## Rust ベースラインとの差分ポリシー

Android と Rust のテレメトリ方針の差分と緩和策。

| 区分 | Rust ベースライン | Android ポリシー | 緩和 / 検証 |
|----------|---------------|----------------|-------------------------|
| authority/peer 識別子 | authority を平文で出力 | `authority_hash` (Blake2b-256, salt ローテーション) | 共有 salt を `iroha_config.telemetry.redaction_salt` 経由で公開; パリティテストでサポート向けの逆引きを保証。 |
| host/network メタデータ | ノードの hostname/IP を出力 | ネットワーク種別 + roaming のみ | ネットワーク健全性ダッシュボードは hostname ではなく可用性カテゴリを使用。 |
| デバイス特性 | N/A (サーバ側) | bucket 化プロフィール (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | chaos リハーサルでマッピングを検証; 詳細が必要な場合のエスカレーションは runbook に記載。 |
| レダクション override | 未サポート | 手動 override トークンを Norito ledger に保存 (`actor_role_masked`, `reason`) | 署名済みリクエストが必要; 監査ログは1年保持。 |
| アテステーショントレース | SRE によるサーバアテステーションのみ | SDK がサニタイズ済み要約を出力 | Rust の attestation validator とハッシュ照合; alias のハッシュ化で漏洩を防止。 |

検証チェックリスト:

- 各シグナルのレダクションユニットテストで、エクスポータ送信前にハッシュ/マスクを確認する。
- Rust と共有の schema diff ツールを nightly で実行し、フィールドパリティを確認する。
- chaos rehearsal スクリプトで override フローと監査ログを検証する。

## 実装タスク (SRE ガバナンス前)

1. **在庫確認** — 上表を Android SDK の実装フックと Norito スキーマ定義でクロスチェック。Owners: Android Observability TL, LLM。
2. **テレメトリスキーマ diff** — Rust メトリクスに対して共有 diff ツールを実行し、SRE レビュー用パリティアーティファクトを生成。Owner: SRE privacy lead。
3. **Runbook ドラフト (2026-02-03 完了)** — `docs/source/android_runbook.md` が override の end‑to‑end フロー (Section 3) と拡張エスカレーション/役割責任 (Section 3.1) を記載し、CLI ヘルパー、インシデント証跡、chaos スクリプトをガバナンス方針に結び付けた。Owners: LLM + Docs/Support。
4. **Enablement コンテンツ** — 2026 年 2 月セッションのブリーフィング資料、ラボ手順、理解度チェック質問を準備。Owners: Docs/Support Manager, SRE enablement team。

## Enablement フローと runbook フック

### 1. ローカル + CI スモーク

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` が Torii sandbox を起動し、SoraFS の canonical multi‑source fixture を再生 ( `ci/check_sorafs_orchestrator_adoption.sh` に委譲 )、Android テレメトリを合成投入する。
  - 交通生成は `scripts/telemetry/generate_android_load.py` が担当し、`artifacts/android/telemetry/load-generator.log` に request/response transcript を記録し、ヘッダー、パス override、dry‑run を尊重する。
  - helper は SoraFS の scoreboard/summary を `${WORKDIR}/sorafs/` にコピーし、モバイルクライアント投入前に AND7 の multi‑source パリティを示す。
- CI も同じツールを使用: `ci/check_android_dashboard_parity.sh` が `scripts/telemetry/compare_dashboards.py` を実行し、`dashboards/grafana/android_telemetry_overview.json`、Rust 参照ダッシュボード、`dashboards/data/android_rust_dashboard_allowances.json` を比較して `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json` を生成。
- chaos rehearsal は `docs/source/sdk/android/telemetry_chaos_checklist.md` に従う; sample‑env スクリプトと dashboard parity check が AND7 burn‑in 監査向けの “ready” evidence bundle を形成する。

### 2. Override 発行と監査トレイル

- `scripts/android_override_tool.py` は override 発行/取り消しのカノニカル CLI。`apply` が署名済み依頼を取り込み、manifest bundle (`telemetry_redaction_override.to` が既定) を出力し、`docs/source/sdk/android/telemetry_override_log.md` にハッシュ化トークン行を追加する。`revoke` は同じ行に revoke タイムスタンプを追加し、`digest` はガバナンス用のサニタイズ済み JSON snapshot を出力する。
- CLI は Markdown テーブルヘッダがない場合 audit log を更新しない。`docs/source/android_support_playbook.md` のコンプライアンス要件に一致する。`scripts/tests/test_android_override_tool_cli.py` がテーブルパーサ、manifest emitter、エラー処理を保護。
- オペレーターは override 実行時に生成 manifest、更新されたログ抜粋 **および** digest JSON を `docs/source/sdk/android/readiness/override_logs/` に添付。ログは 365 日保持。

### 3. 証跡の取得と保持

- すべての rehearsal/インシデントは `artifacts/android/telemetry/` に構造化 bundle を作成:
  - `generate_android_load.py` の負荷生成 transcript と集計カウンタ。
  - `ci/check_android_dashboard_parity.sh` が生成する dashboard diff (`android_vs_rust-<stamp>.json`) と allowlist hash。
  - override log delta (override 付与時)、対応 manifest、更新 digest JSON。
- SRE burn‑in レポートはこれらアーティファクトと `android_sample_env.sh` がコピーした SoraFS scoreboard を参照し、テレメトリ hash → dashboards → override 状態の決定論的チェーンを提供する。

## SDK 間デバイスプロファイル整合

ダッシュボードは Android の `hardware_tier` を `docs/source/sdk/mobile_device_profile_alignment.md` で定義される canonical `mobile_profile_class` に変換し、AND7 と IOS7 が同じコホートを比較できるようにする:

- `lab` — `hardware_tier = emulator` として出力され、Swift の `device_profile_bucket = simulator` と一致。
- `consumer` — `hardware_tier = consumer` (SDK major のサフィックス付き) として出力され、Swift の `iphone_small`/`iphone_large`/`ipad` bucket に集約。
- `enterprise` — `hardware_tier = enterprise` として出力され、Swift の `mac_catalyst` bucket および将来の managed iOS desktop runtimes と整合。

新しい tier は必ず整合ドキュメントと schema diff アーティファクトに追加してからダッシュボードで使用すること。

## ガバナンスと配布

- **Pre-read パッケージ** — 本文書と付録 (schema diff, runbook diff, readiness deck outline) は **2026-02-05** までに SRE ガバナンスメーリングリストへ配布。
- **フィードバックループ** — ガバナンスで得られたコメントは JIRA epic `AND7` に反映され、ブロッカーは `status.md` と Android 週次 stand‑up ノートに記載される。
- **公開** — 承認後、ポリシー概要は `docs/source/android_support_playbook.md` からリンクされ、共通テレメトリ FAQ (`docs/source/telemetry.md`) で参照される。

## 監査とコンプライアンス

- GDPR/CCPA に準拠するため、モバイル購読者データはエクスポート前に除去する; authority hash の salt は四半期ごとにローテートし、共有 secrets vault に保存する。
- enablement アーティファクトと runbook 更新はコンプライアンスレジストリに記録する。
- 四半期レビューで override がクローズドループ (陳腐化アクセスなし) であることを確認する。

## ガバナンス結果 (2026-02-12)

**2026-02-12** の SRE ガバナンスセッションで Android レダクション方針は修正なしで承認された。主要決定 (参照: `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`) :

- **方針承認。** authority ハッシュ、デバイスプロファイルの bucket 化、キャリア名の削除が承認。`android.telemetry.redaction.salt_version` による salt 回転追跡は四半期監査項目となる。
- **検証計画。** unit/integration カバレッジ、夜間 schema diff、四半期 chaos rehearsals を承認。アクション: 各 rehearsal 後に dashboard parity レポートを公開。
- **override ガバナンス。** Norito に記録された override token を 365 日保持で承認。Support engineering が月次 ops sync で override log digest をレビューする。

## フォローアップ状況

1. **デバイスプロファイル整合 (期限 2026-03-01)** — ✅ 完了。`docs/source/sdk/mobile_device_profile_alignment.md` に共有マッピングが定義され、Android `hardware_tier` が canonical `mobile_profile_class` に対応する。

## 次回 SRE ガバナンスブリーフ (Q2 2026)

ロードマップ項目 **AND7** は、次回 SRE ガバナンスで簡潔な Android レダクション pre‑read を求める。本セクションを生きたブリーフとして使い、各会議前に更新すること。

### 準備チェックリスト

1. **証跡バンドル** — 最新 schema diff、ダッシュボードスクリーンショット、override log digest (下表) をエクスポートし、日付付きフォルダ (例: `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) に配置してから招待を送る。
2. **ドリル要約** — 最新の chaos rehearsal log と `android.telemetry.redaction.failure` メトリクスのスナップショットを添付し、Alertmanager の注釈が同一タイムスタンプを指すようにする。
3. **override 監査** — すべてのアクティブ override が Norito レジストリに記録され、会議デッキに要約されていることを確認。期限と該当インシデント ID を含める。
4. **アジェンダ注意** — 会議の 48 時間前に SRE chair に brief リンクを送り、必要な意思決定 (新シグナル、保持変更、override ポリシー更新) をハイライトする。

### 証跡マトリクス

| アーティファクト | 場所 | オーナー | 注記 |
|----------|----------|-------|-------|
| Rust との差分スキーマ | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | 会議の 72 時間以内に生成すること。 |
| Dashboard diff スクリーンショット | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | `sorafs.fetch.*`, `android.telemetry.*`, Alertmanager スナップショットを含める。 |
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | `scripts/android_override_tool.sh digest` を実行 (README 参照) し最新 `telemetry_override_log.md` を対象にする; トークンはハッシュ済み。 |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | KPI summary (stall count, retry ratio, override usage) を添付。 |

### 評議会へのオープン質問

- digest が自動化された今、override 保持期間 365 日を短縮すべきか。
- `android.telemetry.device_profile` は次リリースで共有ラベル `mobile_profile_class` を採用すべきか、それとも Swift/JS SDK の追随を待つべきか。
- Torii Norito-RPC イベントが Android に到達した際、地域データレジデンシの追加ガイダンスが必要か (NRPC‑3 follow‑up)。

### テレメトリスキーマ diff 手順

リリース候補ごと (および Android 計測変更時) に schema diff を少なくとも 1 回実行し、SRE council に最新のパリティアーティファクトと dashboard diff を提供する:

1. 比較対象の Android/Rust テレメトリスキーマをエクスポートする。CI の config は `configs/android_telemetry.json` と `configs/rust_telemetry.json`。
2. `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json` を実行する。
   - 代替としてコミットを渡す (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) と git から直接 config を取得でき、スクリプトがハッシュを artefact に固定する。
3. 生成した JSON を readiness bundle に添付し、`status.md` と `docs/source/telemetry.md` からリンクする。diff は追加/削除フィールドと保持差分を可視化し、監査がツール再実行なしでパリティを確認できる。
4. diff が許容される差異 (例: Android のみの override シグナル) を示した場合は `ci/check_android_dashboard_parity.sh` が参照する allowlist を更新し、理由を diff ディレクトリ README に記載する。

> **アーカイブ規則:** 最新 5 件の diff は `docs/source/sdk/android/readiness/schema_diffs/` に保持し、古いスナップショットは `artifacts/android/telemetry/schema_diffs/` に移動する。これによりガバナンスレビューは常に最新データを参照できる。
