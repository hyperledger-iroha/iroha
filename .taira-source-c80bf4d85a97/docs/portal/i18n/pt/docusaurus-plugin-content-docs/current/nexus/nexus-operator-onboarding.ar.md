---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: O espaço de dados foi criado em Sora Nexus
description: Verifique se o `docs/source/sora_nexus_operator_onboarding.md` está instalado no Nexus.
---

:::note المصدر القانوني
Verifique o valor `docs/source/sora_nexus_operator_onboarding.md`. Verifique se o produto está funcionando corretamente.
:::

# تاهيل مشغلي Data-Space em Sora Nexus

Você pode usar o espaço de dados no Sora Nexus para obter mais informações. وهو يكمل دليل المسارين (`docs/source/release_dual_track_runbook.md`) ومذكرة اختيار القطع (`docs/source/release_artifact_selection.md`) عبر شرح كيفية مواءمة الحزم/الصور التي تم تنزيلها وملفات manifest وقوالب الاعداد مع توقعات الـ lane العالمية قبل تشغيل العقدة.

## الجمهور والمتطلبات المسبقة
- تم اعتمادك من برنامج Nexus واستلمت تعيين data-space (فهرس lane, ومعرف/alias للـ data-space, ومتطلبات سياسة التوجيه).
- يمكنك الوصول الى قطع الاصدار الموقعة المنشورة من Engenharia de Liberação (tarballs, صور, manifestos, توقيعات, مفاتيح عامة).
- قمت بتوليد او استلام مواد مفاتيح الانتاج لدور validador/observador (هوية عقدة Ed25519, مفتاح اجماع BLS + PoP للـ validadores;
- يمكنك الوصول الى اقران Sora Nexus الحاليين سيقومون بعملية bootstrap para.

## الخطوة 1 - تاكيد ملف الاصدار
1. حدد alias الشبكة e ID de cadeia الذي تم منحه لك.
2. Coloque `scripts/select_release_profile.py --network <alias>` (e `--chain-id <id>`) no lugar certo. Verifique se o `release/network_profiles.toml` está funcionando corretamente. O Sora Nexus é o mesmo que o `iroha3`. Não há nenhum problema com a Engenharia de Liberação.
3. Verifique o código de barras do seu computador (exemplo `iroha3-v3.2.0`); ستستخدمه لجلب القطع وملفات manifesto.

## الخطوة 2 - جلب القطع والتحقق منها
1. Use o `iroha3` (`<profile>-<version>-<os>.tar.zst`) e o dispositivo `.sha256`, `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` estão disponíveis para download).
2. تحقق من السلامة قبل فك الضغط:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Verifique `openssl` para que o KMS esteja conectado corretamente.
3. Use `PROFILE.toml` para usar o tarball e o formato JSON:
   -`profile = "iroha3"`
   - O `version`, o `commit` e o `built_at` são removidos.
   - نظام التشغيل/المعمارية تطابق هدف النشر.
4. Use o hash/التوقيع لملف `<profile>-<version>-<os>-image.tar` e o ID da imagem. Em `<profile>-<version>-image.json`.

## الخطوة 3 - تجهيز الاعداد من القوالب
1. Insira o código `config/` no site da empresa.
2. Verifique o valor do `config/`:
   - Use `public_key`/`private_key` para Ed25519. ازل المفاتيح الخاصة من القرص اذا كانت العقدة ستجلبها من HSM; Isso é importante para o HSM.
   - Use `trusted_peers`, `network.address` e `torii.address` para executar o bootstrap e os peers.
   - O `client.toml` é o nome do Torii para a configuração (o que significa que o TLS não é válido ينطبق) وبالاعتمادات التي قمت بتجهيزها لادوات التشغيل.
3. احتفظ بالـ chain ID المقدم في الحزمة ما لم توجه Governança خلاف ذلك صراحة - الـ lane العالمي يتوقع معرف سلسلة É verdade.
4. Digite o nome do arquivo no Sora: `irohad --sora --config <path>`. Você pode usar o SoraFS e multi-lane para obter mais informações.

