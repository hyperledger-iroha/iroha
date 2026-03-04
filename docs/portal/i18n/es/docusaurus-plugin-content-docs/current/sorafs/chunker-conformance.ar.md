---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: دليل مطابقة fragmentador في SoraFS
sidebar_label: fragmentador de archivos
descripción: متطلبات وتدفقات عمل للحفاظ على ملف fragmentador SF1 عبر accesorios y SDK.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/chunker_conformance.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

يوثق هذا الدليل المتطلبات التي يجب على كل تطبيق اتباعها للبقاء متوافقاً مع ملف chunker الحتمي في SoraFS (SF1).
كما يوثق سير إعادة التوليد، وسياسة التوقيع، وخطوات التحقق كي يبقى مستهلكو عبر SDKs متزامنين.

## الملف المعتمد

- Nombre del usuario: `sorafs.sf1@1.0.0` (البديل القديم `sorafs.sf1@1.0.0`)
- بذرة الإدخال (hexadecimal): `0000000000dec0ded`
- Tamaño del archivo: 262144 bytes (256 KiB)
- Tamaño del archivo: 65536 bytes (64 KiB)
- Tamaño del archivo: 524288 bytes (512 KiB)
- Nombre del usuario: `0x3DA3358B4DC173`
- Equipo de cambio: `sorafs-v1-gear`
- Nombre del usuario: `0x0000FFFF`

التطبيق المرجعي: `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب أن ينتج أي تسريع SIMD نفس الحدود والـ resúmenes.

## accesorios de حزمة

`cargo run --locked -p sorafs_chunker --bin export_vectors` يعيد توليد
accesorios ويصدر الملفات التالية ضمن `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}`: fragmento de código abierto Rust, TypeScript y Go.
  يعلن كل ملف المقبض المعتمد كأول إدخال في `profile_aliases`, يتبعه أي بدائل قديمة (مثل
  `sorafs.sf1@1.0.0` o `sorafs.sf1@1.0.0`). يتم فرض الترتيب بواسطة
  `ensure_charter_compliance` Y esta es la respuesta.
- `manifest_blake3.json` — manifiesto de instalación de accesorios.
- `manifest_signatures.json` — توقيعات المجلس (Ed25519) على resumen الخاص بالـ manifiesto.
- `sf1_profile_v1_backpressure.json` y corpus الخام داخل `fuzz/` —
  سيناريوهات بث حتمية تُستخدم في اختبارات للـ fragmentador de contrapresión.

### سياسة التوقيع

يجب أن تشمل إعادة توليد accesorios توقيعاً صالحاً من المجلس. يرفض المولد
الإخراج غير الموقّع ما لم يتم تمرير `--allow-unsigned` صراحة (مخصص
للتجارب المحلية فقط). Haga clic en solo agregar y haga clic en el enlace.

لإضافة توقيع من المجلس:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

يعيد مساعد CI `ci/check_sorafs_fixtures.sh` تشغيل المولد مع
`--locked`. إذا انحرفت accesorios أو غابت التواقيع، تفشل المهمة. استخدم
هذا السكربت في flujos de trabajo الليلية وقبل إرسال تغييرات accesorios.

خطوات التحقق اليدوية:

1. Utilice `cargo test -p sorafs_chunker`.
2. نفّذ `ci/check_sorafs_fixtures.sh` محلياً.
3. Introduzca el código `git status -- fixtures/sorafs_chunker`.

## دليل الترقية

Aquí está el fragmento de SF1:

Nombre del artículo: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) لمتطلبات
البيانات الوصفية وقوالب المقترح وقوائم التحقق.1. Haga clic en `ChunkProfileUpgradeProposalV1` (RFC SF-1).
2. أعد توليد accesorios عبر `export_vectors` وسجل digest الجديد للـ manifest.
3. وقّع الـ manifiesto بحصة المجلس المطلوبة. Esto se debe a que está conectado a `manifest_signatures.json`.
4. Instale accesorios en los SDK (Rust/Go/TS) y utilice otros dispositivos.
5. أعد توليد corpora fuzz إذا تغيرت المعلمات.
6. حدّث هذا الدليل بالمقبض الجديد للملف والبذور وdigest.
7. قدّم التغيير مع الاختبارات المحدثة وتحديثات hoja de ruta.

التغييرات التي تؤثر على حدود الـ trozos أو الـ resúmenes دون اتباع هذه العملية
غير صالحة ولا يجب دمجها.