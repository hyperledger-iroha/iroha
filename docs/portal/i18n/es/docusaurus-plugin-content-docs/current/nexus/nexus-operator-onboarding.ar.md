---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: تاهيل مشغلي espacio de datos de Sora Nexus
descripción: نسخة مطابقة لـ `docs/source/sora_nexus_operator_onboarding.md` تتبع قائمة تدقيق الاصدار الشاملة لمشغلي Nexus.
---

:::nota المصدر القانوني
Utilice el código `docs/source/sora_nexus_operator_onboarding.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# تاهيل مشغلي Data-Space de Sora Nexus

Este es el espacio de datos de Sora Nexus que está disponible. وهو يكمل دليل المسارين (`docs/source/release_dual_track_runbook.md`) ومذكرة اختيار القطع (`docs/source/release_artifact_selection.md`) عبر شرح كيفية مواءمة الحزم/الصور التي تم تنزيلها وملفات manifest وقوالب الاعداد مع توقعات الـ lane العالمية قبل تشغيل العقدة.

## الجمهور والمتطلبات المسبقة
- تم اعتمادك من برنامج Nexus y تعيين data-space (فهرس lane, ومعرف/alias للـ data-space, ومتطلبات سياسة التوجيه).
- يمكنك الوصول الى قطع الاصدار الموقعة المنشورة من Release Engineering (tarballs, صور، manifests, توقيعات، مفاتيح عامة).
- قمت بتوليد او استلام مواد مفاتيح الانتاج لدور validador/observador (هوية عقدة Ed25519؛ مفتاح اجماع BLS + PoP للـ validadores؛ بالاضافة الى اي مفاتيح او alterna للخصائص السرية).
- يمكنك الوصول الى اقران Sora Nexus الحاليين الذين سيقومون بعملية bootstrap لعقدتك.## الخطوة 1 - تاكيد ملف الاصدار
1. حدد alias الشبكة او ID de cadena الذي تم منحه لك.
2. Haga clic en `scripts/select_release_profile.py --network <alias>` (او `--chain-id <id>`) en la pantalla. يقوم المساعد بقراءة `release/network_profiles.toml` ويطبع ملف النشر. بالنسبة لـ Sora Nexus يجب ان تكون النتيجة `iroha3`. لاي قيمة اخرى، توقف واتصل بـ Ingeniería de lanzamiento.
3. سجل وسم الاصدار المشار اليه في اعلان الاصدار (مثلا `iroha3-v3.2.0`); ستستخدمه لجلب القطع وملفات manifiesto.

## الخطوة 2 - جلب القطع والتحقق منها
1. Utilice el controlador `iroha3` (`<profile>-<version>-<os>.tar.zst`) y el controlador (`.sha256`, o `.sig/.pub`). `<profile>-<version>-manifest.json`, y `<profile>-<version>-image.json` (estos son algunos de los datos disponibles).
2. تحقق من السلامة قبل فك الضغط:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Utilice `openssl` para configurar el sistema KMS.
3. Introduzca `PROFILE.toml` en un tarball y en un archivo JSON:
   - `profile = "iroha3"`
   - الحقول `version` و`commit` و`built_at` تطابق اعلان الاصدار.
   - نظام التشغيل/المعمارية تطابق هدف النشر.
4. Utilice el hash/التوقيع لملف `<profile>-<version>-<os>-image.tar` y el ID de la imagen. `<profile>-<version>-image.json`.## الخطوة 3 - تجهيز الاعداد من القوالب
1. استخرج الحزمة وانسخ `config/` الى المكان الذي ستقرأ منه العقدة الاعدادات.
2. تعامل مع الملفات تحت `config/` كقوالب:
   - استبدل `public_key`/`private_key` بمفاتيح Ed25519 الخاصة بالانتاج. ازل المفاتيح الخاصة من القرص اذا كانت العقدة ستجلبها من HSM; وقم بتحديث الاعداد لربطه بموصل HSM.
   - Utilice `trusted_peers`, `network.address` y `torii.address` para ejecutar tareas de arranque y peers.
   - Pantalla táctil `client.toml` Torii Pantalla táctil (si es necesario TLS para conexión inalámbrica) وبالاعتمادات التي قمت بتجهيزها لادوات التشغيل.
3. احتفظ بالـ ID de cadena المقدم في الحزمة ما لم توجه Gobernanza خلاف ذلك صراحة - الـ carril العالمي يتوقع معرف سلسلة قانوني Y.
4. خطط لتشغيل العقدة مع خيار ملف Sora: `irohad --sora --config <path>`. سيقوم محمل الاعدادات برفض اعدادات SoraFS او multicarril اذا كان الخيار غائبا.## الخطوة 4 - مواءمة بيانات espacio de datos وسياسات التوجيه
1. عدل `config/config.toml` بحيث تطابق مقطع `[nexus]` كتالوج espacio de datos الذي قدمه Nexus Consejo:
   - يجب ان يساوي `lane_count` اجمالي الـ carriles المفعلة في الحقبة الحالية.
   - يجب ان تحتوي كل خانة في `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` على `index`/`id` فريد والـ alias المتفق عليها. لا تحذف الادخالات العالمية الحالية; Hay alias المفوضة لك اذا خصص المجلس espacios de datos اضافية.
   - تاكد من ان كل مدخل dataspace يتضمن `fault_tolerance (f)`; Este es el relé de carril `3f+1`.
2. Presione `[[nexus.routing_policy.rules]]` para que funcione correctamente. القالب الافتراضي يوجه تعليمات الحوكمة الى carril `1` y نشر العقود الى carril `2`; Aquí está el espacio de datos en el carril y el alias. نسق مع Ingeniería de lanzamiento قبل تغيير ترتيب القواعد.
3. Utilice `[nexus.da]`, `[nexus.da.audit]` y `[nexus.da.recovery]`. من المتوقع ان يحتفظ المشغلون بالقيم المعتمدة من المجلس; لا تعدلها الا اذا تم التصديق على سياسة محدثة.
4. سجل الاعداد النهائي في متعقب العمليات لديك. Este runbook está disponible en el idioma `config.toml` (en inglés) (en inglés).## الخطوة 5 - التحقق قبل التشغيل
1. شغل مدقق الاعداد المدمج قبل الانضمام الى الشبكة:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   يطبع هذا الاعدادات النهائية ويفشل مبكرا اذا كانت ادخالات الكتالوج/التوجيه غير متسقة او اذا كان génesis والاعدادات غير متطابقة.
2. اذا كنت تنشر باستخدام الحاويات، شغل نفس الامر داخل الصورة بعد تحميلها عبر `docker load -i <profile>-<version>-<os>-image.tar` (تذكر تضمين `--sora`).
3. راجع السجلات بحثا عن تحذيرات حول معرفات marcador de posición de carril/espacio de datos. اذا وجدت، ارجع الى الخطوة 4 - لا يجب ان تعتمد عمليات الانتاج على معرفات marcador de posición المرفقة مع القوالب.
4. نفذ اجراء humo المحلي (مثلا ارسل استعلام `FindNetworkStatus` عبر `iroha_cli`, وتاكد من ان نقاط نهاية التليمتري (((((((((((((((((((((((((((((((((((((((((((((((((((((( como como es que es que la información sobre el producto es `nexus_lane_state_total`, si la información sobre el uso de la información no es correcta)).## الخطوة 6 - التحويل والتسليم
1. خزّن `manifest.json` الذي تم التحقق منه وقطع التوقيع في تذكرة الاصدار حتى يتمكن المدققون من اعادة فحوصاتك.
2. اخطر Nexus Operaciones ان العقدة جاهزة للادراج; اشمل:
   - هوية العقدة (ID de par, اسماء المضيفين, نقطة نهاية Torii).
   - قيم كتالوج carril/espacio de datos الفعلية وسياسات التوجيه.
   - Hashes للثنائيات/الصور التي تم التحقق منها.
3. نسق القبول النهائي للاقران (semillas de chismes y carril) de `@nexus-core`. لا تنضم للشبكة حتى تحصل على الموافقة; يفرض Sora Nexus اشغالا حتميا للـ carriles ويتطلب manifiesto قبول محدث.
4. Utilice los runbooks y anule los archivos de ejecución de la línea de base.

## قائمة تدقيق مرجعية
- [ ] تم التحقق من ملف الاصدار كـ `iroha3`.
- [] تم التحقق من hashes y للحزمة/الصورة.
- [] تم تحديث المفاتيح وعناوين peers ونقاط نهاية Torii لقيم الانتاج.
- [] كتالوج lane/dataspace وسياسة التوجيه في Nexus تطابق تعيين المجلس.
- [ ] مدقق الاعداد (`irohad --sora --config ... --trace-config`) يمر بدون تحذيرات.
- [] تم ارشفة manifests/التواقيع في تذكرة التاهيل وتم اخطار Ops.

للمزيد من السياق حول مراحل هجرة Nexus وتوقعات التليمتري، راجع [Notas de transición Nexus](./nexus-transition-notes).