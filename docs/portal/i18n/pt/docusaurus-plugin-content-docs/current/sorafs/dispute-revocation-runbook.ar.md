---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: دليل تشغيل نزاعات وإلغاءات SoraFS
sidebar_label: دليل تشغيل النزاعات والإلغاءات
description: Você pode usar o SoraFS para obter mais detalhes e obter mais informações. Sim.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/dispute_revocation_runbook.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب توثيق Sphinx القديم.
:::

## الهدف

Você pode usar o SoraFS para obter informações sobre o produto SoraFS. Faça isso com cuidado.

## 1. تقييم الحادث

- **شروط الإطلاق:** رصد خرق SLA (التوفر/فشل PoR), نقص التكرار, أو خلاف في الفوترة.
- **تأكيد التليمترية:** Use o `/v1/sorafs/capacity/state` e o `/v1/sorafs/capacity/telemetry` para `/v1/sorafs/capacity/telemetry`.
- **إخطار أصحاب المصلحة:** Equipe de armazenamento (عمليات المزوّد), Conselho de governança (جهة القرار), Observabilidade (تحديثات لوحات المتابعة).

## 2. إعداد حزمة الأدلة

1. اجمع الآرتيفاكتات الخام (telemetria JSON, سجلات CLI, ملاحظات المدققين).
2. وحّدها في أرشيف حتمي (مثل tarball), Descrição:
   - digerir BLAKE3-256 (`evidence_digest`)
   - Nome de usuário (`application/zip`, `application/jsonl`, e não mais)
   - URI الاستضافة (object storage, ou SoraFS pin, e نقطة نهاية متاحة عبر Torii)
3. Faça o download do seu telefone e coloque-o no lugar certo.

## 3. تقديم النزاع

1. Insira o JSON para `sorafs_manifest_stub capacity dispute`:

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

2. CLI:

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

3. راجع `dispute_summary.json` (تأكد من النوع, resumo do site e والطوابع الزمنية).
4. Use JSON como Torii `/v1/sorafs/capacity/dispute` para obter o valor desejado. Descrição do produto `dispute_id_hex`; Isso é feito por meio de uma chave de fenda.

## 4. الإخلاء والإلغاء

1. **نافذة السماح:** أخطر المزوّد بقرب الإلغاء؛ واسمح بإخلاء البيانات المثبتة عندما تسمح السياسة.
2. **أنشئ `ProviderAdmissionRevocationV1`:**
   - Coloque `sorafs_manifest_stub provider-admission revoke` no lugar certo.
   - تحقّق من التواقيع e Digest الإلغاء.
3. **نشر الإلغاء:**
   - Verifique o valor do Torii.
   - تأكد من حظر adverts الخاصة بالمزوّد (توقع ارتفاع `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **حدّث لوحات المتابعة:** علّم المزوّد على أنه مُلغى, وأشر إلى معرّف النزاع, واربط حزمة الأدلة.

## 5. ما بعد الحادث والمتابعة

- سجّل الجدول الزمني والسبب الجذري وإجراءات المعالجة في متتبع حوادث الحوكمة.
- حدّد التعويض (cortando للرهان, grabbacks للرسوم, وتعويضات العملاء).
- وثّق الدروس المستفادة؛ Verifique o SLA ou o contrato de segurança.

## 6. مواد مرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (referência)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- Nome do usuário: `SoraFS / Capacity Providers`

## قائمة التحقق

- [ ] تم التقاط حزمة الأدلة واحتساب التجزئة.
- [ ] تم التحقق من حمولة النزاع محليًا.
- [ ] Você pode usar o recurso Torii.
- [ ] تم تنفيذ الإلغاء (إن تمت الموافقة).
- [ ] تم تحديث لوحات المتابعة/الأدلة التشغيلية.
- [ ] تم إيداع ما بعد الحادث لدى مجلس الحوكمة.