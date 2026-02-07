---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: حزمة الرجوع للوضع المباشر em SoraFS ‏(SNNet-5a)
sidebar_label: حزمة الوضع المباشر
description: A descrição de SoraFS SoraFS Torii/QUIC é compatível com SNNet-5a.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/direct_mode_pack.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

Se você usar o SoraNet no SoraFS, você pode usar o **SNNet-5a** para usar o SoraNet. Isso pode ser feito por meio de uma mensagem de erro. Faça o download do software no CLI/SDK e instale-o no CLI/SDK. A substituição de SoraFS é realizada por meio de Torii/QUIC.

ينطبق مسار الرجوع على بيئات staging والإنتاج المنظم إلى أن تعبر SNNet-5 حتى SNNet-9 بوابات الجاهزية. Você pode usar o produto SoraFS para obter mais informações sobre o produto. Você pode fazer isso sem problemas.

## 1. Use CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` é compatível com Torii/QUIC. Verifique se o CLI está `direct-only` sem problemas.
- O SDK do `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` está configurado para "fazer o download". Use o código `iroha::ClientOptions` e `iroha_android` para obter enum.
- يمكن لأدوات الـ gateway (`sorafs_fetch` e Python) تحليل مفتاح direct-only عبر مساعدات Norito JSON المشتركة حتى تتلقى الأتمتة السلوك نفسه.

وثّق العلم في كتيبات التشغيل الموجهة للشركاء, ومرر مفاتيح التبديل عبر `iroha_config` بدلا من متغيرات البيئة.

## 2. ملفات سياسة الـ gateway

Use Norito JSON para ser processado. A solução para `docs/examples/sorafs_direct_mode_policy.json` é:

