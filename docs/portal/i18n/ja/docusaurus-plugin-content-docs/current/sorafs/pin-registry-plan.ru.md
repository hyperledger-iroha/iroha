---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: План реализации Pin レジストリ SoraFS
Sidebar_label: План ピン レジストリ
説明: SF-4、охватывающий мазоину состояний レジストリ、фасад Torii、ツールと наблюдаемость。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/pin_registry_plan.md`. Держите обе копии синхронизированными, пока наследственная документация остается активной.
:::

# ピン レジストリ SoraFS (SF-4)

SF-4 は、ピン レジストリとピン レジストリを表示します。
マニフェスト、ピン、API、Torii、
Поркестраторов 。 Этот документ раслидации конкретными план валидации конкретными
задачами реализации, охватывая オンチェーン логику, сервисы на стороне хоста,
備品と операционные требования。

## Область

1. **レジストリ**: Norito マニフェスト、エイリアス、
   цепочек преемственности、эпох хранения и метаданных управления。
2. **Реализация контракта**: детерминированные CRUD-операции для жизненного
   цикла ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **説明**: gRPC/REST エンドポイント、レジストリおよび
   Torii と SDK が必要です。
4. **ツールとフィクスチャ**: CLI ヘルパー、ツールとツール
   マニフェスト、エイリアス、ガバナンス エンベロープ。
5. **操作**: レジストリ、操作、Runbook のレジストリ。

## Модель данных

### Основные записи (Norito)

| Структура | Описание | Поля |
|----------|----------|------|
| `PinRecordV1` |マニフェストを作成します。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` |エイリアス -> CID マニフェストを選択します。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストを確認します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` | Подтверждение провайдера。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` | Снимок политики управления。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` 日 Norito
Rust とヘルパーは、両方の機能を備えています。 Валидация
マニフェスト ツール (チャンカー レジストリのルックアップ、ピン ポリシー ゲーティング)、
контракт、фасады Torii および CLI разделяли идентичные инварианты。

説明:
- Norito と `crates/sorafs_manifest/src/pin_registry.rs` を確認してください。
- Сгенерировать код (Rust + другие SDK) 最低 Norito。
- Обновить документацию (`sorafs_architecture_rfc.md`) が表示されます。

## Реализация контракта

| Задача | Ответственные | Примечания |
|----------|------|----------|
|レジストリ (スレッド/sqlite/オフチェーン) を参照してください。 |コアインフラ/スマートコントラクトチーム |ハッシュ化、浮動小数点を使用します。 |
|エントリ ポイント: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ | Использовать `ManifestValidator` と表示されます。別名 теперь проходит через `RegisterPinManifest` (DTO Torii)、тогда как выделенный `bind_alias` остается вありがとうございます。 |
|例: обеспечивать преемственность (マニフェスト A -> B)、эпохи хранения、уникальность エイリアス。 |ガバナンス評議会 / コアインフラ | Уникальность エイリアス、лимиты хранения и проверки одобрения/вывода предのようにственников живут в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многолыаговой преемственности и учет репликации остаются открытыми. |
| Управляемые параметры: загружать `ManifestPolicyV1` из config/состояния управления;あなたのことを思い出してください。 |ガバナンス評議会 | Предоставить CLI для обновления политик。 |
|例: Norito および `ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`)。 |可観測性 | + ロギングを実行します。 |

要点:
- Юнит-тесты для каждой エントリ ポイント (позитивные + отказные сценарии)。
- プロパティ-тесты для цепочки преемственности (без циклов, монотонные эпохи)。
- ファズ валидации с генерацией случайных マニフェスト (с ограничениями)。

## Сервисный фасад (Интеграция Torii/SDK)| Компонент | Задача | Ответственные |
|----------|----------|----------|
| Сервис Torii | Экспонировать `/v1/sorafs/pin` (送信)、`/v1/sorafs/pin/{cid}` (検索)、`/v1/sorafs/aliases` (リスト/バインド)、`/v1/sorafs/replication` (注文/受領)。 Обеспечить пагинацию + фильтрацию. |ネットワーキング TL / コア インフラ |
| Аттестация |レジストリを参照してください。 Norito、SDK を使用してください。 |コアインフラ |
| CLI | `sorafs_manifest_stub` および CLI `sorafs_pin`、`pin submit`、`alias bind`、`order issue`、`registry export` を参照してください。 |ツーリングWG |
| SDK | Сгенерировать клиентские バインディング (Rust/Go/TS) と Norito; добавить интеграционные тесты。 | SDK チーム |

説明:
- キャッシュ/ETag とエンドポイントの GET を実行します。
- レート制限/認証を Torii で実行します。

## 備品と CI

- フィクスチャ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` スナップショット マニフェスト/エイリアス/順序、`cargo run -p iroha_core --example gen_pin_snapshot` のスナップショット。
- CI: `ci/check_sorafs_fixtures.sh` スナップショットと差分、フィクスチャ CI が表示されます。
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パス плюс отказ при дублировании エイリアス、ガード одобрения/хранения エイリアス、 несовпадающие はチャンカーを処理し、ガードを保持します。 (неизвестные/предодобренные/выведенные/самоссылки);最低。 кейсы `register_manifest_rejects_*` 日、今日は。
- Юнит-тесты теперь покрывают валидацию エイリアス、ガード хранения и проверки премника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многоственности появится, когда заработает мазина состояний.
- Golden JSON は、最新の機能を備えています。

## Телеметрия и наблюдаемость

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) をエンドツーエンドで接続します。

例:
- Структурированный поток событий Norito для аудиторов управления (подписанные?)

Алерты:
- SLA を使用してください。
- Истечение срока 別名 ниже порога。
- Наруления хранения (マニフェスト не продлен до истечения)。

説明:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` 合計のマニフェスト、エイリアス、バックログ、SLA 比率、オーバーレイのレイテンシとスラックの比較オンコール中です。

## Runbook と документация

- Обновить `docs/source/sorafs/migration_ledger.md`、レジストリを確認してください。
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (уже опубликовано) с метриками、алертингом、развертыванием、backup およびПосстановлением。
- Руководство по управлению: описать параметры политики、ワークフロー одобрения、обработку споров.
- API エンドポイントの説明 (Docusaurus ドキュメント)。

## Зависимости и последовательность

1. マニフェスト検証 (ManifestValidator) を実行します。
2. Norito + デフォルト値を入力します。
3. Реализовать контракт + сервис, подключить телеметрию.
4. フィクスチャ、スイートを備えています。
5. ドキュメント/Runbook およびロードマップを参照してください。

SF-4 のテストが完了しました。
REST はエンドポイントを維持します:

- `GET /v1/sorafs/pin` および `GET /v1/sorafs/pin/{digest}` のマニフェスト
  エイリアス バインディング、注文 репликации и объектом аттестации、производным от
  хэза последнего блока。
- `GET /v1/sorafs/aliases` と `GET /v1/sorafs/replication` は、
  別名とバックログ заказов репликации с консистентной пагинацией и
  Єильтрами статуса。

CLI によるアクセス (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`)、レジストリを取得する
API を使用します。