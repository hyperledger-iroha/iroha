---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: paquete de modo directo
título: حزمة الرجوع للوضع المباشر في SoraFS ‏(SNNet-5a)
sidebar_label: حزمة الوضع المباشر
descripción: Los controladores y las fuentes de alimentación y de funcionamiento están conectados a SoraFS y a Torii/QUIC. Utilice SNNet-5a.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/direct_mode_pack.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

La conexión de SoraNet con el servidor SoraFS está basada en **SNNet-5a** y la configuración del sistema operativo. يحافظ المشغلون على وصول قراءة حتمي بينما يكتمل طرح الخصوصية. Instalación de CLI/SDK, configuración de configuración y configuración de CLI/SDK SoraFS في وضع Torii/QUIC المباشر دون لمس نواقل الخصوصية.

Hay varias etapas de preparación y puesta en escena de SNNet-5 y SNNet-9. Para obtener más información, consulte el artículo SoraFS. والمباشر عند الطلب.

## 1. CLI y SDK- `sorafs_cli fetch --transport-policy=direct-only ...` يعطل جدولة المرحلات ويفرض نقل Torii/QUIC. Utilice la CLI para conectar `direct-only`.
- يجب على SDK ضبط `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` كلما عرض مفتاح تبديل "الوضع المباشر". Utilice la enumeración `iroha::ClientOptions` e `iroha_android` para enumerar.
- Establece una puerta de enlace (`sorafs_fetch` y Python) para crear una conexión solo directa con Norito JSON para ejecutar archivos. الأتمتة السلوك نفسه.

وثّق العلم في كتيبات التشغيل الموجهة للشركاء, ومرر مفاتيح التبديل عبر `iroha_config` بدلا من متغيرات البيئة.

## 2. Puerta de enlace de ملفات سياسة الـ

Establece el código Norito JSON. El nombre de `docs/examples/sorafs_direct_mode_policy.json` es:

- `transport_policy: "direct_only"` — يرفض المزوّدين الذين يعلنون فقط عن نقل مرحلات SoraNet.
- `max_providers: 2` — يحد الأقران المباشرين إلى أكثر نقاط Torii/QUIC موثوقية. عدّل وفق سماحات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` — يوسم المقاييس المصدرة بحيث تميز لوحات المتابعة والتدقيق تشغيلات الرجوع.
- Los dispositivos de control remotos (`retry_budget: 2`, `provider_failure_threshold: 3`) deben estar conectados a la red.

Utilice JSON en `sorafs_cli fetch --config` (الأتمتة) y SDK (`config_from_json`) en este archivo. احتفظ بمخرجات الـ marcador (`persist_path`) لمسارات التدقيق.Utilice la puerta de enlace `docs/examples/sorafs_gateway_direct_mode.toml`. يعكس القالب مخرجات `iroha app sorafs gateway direct-mode enable`, مع تعطيل فحوصات sobre/admisión, وتوصيل إعدادات tasa límite الافتراضية, وتعبئة جدول `direct_mode` بأسماء المضيفين المشتقة من الخطة وملخصات manifest. استبدل القيم النائبة بخطة الإطلاق قبل حفظ المقتطف في إدارة التهيئة.

## 3. مجموعة اختبارات الامتثال

Ajustes de configuración de la CLI:

- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يفشل بسرعة عندما يدعم كل advert مرشح مرحلات SoraNet فقط.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` La conexión entre Torii/QUIC y el software SoraNet son compatibles con `direct_only_policy_prefers_direct_transports_when_available`. الجلسة.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` يحلل `docs/examples/sorafs_direct_mode_policy.json` لضمان بقاء الوثائق متوافقة مع أدوات المساعدة.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` يختبر `sorafs_cli fetch --transport-policy=direct-only` أمام بوابة Torii وهمية, موفرا اختبار لبيئات منظمة تثبت النقل المباشر.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` يلف الأمر نفسه مع JSON السياسة وحفظ الـ marcador لأتمتة الإطلاق.

شغّل المجموعة المركزة قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا فشل تجميع الـ workspace بسبب تغييرات upstream, سجّل الخطأ الحاجز في `status.md` and التشغيل عندما تلاحق التبعية.

## 4. تشغيلات fumar مؤتمتةUtilice la CLI para acceder a la puerta de enlace o a los manifiestos de la puerta de enlace. يوجد مساعد humo مخصص في `scripts/sorafs_direct_mode_smoke.sh` ويلف `sorafs_cli fetch` مع سياسة منسق الوضع المباشر وحفظ الـ marcador والتقاط الملخص.

مثال للاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Haga clic en CLI y haga clic en clave=valor (programa `docs/examples/sorafs_direct_mode_smoke.conf`). املأ resumen الخاص بالـ manifiesto y anuncios للموفّر بقيم الإنتاج قبل التشغيل.
- `--policy` contiene `docs/examples/sorafs_direct_mode_policy.json`, que contiene archivos JSON de `sorafs_orchestrator::bindings::config_to_json`. La CLI de `--orchestrator-config=PATH` le permitirá acceder a su dispositivo de forma segura.
- عندما لا يكون `sorafs_cli` على `PATH`, يبنيه المساعد من crate `sorafs_orchestrator` (liberación libre) بحيث تمارس تشغيلات humo مسار الوضع المباشر المرسل.
- المخرجات:
  - الحمولة المجمعة (`--output`, الافتراضي `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`، افتراضيا بجوار الحمولة) يحتوي على منطقة التليمترية وتقارير الموفّرين المستخدمة كدليل إطلاق.
  - Marcador لقطة محفوظة في المسار المعلن في JSON السياسة (مثل `fetch_state/direct_mode_scoreboard.json`). أرشفها مع الملخص في تذاكر التغيير.- أتمتة بوابة الاعتماد: بعد اكتمال الجلب يستدعي المساعد `cargo xtask sorafs-adoption-check` باستخدام مسارات مسارات والملخص المحفوظة. الحد الأدنى للنصاب يساوي افتراضيا عدد الموفّرين الممررين في سطر الأوامر؛ Utilice el `--min-providers=<n>` para obtener el resultado deseado. تُكتب تقارير الاعتماد بجوار الملخص (`--adoption-report=<path>` يمكنه تحديد موقع مخصص) ويمرر المساعد `--require-direct-only` افتراضيا (مطابق لمسار الرجوع) و`--require-telemetry` عندما تمرر العلم المطابق. El programa `XTASK_SORAFS_ADOPTION_FLAGS` y el programa xtask (para el programa `--allow-single-source` están disponibles para su uso). الرجوع وتفرضه). لا تجاوز بوابة الاعتماد باستخدام `--skip-adoption-check` إلا ​​عند التشخيص المحلي؛ تتطلب خارطة الطريق أن تتضمن كل عملية تشغيل منظمة في الوضع المباشر حزمة تقرير الاعتماد.

## 5. قائمة تحقق الإطلاق1. **تجميد التهيئة:** Inserte el código JSON en el archivo `iroha_config` y fíjelo.
2. **تدقيق الـ gateway:** تحقق من أن نقاط Torii تطبق TLS and TLVs للقدرات وسجلات التدقيق قبل التحويل إلى الوضع المباشر. انشر ملف سياسة الـ gateway للمشغلين.
3. **موافقة الامتثال:** شارك دليل التشغيل المحدث مع مراجعي الامتثال/التنظيم وسجل الموافقات على التشغيل خارج طبقة الإخفاء.
4. **تشغيل تجريبي:** نفذ مجموعة اختبارات الامتثال بالإضافة إلى جلب staging مقابل مزودي Torii الموثوقين. أرشف مخرجات marcador y CLI.
5. **التحويل للإنتاج:** أعلن نافذة التغيير، وحول `transport_policy` إلى `direct_only` (إذا كنت قد اخترت `soranet-first`), y los cables de conexión (`sorafs_fetch`, y las teclas de acceso). Una vez instalado SoraNet-first, utiliza SNNet-4/5/5a/5b/6a/7/8/12/13 en `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** أرفق لقطات marcador وملخصات الجلب ونتائج المراقبة في تذكرة التغيير. حدّث `status.md` بتاريخ السريان وأي شذوذات.

