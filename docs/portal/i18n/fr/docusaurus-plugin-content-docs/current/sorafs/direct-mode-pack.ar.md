---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : حزمة الرجوع للوضع المباشر في SoraFS ‏(SNNet-5a)
sidebar_label : حزمة الوضع المباشر
description : Les connexions téléphoniques et les connexions Internet sont effectuées par SoraFS et Torii/QUIC. Utilisez SNNet-5a.
---

:::note المصدر المعتمد
Utilisez le `docs/source/sorafs/direct_mode_pack.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

Utilisez SoraNet pour utiliser SoraFS, pour utiliser **SNNet-5a** avec votre appareil. يحافظ المشغلون على وصول قراءة حتمي بينما يكتمل طرح الخصوصية. Mise en œuvre de la fonctionnalité CLI/SDK et des fonctionnalités de mise à jour et de mise à jour SoraFS et Torii/QUIC sont utilisés pour la connexion.

Il s'agit d'une étape de mise en scène et d'un processus de SNNet-5 ou SNNet-9. احتفظ بالمواد أدناه مع حزمة نشر SoraFS المعتادة حتى يتمكن المشغلون من التبديل بين الوضعين المجهول والمباشر عند الطلب.

## 1. Utiliser la CLI et le SDK- `sorafs_cli fetch --transport-policy=direct-only ...` est compatible avec Torii/QUIC. Utilisez la CLI pour `direct-only` dans le fichier CLI.
- Utilisez le SDK `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` pour utiliser "الوضع المباشر". Utilisez l'énumération `iroha::ClientOptions` et `iroha_android` pour votre énumération.
- Utilisez la passerelle (`sorafs_fetch` et Python) pour utiliser directement uniquement la passerelle Norito JSON avec un accès direct uniquement. الأتمتة السلوك نفسه.

وثّق العلم في كتيبات التشغيل الموجهة للشركاء، ومرر مفاتيح التبديل عبر `iroha_config` بدلا من متغيرات البيئة.

## 2. ملفات سياسة الـ gateway

Utilisez Norito JSON pour créer un fichier. ملف المثال في `docs/examples/sorafs_direct_mode_policy.json` يشفر:

- `transport_policy: "direct_only"` — يرفض المزوّدين الذين يعلنون فقط عن نقل مرحلات SoraNet.
- `max_providers: 2` — يحد الأقران المباشرين إلى أكثر نقاط Torii/QUIC موثوقية. عدّل وفق سماحات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` — يوسم المقاييس المصدرة بحيث تميز لوحات المتابعة والتدقيق تشغيلات الرجوع.
- ميزانيات إعادة المحاولة المحافظة (`retry_budget: 2`, `provider_failure_threshold: 3`) لتجنب إخفاء بوابات سيئة الضبط.

Utilisez JSON pour `sorafs_cli fetch --config` (الأتمتة) et SDK (`config_from_json`) pour votre appareil. احتفظ بمخرجات الـ scoreboard (`persist_path`) لمسارات التدقيق.Utilisez la passerelle pour `docs/examples/sorafs_gateway_direct_mode.toml`. `iroha app sorafs gateway direct-mode enable`, avec enveloppe/admission et taux-limite `direct_mode` بأسماء المضيفين المشتقة من الخطة وملخصات manifeste. استبدل القيم النائبة بخطة الإطلاق قبل حفظ المقتطف في إدارة التهيئة.

## 3. مجموعة اختبارات الامتثال

Utilisez la CLI comme suit :

- `direct_only_policy_rejects_soranet_only_providers` est compatible avec `TransportPolicy::DirectOnly` et est connecté à l'annonce de SoraNet. فقط.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` est compatible avec Torii/QUIC et est compatible avec SoraNet. Fichier.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` يحلل `docs/examples/sorafs_direct_mode_policy.json` لضمان بقاء الوثائق متوافقة مع أدوات Fichier.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` pour `sorafs_cli fetch --transport-policy=direct-only` pour la fumée Torii pour la fumée et la fumée Fichier.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` est compatible avec le tableau de bord JSON et le tableau de bord.

شغّل المجموعة المركزة قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Vous pouvez créer un espace de travail en amont, en utilisant `status.md` et en utilisant le modèle `status.md`. التبعية.

## 4. تشغيلات fumée مؤتمتةLa CLI est utilisée pour la passerelle et les manifestes). يوجد مساعد smoke مخصص في `scripts/sorafs_direct_mode_smoke.sh` ويلف `sorafs_cli fetch` مع سياسة منسق الوضع المباشر وحفظ الـ scoreboard والتقاط الملخص.

