---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: دليل مانيفست الـpuesta en escena
sidebar_label: دليل مانيفست الـpuesta en escena
descripción: قائمة تحقق لتمكين ملف chunker المصادق عليه برلمانياً على نشرات Torii الخاصة بـ staging.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Aquí está el software Docusaurus y el Markdown de Sphinx.
:::

## نظرة عامة

يرشد هذا الدليل إلى تمكين ملف chunker المصادق عليه برلمانياً في نشر Torii الخاص بـ staging قبل ترقية التغيير إلى الإنتاج. يفترض أن ميثاق حوكمة SoraFS تم التصديق عليه وأن الـ accesorios المعتمدة متاحة داخل المستودع.

## 1. المتطلبات المسبقة

1. زامن الـ accesorios المعتمدة والتواقيع:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. حضّر دليل أظرف القبول الذي يقرأه Torii عند الإقلاع (مسار مثال): `/var/lib/iroha/admission/sorafs`.
3. تأكد من أن إعدادات Torii تفعّل مخبأ descubrimiento y تطبيق القبول:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. نشر أظرف القبول

1. انسخ أظرف قبول المزوّدين المعتمدة إلى الدليل المشار إليه في `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Presione Torii (si usa SIGHUP, presione el botón de encendido).
3. راقب السجلات لرسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من انتشار descubrimiento

1. Anuncio del proveedor de انشر حمولة الموقعة (بايتات Norito) الناتجة عن خط المزوّد:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. استعلم عن نقطة descubrimiento y تأكد من ظهور الإعلان مع الأسماء المستعارة المعتمدة:```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Utilice el `profile_aliases` y el `"sorafs.sf1@1.0.0"`.

## 4. Manifiesto y plan de اختبار نقاط نهاية

1. اجلب بيانات manifest الوصفية (يتطلب stream token إذا كان القبول مفعّلًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Insertar JSON y crear:
   - Aquí `chunk_profile_handle` y `sorafs.sf1@1.0.0`.
   - أن `manifest_digest_hex` يطابق تقرير الحتمية.
   - أن `chunk_digests_blake3` تتطابق مع الـ accesorios المعاد توليدها.

## 5. فحوصات التليمترية

- تأكد من أن Prometheus يعرض مقاييس الملف الجديد:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المتابعة مزوّد puesta en escena تحت الاسم المستعار المتوقع وأن تبقى عدادات apagón عند الصفر أثناء تفعيل الملف.

## 6. الجاهزية للإطلاق

1. التقط تقريرًا مختصرًا يتضمن عناوين URL y manifest ولقطة التليمترية.
2. شارك التقرير في قناة Nexus rollout مع نافذة التفعيل المخطط لها في الإنتاج.
3. انتقل إلى قائمة التحقق الخاصة بالإنتاج (Sección 4 في `chunker_registry_rollout_checklist.md`) بعد موافقة أصحاب المصلحة.

الحفاظ على هذا الدليل محدثًا يضمن أن كل إطلاق لـ fragmentador/admisión يتبع نفس الخطوات الحتمية بين puesta en escena y والإنتاج.