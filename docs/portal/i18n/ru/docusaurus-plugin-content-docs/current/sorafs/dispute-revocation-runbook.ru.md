---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
заголовок: Ранбук споров и обзоров SoraFS
Sidebar_label: Ранбук споров и отзывов
Описание: Управление процессом подачи споров по емкости SoraFS, обзоры конфликтов и определенная эвакуация данных.
---

:::note Канонический источник
На этой странице отражено `docs/source/sorafs/dispute_revocation_runbook.md`. Держите копии синхронизированными, пока конфиденциальная документация Сфинкса не будет выведена из обращения.
:::

## Назначение

Этот ранбук осуществляет управление операторами через подачу споров по емкости SoraFS, координацию обзоров и обеспечение детерминированной эвакуации данных.

## 1. Оценить происшествие

- **Условия триггера:** выявление нарушений SLA (uptime/сбой PoR), необходимость репликации или разногласие по биллингу.
- **Подтвердить телеметрию:** зафиксируйте снимки `/v1/sorafs/capacity/state` и `/v1/sorafs/capacity/telemetry` для провайдера.
- **Уведомить стейкхолдеров:** Storage Team (операции провайдера), Governance Council (органные решения), Observability (обновления дашбордов).

## 2. Подготовить пакет доказательств

1. Соберите сырые артефакты (телеметрия JSON, логи CLI, заметки аудитора).
2. Нормализуйте в определённый архив (например, tarball); зафиксируйте:
   - дайджест BLAKE3-256 (`evidence_digest`)
   - тип медиа (`application/zip`, `application/jsonl` и т.д.)
   - Размещение URI (хранилище объектов, вывод SoraFS или конечная точка, доступный через Torii)
3. Сохраните пакет в корзине доказательств управления с доступом с однократной записью.

## 3. Подать спор

1. Создайте спецификацию JSON для `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Запустите CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. Проверьте `dispute_summary.json` (подтвердите тип, дайджест доказательств и временные метки).
4. Отправьте JSON-запрос в Torii `/v1/sorafs/capacity/dispute` через очередь управления-транзакций. Зафиксируйте значение ответа `dispute_id_hex`; Он якорит действия по отзывам и аудиторским отчетам.

## 4. Эвакуация и отзыв

1. **Окно льгот:** уведомите провайдера о грядущем отзыве; Разрешите эвакуацию связанных данных, если это допускает политику.
2. **Сгенерируйте `ProviderAdmissionRevocationV1`:**
   - Используйте `sorafs_manifest_stub provider-admission revoke` с обоснованной причиной.
   - Проверьте наличие и дайджест отзыва.
3. **Опубликовать отзыв:**
   - Отправьте запрос на отзыв на Torii.
   - Убедитесь, что реклама провайдера заблокирована (ожидайте роста `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Обновите дашборды:** от провайдера как отозванного, укажите идентификатор спора и приложите ссылку к пакету доказательств.

## 5. Вскрытие и следственное действие

- Зафиксируйте таймлайн, корневую причину и меры по исправлению ситуации в трекере инцидентов управления.
- Определите реституцию (снижение ставки, возвраты комиссий, возвраты студентов).
- Документируйте выводы; При необходимости обновите пороги SLA или предупредите Диптихи.

## 6. Справочные материалы

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел споров)
- `docs/source/sorafs/provider_admission_policy.md` (отзыв о рабочем процессе)
- Дашборд наблюдения: `SoraFS / Capacity Providers`

## Чеклист- [ ] Пакет доказательств собран и захеширован.
- [ ] Полезная нагрузка спора валидирована локально.
- [ ] Torii-транзакция спора принята.
- [ ] Отзыв выполнен (если одобрен).
- [ ] Дашборды/ранбуки обновлены.
- [ ] Посмертное оформление в советском управлении.