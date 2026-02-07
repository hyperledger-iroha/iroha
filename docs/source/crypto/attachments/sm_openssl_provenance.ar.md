---
lang: ar
direction: rtl
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo لقطة المصدر
% تم الإنشاء: 2026-01-30

# ملخص البيئة

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: التقارير `LibreSSL 3.3.6` (مجموعة أدوات TLS المقدمة من النظام على نظام التشغيل macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: راجع `sm_iroha_crypto_tree.txt` للتعرف على مجموعة تبعيات Rust الدقيقة (صندوق `openssl` v0.10.74، `openssl-sys` v0.9.x، مصادر OpenSSL 3.x المتوفرة عبر صندوق `openssl-src`؛ الميزة تم تمكين `vendored` في `crates/iroha_crypto/Cargo.toml` لبنيات المعاينة الحتمية).

# ملاحظات

- روابط بيئة التطوير المحلية مقابل رؤوس/مكتبات LibreSSL؛ يجب أن تستخدم إصدارات معاينة الإنتاج OpenSSL >= 3.0.0 أو Tongsuo 8.x. استبدل مجموعة أدوات النظام أو قم بتعيين `OPENSSL_DIR`/`PKG_CONFIG_PATH` عند إنشاء حزمة المنتجات النهائية.
- قم بإعادة إنشاء هذه اللقطة داخل بيئة بناء الإصدار لالتقاط تجزئة OpenSSL/Tongsuo tarball الدقيقة (`openssl version -v`، `openssl version -b`، `openssl version -f`) وإرفاق البرنامج النصي/المجموع الاختباري للبناء القابل للتكرار. بالنسبة للإصدارات الموردة، سجل إصدار/التزام الصندوق `openssl-src` المستخدم بواسطة Cargo (مرئي في `target/debug/build/openssl-sys-*/output`).
- يحتاج مضيفو Apple Silicon إلى `RUSTFLAGS=-Aunsafe-code` عند تشغيل مجموعة دخان OpenSSL، لذلك يتم تجميع قواعد تسريع AArch64 SM3/SM4 (لا تتوفر العناصر الجوهرية على نظام التشغيل macOS). يقوم البرنامج النصي `scripts/sm_openssl_smoke.sh` بتصدير هذه العلامة قبل استدعاء `cargo` للحفاظ على اتساق CI والتشغيل المحلي.
- إرفاق مصدر المصدر الأولي (على سبيل المثال، `openssl-src-<ver>.tar.gz` SHA256) بمجرد تثبيت خط أنابيب التغليف؛ استخدام نفس التجزئة في المصنوعات اليدوية CI.