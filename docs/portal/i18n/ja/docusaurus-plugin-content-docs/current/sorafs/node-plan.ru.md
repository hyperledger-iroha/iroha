---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
タイトル: План реализации узла SoraFS
サイドバーラベル: План реализации узла
説明: Преобразует дорожную карту хранилища SF-3 в инженерную работу с вехами, задачами и покрытием тестами.
---

:::note Канонический источник
:::

SF-3 はクレート `sorafs-node`、クレートは Iroha/Torii です。 SoraFS。 Используйте этот план вместе с [гайдом по хранилищу узла](node-storage.md), [политикой допуска] провайдеров](provider-admission-policy.md) и [дорожной картой маркетплейса емкости хранения](storage-capacity-marketplace.md) при выстраиванииそうです。

## Целевой объем (веха M1)

1. **チャンク ストア。** Обернуть `sorafs_car::ChunkStore` в персистентный バックエンド、который хранит байты чанков、манифесты и PoR-деревья в заданной директории данных。
2. **ゲートウェイ。** Открыть Norito HTTP ピン、フェッチ、PoR サンプリング、および телеметрии хранилища внутри Torii。
3. **Прокладка конфигурации.** Добавить структуру конфигурации `SoraFsStorage` (флаг включения, емкость, директории、лимиты конкурентности)、проведенную через `iroha_config`、`iroha_core` および `iroha_torii`。
4. **Квоты/планирование.** 背圧を確認してください。
5. **Телеметрия.** ピンをピンに固定し、フェッチを実行し、PoR サンプリングを実行します。

## Разбиение работ

### A. Структура крейта и модулей

| Задача | Ответственный(е) | Примечания |
|------|-------|-----------|
| `crates/sorafs_node` の例: `config`、`store`、`gateway`、`scheduler`、`telemetry`。 |ストレージチーム | Реэкспортировать переиспользуемые типы для интеграции с Torii. |
| `StorageConfig`、`SoraFsStorage` (ユーザー→実際→デフォルト)。 |ストレージ チーム / 構成 WG | Norito/`iroha_config` を確認してください。 |
| `NodeHandle`、Torii のピン/フェッチが表示されます。 |ストレージチーム |非同期処理を実行します。 |

### B. Персистентный チャンク ストア

| Задача | Ответственный(е) | Примечания |
|------|-------|-----------|
|バックエンド `sorafs_car::ChunkStore` がディスク上に保存されています (`sled`/`sqlite`)。 |ストレージチーム |レイアウト: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| Поддерживать PoR метаданные (деревья 64 KiB/4 KiB) через `ChunkStore::sample_leaves`。 |ストレージチーム |再生を開始します。失敗は早いです。 |
|整合性リプレイ на старте (ピンの再ハッシュ)。 |ストレージチーム | Torii で再生可能です。 |

### C. ゲートウェイ

| Эндпоинт | Поведение | Задачи |
|----------|-----------|----------|
| `POST /sorafs/pin` | `PinProposalV1`、メッセージ、取り込み、CID メッセージ。 |チャンカー、チャンク ストア、チャンク ストアを表示します。 |
| `GET /sorafs/chunks/{cid}` + 範囲クエリ | Отдает байты чанка с заголовками `Content-Chunker`;最低の範囲能力。 |スケジューラ + デバイス (SF-2d 範囲機能) を追加します。 |
| `POST /sorafs/por/sample` | PoR サンプリングと証明バンドルを提供します。 |サンプリング チャンク ストア、Norito JSON ペイロードを参照します。 |
| `GET /sorafs/telemetry` |例: PoR、フェッチを実行します。 | Предоставлять данные для дабордов/операторов。 |

Рантайм-проводка направляет PoR взаимодействия через `sorafs_node::por`: трекер фиксирует каждый `PorChallengeV1`, `PorProofV1` と `AuditVerdictV1`、`CapacityMeter` はガバナンスを強化します。 Torii.【crates/sorafs_node/src/scheduler.rs#L147】

内容:

- Axum は Torii と `norito::json` ペイロードを確認します。
- Norito を参照 (`PinResultV1`、`FetchErrorV1`、テレメトリ構造体)。

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` バックログ、エポック/期限、成功/失敗のタイムスタンプ、バックログ、成功/失敗のタイムスタンプ`sorafs_node::NodeHandle::por_ingestion_status`、Torii と `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` 日дабордов.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. スケジューラと機能| Задача |ださい |
|------|----------|
| Дисковая квота | Отслеживать байты на диске; отклонять новые ピンが `max_capacity_bytes` にあります。 Подготовить хуки эвикции для будущих политик。 |
| Конкурентность fetch | (`max_parallel_fetches`) プロバイダーごとに SF-2d 範囲に対応します。 |
| Очередь ピン |取り込みジョブを実行します。 Norito を確認してください。 |
| Каденция PoR | Фоновый воркер、управляемый `por_sample_interval_secs`。 |

### E. Телеметрия и логирование

Метрики (Prometheus):

- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (Gистограмма с метками `result`)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

Логи / события:

- ガバナンス取り込み (`StorageTelemetryV1`)。
- Алерты при использовании > 90% или превыbolении порога серии PoR-обок.

### F. Стратегия тестирования

1. **Юнит-тесты.** チャンク ストア、вычисления квот、инварианты スケジューラ (см. `crates/sorafs_node/src/scheduler.rs`)。
2. **Интеграционные тесты** (`crates/sorafs_node/tests`)。 Pin → fetch ラウンドトリップ、восстановление после рестарта、отказ по квоте、проверка PoR プルーフ サンプリング。
3. **Torii интеграционные тесты.** Запустить Torii с включенным хранилищем, прогнать HTTP эндпоинты `assert_cmd`。
4. **ロードマップのカオス。** 訓練は、日々の訓練、国際IO、そして訓練の中で行われます。

## Зависимости

- Политика допуска SF-2b — убедиться、что узлы проверяют 入場封筒と広告。
- SF-2c を確認してください。
- 広告 ​​SF-2d — 範囲機能 + 機能を備えています。

## Критерии завергения вехи

- `cargo run -p sorafs_node --example pin_fetch` のフィクスチャ。
- Torii は `--features sorafs-storage` と表示されます。
- Документация ([гайд по хранилищу узла](node-storage.md)) обновлена с デフォルト конфигурацией + примерами CLI;ランブックの更新日。
- ステージング ダッシュボードの説明。それらは、PoR の機能を提供します。

## Документация と операционные 成果物

- Обновить [справочник по хранилищу узла](node-storage.md) с дефолтами конфигурации、использованием CLI および гагами トラブルシューティング。
- SF-3 の [ランブック операций узла](node-operations.md) を参照してください。
- API `/sorafs/*` と OpenAPI マニフェスト、Torii ハンドラーの組み合わせ。