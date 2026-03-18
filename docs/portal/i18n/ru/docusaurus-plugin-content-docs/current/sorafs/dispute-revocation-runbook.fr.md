---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
title: Runbook des Litiges et Révocats SoraFS
Sidebar_label: Судебные разбирательства и отзыв Runbook
описание: Поток управления для хранения судебных дел SoraFS, координатор отзыва и эвакуации определенных действий.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/dispute_revocation_runbook.md`. Gardez les deux копирует синхронизированные копии только потому, что документация Sphinx унаследована до сих пор.
:::

## Объектив

Это руководство для операторов управления по созданию судебных дел SoraFS, координация отзыва и гарантия детерминированной эвакуации доноров.

## 1. Оценка инцидента

- **Условия сокращения:** обнаружение нарушения SLA (невозможность/проверка PoR), дефицит репликации или нарушение факторурации.
- **Подтверждение телеметрии:** захват снимков `/v1/sorafs/capacity/state` и `/v1/sorafs/capacity/telemetry` для просмотра.
- **Уведомление о предстоящих событиях:** Группа хранения (операции по обслуживанию), Совет управления (орган принятия решений), Наблюдательность (создание информационных панелей).

## 2. Подготовьте набор преувов

1. Сборщик артефактов (телеметрия JSON, логи CLI, заметки аудита).
2. Нормализатор в определенном архиве (например, в архиве); грузоотправитель:
   - дайджест BLAKE3-256 (`evidence_digest`)
   - тип носителя (`application/zip`, `application/jsonl` и т. д.)
   - URI d’hébergement (хранилище объектов, контакт SoraFS или конечная точка, доступная через Torii)
3. Сохраните комплект в ведре для сбора прав управления с доступом для однократной записи.

## 3. Отправитель судебного разбирательства

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

2. Ланцес ла CLI:

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

3. Проверьте `dispute_summary.json` (подтвердите тип, дайджест предварительных данных, временные метки).
4. Соберите JSON-запрос по Torii `/v1/sorafs/capacity/dispute` через файл транзакций управления. Захват значения ответа `dispute_id_hex` ; еще несколько действий по отзыву и договоров аудита.

## 4. Эвакуация и отзыв

1. **Fenêtre de grâce:** avertissez le fournisseur de la revocation imminente; autorisez l’évacuation des données épinglées lorsque la politique le permet.
2. **Генерес `ProviderAdmissionRevocationV1` :**
   - Используйте `sorafs_manifest_stub provider-admission revoke` с одобрением.
   - Проверьте подписи и дайджест отзыва.
3. **Опубликовать отзыв:**
   - Сообщите запрос на отзыв по номеру Torii.
   - Убедитесь, что рекламные объявления четырех пользователей заблокированы (посещайте дом `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Подключение к панелям мониторинга:** сигнализирует о возврате, ссылается на идентификатор судебного разбирательства и содержит набор предварительных требований.

## 5. Вскрытие и наблюдение- Зарегистрируйте хронологию, причины расизма и действия по исправлению положения в системе отслеживания инцидентов в управлении.
- Déterminez la restitution (сокращение ставки, возврат средств, возмещение клиентам).
- Документирование уроков; Отметьте в течение дня все необходимые соглашения об уровне обслуживания или оповещения о мониторинге.

## 6. Справочные документы

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел судебные разбирательства)
- `docs/source/sorafs/provider_admission_policy.md` (рабочий процесс отзыва)
- Панель наблюдения: `SoraFS / Capacity Providers`

## Контрольный список

- [ ] Пакет предварительных снимков и фотографий.
- [ ] Полезная нагрузка действительного региона.
- [ ] Судебная транзакция Torii принята.
- [ ] Выполненный отзыв (если он одобрен).
- [ ] Панели мониторинга/runbooks устарели.
- [ ] Посмертное вынесение заключения в правительственный совет.