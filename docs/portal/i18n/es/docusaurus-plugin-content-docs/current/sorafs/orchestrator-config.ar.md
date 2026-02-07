---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: اعداد مُنسِّق SoraFS
sidebar_label: اعداد المُنسِّق
descripción: اضبط مُنسِّق الجلب متعدد المصادر، وفسّر الاخفاقات، وتتبع مخرجات التليمترية.
---

:::nota المصدر المعتمد
Utilice el enlace `docs/source/sorafs/developer/orchestrator.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

# دليل مُنسِّق الجلب متعدد المصادر

يقود مُنسِّق الجلب متعدد المصادر في SoraFS تنزيلات حتمية and متوازية من مجموعة
المزوّدين المنشورة في anuncios المدعومة بالحوكمة. يشرح هذا الدليل كيفية ضبط
المُنسِّق، وما هي إشارات الفشل المتوقعة أثناء عمليات الإطلاق، وأي تدفقات
التليمترية تُظهر مؤشرات الصحة.

## 1. نظرة عامة على الاعداد

يدمج المُنسِّق ثلاثة مصادر للاعداد:

| المصدر | الغرض | الملاحظات |
|--------|-------|-----------|
| `OrchestratorConfig.scoreboard` | Establece archivos de texto y archivos de texto JSON. | Utilice `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | يطبّق حدود وقت التشغيل (ميزانيات إعادة المحاولة، حدود التوازي، مفاتيح التحقق). | Aquí `FetchOptions` y `crates/sorafs_car::multi_fetch`. |
| Herramientas CLI / SDK | تحد عدد النظراء، وتلصق مناطق التليمترية، وتعرض سياسات denegar/impulsar. | `sorafs_cli fetch` يعرّض هذه الأعلام مباشرةً؛ Aquí está el `OrchestratorConfig` de los SDK. |

Archivos JSON desde `crates/sorafs_orchestrator::bindings` desde el servidor
Aquí Norito JSON, está conectado al SDK y al SDK.

### 1.1 versión JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```احفظ الملف عبر طبقات `iroha_config` المعتادة (`defaults/`, usuario, real) حتى
ترث عمليات النشر الحتمية الحدود نفسها على جميع العقد. لملف respaldo مباشر فقط
يتماشى مع lanzamiento الخاص بـ SNNet-5a، راجع
`docs/examples/sorafs_direct_mode_policy.json` والإرشاد المكمل في
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Actualizaciones

Utilice SNNet-9 para acceder a la red inalámbrica. كائن `compliance` جديد
Para obtener Norito JSON, los archivos JSON que aparecen son solo directos:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` يعلن رموز ISO‑3166 alpha‑2 التي تعمل فيها هذه
  النسخة من المُنسِّق. تُحوّل الرموز إلى أحرف كبيرة أثناء التحليل.
- `jurisdiction_opt_outs` يعكس سجل الحوكمة. عندما تظهر أي ولاية تشغيلية ضمن
  القائمة، يفرض المُنسِّق `transport_policy=direct-only` ويُصدر سبب التراجع
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` يسرد resume المانيفست (CID ciegos بصيغة hexadecimal
  بأحرف كبيرة). التطابقات تفرض أيضاً الجدولة directo y وتُظهر
  `compliance_blinded_cid_opt_out` Aquí está.
- `audit_contacts` يسجل عناوين URI التي تتوقع الحوكمة أن ينشرها المشغلون في
  libros de jugadas الخاصة بـ GAR.
- `attestations` يلتقط حزم الامتثال الموقّعة الداعمة للسياسة. كل إدخال يعرّف
  `jurisdiction` اختيارياً (ISO-3166 alfa-2), و`document_uri`, و`digest_hex`
  القانوني بطول 64 حرفاً، وطابع الإصدار `issued_at_ms`, و`expires_at_ms`
  اختيارياً. تتدفق هذه الآثار إلى قائمة تدقيق المُنسِّق كي تربط أدوات الحوكمة
  بين التجاوزات والوثائق الموقعة.مرّر كتلة الامتثال عبر طبقات الاعداد المعتادة كي يحصل المشغلون على تجاوزات
حتمية. يطبق المُنسِّق الامتثال _بعد_ تلميحات modo de escritura: حتى لو طلب SDK
`upload-pq-only`, para obtener más información y utilizar directamente solo
وتفشل سريعاً عندما لا يوجد مزوّدون متوافقون.

