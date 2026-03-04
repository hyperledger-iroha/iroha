---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
title: تاهيل مشغلي data-space pour Sora Nexus
description: نسخة مطابقة لـ `docs/source/sora_nexus_operator_onboarding.md` تتبع قائمة تدقيق الاصدار الشاملة لمشغلي Nexus.
---

:::note المصدر القانوني
Il s'agit de la référence `docs/source/sora_nexus_operator_onboarding.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# تاهيل مشغلي Data-Space pour Sora Nexus

Il s'agit d'un espace de données important pour Sora Nexus. وهو يكمل دليل المسارين (`docs/source/release_dual_track_runbook.md`) ومذكرة اختيار القطع (`docs/source/release_artifact_selection.md`) عبر شرح كيفية مواءمة الحزم/الصور التي تم تنزيلها وملفات manifest وقوالب الاعداد مع توقعات الـ lane العالمية قبل تشغيل العقدة.

## الجمهور والمتطلبات المسبقة
- تم اعتمادك من برنامج Nexus واستلمت تعيين data-space (فهرس lane, ومعرف/alias للـ data-space, ومتطلبات سياسة التوجيه).
- Il s'agit d'une application de Release Engineering (archives tar, manifestes, applications).
- قمت بتوليد او استلام مواد مفاتيح الانتاج لدور validateur/observateur (voir Ed25519؛ مفتاح اجماع BLS + PoP pour validateurs؛ بالاضافة الى اي مفاتيح او bascule للخصائص السرية).
- Utilisez le logiciel Sora Nexus pour utiliser le bootstrap.## الخطوة 1 - تاكيد ملف الاصدار
1. Utilisez l'alias et l'ID de chaîne que vous utilisez.
2. Utilisez `scripts/select_release_profile.py --network <alias>` (`--chain-id <id>`) pour vous connecter à votre ordinateur. يقوم المساعد بقراءة `release/network_profiles.toml` ويطبع ملف النشر. Compatible avec Sora Nexus et avec `iroha3`. Il s'agit de Release Engineering.
3. سجل وسم الاصدار المشار اليه في اعلان الاصدار (مثلا `iroha3-v3.2.0`); ستستخدمه لجلب القطع وملفات manifeste.

## الخطوة 2 - جلب القطع والتحقق منها
1. قم بتنزيل حزمة `iroha3` (`<profile>-<version>-<os>.tar.zst`) et وملفاتها المرافقة (`.sha256`, اختياري `.sig/.pub`, `<profile>-<version>-manifest.json`, et `<profile>-<version>-image.json` sont des exemples de problèmes).
2. تحقق من السلامة قبل فك الضغط:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Utilisez `openssl` pour installer KMS sur votre ordinateur.
3. Utilisez `PROFILE.toml` pour l'archive tar et JSON pour :
   -`profile = "iroha3"`
   - الحقول `version` و`commit` و`built_at` تطابق اعلان الاصدار.
   - نظام التشغيل/المعمارية تطابق هدف النشر.
4. Vous devez utiliser la fonction hash/التوقيع `<profile>-<version>-<os>-image.tar` pour utiliser l'ID de l'image. `<profile>-<version>-image.json`.## الخطوة 3 - تجهيز الاعداد من القوالب
1. استخرج الحزمة وانسخ `config/` الى المكان الذي ستقرأ منه العقدة الاعدادات.
2. Utilisez le code `config/` :
   - استبدل `public_key`/`private_key` بمفاتيح Ed25519 الخاصة بالانتاج. ازل المفاتيح الخاصة من القرص اذا كانت العقدة ستجلبها من HSM; Il s'agit d'un lien vers HSM.
   - عدل `trusted_peers` و`network.address` و`torii.address` لتعكس واجهاتك المتاحة و peers bootstrap المخصصة لك.
   - حدث `client.toml` pour Torii الموجهة للمشغل (بما في ذلك اعداد TLS اذا كان ذلك ينطبق) وبالاعتمادات التي قمت بتجهيزها لادوات التشغيل.
