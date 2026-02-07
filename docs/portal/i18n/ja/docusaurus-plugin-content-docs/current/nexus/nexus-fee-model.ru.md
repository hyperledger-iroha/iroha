---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
タイトル: Обновления модели комиссий Nexus
説明: Зеркало `docs/source/nexus_fee_model.md`, документирующее квитанции расчетов по lane и поверхности согласования.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_fee_model.md`. Держите обе копии синхронизированными, пока мигрируют переводы на японский, иврит, испанский, португальский, французский、русский、арабский、урду。
:::

# Обновления модели комиссий Nexus

Единый роутер расчетов теперь фиксирует детерминированные квитанции по каждой lane, чтобы операторы могли сверять最低、Nexus。

- ロールアウトを実行するには、次の手順を実行します。 `docs/settlement-router.md`。 NX-3 と как SRE должны мониторить のロードマップを確認してください。 роутер в продаклене.
- Конфигурация газового актива (`pipeline.gas.units_per_gas`) включает десятичное значение `twap_local_per_xor`, `liquidity_profile` (`tier1`、`tier2` または `tier3`) または `volatility_class` (`stable`、`elevated`、`dislocated`)。決済ルーター、XOR 、 TWAP およびヘアカット レーン。
- Каждая транзакция, оплачивающая газ, записывает `LaneSettlementReceipt`. Каждый 領収書 хранит предоставленный вызывающим идентификатор источника, локальную микро-сумму, XOR к немедленной оплате、ожидаемый XOR после ヘアカット、фактическую вариацию (`xor_variance_micro`) и метку времени блока в миллисекундах.
- レーン/データスペースと `lane_settlement_commitments` と `/v1/sumeragi/status` のレシートを確認します。 Итоги раскрывают `total_local_micro`, `total_xor_due_micro` および `total_xor_after_haircut_micro`, суммированные по блоку для ночных выгрузокね。
- Новый счетчик `total_xor_variance_micro` отслеживает, сколько запаса безопасности было израсходовано (разница между) начисленным XOR и ожиданием после ヘアカット)、а `swap_metadata` документирует детерминированные параметры конверсии (TWAP、イプシロン、流動性プロファイル) volatility_class)、чтобы аудиторы могли проверить входные параметры котировки независимо от конфигурации рантайма.

レーンとデータスペースのコミットメント、`lane_settlement_commitments` のコミットメントヘアカットとスワップの組み合わせは、Nexus です。