توجد كتالوجات darse de baja المعتمدة في
`governance/compliance/soranet_opt_outs.json`؛ وينشر مجلس الحوكمة التحديثات عبر
إصدارات موسومة. يتوفر مثال كامل للاعداد (بما في ذلك atestaciones) في
`docs/examples/sorafs_compliance_policy.json`, كما يوثق المسار التشغيلي في
[libro de jugadas امتثال GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 CLI y SDK| العلم / الحقل | الاثر |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | يحد عدد المزوّدين الذين يمرون من فلتر لوحة النتائج. Utilice `None` para conectar el cable de alimentación. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | يحد إعادة المحاولة لكل شريحة. تجاوز الحد يرفع `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Hay una latencia/fallo en el sistema. التليمترية الأقدم من `telemetry_grace_secs` تجعل المزوّدين غير مؤهلين. |
| `--scoreboard-out` | يحفظ لوحة النتائج المحسوبة (مزوّدون مؤهلون + غير مؤهلين) للفحص بعد التشغيل. |
| `--scoreboard-now` | يتجاوز طابع لوحة النتائج (ثواني Unix) كي تبقى لقطات accesorios حتمية. |
| `--deny-provider` / gancho سياسة puntuación | يستبعد المزوّدين بشكل حتمي دون حذف anuncios. مفيد للمنع السريع. |
| `--boost-provider=name:delta` | يضبط أرصدة round-robin الموزونة لمزوّد مع الإبقاء على أوزان الحوكمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | يوسم المقاييس والسجلات المهيكلة لتمكين لوحات المتابعة من التجميع حسب الجغرافيا أو موجة الإطلاق. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | الافتراضي `soranet-first` الآن بعد أن أصبح المُنسِّق متعدد المصادر أساسياً. Utilice `direct-only` para obtener información sobre el producto y `soranet-strict` para PQ-only. تظل تجاوزات الامتثال سقفاً صارماً. |SoraNet-first هو الافتراضي الحالي للشحن، ويجب أن تذكر التراجعات المانع SNNet
المعني. بعد تخرّج SNNet-4/5/5a/5b/6a/7/8/12/13 ستشدّد الحوكمة الوضع المطلوب
(نحو `soranet-strict`) ، وحتى ذلك الحين يجب أن تقتصر تجاوزات `direct-only` على
الحالات المدفوعة بالحوادث مع تسجيلها في سجل الإطلاق.

كل الأعلام أعلاه تقبل صيغة `--` في كل من `sorafs_cli fetch` andالثنائي
`sorafs_fetch`. Los SDK son compatibles con los constructores.

### 1.4 ادارة ذاكرة caché للحراس

تقوم CLI الآن بتوصيل محدد حراس SoraNet بحيث يمكن للمشغلين تثبيت relés الدخول
بشكل حتمي قبل lanzamiento de la versión SNNet-5. ثلاثة أعلام جديدة تتحكم
بالسير:

| العلم | الغرض |
|------|-------|
| `--guard-directory <PATH>` | يشير إلى ملف JSON يصف أحدث توافق للـ relés (جزء منه أدناه). تمرير الدليل يحدّث caché الحراس قبل الجلب. |
| `--guard-cache <PATH>` | يحفظ `GuardSet` المشفر بنوريتو. عمليات التشغيل التالية تعيد استخدام cache حتى بدون دليل جديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Haga clic en el botón de encendido (3) y 30 minutos. |
| `--guard-cache-key <HEX>` | La configuración de caché de 32 bits de MAC Blake3 es la siguiente: |

تستخدم حمولة دليل الحراس مخططاً مدمجاً:

Utilice el controlador `--guard-directory` para conectar el dispositivo `GuardDirectorySnapshotV2`.
تحتوي اللقطة الثنائية على:- `version` — نسخة المخطط (حالياً `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  بيانات توافق يجب أن تطابق كل شهادة مضمنة.
- `validation_phase` — بوابة سياسة الشهادات (`1` = السماح بتوقيع Ed25519 واحد،
  `2` = تفضيل توقيعات مزدوجة, `3` = اشتراط التوقيعات المزدوجة).
- `issuers` — جهات الإصدار الحوكمية مع `fingerprint`, `ed25519_public`,
  `mldsa65_public`. تُحسب البصمات كالتالي:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — Utilice el SRCv2 (`RelayCertificateBundleV2::to_cbor()`). تحمل
  كل حزمة واصف relé وأعلام القدرات وسياسة ML-KEM وتوقيعات Ed25519/ML-DSA-65
  المزدوجة.

Utilice la CLI para acceder a la memoria caché
الحراس. لم تعد الرسومات JSON القديمة مقبولة؛ مطلوب لقطات SRCv2.

Utilice la CLI de `--guard-directory` para conectar la memoria caché.
يحافظ المحدد على الحراس المثبتين ضمن نافذة الاحتفاظ والمؤهلين في الدليل؛
وتستبدل relés الجديدة الإدخالات المنتهية. بعد نجاح الجلب تُكتب caché
Para obtener más información, consulte `--guard-cache`.
يمكن لـ SDK إعادة إنتاج هذا السلوك عبر
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` Aplicación
`GuardSet` Aquí está `SorafsGatewayFetchOptions`.`ml_kem_public_hex` Lanzamiento de PQ أثناء
SNNet-5. تعمل مفاتيح المراحل (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) الآن على تخفيض relés الكلاسيكية تلقائياً: عندما يتوفر حارس
PQ يقوم المحدد بإسقاط الدبابيس الكلاسيكية الزائدة لتفضيل المصافحات الهجينة.
Utilice el CLI/SDK para acceder a `anonymity_status`/`anonymity_reason`,
`anonymity_effective_policy`, `anonymity_pq_selected`, `anonymity_classical_selected`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sistema operativo/العجز/فارق
Hay apagones y retrocesos en el suministro de energía.

Esta es la versión SRCv2 de `certificate_base64`. يفك
المُنسِّق كل حزمة ويعيد التحقق من تواقيع Ed25519/ML-DSA ويحتفظ بالشهادة
المحللة بجانب caché الحراس. عند وجود شهادة تصبح المصدر المعتمد لمفاتيح PQ
وتفضيلات المصافحة y الترجيح؛ وتُهمل الشهادات المنتهية ويعود المحدد إلى حقول
الوصف القديمة. تنتشر الشهادات عبر إدارة دورة حياة الدوائر وتُعرض عبر
`telemetry::sorafs.guard` y `telemetry::sorafs.circuit` Establece una nueva versión
وحزم المصافحة وما إذا لوحظت توقيعات مزدوجة لكل حارس.

Utilice la CLI para acceder a los siguientes enlaces:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

ينزّل `fetch` اللقطة SRCv2 y منها قبل الكتابة على القرص، بينما يعيد
`verify` Archivos de configuración de archivos JSON y archivos JSON
Conecte el CLI/SDK.

### 1.5 مدير دورة حياة الدوائرعندما يتوفر دليل relés y caché الحراس معاً، يقوم المُنسِّق بتفعيل مدير دورة
حياة الدوائر لبناء وتجديد دوائر SoraNet قبل كل جلب. يقع الاعداد ضمن
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) عبر حقلين جديدين:

- `relay_directory`: La conexión entre SNNet-3 y los saltos intermedios/de salida están disponibles
  حتمي.
- `circuit_manager`: اعداد اختياري (ممكّن افتراضياً) يتحكم في TTL للدوائر.

Aquí Norito Texto JSON `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

تنقل SDKs بيانات الدليل عبر
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) ، وتقوم CLI بتوصيله تلقائياً عند تمرير
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

