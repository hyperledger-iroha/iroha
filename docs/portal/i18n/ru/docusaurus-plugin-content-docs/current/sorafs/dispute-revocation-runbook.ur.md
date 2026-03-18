---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
title: SoraFS تنازع اور منسوخی رن بک
Sidebar_label: Нажмите здесь, чтобы добавить ссылку.
описание: SoraFS. Если вы хотите, чтобы вы знали, как это сделать, вы можете сделать это.
---

:::примечание
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx ڈاکیومنٹیشن ریٹائر نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

Если вы хотите использовать SoraFS, вы можете использовать это приложение Если вы хотите, чтобы у вас было хорошее настроение, вы можете сделать это, когда хотите رہنمائی کرتی ہے۔

## 1. واقعے کا جائزہ

- **Ошибки:** условия SLA (время безотказной работы/сбой PoR), нехватка репликации, разногласия по выставлению счетов и другие проблемы.
- **Обратите внимание:** Используйте снимки `/v1/sorafs/capacity/state` и `/v1/sorafs/capacity/telemetry`. کریں۔
- **Общие сведения:** Группа хранения (операции поставщика), Совет управления (орган принятия решений), Наблюдательность (обновления информационной панели).

## 2. شواہد کا پیکج تیار کریں

1. Наличие артефактов (телеметрия JSON, журналы CLI, заметки аудитора).
2. Создать архив (tarball) и нормализовать работу. Ответ:
   - Дайджест BLAKE3-256 (`evidence_digest`)
   - тип носителя (`application/zip`, `application/jsonl` وغیرہ)
   - URI хостинга (хранилище объектов, контакт SoraFS, или Torii-доступная конечная точка)
3. Корзина для сбора доказательств. Возможность однократной записи.

## 3. تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` или спецификация JSON:

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

2. Возможности CLI:

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

3. `dispute_summary.json` ریویو کریں (вид, дайджест доказательств, временные метки)۔
4. Создайте файл Torii `/v1/sorafs/capacity/dispute` для просмотра JSON بھیجیں۔ جواب کی قدر `dispute_id_hex` محفوظ کریں؛ یہی بعد کی منسوخی کارروائیوں اور آڈٹ رپورٹس کا اینکر ہے۔

## 4. انخلا اور منسوخی

1. **Окно благодати:** پالیسی اجازت دے تو закрепленные данные کے انخلا کی اجازت دیں۔
2. **`ProviderAdmissionRevocationV1` Примечание:**
   - Для получения дополнительной информации используйте `sorafs_manifest_stub provider-admission revoke`.
   - دستخط اور дайджест отзыва
3. **Отличный вариант:**
   - منسوخی ریکوئسٹ Torii کو جمع کریں۔
   - یقینی بنائیں کہ پرووائیڈر реклама بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے)۔
4. **Панель управления доступна:** Вы можете отозвать идентификатор, если хотите оспорить идентификатор. Пакет доказательств لنک کریں۔

## 5. Вскрытие тела

- Устранение первопричины и устранение причин, а также возможность отслеживания инцидентов.
- реституция طے کریں (снижение ставок, возврат комиссий, возврат средств клиентам)۔
- سیکھے گئے اسباق دستاویز کریں؛ Поддержка пороговых значений SLA и оповещений мониторинга.

## 6. حوالہ جاتی مواد

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (раздел споров)
- `docs/source/sorafs/provider_admission_policy.md` (рабочий процесс отзыва)
- Панель наблюдения: `SoraFS / Capacity Providers`

## چیک لسٹ- [ ] пакет доказательств حاصل کر کے کر لیا گیا۔
- [ ] спор о полезной нагрузке مقامی طور پر validate کیا گیا۔
- [ ] Torii спор ٹرانزیکشن قبول ہوئی۔
- [ ] منسوخی نافذ کی گئی (اگر منظور ہو)۔
- [ ] информационные панели/runbooks اپڈیٹ ہوئے۔
- [ ] Посмертное исследование, которое может быть проведено после смерти