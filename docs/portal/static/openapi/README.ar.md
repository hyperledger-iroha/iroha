---
lang: ar
direction: rtl
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

توقيع OpenAPI
-------------

- يجب توقيع مواصفة OpenAPI الخاصة بـ Torii (`torii.json`)، ويتم التحقق من المانيفست بواسطة `cargo xtask openapi-verify`.
- مفاتيح التوقيع المسموح بها موجودة في `allowed_signers.json`؛ قم بتدوير هذا الملف كلما تغيّر مفتاح التوقيع. أبقِ حقل `version` على القيمة `1`.
- يفرض CI (`ci/check_openapi_spec.sh`) قائمة السماح بالفعل لكل من مواصفتَي latest و current. إذا استهلكت بوابة أخرى أو خط أنابيب آخر المواصفة الموقّعة، فاجعل خطوة التحقق تشير إلى ملف allowlist نفسه لتجنب الانحراف.
- لإعادة التوقيع بعد تدوير المفاتيح:
  1. حدّث `allowed_signers.json` بالمفتاح العام الجديد.
  2. أعد توليد المواصفة ووقّعها: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. أعد تشغيل `ci/check_openapi_spec.sh` (أو `cargo xtask openapi-verify` يدويًا) للتأكد من أن المانيفست يطابق allowlist.
