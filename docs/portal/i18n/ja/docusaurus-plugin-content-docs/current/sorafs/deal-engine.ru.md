---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
タイトル: Движок сделок SoraFS
サイドバーラベル: Движок сделок
説明: Обзор движка сделок SF-8、интеграции Torii и телеметрических поверхностей。
---

:::note Канонический источник
:::

# Движок сделок SoraFS

SF-8 の最新バージョン SoraFS、最低価格
детерминированный учет соглазений на хранение извлечение между
клиентами и провайдерами. Norito ペイロード、
определенными в `crates/sorafs_manifest/src/deal.rs`, покрывая условия сделки,
債券、вероятностные микроплатежи и записи расчетов。

SoraFS ワーカー (`sorafs_node::NodeHandle`) を取得します。
экземпляр `DealEngine` для каждого процесса узла. Движок:

- `DealTermsV1` を参照してください。
- XOR による料金の計算。
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  BLAKE3のサンプリング。や
- スナップショット台帳とペイロードを管理し、ガバナンスを監視します。

Юнит-тесты покрывают валидацию, выбор микроплатежей и расчетные потоки, чтобы
API を使用します。ガバナンス ペイロードの詳細
`DealSettlementV1`, напрямую подключаясь к パイプライン публикации SF-12, и обновляют серию
OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) と Torii および
SLO の実施。 Следующие заги фокусируются на автоматизации スラッシュ、инициируемой
これは、政府の統治を意味します。

Телеметрия использования также питает набор метрик `sorafs.node.micropayment_*`:
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`、これを確認してください
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。合計数 раскрывают вероятностный
лотерейный поток, чтобы операторы могли коррелировать выигрыbolи микроплатежей и
最高のパフォーマンスを見せてください。

## Интеграция Torii

Torii エンドポイントの使用法、エンドポイントの使用法
配線方法:

- `POST /v1/sorafs/deal/usage` は、`DealUsageReport` と возвращает телеметрию
  детерминированные результаты учета (`UsageOutcome`)。
- `POST /v1/sorafs/deal/settle` стримя текущий ウィンドウ
  итоговый `DealSettlementRecord` вместе с Base64-кодированным `DealSettlementV1`,
  ガバナンス DAG が必要です。
- Лента Torii `/v1/events/sse` транслирует записи `SorafsGatewayEvent::DealUsage`、
  суммирующие каждую отправку 使用状況 (エポック、измеренные GiB 時間、счетчики билетов,
  料金)、`SorafsGatewayEvent::DealSettlement`、
  BLAKE3 ダイジェスト/サイズ/base64 のスナップショット台帳を確認する
  ガバナンス артефакта на диске, и алерты `SorafsGatewayEvent::ProofHealth` при превылении
  PDP/PoTR (провайдер、окно、состояние ストライク/クールダウン、сумма зтрафа)。 Потребители могут
  ポーリングの検証と健全性の確認を行うことができます。

SoraFS クォータ フレームワークを使用してエンドポイントを設定する
`torii.sorafs.quota.deal_telemetry`, позволяя операторам настраивать допустимую
Їастоту отправки для каждого деплоя.