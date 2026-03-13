---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
title: Runbook de disputas e revogacoes da SoraFS
Sidebar_label: Сборник споров и отказов
описание: Поток управления для регистрации споров о емкости SoraFS, координатор отзыва и эвакуация данных детерминированной формы.
---

:::примечание Fonte canonica
Эта страница отражает `docs/source/sorafs/dispute_revocation_runbook.md`. Мантенья был в роли синхронизированных копий, которые ели документальный Сфинкс, оставшийся в отставке.
:::

## Предложение

Этот Runbook помогает операторам управления открывать споры о возможностях SoraFS, координировать действия по отзыву и гарантировать, что эвакуация данных будет завершена в детерминированной форме.

## 1. Информация о происшествии

- **Сообщения об отказе:** обнаружение нарушений SLA (время безотказной работы/сбой PoR), дефицит репликации или расхождения в обмене.
- **Подтвердите телеметрию:** сделайте снимки `/v2/sorafs/capacity/state` и `/v2/sorafs/capacity/telemetry`, чтобы выполнить проверку.
- **Интересные уведомления:** Группа хранения (оперативные действия), Совет управления (орган принятия решений), Наблюдательность (настроенные информационные панели).

## 2. Подготовка пакета доказательств

1. Colete artefatos brutos (телеметрия JSON, журналы CLI, заметки аудитории).
2. Нормализовать детерминированный архив (например, в архиве); зарегистрироваться:
   - дайджест BLAKE3-256 (`evidence_digest`)
   - типо де мидия (`application/zip`, `application/jsonl` и т. д.)
   - URI de hospedagem (хранилище объектов, контакт SoraFS или доступ к конечной точке через Torii)
3. Возьмите пакет с доказательствами управления с доступом для записи один раз.

##3. Регистратор диспута

1. Введите спецификацию JSON для `sorafs_manifest_stub capacity dispute`:

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

2. Выполните CLI:

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

3. Исправьте `dispute_summary.json` (подтвердите тип, дайджест доказательств и временных меток).
4. Отправьте запрос JSON для Torii `/v2/sorafs/capacity/dispute` через файл транзакций управления. Захват доблести ответа `dispute_id_hex`; ele ancora как acoes de revogacao postiores и os relatorios de Audiia.

## 4. Эвакуация и возврат

1. **Жанела де Граса:** уведомление о неизбежном отказе; Разрешение на эвакуацию фиксированных людей, когда разрешена политика.
2. **Гере `ProviderAdmissionRevocationV1`:**
   - Используйте `sorafs_manifest_stub provider-admission revoke` с одобрением мотивации.
   - Verifique Assinaturas и дайджест отзыва.
3. **Публикация отзыва:**
   - Зависть от отзывного требования для Torii.
   - Гарантия, что реклама будет заблокирована (ожидаю, что `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` увеличится).
4. **Актуализация информационных панелей:** имя отозванного документа, ссылка на удостоверение личности спора и винкуле или пакет доказательств.

## 5. Вскрытие и сопровождение

- Зарегистрируйте время, вызывающее причину, как средство исправления ситуации и отслеживание происшествий в управлении.
- Определить реституцию (сокращение ставки, возврат налогов, возврат клиентов).
- Документы об обучении; актуализировать ограничения SLA или оповещения о мониторинге, если это необходимо.

## 6. Справочные материалы- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (секретные споры)
- `docs/source/sorafs/provider_admission_policy.md` (переход к возврату)
- Панель наблюдения: `SoraFS / Capacity Providers`

## Контрольный список

- [ ] Пакет доказательств, снятых и захваченных.
- [ ] Полезная нагрузка для локального спора.
- [ ] Сделка по спору без Torii.
- [ ] Выполнение отмены (подтверждение).
- [ ] Настроенные информационные панели/журналы.
- [ ] Посмертный архив в составе правительственного совета.