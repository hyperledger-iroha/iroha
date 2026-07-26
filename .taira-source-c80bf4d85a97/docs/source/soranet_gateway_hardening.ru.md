---
lang: ru
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

# Укрепление Gateway SoraGlobal (SNNet-15H)

Помощник по укреплению собирает доказательства безопасности и приватности перед продвижением сборок Gateway.

## Команда
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Выходные данные
- `gateway_hardening_summary.json` — статус по каждому входу (SBOM, отчет о уязвимостях, политика HSM, профиль sandbox) и сигнал ретенции. Отсутствующие входы показывают `warn` или `error`.
- `gateway_hardening_summary.md` — читабельная сводка для governance-пакетов.

## Примечания к приемке
- Отчеты SBOM и уязвимостей должны существовать; отсутствие входов понижает статус.
- Ретенция более 30 дней помечается как `warn` для ревью; задайте более строгие значения по умолчанию перед GA.
- Используйте сводные артефакты как вложения для ревью GAR/SOC и инцидент‑ранбуков.