يجدد المدير الدوائر كلما تغيرت بيانات الحارس (punto final أو مفتاح PQ أو الطابع
الزمني للتثبيت) أو عند انتهاء TTL. يقوم المساعد `refresh_circuits` مستدعى
قبل كل جلب (`crates/sorafs_orchestrator/src/lib.rs:1346`) بإصدار سجلات
`CircuitEvent` لتمكين المشغلين من تتبع قرارات دورة الحياة. اختبار remojo
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) يثبت استقرار الكمون عبر ثلاث
دورات تبديل للحراس؛ راجع التقرير في
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Aplicación QUIC محلي

يمكن للمُنسِّق اختيارياً تشغيل وكيل QUIC محلي بحيث لا تضطر إضافات المتصفح
El SDK incluye archivos adjuntos y caché de caché. يرتبط الوكيل بعنوان
loopback, y el QUIC, y el manifiesto de Norito, y los que están conectados
caché الاختياري إلى العميل. تُعد أحداث النقل التي يصدرها الوكيل ضمن
`sorafs_orchestrator_transport_events_total`.

Utilice el código `local_proxy` para crear un archivo JSON:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` يتحكم بمكان استماع الوكيل (استخدم المنفذ `0` لطلب منفذ مؤقت).
- `telemetry_label` يمرر إلى المقاييس لتمييز الوكلاء عن جلسات الجلب.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض cache الحراس بالمفتاح ذاته
  Haga clic en CLI/SDK para instalar el software.
- `emit_browser_manifest` يبدّل ما إذا كان المصافحة تعيد manifiesto يمكن
  للامتدادات حفظه والتحقق منه.
- `proxy_mode` يحدد ما إذا كان الوكيل يجسر الحركة محلياً (`bridge`) أو يكتفي
  بإصدار بيانات وصفية (`metadata-only`) está instalado en los SDK de SoraNet.
  الافتراضي `bridge`; استخدم `metadata-only` عندما تريد إظهار manifiesto فقط.
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs`
  تلميحات إضافية للمتصفح لتقدير التوازي وفهم إعادة استخدام الدوائر.
