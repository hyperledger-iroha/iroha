---
lang: ar
direction: rtl
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/release_artifact_selection.md -->

# اختيار artefacts اصدار Iroha

توضح هذه المذكرة اي artefacts (bundles وصور حاويات) يجب على المشغلين نشرها لكل ملف اصدار.

## الملفات

- **iroha2 (Self-hosted networks)** — تهيئة lane واحدة تطابق `defaults/genesis.json` و `defaults/client.toml`.
- **iroha3 (SORA Nexus)** — تهيئة Nexus متعددة lanes باستخدام قوالب `defaults/nexus/*`.

## Bundles (ثنائيات)

يتم انتاج bundles عبر `scripts/build_release_bundle.sh` مع `--profile` مضبوط على `iroha2` او `iroha3`.

يحتوي كل tarball على:

- `bin/` — `irohad` و `iroha` و `kagami` مبنية بملف النشر.
- `config/` — تهيئة genesis/client حسب الملف (single vs. nexus). bundles الخاصة بـ Nexus تتضمن `config.toml` مع معاملات lanes و DA.
- `PROFILE.toml` — بيانات وصفية تصف الملف والتهيئة والنسخة و commit و OS/arch ومجموعة الميزات المفعلة.
- artefacts بيانات وصفية تكتب بجانب tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` و `.pub` (عند توفير `--signing-key`)
  - `<profile>-<version>-manifest.json` يسجل مسار tarball و hash وتفاصيل التوقيع

## صور الحاويات

يتم انتاج صور الحاويات عبر `scripts/build_release_image.sh` بنفس معاملات profile/config.

المخرجات:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- توقيع/مفتاح عام اختياري (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` يسجل tag ومعرف الصورة و hash وبيانات التوقيع

## اختيار artefact الصحيح

1. حدد سطح النشر:
   - **SORA Nexus / multi-lane** -> استخدم bundle و image الخاصين بـ `iroha3`.
   - **Self-hosted single-lane** -> استخدم artefacts الخاصة بـ `iroha2`.
   - عند الشك، شغل `scripts/select_release_profile.py --network <alias>` او `--chain-id <id>`؛ يقوم helper بمواءمة الشبكات مع الملف الصحيح عبر `release/network_profiles.toml`.
2. حمّل tarball المطلوب وملفات manifest المرافقة. تحقق من SHA256 والامضاء قبل فك الضغط:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. استخرج bundle (`tar --use-compress-program=zstd -xf <tar>`) وضع `bin/` في PATH الخاص بالنشر. طبق overrides للتهيئة المحلية عند الحاجة.
4. حمّل صورة الحاوية باستخدام `docker load -i <profile>-<version>-<os>-image.tar` اذا كنت تستخدم نشر عبر الحاويات. تحقق من hash/التوقيع كما في الاعلى قبل التحميل.

## قائمة تحقق لتكوين Nexus

- يجب ان يتضمن `config/config.toml` اقسام `[nexus]` و `[nexus.lane_catalog]` و `[nexus.dataspace_catalog]` و `[nexus.da]`.
- تاكد من ان قواعد توجيه lanes تطابق توقعات governance (`nexus.routing_policy`).
- تحقق من ان عتبات DA (`nexus.da`) ومعاملات fusion (`nexus.fusion`) متوافقة مع اعدادات المجلس المعتمدة.

## قائمة تحقق لتكوين single-lane

- يجب ان يحتوي `config/config.d` (ان وجد) على overrides لـ single-lane فقط بدون اقسام `[nexus]`.
- تاكد ان `config/client.toml` يشير الى Torii endpoint وقائمة peers المقصودة.
- يجب ان يحافظ genesis على domains/assets الكانونية لشبكة self-hosted.

## مرجع سريع للادوات

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — مسار onboarding شامل لمشغلي data-space في Sora Nexus بعد اختيار artefacts.

</div>