- `transport_policy: "direct_only"` — يرفض المزوّدين الذين يعلنون فقط عن نقل مرحلات SoraNet.
- `max_providers: 2` — Você pode usar o método Torii/QUIC. عدّل وفق سماحات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` — Você pode usar o código de segurança para obter mais informações e obter mais informações.
- ميزانيات إعادة المحاولة المحافظة (`retry_budget: 2`, `provider_failure_threshold: 3`) لتجنب إخفاء بوابات سيئة الضبط.

Usar JSON para `sorafs_cli fetch --config` (`sorafs_cli fetch --config`) e SDK (`config_from_json`) é uma solução para o problema. احتفظ بمخرجات الـ scoreboard (`persist_path`) لمسارات التدقيق.

Instale o gateway no `docs/examples/sorafs_gateway_direct_mode.toml`. O valor do envelope `iroha app sorafs gateway direct-mode enable` é definido como envelope/admissão e taxa de limite de taxa. Use `direct_mode` para exibir o manifesto do arquivo. Verifique se o produto está funcionando corretamente durante a operação.

## 3. Como usar o código de barras

تتضمن جاهزية الوضع المباشر الآن تغطية في المنسق وفي حزم CLI:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يفشل بسرعة عندما يدعم كل advert مرشح مرحلات SoraNet فقط.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` é um recurso de Torii/QUIC que está conectado e conectado ao SoraNet através do SoraNet. الجلسة.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` يحلل `docs/examples/sorafs_direct_mode_policy.json` لضمان بقاء الوثائق متوافقة مع أدوات المساعدة.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` يختبر `sorafs_cli fetch --transport-policy=direct-only` أمام بوابة Torii وهمية, موفرا اختبار smoke لبيئات منظمة تثبت النقل المباشر.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` é um arquivo JSON e um placar de placar.

شغّل المجموعة المركزة قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Para que o espaço de trabalho seja definido como upstream, você pode usar o espaço de trabalho em `status.md` e usar o recurso `status.md` تلاحق التبعية.

## 4. تشغيلات fumaça مؤتمتة

A CLI é usada para configurar o gateway ou os manifestos). Você pode usar fumaça em `scripts/sorafs_direct_mode_smoke.sh` e `sorafs_cli fetch` para obter o placar do placar والتقاط الملخص.

O que fazer:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Use a CLI e o valor key=value (`docs/examples/sorafs_direct_mode_smoke.conf`). املأ digest الخاص بالـ manifesto e anúncios للموفّر بقيم الإنتاج قبل التشغيل.
- `--policy` é `docs/examples/sorafs_direct_mode_policy.json`, mas o JSON não é `sorafs_orchestrator::bindings::config_to_json`. A interface CLI do `--orchestrator-config=PATH` permite que você verifique o valor do arquivo.
- Coloque o `sorafs_cli` no `PATH`, coloque o engradado `sorafs_orchestrator` (liberação) sem problemas تشغيلات fumaça مسار الوضع المباشر المرسل.
- Informações:
  - Número de telefone (`--output`, número `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`, افتراضيا بجوار الحمولة) يحتوي على منطقة التليمترية وتقارير الموفّرين Você pode fazer isso.
  - O placar do placar está no formato JSON (como `fetch_state/direct_mode_scoreboard.json`). Você pode fazer isso com uma chave de fenda.
- أتمتة بوابة الاعتماد: بعد اكتمال الجلب يستدعي المساعد `cargo xtask sorafs-adoption-check` باستخدام مسارات placar والملخص المحفوظة. Você pode usar o aplicativo para obter mais informações sobre o assunto em questão. Verifique se `--min-providers=<n>` está funcionando corretamente. Verifique o valor do cartão de crédito (`--adoption-report=<path>` para obter mais informações) e `--require-direct-only` é compatível (não é compatível) e `--require-telemetry` é um problema. Use `XTASK_SORAFS_ADOPTION_FLAGS` para executar e xtask إضافية (como `--allow-single-source` para obter mais informações البوابة مع الرجوع وتفرضه). Para obter mais informações sobre o `--skip-adoption-check`, você pode usar o `--skip-adoption-check` para obter mais informações. Certifique-se de que o produto esteja funcionando corretamente durante o processo de lavagem.

## 5. قائمة تحقق الإطلاق1. **تجميد التهيئة:** خزّن ملف JSON للوضع المباشر في مستودع `iroha_config` وسجل الهاش في تذكرة التغيير.
2. **تدقيق الـ gateway:** تحقق من أن نقاط Torii تطبق TLS e TLVs للقدرات وسجلات التدقيق قبل التحويل إلى الوضع المباشر. Verifique se o gateway está conectado.
3. **موافقة الامتثال:** شارك دليل التشغيل المحدث مع مراجعي الامتثال/التنظيم وسجل الموافقات على Verifique se há algum problema.
4. **تشغيل تجريبي:** نفذ مجموعة اختبارات الامتثال بالإضافة إلى جلب staging مقابل مزودي Código Torii. أرشف مخرجات scoreboard e CLI.
5. **Escolha o código:** Você pode usar o `transport_policy` ou `direct_only` (ou seja, você pode usar o `direct_only`). `soranet-first`). وثّق خطة الرجوع إلى SoraNet-first está usando SNNet-4/5/5a/5b/6a/7/8/12/13 em `roadmap.md:532`.
6. **مراجعة ما بعد التغير:** أرفق لقطات scoreboard وملخصات الجلب ونتائج المراقبة في تذكرة التغيير. O `status.md` está danificado e danificado.

Verifique se o dispositivo `sorafs_node_ops` está disponível no site da empresa. Se o SNNet-5 for GA, você pode usar o SNNet-5 para obter mais informações no site.

## 6. متطلبات الأدلة وبوابة الاعتماد

Não há nenhum problema com o SF-6c. O placar do placar e o envelope são exibidos no manifesto e no arquivo `cargo xtask sorafs-adoption-check`. وضع الرجوع. Verifique o valor do cartão de crédito e verifique o valor do produto no momento da instalação.

- **بيانات النقل الوصفية:** يجب أن يصرح `scoreboard.json` ou `transport_policy="direct_only"` (e `transport_policy_override=true` عندما تُجبر خفض المستوى). Você pode fazer isso com uma chave de fenda que você possa usar para obter mais informações. Não use nenhum recurso para isso.
- **عدادات المزوّدين:** يجب أن تحفظ جلسات gateway-only `provider_count=0` e أن تملأ `gateway_provider_count=<n>` بعدد مزودي Código Torii. O JSON é definido como: CLI/SDK é usado para definir o valor do arquivo.
- **Manifesto de دليل:** عندما تشارك بوابات Torii, مرر `--gateway-manifest-envelope <path>` الموقع (أو ما يعادله في SDK) حتى يتم Use `gateway_manifest_provided` e `gateway_manifest_id`/`gateway_manifest_cid` para `scoreboard.json`. Use o `summary.json` para `manifest_id`/`manifest_cid`. Você pode fazer isso sem parar.
- **توقعات التليمترية:** عندما ترافق التليمترية الالتقاط, شغّل البوابة مع `--require-telemetry` حتى يثبت Limpe o local de trabalho. يمكن للتجارب المعزولة هوائيا, أن تتجاوز العلم, لكن يجب على CI وتذاكر التغيير توثيق الغياب.

Exemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

أرفق `adoption_report.json` مع الـ placar والملخص و envelope الخاص بالـ manifesto وحزمة سجلات fumaça. Você pode usar o CI (`ci/check_sorafs_orchestrator_adoption.sh`) para obter informações sobre o CI (`ci/check_sorafs_orchestrator_adoption.sh`). قابلة للتدقيق.