- `car_bridge` (اختياري) يشير إلى cache محلي لأرشيفات CAR. Fuente de alimentación `extension`
  في اللاحقة المضافة عند غياب `*.car`؛ Nombre del producto: `allow_zst = true`
  `*.car.zst` المضغوطة مباشرة.
- `kaigi_bridge` (اختياري) يعرّض مسارات Kaigi للوكيل. يعلن `room_policy` ما إذا
  كان الجسر يعمل بوضع `public` أو `authenticated` حتى يختار المتصفح ملصقات GAR
  الصحيحة مسبقاً.
- `sorafs_cli fetch` y `--local-proxy-mode=bridge|metadata-only` y
  `--local-proxy-norito-spool=PATH` Bobinas de repuesto y bobinas de bobina
  Utilice el formato JSON.
- `downgrade_remediation` (اختياري) يضبط خطاف خفض المستوى التلقائي. عند التفعيل
  يراقب المُنسِّق تليمترية relé لرصد اندفاعات reducciones de categoría, وبعد تجاوز`threshold` خلال `window_secs` يفرض على الوكيل الانتقال إلى `target_mode`
  (الافتراضي `metadata-only`). وبعد توقف degradaciones يعود الوكيل إلى `resume_mode`
  Consulte `cooldown_secs`. Relé de relé `modes`
  (افتراضياً retransmisiones الدخول).

عندما يعمل الوكيل بوضع puente يخدم خدمتين للتطبيق:

- **`norito`** — يتم حل هدف البث الخاص بالعميل نسبةً إلى
  `norito_bridge.spool_dir`. تتم تنقية الأهداف (لا transversal ولا مسارات مطلقة)،
  وعندما لا يحتوي الملف على امتداد يتم تطبيق اللاحقة المحددة قبل بث الحمولة.
- **`car`** — Haga clic en `car_bridge.cache_dir` y haga clic en él.
  الافتراضي، وترفض الحمولة المضغوطة ما لم يتم تفعيل `allow_zst`. الجسور
  الناجحة ترد بـ `STREAM_ACK_OK` قبل نقل الأرشيف كي يتمكن العملاء من تنفيذ
  التحقق بالأنابيب.

Utilice HMAC como etiqueta de caché (para usar la etiqueta de caché)
المصافحة) ويسجل رموز سبب التليمترية `norito_*` / `car_*` حتى تميز لوحات
المتابعة بين النجاح، وفقدان الملفات، وفشل التنقية بسرعة.

`Orchestrator::local_proxy().await` يعرّض المقبض التشغيلي حتى يتمكن المستدعون
من قراءة شهادة PEM, أو جلب manifiesto للمتصفح، أو طلب إيقاف لطيف عند خروج
التطبيق.

عند التفعيل، يقدم الوكيل الآن سجلات **manifiesto v2**. إضافة إلى الشهادة ومفتاح
caché الحراس، تضيف v2 ما يلي:- `alpn` (`"sorafs-proxy/1"`) y `capabilities` para evitar errores.
- `session_id` de afinidad y de afinidad de `cache_tagging`
  HMAC está disponible.
- Programadores y programadores (`circuit`, `guard_selection`, `route_hints`) gratis
  واجهة أغنى قبل فتح البث.