مثال للاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Utilisez la CLI et la clé = valeur pour (راجع `docs/examples/sorafs_direct_mode_smoke.conf`). املأ digest الخاص بالـ manifest ومدخلات annonces للموفّر بقيم الإنتاج قبل التشغيل.
- `--policy` remplace `docs/examples/sorafs_direct_mode_policy.json`, ce qui signifie que JSON est compatible avec `sorafs_orchestrator::bindings::config_to_json`. يقبل CLI السياسة عبر `--orchestrator-config=PATH` لتمكين تشغيلات قابلة لإعادة الإنتاج دون ضبط الأعلام يدويا.
- Utilisez le `sorafs_cli` pour `PATH`, utilisez la caisse `sorafs_orchestrator` (libération complète) pour fumer de la fumée مسار الوضع المباشر المرسل.
- المخرجات:
  - Numéro de téléphone (`--output`, Numéro de téléphone `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`, افتراضيا بجوار الحمولة) يحتوي على منطقة التليمترية وتقارير الموفّرين المستخدمة كدليل إطلاق.
  - Tableau de bord utilisé pour le tableau de bord JSON (`fetch_state/direct_mode_scoreboard.json`). أرشفها مع الملخص في تذاكر التغيير.- أتمتة بوابة الاعتماد: بعد اكتمال الجلب يستدعي المساعد `cargo xtask sorafs-adoption-check` باستخدام مسارات scoreboard والملخص المحفوظة. الحد الأدنى للنصاب يساوي افتراضيا عدد الموفّرين الممررين في سطر الأوامر؛ استبدله بـ `--min-providers=<n>` عند الحاجة إلى عينة أكبر. تُكتب تقارير الاعتماد بجوار الملخص (`--adoption-report=<path>` يمكنه تحديد موقع مخصص) ويمرر المساعد `--require-direct-only` افتراضيا (مطابق لمسار الرجوع) و`--require-telemetry` عندما تمرر العلم المطابق. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لتمرير وسائط xtask إضافية (مثل `--allow-single-source` أثناء خفض معتمد حتى تتسامح البوابة مع الرجوع وتفرضه). لا تتجاوز بوابة الاعتماد باستخدام `--skip-adoption-check` إلا ​​عند التشخيص المحلي؛ تتطلب خارطة الطريق أن تتضمن كل عملية تشغيل منظمة في الوضع المباشر حزمة تقرير الاعتماد.

## 5. قائمة تحقق الإطلاق1. **Traitement en ligne :** Utilisez JSON pour créer un lien vers `iroha_config` et vers un lien vers le serveur.
2. **Votre passerelle :** Votre passerelle Torii est compatible avec TLS et TLV et avec le lien suivant المباشر. انشر ملف سياسة الـ gateway للمشغلين.
3. **موافقة الامتثال:** شارك دليل التشغيل المحدث مع مراجعي الامتثال/التنظيم وسجل الموافقات على التشغيل خارج طبقة الإخفاء.
4. **Caractéristiques du produit :** نفذ مجموعة اختبارات الامتثال بالإضافة إلى جلب staging مقابل مزودي Torii الموثوقين. Utilisez le tableau de bord et la CLI.
5. **التحويل للإنتاج:** أعلن نافذة التغيير، وحول `transport_policy` إلى `direct_only` (إذا كنت قد اخترت `soranet-first`) ou `sorafs_fetch`, ou `sorafs_fetch`, ou `soranet-first`. Il s'agit de SoraNet-first comme SNNet-4/5/5a/5b/6a/7/8/12/13 dans `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** أرفق لقطات scoreboard وملخصات الجلب ونتائج المراقبة في تذكرة التغيير. حدّث `status.md` by السريان وأي شذوذات.

احتفظ بالقائمة بجوار دليل `sorafs_node_ops` حتى يتمكن المشغلون من التمرين قبل التحويل الفعلي. Il s'agit de SNNet-5 pour GA, qui est également compatible avec les réseaux sociaux.

## 6. متطلبات الأدلة وبوابة الاعتمادIl s'agit d'un accessoire pour le SF-6c. Tableau de bord et enveloppe pour le manifeste et l'enveloppe pour le manifeste `cargo xtask sorafs-adoption-check` pour le tableau de bord et l'enveloppe الرجوع. تؤدي الحقول الناقصة إلى فشل البوابة، لذا سجّل البيانات الوصفية المتوقعة في تذاكر التغيير.

- **بيانات النقل الوصفية:** يجب أن يصرح `scoreboard.json` by `transport_policy="direct_only"` (et `transport_policy_override=true` عندما تُجبر خفض المستوى). أبقِ حقول سياسة الإخفاء المزدوجة معبأة حتى عندما ترث القيم الافتراضية كي يرى المراجعون ما إذا انحرفت عن خطة الإخفاء المرحلية.
- **Périphérique d'installation :** est compatible avec la passerelle uniquement `provider_count=0` et `gateway_provider_count=<n>` pour Torii. المستخدمين. Utilisation de JSON : Le CLI/SDK fournit des fonctionnalités et des fonctionnalités supplémentaires.
- **Manifeste :** Le fichier Torii et `--gateway-manifest-envelope <path>` (pour le SDK) sont également disponibles. `gateway_manifest_provided` et `gateway_manifest_id`/`gateway_manifest_cid` et `scoreboard.json`. تأكد من أن `summary.json` يحمل `manifest_id`/`manifest_cid` المطابق؛ تفشل عملية الاعتماد إذا غاب الزوج من أي ملف.
- **توقعات التليمترية:** عندما ترافق التليمترية التقاط، شغّل البوابة مع `--require-telemetry` حتى يثبت تقرير الاعتماد أن المقاييس أُرسلت. يمكن للتجارب المعزولة هوائيا أن تتجاوز العلم، لكن يجب على CI وتذاكر التغيير توثيق الغياب.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Il s'agit du `adoption_report.json`, un tableau d'affichage et une enveloppe pour le manifeste et la fumée. تعكس هذه القطع ما يطبقه عمل الاعتماد في CI (`ci/check_sorafs_orchestrator_adoption.sh`) et عمليات خفض الوضع المباشر قابلة للتدقيق.