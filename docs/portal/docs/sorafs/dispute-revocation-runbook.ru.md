---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9a1c32940f993a44c63843d0936768af1af37d6677d7b60a5c86b659742bb71
source_last_modified: "2025-11-06T19:51:05.537172+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: dispute-revocation-runbook
title: Ранбук споров и отзывов SoraFS
sidebar_label: Ранбук споров и отзывов
description: Процесс governance для подачи споров по емкости SoraFS, координации отзывов и детерминированной эвакуации данных.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/dispute_revocation_runbook.md`. Держите обе копии синхронизированными, пока устаревшая документация Sphinx не будет выведена из обращения.
:::

## Назначение

Этот ранбук проводит операторов governance через подачу споров по емкости SoraFS, координацию отзывов и обеспечение детерминированной эвакуации данных.

## 1. Оценить инцидент

- **Условия триггера:** выявление нарушения SLA (uptime/сбой PoR), дефицит репликации или разногласие по биллингу.
- **Подтвердить телеметрию:** зафиксируйте snapshots `/v2/sorafs/capacity/state` и `/v2/sorafs/capacity/telemetry` для провайдера.
- **Уведомить стейкхолдеров:** Storage Team (операции провайдера), Governance Council (орган решения), Observability (обновления дашбордов).

## 2. Подготовить пакет доказательств

1. Соберите сырые артефакты (telemetry JSON, логи CLI, заметки аудитора).
2. Нормализуйте в детерминированный архив (например, tarball); зафиксируйте:
   - digest BLAKE3-256 (`evidence_digest`)
   - тип медиа (`application/zip`, `application/jsonl` и т.д.)
   - URI размещения (object storage, SoraFS pin или endpoint, доступный через Torii)
3. Сохраните пакет в governance evidence bucket с доступом write-once.

## 3. Подать спор

1. Создайте JSON spec для `sorafs_manifest_stub capacity dispute`:

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
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. Проверьте `dispute_summary.json` (подтвердите тип, digest доказательств и временные метки).
4. Отправьте JSON запроса в Torii `/v2/sorafs/capacity/dispute` через очередь governance-транзакций. Зафиксируйте значение ответа `dispute_id_hex`; оно якорит последующие действия по отзыву и аудиторские отчеты.

## 4. Эвакуация и отзыв

1. **Окно льготы:** уведомите провайдера о грядущем отзыве; разрешите эвакуацию закрепленных данных, когда это допускает политика.
2. **Сгенерируйте `ProviderAdmissionRevocationV1`:**
   - Используйте `sorafs_manifest_stub provider-admission revoke` с утвержденной причиной.
   - Проверьте подписи и digest отзыва.
3. **Опубликуйте отзыв:**
   - Отправьте запрос на отзыв в Torii.
   - Убедитесь, что adverts провайдера заблокированы (ожидайте роста `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Обновите дашборды:** отметьте провайдера как отозванного, укажите ID спора и приложите ссылку на пакет доказательств.

## 5. Post-mortem и последующие действия

- Зафиксируйте таймлайн, корневую причину и меры ремедиации в трекере инцидентов governance.
- Определите реституцию (slashing stake, clawbacks комиссий, возвраты клиентам).
- Документируйте выводы; обновите пороги SLA или алерты мониторинга при необходимости.

## 6. Справочные материалы

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел споров)
- `docs/source/sorafs/provider_admission_policy.md` (workflow отзыва)
- Дашборд наблюдаемости: `SoraFS / Capacity Providers`

## Чеклист

- [ ] Пакет доказательств собран и захеширован.
- [ ] Payload спора валидирован локально.
- [ ] Torii-транзакция спора принята.
- [ ] Отзыв выполнен (если одобрен).
- [ ] Дашборды/ранбуки обновлены.
- [ ] Post-mortem оформлен в совете governance.
