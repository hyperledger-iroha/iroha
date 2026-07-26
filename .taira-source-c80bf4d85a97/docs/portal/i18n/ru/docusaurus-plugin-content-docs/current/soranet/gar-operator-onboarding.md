---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Используйте этот brief, чтобы развернуть конфигурацию compliance SNNet-9 с повторяемым и удобным для аудита процессом. Совмещайте его с юрисдикционным обзором, чтобы все операторы использовали одинаковые digests и единый формат доказательств.

## Шаги

1. **Соберите конфигурацию**
   - Импортируйте `governance/compliance/soranet_opt_outs.json`.
   - Объедините ваши `operator_jurisdictions` с digests аттестаций, опубликованными
     в [юрисдикционном обзоре](gar-jurisdictional-review).
2. **Проверьте**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Опционально: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Зафиксируйте доказательства**
   - Сохраните в `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (финальный блок compliance)
     - `attestations.json` (URIs + digests)
     - логи проверки
     - ссылки на подписанные PDFs/Norito envelopes
4. **Активируйте**
   - Поставьте тег rollout (`gar-opt-out-<date>`), переопубликуйте конфиги orchestrator/SDK,
     и подтвердите, что события `compliance_*` появляются в ожидаемых логах.
5. **Закройте**
   - Передайте evidence bundle в Governance Council.
   - Запишите окно активации и апруверов в GAR logbook.
   - Запланируйте следующие даты ревью из таблицы юрисдикционного обзора.

## Быстрый чеклист

- [ ] `jurisdiction_opt_outs` совпадает с каноническим каталогом.
- [ ] Digests аттестаций скопированы точно.
- [ ] Команды валидации выполнены и архивированы.
- [ ] Evidence bundle сохранен в `artifacts/soranet/compliance/<date>/`.
- [ ] Тег rollout + GAR logbook обновлены.
- [ ] Напоминания о следующем ревью настроены.

## См. также

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
