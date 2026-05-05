---
lang: ru
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d23c9d6a77942b5918b933631b890addbc0ecdfef51e9ff427a4069d2cc37902
source_last_modified: "2025-11-15T16:27:31.089720+00:00"
translation_last_reviewed: 2026-01-01
---

# Каталог суффиксов Sora Name Service

Roadmap SNS отслеживает каждый утвержденный суффикс (SN-1/SN-2). Эта страница
отражает каталог-источник истины, чтобы операторы, запускающие registrars,
DNS gateways или инструменты кошельков, могли загружать те же параметры без
парсинга статусных документов.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumers:** `iroha sns policy`, SNS onboarding kits, KPI dashboards и
  DNS/Gateway release scripts читают один и тот же JSON bundle.
- **Statuses:** `active` (регистрации разрешены), `paused` (временно ограничен),
  `revoked` (объявлен, но сейчас недоступен).

## Схема каталога

| Поле | Тип | Описание |
|------|-----|----------|
| `suffix` | string | Человекочитаемый суффикс с ведущей точкой. |
| `suffix_id` | `u16` | Идентификатор, хранимый в ledger как `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` или `revoked`, описывающие готовность к запуску. |
| `steward_account` | string | Аккаунт, ответственный за stewardship (совпадает с policy hooks регистратора). |
| `fund_splitter_account` | string | Аккаунт, который получает платежи до маршрутизации по `fee_split`. |
| `payment_asset_id` | string | Актив для settlement (`61CtjvNd9T3THAR65GsMVHr82Bjc` для начальной когорты). |
| `min_term_years` / `max_term_years` | integer | Границы срока покупки из политики. |
| `grace_period_days` / `redemption_period_days` | integer | Окна безопасности продления, применяемые Torii. |
| `referral_cap_bps` | integer | Максимальный referral carve-out, разрешенный управлением (basis points). |
| `reserved_labels` | array | Защищенные управлением объекты меток `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Объекты tier с `label_regex`, `base_price`, `auction_kind` и границами длительности. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` распределение в basis points. |
| `policy_version` | integer | Монотонный счетчик, увеличиваемый при редактировании политики управлением. |

## Текущий каталог

| Суффикс | ID (`hex`) | Steward | Fund splitter | Статус | Платежный актив | Лимит referral (bps) | Срок (min - max лет) | Grace / Redemption (дни) | Ценовые уровни (regex -> базовая цена / аукцион) | Зарезервированные метки | Разделение fees (T/S/R/E bps) | Версия политики |
|---------|------------|---------|---------------|--------|-----------------|----------------------|----------------------|---------------------------|--------------------------------------------------|-------------------------|------------------------------|---------------|
| `.sora` | `0x0001` | `<i105-account-id>` | `<i105-account-id>` | Активен | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> <i105-account-id>` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `<i105-account-id>` | `<i105-account-id>` | Приостановлен | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> <i105-account-id>`, `guardian -> <i105-account-id>` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `<i105-account-id>` | `<i105-account-id>` | Отозван | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## Фрагмент JSON

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "<i105-account-id>",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## Заметки по автоматизации

1. Загрузите JSON snapshot и сделайте hash/подпись перед распространением операторам.
2. Инструменты registrar должны показывать `suffix_id`, ограничения срока и цены
   из каталога, когда запрос попадает в `/v1/sns/*`.
3. Helpers DNS/Gateway читают метаданные зарезервированных меток при генерации
   шаблонов GAR, чтобы ответы DNS оставались согласованными с контролем управления.
4. Задания KPI annex помечают exports дашбордов метаданными суффикса, чтобы алерты
   соответствовали состоянию запуска, зафиксированному здесь.
