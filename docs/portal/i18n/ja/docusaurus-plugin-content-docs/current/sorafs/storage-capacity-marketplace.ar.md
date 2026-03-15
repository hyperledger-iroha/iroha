---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
タイトル: سوق سعة التخزين في SoraFS
サイドバーラベル: ニュース
説明: SF-2c の開発、開発、開発、開発、開発。
---

:::note ノート
هذه الصفحة تعكس `docs/source/sorafs/storage_capacity_marketplace.md`。最高のパフォーマンスを見せてください。
:::

# سوق سعة التخزين في SoraFS (مسودة SF-2c)

يقدّم بند خارطة الطريق SF-2c سوقا محكوما حيث يعلن مزودو التخزين عن سعة ملتزم بها،ログインしてください。最高のパフォーマンスを見せてください。

## ああ

- التعبير عن التزامات سعة المزودين (إجمالي البايتات، حدود لكل lan، تاريخ الانتهاء) بصيغةソラネット、Torii をご覧ください。
- ピンを固定し、ステークを固定します。
- 稼働時間 (稼働時間) を確認してください。
- 最高のパフォーマンスを見せてください。

## عرض المزيد

| और देखेंああ |ログイン | ログイン
|----------|---------------|----------|
| `CapacityDeclarationV1` | حمولة Norito تصف معرف المزود، دعم ملف تعريف chunker، GiB الملتزمة، حدود خاصة بـ LANE،ステーキングを賭けてください。 | المخطط + المدقق في `sorafs_manifest::capacity`。 |
| `ReplicationOrder` | CID マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェストSLA。 | Norito Torii + スマート コントラクト。 |
| `CapacityLedger` |オンチェーン/オフチェーンのセキュリティを確認します。 |スマート コントラクトとスタブ、オフチェーンのスナップショット、スタブ。 |
| `MarketplacePolicy` |賭け金を賭けてください。 |構成構造体 `sorafs_manifest` + وثيقة حوكمة。 |

### المخططات المنفذة (الحالة)

## いいえ

### 1. いいえ。

|ああ |所有者 |重要 |
|------|----------|------|
| `CapacityDeclarationV1` と `ReplicationOrderV1` と `CapacityTelemetryV1`。 |ストレージ チーム / ガバナンス | Noritoログインしてください。 |
|パーサー + バリデーター `sorafs_manifest`。 |ストレージチーム | ID はステークです。 |
|メタデータはチャンカー `min_capacity_gib` に含まれます。 |ツーリングWG | ساعد العملاء على فرض الحد الأدنى من متطلبات العتاد لكل ملف تعريف. |
|認証済み `MarketplacePolicy` 認証済み。 |ガバナンス評議会 |ドキュメントのポリシーのデフォルト。 |

#### تعريفات المخطط (منفذة)

- يلتقط `CapacityDeclarationV1` は、チャンカーを処理します。機能は、チャンカーを処理します。レーンとメタデータを確認してください。ステーク ステーク エイリアス レーン レーン レーンを処理します。 GiB أحادية.【crates/sorafs_manifest/src/capacity.rs:28】
- يربط `ReplicationOrderV1` は、SLA のマニフェストをマニフェストします。検証バリデータはチャンカーを処理します。期限は Torii です。 [crates/sorafs_manifest/src/capacity.rs:301]
- يعبر `CapacityTelemetryV1` スナップショット الحقبة (GiB المعلنة مقابل المستخدمة، عدادات النسخ، نسب uptime/PoR) التي最高です。 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- ヘルパー (`CapacityMetadataEntry` و`PricingScheduleV1` レーン/割り当て/SLA) のヘルプ。 CI ダウンストリーム ツールの開発。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` スナップショット `/v1/sorafs/capacity/state` 手数料台帳خلف Norito JSON حتمي.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 管理者は、管理者と管理者を処理します。 فحوص نطاق التليمترية حتى تظهر الانحدارات فوراً في CI.【crates/sorafs_manifest/src/capacity.rs:792】
- 情報: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` 仕様、Norito ペイロード、base64 BLOB、JSONフィクスチャを `/v1/sorafs/capacity/declare` و`/v1/sorafs/capacity/telemetry` で確認してください。 محلي.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 参考治具 `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`。### 2. いいえ。

|ああ |所有者 |重要 |
|------|----------|------|
| Torii `/v1/sorafs/capacity/declare` و`/v1/sorafs/capacity/telemetry` و`/v1/sorafs/capacity/orders` Norito JSON。 | Torii チーム | حاكاة منطق التحقق؛ Norito JSON ヘルパー。 |
|スナップショットと `CapacityDeclarationV1` のメタデータ、オーケストレーター、ゲートウェイ。 |ツーリング WG / オーケストレーター チーム | تمديد `provider_metadata` スコア レーン。 |
|オーケストレーター/ゲートウェイの割り当てとフェールオーバーが必要です。 |ネットワーキング TL / ゲートウェイ チーム |スコアボード ビルダーのスコアボード ビルダー。 |
| CLI: `sorafs_cli`、`capacity declare`、`capacity telemetry`、`capacity orders import`。 |ツーリングWG | JSON スコア + スコアボード。 |

### 3. いいえ。

|ああ |所有者 |重要 |
|------|----------|------|
| إقرار `MarketplacePolicy` (الحد الأدنى لـ stake، مضاعفات العقوبة، وتواتر التدقيق)。 |ガバナンス評議会 |ドキュメントを参照してください。 |
|議会の宣言。 |ガバナンス評議会 / スマートコントラクトチーム | Norito イベント + マニフェストの取り込み。 |
| تنفيذ جدول العقوبات (تخفيض الرسوم، スラッシュ للبوند) المرتبط بانتهاكات SLA المقاسة عن بعد。 |ガバナンス評議会 / 財務 | مواءمة ذلك مع مخرجات التسوية في `DealEngine`。 |
|ログインしてください。 |ドキュメント / ガバナンス |紛争ランブック + CLI ヘルパー。 |

