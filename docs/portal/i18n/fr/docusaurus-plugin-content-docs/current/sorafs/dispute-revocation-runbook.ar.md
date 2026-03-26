---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : litige-révocation-runbook
titre : دليل تشغيل نزاعات وإلغاءات SoraFS
sidebar_label : دليل تشغيل النزاعات والإلغاءات
description: سير عمل الحوكمة لتقديم نزاعات سعة SoraFS وتنسيق الإلغاءات وإخلاء البيانات بشكل حتمي.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/dispute_revocation_runbook.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب توثيق Sphinx القديم.
:::

## الهدف

يرشد هذا الدليل مشغلي الحوكمة خلال تقديم نزاعات سعة SoraFS, وتنسيق الإلغاءات، وضمان اكتمال إخلاء البيانات بشكل حتمي.

## 1. تقييم الحادث

- **شروط الإطلاق :** رصد خرق SLA (التوفر/فشل PoR) et نقص التكرار، أو خلاف في الفوترة.
- **أكيد التليمترية:** التقط لقطات `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` للمزوّد.
- **إخطار أصحاب المصلحة :** Équipe de stockage (عمليات المزوّد), Conseil de gouvernance (جهة القرار) et Observabilité (تحديثات لوحات المتابعة).

## 2. إعداد حزمة الأدلة

1. اجمع الآرتيفاكتات الخام (télémétrie JSON, سجلات CLI, ملاحظات المدققين).
2. وحّدها في أرشيف حتمي (مثل tarball)؛ وسجّل:
   - résumé BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`, `application/jsonl`, وما إلى ذلك)
   - URI de stockage (stockage d'objets, ou broche SoraFS, ou connexion avec Torii)
3. خزّن الحزمة في حاوية جمع الأدلة الخاصة بالحوكمة مع وصول كتابة لمرة واحدة.

## 3. تقديم النزاع

1. Utiliser JSON pour `sorafs_manifest_stub capacity dispute` :

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

2. Cliquez sur CLI :

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```3. راجع `dispute_summary.json` (تأكد من النوع، digest الأدلة، والطوابع الزمنية).
4. Utilisez JSON pour Torii `/v1/sorafs/capacity/dispute` pour créer des liens. التقط قيمة الاستجابة `dispute_id_hex`؛ فهي تثبّت إجراءات الإلغاء اللاحقة وتقارير التدقيق.

## 4. الإخلاء والإلغاء

1. **نافذة السماح :** أخطر المزوّد بقرب الإلغاء؛ واسمح بإخلاء البيانات المثبتة عندما تسمح السياسة.
2. **أنشئ `ProviderAdmissionRevocationV1` :**
   - استخدم `sorafs_manifest_stub provider-admission revoke` مع السبب المعتمد.
   - تحقّق من التواقيع وdigest الإلغاء.
3. **نشر الإلغاء :**
   - Sélectionnez Torii.
   - تأكد من حظر adverts الخاصة بالمزوّد (توقع ارتفاع `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **حدّث لوحات المتابعة:** علّم المزوّد على أنه مُلغى، وأشر إلى معرّف النزاع، واربط حزمة الأدلة.

## 5. ما بعد الحادث والمتابعة

- سجّل الجدول الزمني والسبب الجذري وإجراءات المعالجة في متتبع حوادث الحوكمة.
- حدّد التعويض (réduction des récupérations للرهان، للرسوم، وتعويضات العملاء).
- وثّق الدروس المستفادة؛ Il s'agit d'un contrat SLA et d'un contrat SLA.

## 6. مواد مرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم النزاعات)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- Nom du produit : `SoraFS / Capacity Providers`

## قائمة التحقق

- [ ] تم التقاط حزمة الأدلة واحتساب التجزئة.
- [ ] تم التحقق من حمولة النزاع محليًا.
- [ ] تم قبول معاملة النزاع في Torii.
- [ ] تم تنفيذ الإلغاء (إن تمت الموافقة).
- [ ] تم تحديث لوحات المتابعة/الأدلة التشغيلية.
- [ ] تم إيداع ما بعد الحادث لدى مجلس الحوكمة.