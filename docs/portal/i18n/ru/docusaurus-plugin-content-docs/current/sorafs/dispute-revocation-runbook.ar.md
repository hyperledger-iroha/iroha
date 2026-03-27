---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: книга споров-отзывов
Название: دليل تشغيل نزاعات وإلغاءات SoraFS
Sidebar_label: دليل تشغيل النزاعات والإلغاءات
описание: سير عمل الحوكمة لتقديم نزاعات سعة SoraFS وتنسيق الإلغاءات وإخلاء Он сказал, что это действительно так.
---

:::примечание
Был установлен `docs/source/sorafs/dispute_revocation_runbook.md`. Он был назван в честь Сфинкса в Сфинксе.
:::

## الهدف

Он был создан в 18NT00000002X, на сайте SoraFS. Он был убит в 2007 году в 1980-х годах.

## 1. تقييم الحادث

- **Обязательство:** Соглашение об уровне обслуживания (соглашение/открытие PoR), а также соглашение об уровне обслуживания.
- **Отключено:** Установите `/v1/sorafs/capacity/state` и `/v1/sorafs/capacity/telemetry`.
- **Обработка данных:** Группа хранения (عمليات المزوّد), Совет управления (جهة القرار), Наблюдательность (تحديثات لوحات). المتابعة).

## 2. إعداد حزمة الأدلة

1. Создание файла телеметрии (телеметрия JSON, интерфейс CLI, удаленный доступ).
2. Создано в формате tarball; Ответ:
   - дайджест BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`, `application/jsonl`, وما إلى ذلك)
   - URI-запрос (хранилище объектов, контакт SoraFS, а также входной сигнал Torii)
3. Он был выбран в честь Дня святого Валентина в Вашингтоне. واحدة.

## 3. تقديم النزاع

1. Создайте файл JSON для `sorafs_manifest_stub capacity dispute`:

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

2. Интерфейс командной строки:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. راجع `dispute_summary.json` (отправлено в дайджест الأدلة, والطوابع الزمنية).
4. Создайте JSON-файл Torii `/v1/sorafs/capacity/dispute` и сохраните его. Дополнительный файл `dispute_id_hex`; Он был убит в 1990-х годах.

## 4. الإخلاء والإلغاء

1. **Настройка:** أخطر المزوّد بقرب الإلغاء؛ Он был убит в 2007 году.
2. **Вход `ProviderAdmissionRevocationV1`:**
   - استخدم `sorafs_manifest_stub provider-admission revoke` в приложении.
   - تحقّق в التواقيع и дайджесте الإلغاء.
3. **Напоминание:**
   - أرسل طلب الإلغاء إلى Torii.
   - Показана реклама الخاصة بالمزوّد (توقع ارتفاع `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Обращение к врачу:** علّم المزوّد على أنه مُلغى, وأشر إلى معرّف Вы можете сделать это.

## 5. ما بعد الحادث والمتابعة

- Скалли Уилсон и его коллега по работе над фильмом "Мир" حوكمة.
- حدّد التعويض (режущие للرهان, откатные удары للرسوم, وتعويضات العملاء).
- وثّق الدروس المستفادة؛ Это соглашение об уровне обслуживания и соглашение о сотрудничестве.

## 6. Удалить

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم النزاعات)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- Код запроса: `SoraFS / Capacity Providers`.

## قائمة التحقق

- [ ] تم التقاط حزمة الأدلة واحتساب التجزئة.
- [ ] تم التحقق من حمولة النزاع محليًا.
- [ ] تم قبول معاملة النزاع في Torii.
- [ ] تم تنفيذ الإلغاء (إن تمت الموافقة).
- [ ] تم تحديث لوحات المتابعة/الأدلة التشغيلية.
- [ ] تم إيداع ما بعد الحادث لدى مجلس الحوكمة.