- `telemetry_v2` مع مفاتيح أخذ العينات والخصوصية للأدوات المحلية.
- Para `STREAM_ACK_OK` y `cache_tag_hex`. يعكس العميل القيمة في ترويسة
  `x-sorafs-cache-tag` Para conexiones HTTP y TCP para conexiones de red
  مشفرة عند التخزين.

هذه الحقول متوافقة للخلف — يمكن للعملاء الأقدم تجاهل المفاتيح الجديدة
والاعتماد على مجموعة v1.

## 2. دلالات الفشل

يفرض المُنسِّق تحققاً صارماً من القدرات والميزانيات قبل نقل أي بايت. تقع
الإخفاقات في ثلاث فئات:1. **إخفاقات الأهلية (قبل التنفيذ).** المزوّدون الذين يفتقرون لقدرة النطاق، أو
   anuncios منتهية، أو تليمترية قديمة يتم تسجيلهم في لوحة النتائج ويُستبعدون من
   الجدولة. Configuración de CLI para `ineligible_providers` desde el dispositivo
   المشغلون من فحص انحراف الحوكمة دون كشط السجلات.
2. **الاستنزاف أثناء التشغيل.** يتتبع كل مزوّد الإخفاقات المتتالية. عند بلوغ
   `provider_failure_threshold` يتم وسم المزوّد `disabled` لبقية الجلسة. إذا
   انتقل جميع المزوّدين إلى `disabled` يعيد المُنسِّق
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **إجهاضات حتمية.** تظهر الحدود الصارمة كأخطاء مهيكلة:
   - `MultiSourceError::NoCompatibleProviders` — يتطلب المانيفست مدى شرائح أو
     محاذاة لا يمكن للمزوّدين الباقين تلبيتها.
   - `MultiSourceError::ExhaustedRetries` — تم استهلاك ميزانية إعادة المحاولة لكل
     شريحة.
   - `MultiSourceError::ObserverFailed` — رفض المراقبون aguas abajo (خطاطيف البث)
     شريحة تم التحقق منها.

تتضمن كل أخطاء فهرس الشريحة المخالفة وعند توفره سبب فشل المزوّد النهائي. تعامل
مع هذه الأخطاء كمعوّقات إصدار — إعادة المحاولة بذات المدخلات ستعيد الفشل حتى
يتغير anuncio أو التليمترية أو صحة المزوّد الأساسي.

### 2.1 حفظ لوحة النتائج

Asegúrese de que `persist_path` esté conectado a la red eléctrica. يحتوي
Nombre JSON:

- `eligibility` (`eligible` y `ineligible::<reason>`).
- `weight` (الوزن المطبع المعين لهذا التشغيل).
- بيانات `provider` الوصفية (المعرّف، puntos finales, ميزانية التوازي).أرشِف لقطات لوحة النتائج مع آثار الإطلاق حتى تبقى قرارات الحظر والrollout
قابلة للتدقيق.

## 3. التليمترية وتصحيح الاخطاء

### 3.1 versión Prometheus

يصدر المُنسِّق المقاييس التالية عبر `iroha_telemetry`:| المقياس | Etiquetas | الوصف |
|---------|--------|-------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | مقياس عدّاد للجلب الجاري. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | هيستوغرام يسجل زمن الجلب من طرف إلى طرف. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | عدّاد للإخفاقات النهائية (استنزاف إعادة المحاولة، عدم وجود مزوّدين، فشل المراقب). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | عدّاد لمحاولات إعادة المحاولة لكل مزوّد. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | عدّاد لإخفاقات المزوّد على مستوى الجلسة المؤدية للتعطيل. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | عدد قرارات سياسة الإخفاء (تحقق/تدهور) بحسب مرحلة rollout وسبب التراجع. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | هيستوغرام لحصة relés PQ ضمن مجموعة SoraNet المختارة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | هيستوغرام لنسب توفر relevos PQ في لقطة marcador. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | هيستوغرام لعجز السياسة (الفجوة بين الهدف والحصة الفعلية). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | هيستوغرام لحصة relés الكلاسيكية في كل جلسة. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | هيستوغرام لعدد relés الكلاسيكية المختارة في كل جلسة. |ادمج المقاييس في لوحات puesta en escena قبل تشغيل مفاتيح الإنتاج. التخطيط الموصى به
يعكس خطة observabilidad del SF-6:

