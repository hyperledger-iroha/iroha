---
lang: ar
direction: rtl
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/docker_build.md -->

# صورة Docker Builder

هذه الحاوية معرفة في `Dockerfile.build` وتضم جميع تبعيات toolchain المطلوبة لعمليات CI وبناء
الاصدارات محليا. الصورة تعمل الان كمستخدم غير root بشكل افتراضي، لذلك تستمر عمليات Git في
العمل مع حزمة `libgit2` على Arch Linux دون الحاجة الى حل `safe.directory` العام.

## معاملات البناء

- `BUILDER_USER` - اسم تسجيل الدخول الذي ينشأ داخل الحاوية (الافتراضي: `iroha`).
- `BUILDER_UID` - رقم هوية المستخدم (الافتراضي: `1000`).
- `BUILDER_GID` - رقم هوية المجموعة الاساسية (الافتراضي: `1000`).

عند ربط workspace من جهازك، مرر قيم UID/GID المطابقة كي تبقى artefacts المولدة قابلة للكتابة:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

ادلة toolchain (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`) مملوكة للمستخدم
المضبوط بحيث تبقى اوامر Cargo و rustup و Poetry تعمل بالكامل بعد اسقاط صلاحيات root.

## تشغيل البناء

اربط workspace الخاص بك بـ `/workspace` (وهو `WORKDIR` للحاوية) عند تشغيل الصورة. مثال:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

الصورة تحتفظ بعضوية مجموعة `docker` كي تبقى اوامر Docker المتداخلة (مثل `docker buildx bake`)
متاحة لسير عمل CI التي تربط PID والمقبس من المضيف. عدل تعيينات المجموعات حسب بيئتك.

## artefacts الخاصة بـ Iroha 2 مقابل Iroha 3

مساحة العمل تصدر الان ثنائيات منفصلة لكل خط اصدار لتجنب التعارض:
`iroha3`/`iroha3d` (افتراضي) و `iroha2`/`iroha2d` (Iroha 2). استخدم المساعدات لانتاج الزوج
المطلوب:

- `make build` (او `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) لـ Iroha 3
- `make build-i2` (او `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) لـ Iroha 2

المحدد يثبت مجموعات الميزات (`telemetry` + `schema-endpoint` مع علامة الخط `build-i{2,3}`)
حتى لا تلتقط builds الخاصة بـ Iroha 2 افتراضيات Iroha 3 بالخطا.

حزم الاصدار المبنية عبر `scripts/build_release_bundle.sh` تختار اسماء الثنائيات الصحيحة
تلقائيا عند ضبط `--profile` على `iroha2` او `iroha3`.

</div>
