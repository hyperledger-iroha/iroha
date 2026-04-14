<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Runbook مشغل QR غير متصل

يحدد دليل التشغيل هذا الإعدادات العملية المسبقة لـ `ecc`/dimension/fps لضوضاء الكاميرا
البيئات عند استخدام نقل QR دون اتصال بالإنترنت.

### الإعدادات المسبقة الموصى بها

| البيئة | النمط | إي سي سي | البعد | إطارا في الثانية | حجم القطعة | مجموعة التكافؤ | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- | --- |
| التحكم في الإضاءة قصيرة المدى | `sakura` | `M` | `360` | `12` | `360` | `0` | أعلى إنتاجية، والحد الأدنى من التكرار. |
| ضجيج كاميرا الهاتف المحمول النموذجي | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | الإعداد المسبق المتوازن المفضل (`~3 KB/s`) للأجهزة المختلطة. |
| وهج عالي، ضبابية الحركة، كاميرات منخفضة الجودة | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | إنتاجية أقل، وأقوى مرونة في فك التشفير. |

### قائمة التحقق من التشفير/فك التشفير

1. قم بالتشفير باستخدام مقابض النقل الصريحة.
2. التحقق من الصحة من خلال التقاط حلقة الماسح الضوئي قبل بدء التشغيل.
3. قم بتثبيت ملف تعريف النمط نفسه في مساعدات تشغيل SDK للحفاظ على تكافؤ المعاينة.

مثال:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### التحقق من صحة حلقة الماسح الضوئي (ملف تعريف sakura-storm بسرعة 3 كيلوبايت/ثانية)

استخدم نفس ملف تعريف النقل عبر جميع مسارات الالتقاط:

-`chunk_size=336`
-`parity_group=4`
-`fps=12`
-`style=sakura-storm`

أهداف التحقق:- آي أو إس: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- أندرويد: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- المتصفح/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

القبول:

- تنجح إعادة بناء الحمولة النافعة بالكامل مع إسقاط إطار بيانات واحد لكل مجموعة تكافؤ.
- لا يوجد عدم تطابق في المجموع الاختباري/تجزئة الحمولة في حلقة الالتقاط العادية.