1. **الجلب النشط** — تنبيه إذا ارتفع القياس دون اكتمالات مقابلة.
2. **نسبة إعادة المحاولة** — تحذير عند تجاوز عدادات `retry` للخطوط الاساسية.
3. **إخفاقات المزوّد** — تشغيل buscapersonas عند تجاوز أي مزوّد `session_failure > 0`
   Hace 15 días.

### 3.2 اهداف السجل المهيكل

ينشر المُنسِّق أحداثاً مهيكلة إلى اهداف حتمية:

- `telemetry::sorafs.fetch.lifecycle` — علامات `start` و`complete` مع عدد الشرائح
  والمحاولات والمدة الاجمالية.
- `telemetry::sorafs.fetch.retry` — احداث إعادة المحاولة (`provider`, `reason`,
  `attempts`) لدعم triage يدوي.
- `telemetry::sorafs.fetch.provider_failure` — مزوّدون تم تعطيلهم بسبب تكرار
  الاخطاء.
- `telemetry::sorafs.fetch.error` — اخفاقات نهائية مع `reason` وبيانات مزوّد
  اختيارية.

وجّه هذه التدفقات إلى مسار السجلات Norito الحالي لكي تمتلك الاستجابة للحوادث
مصدر حقيقة واحد. تُظهر أحداث دورة الحياة مزيج PQ/الكلاسيكي عبر
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
ومعاداتها المرافقة، ما يجعل ربط لوحات المتابعة سهلاً دون كشط المقاييس. أثناء
implementaciones الخاصة بـ GA، ثبّت مستوى السجلات على `info` لأحداث دورة الحياة/إعادة
المحاولة واعتمد `warn` للأخطاء النهائية.

### 3.3 archivos JSON

Basado en `sorafs_cli fetch` y SDK Rust, este es el siguiente:- `provider_reports` مع اعداد النجاح/الفشل وما إذا تم تعطيل المزوّد.
- `chunk_receipts` توضح المزوّد الذي لبّى كل شريحة.
- Números `retry_stats` y `ineligible_providers`.

أرشِف ملف الملخص عند تتبع مزوّدين سيئين — تتطابق الإيصالات مباشرة مع بيانات
السجل أعلاه.

## 4. قائمة تشغيل تشغيلية

1. **حضّر الاعداد في CI.** شغّل `sorafs_fetch` بالاعداد المستهدف، ومرر
   `--scoreboard-out` لالتقاط عرض الأهلية، وقارن مع الإصدار السابق. أي مزوّد
   غير مؤهل بشكل غير متوقع يوقف الترقية.
2. **تحقق من التليمترية.** تأكد من أن النشر يصدر مقاييس `sorafs.fetch.*` وسجلات
   مهيكلة قبل تمكين الجلب متعدد المصادر للمستخدمين. غياب المقاييس يشير عادةً
   إلى أن واجهة المُنسِّق لم تُستدعَ.
3. **وثّق التجاوزات.** عند تطبيق إعدادات طارئة `--deny-provider` أو
   `--boost-provider`, utilice JSON (CLI) para ejecutar el archivo. يجب أن تعكس
   عمليات الرجوع إزالة التجاوز والتقاط لقطة marcador جديدة.
4. **أعد تشغيل اختبارات humo.** بعد تعديل ميزانيات إعادة المحاولة أو حدود
   المزوّدين، أعد جلب accesorio القياسي (`fixtures/sorafs_manifest/ci_sample/`) وتأكد
   أن إيصالات الشرائح ما زالت حتمية.

اتباع الخطوات أعلاه يحافظ على قابلية إعادة الإنتاج عبر implementaciones المرحلية ويوفر
التليمترية اللازمة للاستجابة للحوادث.

### 4.1 Actualizacionesيمكن للمشغلين تثبيت مرحلة النقل/الاخفاء النشطة دون تعديل الاعداد الاساسي عبر
Aquí `policy_override.transport_policy` y `policy_override.anonymity_policy`.
JSON escrito en `orchestrator` (en inglés `--transport-policy-override=` /
`--anonymity-policy-override=` o `sorafs_cli fetch`). عند وجود أي anulación,
يتجاوز المُنسِّق caída de red de respaldo المعتاد: إذا تعذر تحقيق طبقة PQ المطلوبة،
Utilice el `no providers` para obtener más información. الرجوع للسلوك الافتراضي
يتم ببساطة عبر مسح حقول anulación.