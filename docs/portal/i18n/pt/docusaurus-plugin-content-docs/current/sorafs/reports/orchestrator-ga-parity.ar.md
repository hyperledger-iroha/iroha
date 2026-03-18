---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير تكافؤ GA لمنسق SoraFS

Você pode usar multi-fetch no SDK para obter mais informações sobre o SDK.
بايتات carga útil وإيصالات pedaço وتقارير provedor ونتائج placar تبقى متطابقة عبر
التطبيقات. كل arnês يستهلك الحزمة القياسية متعددة المزوّدين تحت
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que é um dispositivo de segurança SF1 e
provedor الوصفية ولقطة telemetria e orquestrador.

## خط الأساس في Rust

- **Edição:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** يشغّل خطة `MultiPeerFixture` مرتين عبر orquestrador داخل العملية, مع التحقق
  Ele fornece carga útil, um provedor de pedaços e um placar. كما
  O conjunto de trabalho é definido como o conjunto de trabalho (`max_parallel x max_chunk_length`).
- **حاجز الأداء:** يجب أن يكتمل كل تشغيل خلال 2 s على عتاد CI.
- **سقف مجموعة العمل:** مع ملف تعريف SF1 يفرض chicote `max_parallel = 3`, مما ينتج
  نافذة <= 196608 بايت.

Como fazer isso:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Aproveite o SDK JavaScript

- **Edição:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يعيد تشغيل نفس fixture عبر `iroha_js_host::sorafsMultiFetchLocal`, ويقارن
  cargas úteis e recibos e provedor e placar عبر تشغيلين متتاليين.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s؛ يطبع arnês المدة المقاسة وسقف
  Número de telefone (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

O que fazer:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Aproveite o Swift SDK

- **Edição:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** يشغّل مجموعة التكافؤ المعرّفة em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  O dispositivo de fixação SF1 é o mesmo que Norito (`sorafsLocalFetch`). arnês
  من بايتات carga útil e pedaço وتقارير provedor ومدخلات placar باستخدام نفس
  O provedor de serviços de Internet e o provedor de telemetria são baseados em Rust/JS.
- **تهيئة الجسر:** يفك chicote ضغط `dist/NoritoBridge.xcframework.zip` عند الطلب ويحمل
  Instale o macOS usando `dlopen`. Usando xcframework e instalando SoraFS, instale-o
  `cargo build -p connect_norito_bridge --release` e `cargo build -p connect_norito_bridge --release`
  `target/release/libconnect_norito_bridge.dylib`, não é compatível com CI.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s على عتاد CI; ويطبع arnês
  Nome de usuário e número de telefone (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

O que fazer:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Aproveite o Python

- **Edição:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** O wrapper está no `iroha_python.sorafs.multi_fetch_local` e dataclasses
  O dispositivo de fixação do dispositivo de fixação é o dispositivo de fixação mais rápido do mundo.
  roda. Obtenha um provedor de serviços de terceiros com `providers.json`, e
  Para telemetria, ويتحقق من بايتات carga útil e pedaço ou provedor e محتوى
  placar semelhante a Rust/JS/Swift.
- **متطلب مسبق:** شغّل `maturin develop --release` (ou roda de roda) em `_crypto` ربط
  `sorafs_multi_fetch_local` é um pytest; يتخطى arnês تلقائيا عندما يكون
  Não há problema.
- **حاجز الأداء:** نفس حد <= 2 s مثل حزمة Rust؛ يسجل pytest عدد البايتات المجمعة
  وملخص مشاركة fornecedores لقطعة الإصدار.Você pode usar o arnês (Rust, Python, JS e Swift)
Verifique a carga útil e a carga útil da carga útil
construir. Use `ci/sdk_sorafs_orchestrator.sh` para usar em aplicativos (Rust e Python
vinculações e JS e Swift) por meio de um link Não há nenhum problema em CI que possa ser feito por você.
O código-fonte é o `matrix.md` (SDK/الحالة/المدة) بتذكرة
Você pode usar o software para obter mais informações sobre o produto.