### 4. 答えは次のとおりです。

|ああ |所有者 |重要 |
|------|----------|------|
| Torii と `CapacityTelemetryV1` を取り込みます。 | Torii チーム | GiB 時間の PoR と稼働時間。 |
| `sorafs_node` は、SLA と SLA を組み合わせたものです。 |ストレージチーム |チャンカーを処理します。 |
|支払い額: 支払い額 + XOR 支払い額 支払い額台帳。 |財務/保管チーム |ディールエンジン/財務省輸出。 |
|ダッシュボード/アラート (バックログの取り込み)。 |可観測性 | Grafana は SF-6/SF-7 です。 |

- Torii `/v1/sorafs/capacity/telemetry` و`/v1/sorafs/capacity/state` (JSON + Norito) スナップショット台帳を管理するための元帳を管理する【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- 評価 `PinProviderRegistry` 評価 - 評価 - 評価 - エンドポイント評価CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) のエイリアス、ハッシュ エイリアス。
- スナップショット `CapacityTelemetrySnapshot` スナップショット `metering` レビュー Prometheus エクスポートGrafana セキュリティ `docs/source/grafana_sorafs_metering.json` セキュリティ セキュリティ `docs/source/grafana_sorafs_metering.json` セキュリティ セキュリティ GiB 時間 والرسوم nano-SORA المتوقعة والامتثال لـ SLA في الوقت الحقيقي.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- メータリング スムージングのスナップショット `smoothed_gib_hours` و`smoothed_por_success_bps` の EMA の測定支払い額を計算します。【crates/sorafs_node/src/metering.rs:401】

### 5. 重要なこと

|ああ |所有者 |重要 |
|------|----------|------|
| تعريف حمولة `CapacityDisputeV1` (証拠証明)。 |ガバナンス評議会 | Norito + دقق。 |
| CLI لتقديم の論争 (証拠)。 |ツーリングWG |証拠をハッシュする。 |
| SLA (تصعيد تلقائي إلى 紛争)。 |可観測性 | عتبات تنبيه وخطافات حوكمة. |
| توثيق playbook الإلغاء (فترة سماح، إخلاء بيانات 固定)。 |ドキュメント / ストレージ チーム |オペレーターのランブック。 |

## متطلبات الاختبار و CI

- ログインしてください (`sorafs_manifest`)。
- 計算: 計算 → 計算 → 計量 → 支払い。
- ワークフロー CI 情報 (توسيع `ci/check_sorafs_fixtures.sh`)。
- レジストリ API (محاكاة 10k مزود و100k أمر)。

## าาาาาารรรรรรรรรรรรรยยย- ダッシュボード:
  - ログインしてください。
  - バックログの記録。
  - SLA (稼動時間、PoR)。
  - 最高のパフォーマンス。
- アラート:
  - 重要な問題を解決してください。
  - SLA を参照してください。
  - ニュース、ニュース、ニュース。

## خرجات التوثيق

- ニュース、ニュース、ニュース、ニュース、ニュース。
- ニュース、ニュース、ニュース、ニュース、ニュース。
- API を使用して、API を使用します。
- 市場マーケットプレイス。

## قائمة تحقق جاهزية GA

**SF-2c** يبوّب إطلاق الإنتاج على أدلة ملموسة عبر المحاسبة ومعالجةありがとうございます。最高のパフォーマンスを見せてください。

### 夜間の会計と XOR 調整
- スナップショット、XOR 台帳、およびスナップショット:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ينهي المساعد التنفيذ بكود غير صفري عند وجود تسويات أو غرامات مفقودة/زائدة، ويصدر Prometheus テキストファイル。
- テスト `SoraFSCapacityReconciliationMismatch` (テスト `dashboards/alerts/sorafs_capacity_rules.yml`)
  يطلق عندما تشير مقاييس 和解 إلى فجوات؛ حت علومات موجودة تحت
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- JSON とハッシュ `docs/examples/sorafs_capacity_marketplace_validation/`
  ガバナンス パケット。

### 論争と証拠の隠蔽
- 紛争 `sorafs_manifest_stub capacity dispute` (回答:
  `cargo test -p sorafs_car --test capacity_cli`) ペイロードが 1 つあります。
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` حزم العقوبات
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) 論争とスラッシュを意味します。
- البع `docs/source/sorafs/dispute_revocation_runbook.md` لالتقاط الأدلة والتصعيد؛ストライキを実行してください。

### プロバイダーのオンボーディングと終了スモーク テスト
- アーティファクトの作成/編集、`sorafs_manifest_stub capacity ...` وأعد تشغيل ختبارات CLI قبل الإرسال (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)。
- Torii (`/v1/sorafs/capacity/declare`) と `/v1/sorafs/capacity/state` と Grafana。 `docs/source/sorafs/capacity_onboarding_runbook.md` です。
- アーティファクトと和解 `docs/examples/sorafs_capacity_marketplace_validation/`。

## いいえ

1. SF-2b (アドミッション ポリシー) - 市場マーケットプレイス。
2. تنفيذ طبقة المخطط + السجل (هذا المستند) قبل تكامل Torii.
3. 計量パイプラインの評価。
4. 測定: ステージングの測定。

ロードマップを確認してください。ロードマップ (コントロール プレーン、メータリング、およびメータリング) 機能が完了しました。