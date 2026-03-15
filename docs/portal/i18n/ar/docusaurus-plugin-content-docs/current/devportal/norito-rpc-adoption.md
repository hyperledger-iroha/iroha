---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# جدول اعتماد Norito-RPC

> الملاحظات التخطيطية المرجعية موجودة في `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> هذه النسخة في البوابة تلخص توقعات الإطلاق لمؤلفي SDK والمشغلين والمراجعين.

## الأهداف

- مواءمة كل SDK (Rust CLI, Python, JavaScript, Swift, Android) على نقل Norito-RPC الثنائي قبل تفعيل AND4 في الإنتاج.
- الحفاظ على بوابات المراحل وحزم الأدلة وخطافات القياس عن بعد بشكل حتمي لكي تتمكن الحوكمة من تدقيق الإطلاق.
- تسهيل التقاط أدلة fixtures و canary عبر المساعدات المشتركة التي يشير إليها البند NRPC-4 في خارطة الطريق.

## الجدول الزمني للمراحل

| المرحلة | النافذة | النطاق | معايير الخروج |
|---------|---------|--------|---------------|
| **P0 - تكافؤ المختبر** | Q2 2025 | مجموعات smoke لـ Rust CLI و Python تشغل `/v1/norito-rpc` في CI، مساعد JS يجتاز الاختبارات الوحدوية، وحاضنة Android mock تمارس النقلين. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` و `javascript/iroha_js/test/noritoRpcClient.test.js` باللون الأخضر في CI؛ حاضنة Android موصولة بـ `./gradlew test`. |
| **P1 - معاينة SDK** | Q3 2025 | حزمة fixtures المشتركة مسجلة، `scripts/run_norito_rpc_fixtures.sh --sdk <label>` تسجل السجلات + JSON في `artifacts/norito_rpc/`، وإشارات نقل Norito الاختيارية ظاهرة في عينات SDK. | بيان fixtures موقع، تحديثات README توضح الاستخدام الاختياري، واجهة Swift preview متاحة خلف علم IOS2. |
| **P2 - Staging / معاينة AND4** | Q1 2026 | مجموعات Torii في staging تفضل Norito، عملاء AND4 preview في Android وحزم تكافؤ IOS2 في Swift تستخدم النقل الثنائي افتراضيا، ولوحة القياس عن بعد `dashboards/grafana/torii_norito_rpc_observability.json` ممتلئة. | `docs/source/torii/norito_rpc_stage_reports.md` تلتقط canary، و `scripts/telemetry/test_torii_norito_rpc_alerts.sh` ينجح، وإعادة تشغيل حاضنة Android mock تلتقط حالات النجاح/الخطأ. |
| **P3 - GA في الإنتاج** | Q4 2026 | يصبح Norito النقل الافتراضي لكل SDK؛ يبقى JSON كخيار fallback brownout. وظائف الإصدار تؤرشف آثار التكافؤ مع كل وسم. | قائمة إصدار release تجمع مخرجات Norito smoke لـ Rust/JS/Python/Swift/Android؛ يتم فرض عتبات التنبيه لمستويات SLO لمعدل أخطاء Norito مقابل JSON؛ `status.md` وملاحظات الإصدار تشير إلى أدلة GA. |

## مخرجات SDK وخطافات CI

- **Rust CLI وحاضنة التكامل** - وسّع اختبارات smoke في `iroha_cli pipeline` لفرض نقل Norito بمجرد توفر `cargo xtask norito-rpc-verify`. احم ذلك بـ `cargo test -p integration_tests -- norito_streaming` (المختبر) و `cargo xtask norito-rpc-verify` (staging/GA)، مع حفظ الآثار تحت `artifacts/norito_rpc/`.
- **SDK Python** - اجعل smoke الإصدار (`python/iroha_python/scripts/release_smoke.sh`) افتراضيا على Norito RPC، واحتفظ بـ `run_norito_rpc_smoke.sh` كنقطة دخول CI، ووثق التكافؤ في `python/iroha_python/README.md`. هدف CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **SDK JavaScript** - ثبّت `NoritoRpcClient`، ودع مساعدات governance/query تفضل Norito عندما `toriiClientConfig.transport.preferred === "norito_rpc"`، والتقط عينات end-to-end في `javascript/iroha_js/recipes/`. يجب على CI تشغيل `npm test` بالإضافة إلى مهمة `npm run test:norito-rpc` المعزولة بالحاويات قبل النشر؛ provenance ترفع سجلات Norito smoke إلى `javascript/iroha_js/artifacts/`.
- **SDK Swift** - وصل نقل Norito bridge خلف علم IOS2، ونسق cadence للـ fixtures، وتأكد من أن مجموعة تكافؤ Connect/Norito تعمل داخل مسارات Buildkite المشار إليها في `docs/source/sdk/swift/index.md`.
- **SDK Android** - عملاء AND4 preview وحاضنة Torii mock تعتمد Norito، مع توثيق قياس retry/backoff في `docs/source/sdk/android/networking.md`. الحاضنة تشارك fixtures مع SDKs اخرى عبر `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## الأدلة والأتمتة

- `scripts/run_norito_rpc_fixtures.sh` يغلّف `cargo xtask norito-rpc-verify`، يلتقط stdout/stderr، ويصدر `fixtures.<sdk>.summary.json` حتى يمتلك ملاك SDK أثرا حتميا لإرفاقه بـ `status.md`. استخدم `--sdk <label>` و `--out artifacts/norito_rpc/<stamp>/` للحفاظ على حزم CI مرتبة.
- `cargo xtask norito-rpc-verify` يفرض تكافؤ hash المخطط (`fixtures/norito_rpc/schema_hashes.json`) ويفشل إذا أعاد Torii `X-Iroha-Error-Code: schema_mismatch`. اقرن كل فشل بالتقاط fallback JSON لأغراض التصحيح.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` و `dashboards/grafana/torii_norito_rpc_observability.json` يحددان عقود التنبيه لـ NRPC-2. شغّل السكربت بعد كل تعديل للوحة المعلومات واحفظ خرج `promtool` في حزمة canary.
- `docs/source/runbooks/torii_norito_rpc_canary.md` يصف تدريبات staging والإنتاج؛ حدّثه كلما تغيرت hashes الخاصة بالـ fixtures أو بوابات التنبيه.

## قائمة مراجعة المراجعين

قبل تعليم milestone NRPC-4، تحقق من:

1. أحدث hashes لحزمة fixtures تطابق `fixtures/norito_rpc/schema_hashes.json` وأن أثر CI المقابل مسجل تحت `artifacts/norito_rpc/<stamp>/`.
2. README الخاصة بالـ SDK ووثائق البوابة تشرح كيفية فرض fallback JSON وتشير إلى أن نقل Norito هو الافتراضي.
3. لوحات القياس عن بعد تعرض لوحات معدل الخطأ dual-stack مع روابط التنبيه، وأن dry run الخاص بـ Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) مرفق بالمتعقب.
4. جدول الاعتماد هنا يطابق مدخل المتعقب (`docs/source/torii/norito_rpc_tracker.md`) وخارطة الطريق (NRPC-4) تشير إلى نفس حزمة الأدلة.

الالتزام بالجدول يحافظ على سلوك cross-SDK قابلا للتنبؤ ويسمح للحوكمة بتدقيق اعتماد Norito-RPC بدون طلبات مخصصة.