## الخطوة 4 - مواءمة بيانات espaço de dados e espaço de dados وسياسات التوجيه
1. عدل `config/config.toml` بحيث تطابق مقطع `[nexus]` كتالوج data-space الذي قدمه Nexus Conselho:
   - Selecione `lane_count` para remover as pistas do local.
   - Não há nenhum nome em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` ou `index`/`id` فريد والـ aliases Não. لا تحذف الادخالات العالمية الحالية; Use aliases para criar espaços de dados.
   - تاكد من ان كل مدخل dataspace يتضمن `fault_tolerance (f)`; Este é o relé de pista `3f+1`.
2. Selecione `[[nexus.routing_policy.rules]]` para remover o problema. A faixa `1` e a faixa `2`; اضف او عدل القواعد حتى يذهب المرور المخصص لـ espaço de dados الخاص بك الى الـ lane e alias الصحيحين. A Engenharia de Liberação é a solução para a engenharia de liberação.
3. Selecione `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. من المتوقع ان يحتفظ المشغلون بالقيم المعتمدة من المجلس; Não há necessidade de fazer isso.
4. Coloque o produto no lugar certo. Use o runbook para obter o `config.toml` do banco de dados (مع تنقيح الاسرار).## الخطوة 5 - التحقق قبل التشغيل
1. شغل مدقق الاعداد المدمج قبل الانضمام الى الشبكة:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   يطبع هذا الاعدادات النهائية ويفشل مبكرا اذا كانت ادخالات الكتالوج/التوجيه غير متسقة او اذا كان genesis e الاعدادات غير متطابقة.
2. Você pode usar o software de gerenciamento de arquivos `docker load -i <profile>-<version>-<os>-image.tar` (referência `--sora`).
3. Coloque o espaço reservado para pista/espaço de dados. اذا وجدت, ارجع الى الخطوة 4 - لا يجب ان تعتمد عمليات الانتاج على معرفات placeholder المرفقة مع القوالب.
4. Evite fumaça de cigarro (por exemplo, `FindNetworkStatus` ou `iroha_cli`, وتاكد من ان نقاط نهاية Verifique o `nexus_lane_state_total`, e verifique se ele está funcionando corretamente e sem problemas).

## الخطوة 6 - التحويل والتسليم
1. Insira `manifest.json` no site da empresa e no site da empresa. اعادة فحوصاتك.
2. اخطر Nexus Operations ان العقدة جاهزة للادراج; Mais:
   - هوية العقدة (ID de par, اسماء المضيفين, نقطة نهاية Torii).
   - Esta é a faixa/espaço de dados e o espaço de dados.
   - Hashes são usados/definidos como hash.
3. نسق القبول النهائي للاقران (sementes de fofoca e pista) em `@nexus-core`. لا تنضم للشبكة حتى تحصل على الموافقة; O Sora Nexus remove as pistas e o manifesto do arquivo.
4. بعد تشغيل العقدة, حدث runbooks لديك باي overrides ادخلتها وسجل وسم الاصدار حتى تبدأ الدورة A linha de base é a mesma.

## قائمة تدقيق مرجعية
- [ ] تم التحقق من ملف الاصدار كـ `iroha3`.
- [ ] تم التحقق من hashes والتواقيع للحزمة/الصورة.
- [ ] تم تحديث المفاتيح وعناوين peers ونقاط نهاية Torii لقيم الانتاج.
- [ ] a faixa lane/dataspace está localizada em Nexus.
- [ ] مدقق الاعداد (`irohad --sora --config ... --trace-config`) يمر بدون تحذيرات.
- [ ] تم ارشفة manifests/التواقيع في تذكرة التاهيل وتم اخطار Ops.

للمزيد من السياق حول مراحل هجرة Nexus وتوقعات التليمتري, راجع [Notas de transição Nexus](./nexus-transition-notes).