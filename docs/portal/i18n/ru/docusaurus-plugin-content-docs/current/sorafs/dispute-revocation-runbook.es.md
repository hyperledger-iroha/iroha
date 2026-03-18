---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
title: Runbook de disputas y revocaciones de SoraFS
Sidebar_label: Сборник споров и отзывов
описание: Губернаторский переход для представления споров о емкости SoraFS, координации отзыва и эвакуации данных детерминированной формы.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/dispute_revocation_runbook.md`. Мантену пришлось скопировать синхронизированные документы, чтобы удалить наследственную документацию о Сфинксе.
:::

## Предложение

Этот runbook предназначен для операторов правительства, чтобы представить споры о емкости SoraFS, координировать отзыв и гарантировать, что эвакуация данных будет завершена в определенной форме.

## 1. Оцените инцидент

- **Условия активации:** обнаружение нарушения SLA (время активации/выпадение PoR), дефицит репликации или прекращение фактурирования.
- **Подтвердить телеметрию:** сделать снимки `/v1/sorafs/capacity/state` и `/v1/sorafs/capacity/telemetry` для поставщика.
- **Уведомление об интересующих сторонах:** Группа хранения (операции провайдера), Совет управления (решительный орган), Наблюдательность (актуализация информационных панелей).

## 2. Подготовьте пакет доказательств

1. Грубое копирование артефактов (телеметрия JSON, журналы CLI, аудиторские записи).
2. Нормализация в определенном архиве (например, в архиве); регистрация:
   - дайджест BLAKE3-256 (`evidence_digest`)
   - тип носителя (`application/zip`, `application/jsonl` и т. д.)
   - URI-адрес alojamiento (хранилище объектов, контакт SoraFS или конечная точка, доступная для Torii)
3. Храните пакет в ведре для хранения доказательств губернатора с доступом к уникальной письменной форме.

## 3. Представить спор

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

2. Включение CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. Revisa `dispute_summary.json` (подтверждение типа, дайджест доказательств и временные метки).
4. Отправьте запрос в формате JSON на Torii `/v1/sorafs/capacity/dispute` и на страницы государственных транзакций. Каптура доблести спасения `dispute_id_hex`; а также действия по отзыву задних дверей и информированию аудитории.

## 4. Эвакуация и отзыв

1. **Вентана де грация:** уведомление о неизбежном отзыве; разрешите эвакуацию данных из Фихадо, если политика будет разрешена.
2. **Род `ProviderAdmissionRevocationV1`:**
   - США `sorafs_manifest_stub provider-admission revoke` с проверенным разумом.
   - Проверка фирм и дайджест отзыва.
3. **Публикация отзыва:**
   - Отправьте запрос на отзыв на номер Torii.
   - Убедитесь, что рекламные объявления блокируются (видимо, что `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` увеличивается).
4. **Актуализация информационных панелей:** отображать доказательства как отозванные, ссылаться на идентификатор спора и открывать пакет доказательств.

## 5. Вскрытие и заключение- Зарегистрируйте время, причину возникновения и действия по исправлению ситуации в трекере происшествий правительства.
- Determina la restitución (снижение ставок, возврат комиссий, возврат клиентов).
- Подготовка документов; актуализация зон SLA или оповещений мониторинга, если это необходимо.

## 6. Справочные материалы

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел споров)
- `docs/source/sorafs/provider_admission_policy.md` (переход к отзыву)
- Панель наблюдения: `SoraFS / Capacity Providers`

## Контрольный список

- [ ] Пакет доказательств, снятых и захваченных.
- [ ] Полезная нагрузка, действующая локально.
- [ ] Сделка по спору на Torii принята.
- [ ] Отзыв (если это возможно).
- [ ] Актуализированы информационные панели/runbooks.
- [ ] Посмертное представление перед советом губернатора.