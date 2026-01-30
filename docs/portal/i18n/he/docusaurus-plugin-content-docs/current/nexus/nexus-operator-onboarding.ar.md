---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operator-onboarding
title: تاهيل مشغلي data-space في Sora Nexus
description: نسخة مطابقة لـ `docs/source/sora_nexus_operator_onboarding.md` تتبع قائمة تدقيق الاصدار الشاملة لمشغلي Nexus.
---

:::note المصدر القانوني
تعكس هذه الصفحة `docs/source/sora_nexus_operator_onboarding.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# تاهيل مشغلي Data-Space في Sora Nexus

يلتقط هذا الدليل التدفق الشامل الذي يجب ان يتبعه مشغلو data-space في Sora Nexus بعد الاعلان عن اصدار. وهو يكمل دليل المسارين (`docs/source/release_dual_track_runbook.md`) ومذكرة اختيار القطع (`docs/source/release_artifact_selection.md`) عبر شرح كيفية مواءمة الحزم/الصور التي تم تنزيلها وملفات manifest وقوالب الاعداد مع توقعات الـ lane العالمية قبل تشغيل العقدة.

## الجمهور والمتطلبات المسبقة
- تم اعتمادك من برنامج Nexus واستلمت تعيين data-space (فهرس lane، ومعرف/alias للـ data-space، ومتطلبات سياسة التوجيه).
- يمكنك الوصول الى قطع الاصدار الموقعة المنشورة من Release Engineering (tarballs، صور، manifests، توقيعات، مفاتيح عامة).
- قمت بتوليد او استلام مواد مفاتيح الانتاج لدور validator/observer (هوية عقدة Ed25519؛ مفتاح اجماع BLS + PoP للـ validators؛ بالاضافة الى اي مفاتيح او toggles للخصائص السرية).
- يمكنك الوصول الى اقران Sora Nexus الحاليين الذين سيقومون بعملية bootstrap لعقدتك.

## الخطوة 1 - تاكيد ملف الاصدار
1. حدد alias الشبكة او chain ID الذي تم منحه لك.
2. شغل `scripts/select_release_profile.py --network <alias>` (او `--chain-id <id>`) على نسخة من هذا المستودع. يقوم المساعد بقراءة `release/network_profiles.toml` ويطبع ملف النشر. بالنسبة لـ Sora Nexus يجب ان تكون النتيجة `iroha3`. لاي قيمة اخرى، توقف واتصل بـ Release Engineering.
3. سجل وسم الاصدار المشار اليه في اعلان الاصدار (مثلا `iroha3-v3.2.0`); ستستخدمه لجلب القطع وملفات manifest.

## الخطوة 2 - جلب القطع والتحقق منها
1. قم بتنزيل حزمة `iroha3` (`<profile>-<version>-<os>.tar.zst`) وملفاتها المرافقة (`.sha256`، اختياري `.sig/.pub`، `<profile>-<version>-manifest.json`، و `<profile>-<version>-image.json` اذا كنت تنشر عبر الحاويات).
2. تحقق من السلامة قبل فك الضغط:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   استبدل `openssl` بالمدقق المعتمد من المؤسسة اذا كنت تستخدم KMS مدعوما بالاجهزة.
3. افحص `PROFILE.toml` داخل الـ tarball وملفات JSON لتاكيد:
   - `profile = "iroha3"`
   - الحقول `version` و`commit` و`built_at` تطابق اعلان الاصدار.
   - نظام التشغيل/المعمارية تطابق هدف النشر.
4. اذا كنت تستخدم صورة الحاوية، اعد التحقق من hash/التوقيع لملف `<profile>-<version>-<os>-image.tar` وتاكد من image ID المسجل في `<profile>-<version>-image.json`.

## الخطوة 3 - تجهيز الاعداد من القوالب
1. استخرج الحزمة وانسخ `config/` الى المكان الذي ستقرأ منه العقدة الاعدادات.
2. تعامل مع الملفات تحت `config/` كقوالب:
   - استبدل `public_key`/`private_key` بمفاتيح Ed25519 الخاصة بالانتاج. ازل المفاتيح الخاصة من القرص اذا كانت العقدة ستجلبها من HSM; وقم بتحديث الاعداد لربطه بموصل HSM.
   - عدل `trusted_peers` و`network.address` و`torii.address` لتعكس واجهاتك المتاحة و peers bootstrap المخصصة لك.
   - حدث `client.toml` بنقطة نهاية Torii الموجهة للمشغل (بما في ذلك اعداد TLS اذا كان ذلك ينطبق) وبالاعتمادات التي قمت بتجهيزها لادوات التشغيل.
3. احتفظ بالـ chain ID المقدم في الحزمة ما لم توجه Governance خلاف ذلك صراحة - الـ lane العالمي يتوقع معرف سلسلة قانوني واحد.
4. خطط لتشغيل العقدة مع خيار ملف Sora: `irohad --sora --config <path>`. سيقوم محمل الاعدادات برفض اعدادات SoraFS او multi-lane اذا كان الخيار غائبا.

## الخطوة 4 - مواءمة بيانات data-space وسياسات التوجيه
1. عدل `config/config.toml` بحيث تطابق مقطع `[nexus]` كتالوج data-space الذي قدمه Nexus Council:
   - يجب ان يساوي `lane_count` اجمالي الـ lanes المفعلة في الحقبة الحالية.
   - يجب ان تحتوي كل خانة في `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` على `index`/`id` فريد والـ aliases المتفق عليها. لا تحذف الادخالات العالمية الحالية; اضف aliases المفوضة لك اذا خصص المجلس data-spaces اضافية.
   - تاكد من ان كل مدخل dataspace يتضمن `fault_tolerance (f)`; يتم تحديد لجان lane-relay بحجم `3f+1`.
2. حدث `[[nexus.routing_policy.rules]]` لالتقاط السياسة المعطاة لك. القالب الافتراضي يوجه تعليمات الحوكمة الى lane `1` ونشر العقود الى lane `2`; اضف او عدل القواعد حتى يذهب المرور المخصص لـ data-space الخاص بك الى الـ lane والـ alias الصحيحين. نسق مع Release Engineering قبل تغيير ترتيب القواعد.
3. راجع عتبات `[nexus.da]` و`[nexus.da.audit]` و`[nexus.da.recovery]`. من المتوقع ان يحتفظ المشغلون بالقيم المعتمدة من المجلس; لا تعدلها الا اذا تم التصديق على سياسة محدثة.
4. سجل الاعداد النهائي في متعقب العمليات لديك. يتطلب runbook ثنائي المسار ارفاق `config.toml` الفعلي (مع تنقيح الاسرار) بتذكرة التاهيل.

## الخطوة 5 - التحقق قبل التشغيل
1. شغل مدقق الاعداد المدمج قبل الانضمام الى الشبكة:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   يطبع هذا الاعدادات النهائية ويفشل مبكرا اذا كانت ادخالات الكتالوج/التوجيه غير متسقة او اذا كان genesis والاعدادات غير متطابقة.
2. اذا كنت تنشر باستخدام الحاويات، شغل نفس الامر داخل الصورة بعد تحميلها عبر `docker load -i <profile>-<version>-<os>-image.tar` (تذكر تضمين `--sora`).
3. راجع السجلات بحثا عن تحذيرات حول معرفات lane/data-space placeholder. اذا وجدت، ارجع الى الخطوة 4 - لا يجب ان تعتمد عمليات الانتاج على معرفات placeholder المرفقة مع القوالب.
4. نفذ اجراء smoke المحلي (مثلا ارسل استعلام `FindNetworkStatus` عبر `iroha_cli`، وتاكد من ان نقاط نهاية التليمتري تعرض `nexus_lane_state_total`، وتحقق من ان مفاتيح البث تم تدويرها او استيرادها حسب الحاجة).

## الخطوة 6 - التحويل والتسليم
1. خزّن `manifest.json` الذي تم التحقق منه وقطع التوقيع في تذكرة الاصدار حتى يتمكن المدققون من اعادة فحوصاتك.
2. اخطر Nexus Operations ان العقدة جاهزة للادراج; اشمل:
   - هوية العقدة (peer ID، اسماء المضيفين، نقطة نهاية Torii).
   - قيم كتالوج lane/data-space الفعلية وسياسات التوجيه.
   - Hashes للثنائيات/الصور التي تم التحقق منها.
3. نسق القبول النهائي للاقران (gossip seeds وتخصيص lane) مع `@nexus-core`. لا تنضم للشبكة حتى تحصل على الموافقة; يفرض Sora Nexus اشغالا حتميا للـ lanes ويتطلب manifest قبول محدث.
4. بعد تشغيل العقدة، حدث runbooks لديك باي overrides ادخلتها وسجل وسم الاصدار حتى تبدأ الدورة التالية من هذه baseline.

## قائمة تدقيق مرجعية
- [ ] تم التحقق من ملف الاصدار كـ `iroha3`.
- [ ] تم التحقق من hashes والتواقيع للحزمة/الصورة.
- [ ] تم تحديث المفاتيح وعناوين peers ونقاط نهاية Torii لقيم الانتاج.
- [ ] كتالوج lane/dataspace وسياسة التوجيه في Nexus تطابق تعيين المجلس.
- [ ] مدقق الاعداد (`irohad --sora --config ... --trace-config`) يمر بدون تحذيرات.
- [ ] تم ارشفة manifests/التواقيع في تذكرة التاهيل وتم اخطار Ops.

للمزيد من السياق حول مراحل هجرة Nexus وتوقعات التليمتري، راجع [Nexus transition notes](./nexus-transition-notes).
