---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير تكافؤ GA لمنسق SoraFS

Il s'agit d'une fonctionnalité de multi-fetch pour le SDK et d'une solution de récupération multiple.
Ajouter la charge utile et le chunk au fournisseur et au tableau de bord
التطبيقات. كل harnais يستهلك الحزمة القياسية متعددة المزوّدين تحت
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, et les détails du SF1
fournisseur الوصفية ولقطة télémétrie et orchestrateur.

## خط الأساس في Rust

- **أمر :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق :** يشغّل خطة `MultiPeerFixture` مرتين عبر orchestrator داخل العملية، مع التحقق
  Il y a un morceau de charge utile et un tableau de bord du fournisseur. كما
  Il s'agit d'un ensemble de travail (`max_parallel x max_chunk_length`).
- **حاجز الأداء:** يجب أن يكتمل كل تشغيل خلال 2 s على عتاد CI.
- **سقف مجموعة العمل:** مع ملف تعريف SF1 يفرض harnais `max_parallel = 3`, مما ينتج
  نافذة <= 196608 بايت.

مثال لمخرجات السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harness pour le SDK JavaScript

- **أمر :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يعيد تشغيل نفس luminaire عبر `iroha_js_host::sorafsMultiFetchLocal`, ويقارن
  charges utiles, reçus, fournisseur, tableau de bord et tableau de bord.
- **حاجز الأداء :** يجب أن ينتهي كل تنفيذ خلال 2 s؛ يطبع harnais المدة المقاسة وسقف
  Paramètres de l'appareil (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Exploiter le SDK Swift

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق :** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  Pour le luminaire SF1, utilisez Norito (`sorafsLocalFetch`). harnais يتحقق
  Il s'agit de la charge utile et du bloc de données du fournisseur et du tableau de bord.
  Un fournisseur de services de télémétrie et de télémétrie est utilisé par Rust/JS.
- **Caractéristiques :** Ce harnais est `dist/NoritoBridge.xcframework.zip` pour le harnais
  Pour macOS, utilisez `dlopen`. Utilisez xcframework pour créer un SoraFS,
  `cargo build -p connect_norito_bridge --release` ويربط مع
  `target/release/libconnect_norito_bridge.dylib`, il s'agit d'un lien vers CI.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s على عتاد CI؛ ويطبع harnais المدة
  المقاسة وسقف البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais pour Python

- **أمر :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** يمارس wrapper عالي المستوى `iroha_python.sorafs.multi_fetch_local` et classes de données
  المقيّدة بالأنواع كي تمر luminaire القياسية عبر نفس الواجهة التي يستخدمها مستهلكو
  roue. يعيد الاختبار بناء بيانات supplier الوصفية من `providers.json`, ويحقن
  Pour la télémétrie, pour la charge utile et pour le bloc et pour le fournisseur.
  scoreboard est utilisé par Rust/JS/Swift.
- **متطلب مسبق:** `maturin develop --release` (أو ثبّت wheel) كي يعرض `_crypto` ربط
  `sorafs_multi_fetch_local` pour tester pytest يتخطى harnais تلقائيا عندما لا يكون
  الربط متاحا.
- **حاجز الأداء:** نفس حد <= 2 s pour Rust؛ يسجل pytest عدد البايتات المجمعة
  Il existe des fournisseurs de services en ligne.Vous avez besoin d'un harnais (Rust, Python, JS et Swift)
حتى يتمكن التقرير المؤرشف من مقارنة إيصالات payload والقياسات بشكل موحد قبل ترقية
construire. `ci/sdk_sorafs_orchestrator.sh` est une application pour les logiciels malveillants (Rust et Python
liaisons (JS et Swift) pour les utilisateurs ينبغي أن تُرفق مخرجات CI مقتطف السجل من هذا
Mise à jour de la version `matrix.md` (SDK/حالة/المدة) pour la mise à jour
حتى يتمكن المراجعون من تدقيق مصفوفة التكافؤ دون إعادة تشغيل المجموعة محليا.