3. Définir l'ID de la chaîne dans la section "Gouvernance" - "Lane" قانوني واحد.
4. La personne à contacter est Sora : `irohad --sora --config <path>`. Il s'agit d'un système multivoie SoraFS et d'un système multivoies.## الخطوة 4 - مواءمة بيانات data-space et سياسات التوجيه
1. `config/config.toml` par `[nexus]` pour l'espace de données dans Nexus Conseil :
   - يجب ان يساوي `lane_count` اجمالي الـ voies المفعلة في الحقبة الحالية.
   - يجب ان تحتوي كل خانة في `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` et `index`/`id` فريد والـ alias المتفق عليها. لا تحذف الادخالات العالمية الحالية; Les alias sont également des espaces de données.
   - تاكد من ان كل مدخل dataspace يتضمن `fault_tolerance (f)` ; Il s'agit d'un relais de voie selon `3f+1`.
2. Utilisez `[[nexus.routing_policy.rules]]` pour la prise en charge de l'appareil. La voie `1` et la voie `2` sont également concernées; Il s'agit d'un alias de l'espace de données sur la voie et l'alias de l'utilisateur. Il s'agit de Release Engineering.
3. Utilisez `[nexus.da]` et `[nexus.da.audit]` et `[nexus.da.recovery]`. من المتوقع ان يحتفظ المشغلون بالقيم المعتمدة من المجلس; لا تعدلها الا اذا تم التصديق على سياسة محدثة.
4. سجل الاعداد النهائي في متعقب العمليات لديك. يتطلب runbook ثنائي المسار ارفاق `config.toml` الفعلي (مع تنقيح الاسرار) بتذكرة التاهيل.## الخطوة 5 - التحقق قبل التشغيل
1. شغل مدقق الاعداد المدمج قبل الانضمام الى الشبكة:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   يطبع هذا الاعدادات النهائية ويفشل مبكرا اذا كانت ادخالات الكتالوج/التوجيه غير متسقة او اذا كان genesis والاعدادات غير متطابقة.
2. اذا كنت تنشر باستخدام الحاويات، شغل نفس الامر داخل الصورة بعد تحميلها عبر `docker load -i <profile>-<version>-<os>-image.tar` (تذكر تضمين `--sora`).
3. Utilisez l'espace réservé pour la voie/l'espace de données. اذا وجدت، ارجع الى الخطوة 4 - لا يجب ان تعتمد عمليات الانتاج على معرفات placeholder المرفقة مع القوالب.
4. Utilisez le fumoir pour fumer (pour utiliser `FindNetworkStatus` ou `iroha_cli`, pour vous assurer qu'il n'y a pas de fumée). تعرض `nexus_lane_state_total`, وتحقق من ان مفاتيح البث تم تدويرها او استيرادها حسب الحاجة).## الخطوة 6 - التحويل والتسليم
1. خزّن `manifest.json` الذي تم التحقق منه وقطع التوقيع في تذكرة الاصدار حتى يتمكن المدققون من اعادة فحوصاتك.
2. اخطر Nexus Opérations ان العقدة جاهزة للادراج; Lire:
   - هوية العقدة (peer ID, اسماء المضيفين، نقطة نهاية Torii).
   - قيم كتالوج lane/data-space الفعلية وسياسات التوجيه.
   - Hachages للثنائيات/الصور التي تم التحقق منها.
3. نسق القبول النهائي للاقران (Gossip Seeds وتخصيص Lane) avec `@nexus-core`. لا تنضم للشبكة حتى تحصل على الموافقة; يفرض Sora Nexus اشغالا حتميا للـ voies et manifeste قبول محدث.
4. Les runbooks utilisés pour remplacer la ligne de base par la ligne de base.

## قائمة تدقيق مرجعية
-[ ] تم التحقق من ملف الاصدار كـ `iroha3`.
- [ ] تم التحقق من hashs والتواقيع للحزمة/الصورة.
- [ ] تم تحديث المفاتيح وعناوين peers ونقاط نهاية Torii لقيم الانتاج.
- [ ] كتالوج lane/dataspace وسياسة التوجيه في Nexus تطابق تعيين المجلس.
- [ ] مدقق الاعداد (`irohad --sora --config ... --trace-config`) يمر بدون تحذيرات.
- [ ] تم ارشفة manifestes/التواقيع في تذكرة التاهيل وتم اخطار Ops.

للمزيد من السياق حول مراحل هجرة Nexus وتوقعات التليمتري، راجع [Nexus notes de transition](./nexus-transition-notes).