Utilice el dispositivo `sorafs_node_ops` para conectar el dispositivo a la fuente de alimentación. Utilice SNNet-5 y GA para obtener más información.

## 6. متطلبات الأدلة وبوابة الاعتمادAquí está el SF-6c. Marcador y sobre del manifiesto y del sobre del manifiesto y del mensaje `cargo xtask sorafs-adoption-check` de la marca `cargo xtask sorafs-adoption-check` الرجوع. تؤدي الحقول الناقصة إلى فشل البوابة, لذا سجّل البيانات الوصفية المتوقعة في تذاكر التغيير.

- **بيانات النقل الوصفية:** يجب أن يصرح `scoreboard.json` بـ `transport_policy="direct_only"` (y `transport_policy_override=true` عندما تُجبر خفض المستوى). أبقِ حقول سياسة الإخفاء المزدوجة معبأة حتى عندما ترث القيم الافتراضية كي يرى المراجعون ما إذا انحرفت عن خطة الإخفاء المرحلية.
- **عدادات المزوّدين:** يجب أن تحفظ جلسات gateway-only `provider_count=0` and تملأ `gateway_provider_count=<n>` بعدد مزودي Torii المستخدمين. Utilice el formato JSON: haga clic en CLI/SDK para crear archivos adjuntos.
- **دليل manifiesto:** عندما تشارك بوابات Torii, مرر `--gateway-manifest-envelope <path>` الموقع (أو ما يعادله في SDK) حتى يتم تسجيل `gateway_manifest_provided` y `gateway_manifest_id`/`gateway_manifest_cid` en `scoreboard.json`. Para obtener información sobre `summary.json` y `manifest_id`/`manifest_cid`, تفشل عملية الاعتماد إذا غاب الزوج من أي ملف.
- **توقعات التليمترية:** عندما ترافق التليمترية الالتقاط، شغّل البوابة مع `--require-telemetry` حتى يثبت تقرير الاعتماد أن المقاييس أُرسلت. يمكن للتجارب المعزولة هوائيا أن تتجاوز العلم، لكن يجب على CI وتذاكر التغيير توثيق الغياب.

Nombre:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```أرفق `adoption_report.json` مع الـ marcador والملخص والملخص والملص والـ manifiesto وحزمة سجلات humo. تعكس هذه القطع ما يطبقه عمل الاعتماد في CI (`ci/check_sorafs_orchestrator_adoption.sh`) Y تبقي عمليات خفض الوضع المباشر قابلة للتدقيق.