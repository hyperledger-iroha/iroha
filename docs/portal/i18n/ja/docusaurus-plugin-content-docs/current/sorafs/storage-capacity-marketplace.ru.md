---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
タイトル: Маркетплейс емкости хранения SoraFS
サイドバーラベル: Маркетплейс емкости
説明: SF-2c のバージョン、レプリケーション順序、ガバナンス フック。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/storage_capacity_marketplace.md`。 Держите обе копии синхронными, пока активна устаревлая документация.
:::

# Маркетплейс емкости хранения SoraFS (черновик SF-2c)

SF-2c のロードマップ、市場、プロバイダーの決定
複製オーダーと手数料
Апропорционально предоставленной доступности。成果物を完成させる
実行可能なアクションが必要です。

## Цели

- プロバイダーの接続 (общие байты, лимиты по lane, срок действия)
  ガバナンス、SoraNet および Torii を確認してください。
- プロバイダーのピン、ステーク、ポリシー、
  Похраняя детерминированное поведение。
- Измерять доставку хранения (успех репликации、稼働時間、証明 целостности) и
  料金は別途かかります。
- 失効と異議申し立て、プロバイダーの要求を解決します。
  必要があります。

## Доменные концепции

| Концепция | Описание | Первичный の成果物 |
|----------|---------------|----------|
| `CapacityDeclarationV1` | Norito ペイロード、ID プロバイダー、チャンカー、GiB、レーン、ヒント、価格設定、ステーキング コミットメント、およびステーキング コミットメント。 | Схема + валидатор в `sorafs_manifest::capacity`. |
| `ReplicationOrder` |ガバナンス、CID マニフェスト、プロバイダー、SLA の管理。 | Norito は、Torii + API を使用します。 |
| `CapacityLedger` |オンチェーン/オフチェーン レジストリ、レプリケーション注文、手数料。 |オフチェーン スタブとスナップショットを確認します。 |
| `MarketplacePolicy` | Политика ガバナンス、определяющая минимальный ステーク、требования аудита и кривые bolтрафов。 |構成構造体と `sorafs_manifest` + ガバナンス。 |

### Реализованные схемы (статус)

## Разбиение работ

### 1. Слой схем и реестра

| Задача |所有者 | Примечания |
|------|----------|------|
| `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1` を確認してください。 |ストレージ チーム / ガバナンス | Norito;さまざまな機能を備えています。 |
|パーサー + バリデーターを `sorafs_manifest` で取得します。 |ストレージチーム | Обеспечить монотонные ID、ограничения емкости、требования по stake。 |
|メタデータ チャンカー значением `min_capacity_gib` для каждого профиля. |ツーリングWG |ハードウェアが必要です。 |
| Подготовить документ `MarketplacePolicy`、описывающий 入場ガードレール и график зтрафов。 |ガバナンス評議会 |ポリシーのデフォルトをドキュメントで確認します。 |

#### Определения схем (реализованы)- `CapacityDeclarationV1` фиксирует подписанные обязательства емкости для каждого プロバイダー、включая канонические はチャンカー、ссылки на 機能を処理します。キャップ、レーン、ヒント、価格設定、メタデータ。ステーク、ハンドル、エイリアス、キャップ、レーン、合計、キャップмонотонный учет GiB.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` マニフェストは、ガバナンス、ガバナンス、SLA および割り当てをマニフェストします。チャンカー、プロバイダー、期限を指定して、Torii レジストリを処理します。順番です。【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` описывает スナップショット эпох (заявленные vs использованные GiB、счетчики репликации、проценты uptime/PoR)、которые手数料がかかります。 Проверки границ удерживают использование внутри деклараций, а проценты - в пределах 0-100%。【crates/sorafs_manifest/src/capacity.rs:476】
- ヘルパー (`CapacityMetadataEntry`、`PricingScheduleV1`、レーン/割り当て/SLA) をサポートするヘルパーовибок、которые могут переиспользовать CI およびダウンストリーム ツール。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` オンチェーン スナップショット через `/v2/sorafs/capacity/state`、プロバイダーと料金台帳を表示します。 Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- ハンドル、ハンドル、レーン、ガードрепликации и проверки диапазонов телеметрии, чтобы регрессии всплывали сразу в CI.【crates/sorafs_manifest/src/capacity.rs:792】
- オペレーター ツール: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` の仕様と Norito ペイロード、base64 BLOB および JSON サマリー、説明могли подготовить フィクスチャ `/v2/sorafs/capacity/declare`、`/v2/sorafs/capacity/telemetry` およびレプリケーション順序フィクスチャ с локальной валидацией.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 リファレンスフィクスチャーは `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`、`order_v1.to`) と `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` です。

### 2. コントロール プレーン

| Задача |所有者 | Примечания |
|------|----------|------|
| Torii `/v2/sorafs/capacity/declare`、`/v2/sorafs/capacity/telemetry`、`/v2/sorafs/capacity/orders`、Norito JSON ペイロード。 | Torii チーム | Зеркалировать логику валидации; Norito JSON ヘルパー。 |
|スナップショット `CapacityDeclarationV1` とメタデータ スコアボード オーケストレーターとフェッチ ゲートウェイをサポートします。 |ツーリング WG / オーケストレーター チーム | `provider_metadata` の容量、レーンのスコアを表示します。 |
|レプリケーションの順序、クライアントのオーケストレーター/ゲートウェイ、割り当て、フェイルオーバーのヒントを確認します。 |ネットワーキング TL / ゲートウェイ チーム |スコアボード ビルダーは、ガバナンスのレプリケーション順序を制御します。 |
| CLI ツール: `sorafs_cli` と `capacity declare`、`capacity telemetry`、`capacity orders import`。 |ツーリングWG | Предоставить детерминированный JSON + スコアボードを出力します。 |

### 3. 市場とガバナンスの融合

| Задача |所有者 | Примечания |
|------|----------|------|
| Утвердить `MarketplacePolicy` (минимальный stake, мультипликаторы øтрафов, периодичность аудита)。 |ガバナンス評議会 |ドキュメントを参照してください。 |
|統治のフック、議会の承認、更新、宣言の取り消し。 |ガバナンス評議会 / スマートコントラクトチーム | Norito イベント + 取り込みマニフェスト。 |
| Реализовать график зтрафов (手数料、保証金の削減)、SLA が必要です。 |ガバナンス評議会 / 財務 | Согласовать с は決済 `DealEngine` を出力します。 |
|紛争が発生しました。 |ドキュメント / ガバナンス |紛争ランブック + ヘルパー CLI を使用します。 |

### 4. メータリングと最低料金| Задача |所有者 | Примечания |
|------|----------|------|
| Torii と `CapacityTelemetryV1` を組み合わせて、メータリングを取り込みます。 | Torii チーム | GiB 時間、最低 PoR、アップタイムを実現します。 |
|パイプライン メータリング `sorafs_node` は注文 + SLA を保証します。 |ストレージチーム |レプリケーション命令とチャンカーの処理を行います。 |
|パイプライン決済: 支払いと支払い、XOR とガバナンス対応の要約、元帳の計算。 |財務/保管チーム | Подключить к ディールエンジン/財務省輸出。 |
|ダッシュボード/アラートとメータリング (バックログの取り込み、計算) を提供します。 |可観測性 | Grafana、SF-6/SF-7 に対応しています。 |

- Torii теперь публикует `/v2/sorafs/capacity/telemetry` и `/v2/sorafs/capacity/state` (JSON + Norito)、テレメトリスナップショットの記録 - 記録簿の記録 аудита или упаковки доказательств.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Интеграция `PinProviderRegistry` гарантирует、エンドポイントのレプリケーション順序。ヘルパー CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) による自動化は、ハッシュ化とエイリアスを使用して実行されます。
- メータリング スナップショット、`CapacityTelemetrySnapshot`、スナップショット `metering`、Prometheus のエクスポートGrafana ボードと `docs/source/grafana_sorafs_metering.json`、時間、GiB 時間、nano-SORA 手数料、 SLA を参照してください。【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- メータリング スムージング、スナップショット `smoothed_gib_hours` および `smoothed_por_success_bps`、EMA の表示支払いに関するガバナンスに関する情報。[crates/sorafs_node/src/metering.rs:401]

### 5. 紛争と取り消し

| Задача |所有者 | Примечания |
|------|----------|------|
|ペイロード `CapacityDisputeV1` (証拠、証拠、プロバイダー) を確認します。 |ガバナンス評議会 | Norito схема + валидатор. |
| Поддержка CLI для подачи 紛争 и ответов (添付ファイルの証拠)。 |ツーリングWG |証拠をハッシュ化します。 |
| SLA (紛争の自動エスカレーション) を要求します。 |可観測性 |アラートとガバナンスフックを指します。 |
|プレイブックの取り消し (猶予期間、固定データ)。 |ドキュメント / ストレージ チーム |ポリシードキュメントとオペレーターランブックを参照してください。 |

## Требования к тестированию и CI

- Юнит-тесты для всех новых валидаторов схем (`sorafs_manifest`)。
- 手順: 複製順序 → メータリング → 支払い。
- CI ワークフローのサンプル деклараций/телеметрии емкости и проверки синхронизации подписей (расзирить `ci/check_sorafs_fixtures.sh`)。
- レジストリ API の負荷テスト (10,000 プロバイダー、100,000 注文)。

## Телеметрия и даборды

- 例文:
  - プロバイダーとプロバイダーの比較。
  - バックログのレプリケーション注文と、その注文。
  - SLA (稼働率 %、最低 PoR) を保証します。
  - 料金と手数料がかかります。
- アラート:
  - プロバイダー ниже минимальной заявленной емкости。
  - レプリケーション順序は SLA に準拠します。
  - メータリングパイプライン。

## Документационные материалы

- Руководство оператора по декларации емкости, продлению обязательств и мониторингу использования.
- 統治、命令、紛争の解決。
- エンドポイントとレプリケーション順序の API リファレンス。
- マーケットプレイス FAQ をご覧ください。

## Чеклист готовности к GA

ロードマップ **SF-2c** の量産ロールアウトとその実行日の決定
紛争や紛争が発生します。 Используйте артефакты ниже, чтобы держать критерии
Сриемки в синхроне с реализацией。### Ночной учет и сверка XOR
- XOR 元帳のスナップショットと、XOR 台帳のスナップショット:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  解決策は、決済と決済の両方です。
  выдаст текстовый файл Prometheus の概要。
- アラート `SoraFSCapacityReconciliationMismatch` (× `dashboards/alerts/sorafs_capacity_rules.yml`)
  срабатывает、когда 和解 метрики сообщают о расхождениях;ダッシュボード
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- JSON の概要とハッシュ、`docs/examples/sorafs_capacity_marketplace_validation/`
  ガバナンス パケットです。

### 争いと斬り合い
- 紛争 `sorafs_manifest_stub capacity dispute` (テスト:
  `cargo test -p sorafs_car --test capacity_cli`)、ペイロードが表示されます。
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` または наборы
  втрафов (`record_capacity_telemetry_penalises_persistent_under_delivery`)、чтобы доказать
  議論やスラッシュ。
- Следуйте `docs/source/sorafs/dispute_revocation_runbook.md` для захвата доказательств и
  эскалации;承認ストライキと検証レポート。

### Смоук-тесты онбординга および выхода プロバイダー
- Регенерируйте アーティファクト деклараций/телеметрии через `sorafs_manifest_stub capacity ...` и
  CLI テストは (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`) をテストします。
- Отправляйте через Torii (`/v2/sorafs/capacity/declare`), затем фиксируйте
  `/v2/sorafs/capacity/state` は Grafana です。 Следуйте flow выхода в
  `docs/source/sorafs/capacity_onboarding_runbook.md`。
- アーティファクトと調整出力を表示します。
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## Зависимости и последовательность

1. SF-2b (アドミッション ポリシー) - プロバイダーのマーケットプレイス。
2. Реализовать слой схемы + レジストリ (этот документ) は、Torii を参照します。
3. メータリング パイプラインを監視します。
4. 説明: ガバナンスとステージングの料金および測定データの組み合わせ。

ロードマップの詳細を確認してください。ロードマップの詳細、
как каждая основная секция (схемы、コントロール プレーン、интеграция、メータリング、обработка 紛争) достигнет
機能が完了しました。