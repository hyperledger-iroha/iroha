---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تكوين الأوركسترا
العنوان: تكوين orquestrador SoraFS
Sidebar_label: تكوين orquestrador
الوصف: قم بتكوين أداة جلب العناصر المتعددة وتفسير الأخطاء وإخراج أداة القياس عن بعد.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/developer/orchestrator.md`. قم بحفظ النسخ المتزامنة حتى تتم إزالة المستندات البديلة.
:::

# أداة جلب orquestrador متعددة الأصل

يقوم أداة جلب الملفات المتعددة الأصل SoraFS بإجراء تنزيلات محددة
موازية من مجموعة من مقدمي العروض المنشورة في الإعلانات المستجيبة
بيلا الحاكم. يتم شرح هذا الدليل كتكوين أو أوركيسترادور، من أي مكان
الترقب أثناء عمليات النشر وما يحدث من تدفقات القياس عن بعد
مؤشرات السعادة.

## 1.الوجهة العامة للتكوين

يجمع الأوركسترا بين ثلاثة خطوط تكوين:| فونتي | اقتراح | نوتاس |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | تطبيع عدد البيزوات، والتحقق من دقة القياس عن بعد، والحفاظ على لوحة النتائج JSON المستخدمة للمستمعين. | تم التثبيت بواسطة `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | تطبيق حدود وقت التشغيل (ميزانيات إعادة المحاولة، حدود المزامنة، تبديل التحقق). | خريطة لـ `FetchOptions` في `crates/sorafs_car::multi_fetch`. |
| معلمات CLI / SDK | الحد من عدد الأقران ومناطق القياس عن بعد وإظهار سياسات الرفض/التعزيز. | `sorafs_cli fetch` يعرض هذه العلامات مباشرة؛ نشر نظام التشغيل SDKs عبر `OrchestratorConfig`. |

المساعدين JSON em `crates/sorafs_orchestrator::bindings` serializam a
تكوين كامل في Norito JSON، والتنقل بين روابط SDK
ه السيارات.

### 1.1 مثال لتكوين JSON

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
```

Persista o arquivo atraves do empilhamento المعتاد do `iroha_config` (`defaults/`,
المستخدم، الفعلي) لكي تحدد عمليات النشر حدود النظام الداخلي
العقد. للحصول على ملف احتياطي مباشر فقط عند طرح SNNet-5a،
راجع `docs/examples/sorafs_direct_mode_policy.json` باتجاه الشرق
المراسل في `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 تجاوزات الامتثال

SNNet-9 متوافق مع التوجه المتكامل للحاكم. أم
الكائن الجديد `compliance` والتكوين Norito JSON يلتقط العناصر المنحوتة
forcam o خط الأنابيب لجلب الوضع المباشر فقط:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` يعلن عن رموز ISO-3166 alpha-2 الموجودة في هذا المكان
  مثيل لأوبرا orquestrador. Os codegos saonormalizados para maiusculas
  خلال أو تحليل.
- `jurisdiction_opt_outs` يتم تسجيله أو تسجيله في الإدارة. عندما يكون الأمر كذلك
  يظهر المشغل القانوني في القائمة، أو يتم تطبيقه
  `transport_policy=direct-only` ويصدر دافعًا احتياطيًا
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` قائمة ملخصات البيان (CIDs التالية، المشفرة
  عرافة maiusculo). الحمولات الصافية للمراسلين تم دمجها مع أجندة الكاميرا المباشرة فقط
  واختبر الإجراء الاحتياطي `compliance_blinded_cid_opt_out` في القياس عن بعد.
- `audit_contacts` يتم تسجيله كعناوين URI التي يجب أن يحكمها المشغلون
  كتب اللعب العامة GAR.
- `attestations` يلتقط حزم المطابقة التي تم الاستيلاء عليها والتي تدعمها
  سياسة. كل مدخل يحدد uma `jurisdiction` اختياري (codigo ISO-3166
  alpha-2)، أم `document_uri`، أو `digest_hex` الكنسي ذو 64 حرفًا، أو
  الطابع الزمني للإرسال `issued_at_ms` و`expires_at_ms` اختياري. جوهر
  مصنوعات غذائية أو قائمة مرجعية لسماعات الأوركسترا من أجل ذلك
  أدوات الحكم possam vincular تتجاوز مستندات القتل.Forneca o block de conformidade via o empilhamento custom de configuracao para
أن المشغلين يقبلون تجاوز المحددات. يا أوركيسترادور تطبيق أ
مطابقة _depois_ تلميحات وضع الكتابة: أثناء طلب SDK
`upload-pq-only`، إلغاء الاشتراك في القانون أو البيان الخاص بالنقل
للتوجيه المباشر فقط والبدء بسرعة عندما لا يكون هناك أي تطابق.

الكتالوجات الأساسية لإلغاء الاشتراك حية
`governance/compliance/soranet_opt_outs.json`; o مجلس الإدارة العام
atualizacoes عبر الإصدارات tagueadas. مثال كامل للتكوين
(بما في ذلك الشهادات) هذا متاح
`docs/examples/sorafs_compliance_policy.json`، وهذه العملية التشغيلية
رقم الالتقاط
[قواعد اللعبة المتوافقة مع GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 إعدادات CLI وSDK| العلم / كامبو | افيتو |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | الحد الأقصى للكمية المسموح بها هو البقاء على قيد الحياة عند تصفية لوحة النتائج. قم بتعريف `None` لاستخدام جميع المستندات الاختيارية. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | الحد من إعادة المحاولة لكل قطعة. تجاوز أو الحد الأقصى `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta لقطات الكمون/الخطأ ليست منشئ لوحة النتائج. القياس عن بعد قديم على مستوى `telemetry_grace_secs` تم إثباته على أنه غير صحيح. |
| `--scoreboard-out` | استمر في حساب لوحة النتائج (المزودون الاختياريون + غير المختارين) للتحقق من نقطة التشغيل. |
| `--scoreboard-now` | قم بإخفاء الطابع الزمني للوحة النتائج (ثاني Unix) للحفاظ على إعدادات تحديد المباريات. |
| `--deny-provider` / خطاف النتيجة السياسية | باستثناء إثباتات الشكل الحتمي دون حذف الإعلانات. الاستفادة من القائمة السوداء السريعة. |
| `--boost-provider=name:delta` | قم بتعديل الائتمانات من خلال مدقق يحافظ على أموال الإدارة السليمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | قم بتدوير مقاييس التخفيضات والسجلات المُصممة حتى تتمكن لوحات المعلومات من الترشيح حسب الموقع الجغرافي أو عند بدء التشغيل. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | هذا هو المكان الذي يوجد فيه `soranet-first` وهو أوركسترا متعدد الأصول وقاعدة. استخدم `direct-only` للتحضير للرجوع إلى إصدار أقدم أو متابعة توجيه مطابقة، واحتفظ بـ `soranet-strict` للطيارين PQ فقط؛ تجاوزات المطابقة المستمرة ترسل أو تيتو جامدة. |

SoraNet-first ومساحة البيئة، والتراجعات لها خاصية القفل أو القفل
SNNet ذات الصلة. بعد أن تخرجت SNNet-4/5/5a/5b/6a/7/8/12/13، أ
الحاكمة التي تتحمل الوضعية المطلوبة (rumo a `soranet-strict`)؛ أكلت لا,
يؤدي هذا إلى تجاوز الدوافع وراء الأحداث من أجل إعطاء الأولوية `direct-only`، وما إلى ذلك
لن يتم تسجيل أي سجل للطرح.

جميع أعلام نظام التشغيل Acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
تم تطوير binario `sorafs_fetch`. تظهر أدوات تطوير البرامج (SDKs) كوسائل ظاهرية
من أجل أفضل أنواع البنائين.

### 1.4 وظيفة الحراسة المؤقتة

سيتم دمج CLI أو محدد حراس SoraNet حتى يتمكن المشغلون من الوصول
إصلاح مرحلات إدخال الشكل الحتمي قبل بدء التشغيل بالكامل
نقل SNNet-5. ثلاثة أعلام جديدة للتحكم في التدفق:| علم | اقتراح |
|------|-----------|
| `--guard-directory <PATH>` | Aponta لملف JSON الذي يصف توافق التتابعات الأحدث (مجموعة فرعية بعد ذلك). قم بتمرير الدليل أو تحديثه أو تخزين ذاكرة التخزين المؤقت قبل التنفيذ أو الجلب. |
| `--guard-cache <PATH>` | استمر في ترميز `GuardSet` في Norito. يتم تنفيذ عمليات إعادة الاستخدام اللاحقة لذاكرة التخزين المؤقت في الدليل الجديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | يتجاوز الخيارات لعدد واقيات الإدخال للتثبيت (البطاقة 3) وبطاقة الحفظ (البطاقة 30 يومًا). |
| `--guard-cache-key <HEX>` | هناك اختياري يستخدم 32 بايت لتمييز ذاكرات التخزين المؤقت باستخدام MAC Blake3 حتى يتم حفظ الملف الذي تم التحقق منه قبل إعادة استخدامه. |

كدليل حراسة البضائع المستخدمة في esquema Compacto:

يا علم `--guard-directory` أغورا في انتظار الحمولة `GuardDirectorySnapshotV2`
تم ترميزه في Norito. يا لقطة ثنائية الفكر:- `version` — العكس هو الصحيح (`2` في الوقت الحالي).
- `directory_hash`، `published_at_unix`، `valid_after_unix`، `valid_until_unix` —
  Metadados de Conso الذي يجب أن يتوافق مع كل شهادة متضمنة.
- `validation_phase` — بوابة السياسة للشهادات (`1` = السماح لنا
  assinatura Ed25519، `2` = تفضيل assinaturas duplas، `3` = exigir assinaturas
  مزدوج).
- `issuers` - باعثات الإدارة مع `fingerprint`، `ed25519_public` e
  `mldsa65_public`. بصمات الأصابع sao calculados como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — قائمة حزم SRCv2 (جديدة)
  `RelayCertificateBundleV2::to_cbor()`). كل حزمة carrega o واصف تفعل
  التتابع وأعلام القدرة والسياسة ML-KEM والقتلة المزدوجة Ed25519/ML-DSA-65.

A CLI تحقق من كل حزمة تتعارض مع رؤساء الباعث المعلنين مسبقًا
دليل mesclar مع حراس ذاكرة التخزين المؤقت. Esbocos JSON بدائل لا أكثر
اسيتوس. لقطات SRCv2 sao obrigatorios.استدعاء CLI com `--guard-directory` للتنسيق أو التوافق الأحدث مع o
ذاكرة التخزين المؤقت موجودة. يحافظ المحدد على الحراسة المثبتة في مكانه
janela de retencao e sao elegiveis no Directory; novos تتابع الإدخالات البديلة
انتهاء الصلاحية. بعد جلبها بنجاح، أو تحديث ذاكرة التخزين المؤقت، أو كتابتها
لا يوجد طريق مباشر عبر `--guard-cache`، استمر في الجلسات اللاحقة
com.criteriosas. يمكن لـ SDKs إنتاج نفس السلوك
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` ه
انتقل إلى `GuardSet` الناتج عن `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` يسمح للمحدد بمنح الأولوية للحماية مع السعة PQ
عند بدء تشغيل SNNet-5، يتم ذلك. تبديل المفاتيح (`anon-guard-pq`،
`anon-majority-pq`، `anon-strict-pq`) أغورا ريبايسام ينقل الكلاسيكيات
تلقائيًا: عندما يتم توفير حارس PQ أو اختيار إزالة الدبابيس
كلاسيكيات تفوق الجلسات اللاحقة المفضلة في المصافحة الهجينة.
تظهر السيرة الذاتية لـ CLI/SDK نتيجة ضبابية عبر `anonymity_status`/
`anonymity_reason`، `anonymity_effective_policy`، `anonymity_pq_selected`،
`anonymity_classical_selected`، `anonymity_pq_ratio`، `anonymity_classical_ratio`
المجالات التكميلية للمرشحين/العجز/دلتا العرض، الإعصار الواضح
Brownouts والاحتياطيات الكلاسيكية.يمكن لأدلة الحراس الآن تضمين حزمة SRCv2 كاملة عبر
`certificate_base64`. يا orquestrador decodifica كل حزمة، إعادة التحقق من صحتها
Assinaturas Ed25519/ML-DSA واستعادة الشهادة التي تم تحليلها إلى جانب ذاكرة التخزين المؤقت
حراس. عند تقديم الشهادة، ستنتقل إلى مصدر Canonica
هناك حاجة إلى PQ وتفضيلات المصافحة والتفكير؛ شهادات انتهاء الصلاحية ساو
تم حذفه وإرجاع المحدد إلى البدائل البديلة للواصف. الشهادات
Propagam-se pela gestao do ciclo de vida de Circuitos e sao expostos via
`telemetry::sorafs.guard` و`telemetry::sorafs.circuit`، الذي قمت بالتسجيل فيه
التحقق من الصحة، مجموعات المصافحة e se assinaturas duplas foram observadas para
حارس كادا.

استخدم مساعدي CLI لتوفير اللقطات المتزامنة مع الناشرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` قم بإخفاء والتحقق من لقطة SRCv2 قبل الرسم على القرص أثناء ذلك
`verify` إعادة إنتاج خط أنابيب التحقق من المصنوعات اليدوية من معدات أخرى،
قم بإصدار ملخص JSON يُشير إلى أداة اختيار حراس CLI/SDK.

### 1.5 مؤشر دورة حياة الدائرة

عندما يكون دليل الترحيل عبارة عن ذاكرة تخزين مؤقت للحارس أو المنسق
تنشيط أو إدارة دورة حياة الدوائر من أجل البناء المسبق والتجديد
دوائر SoraNet قبل كل شيء جلب. تم تكوين الحياة على `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) عبر دويس نوفو كامبوس:- `relay_directory`: نقل لقطة الدليل إلى SNNet-3 للقفز
  يتم تحديد وسط/خروج من شكل التحديد.
- `circuit_manager`: التكوين الاختياري (التأهيل للبطاقة) الذي يتحكم في
  TTL تفعل الدائرة.

Norito JSON Agora aceita um bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

يقوم Os SDKs encaminham dados بالدليل عبر
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)، وCLI أو الاتصال تلقائيًا دائمًا
`--guard-directory` والتنفيذ (`crates/iroha_cli/src/commands/sorafs.rs:365`).

يقوم المدير بتجديد الدوائر باستمرار بحيث تقوم Metadados بحماية Mudam (نقطة النهاية، Chave
PQ أو الطابع الزمني الذي تم إصلاحه) أو عند انتهاء صلاحية TTL. يا مساعد `refresh_circuits`
استدعاء مسبق للجلب (`crates/sorafs_orchestrator/src/lib.rs:1346`)
قم بإصدار السجلات `CircuitEvent` حتى يتمكن المشغلون من اتخاذ قرارات سيكلو
دي فيدا. يا اختبار النقع `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) يوضح حالة الكمون
اجتياز ثلاث دوارات من الحراس؛ شاهد العلاقة
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 وكيل QUIC محلي

يمكن للأوركسترادور اختياريًا بدء وكيل QUIC محلي للنطاقات
متصفح ومحولات SDK غير دقيقة لإدارة الشهادات أو كل منها
مخبأ دي حراس. من خلال رابط الوكيل إلى استرجاع داخلي، يتم الاتصال بـ QUIC e
قم بإرجاع البيان Norito الذي تم الكشف عنه أو الشهادة وحافظ على ذاكرة التخزين المؤقت
اختياري للعميل. أحداث النقل الصادرة عن الوكيل عبر جهات الاتصال
`sorafs_orchestrator_transport_events_total`.إمكانية استخدام الوكيل من خلال الكتلة الجديدة `local_proxy` بدون JSON do orquestrador:

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
```- `bind_addr` للتحكم في مفتاح الوكيل (استخدم المنفذ `0` لطلب المنفذ
  الأفيميرا).
- نشر `telemetry_label` كمقاييس لتمييز لوحات المعلومات
  جلسات الجلب الوكيلة.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض أو نفس ذاكرة التخزين المؤقت
  الحراس الذين يستخدمون CLI/SDKs، يحافظون على النطاقات للتنقل المستمر.
- `emit_browser_manifest` البديل هو أن المصافحة تؤول إلى بيان ما
  الامتدادات podem armazenar e validar.
- `proxy_mode` يتم تحديده أو إرساله عبر الوكيل Faz Bridge المحلي (`bridge`)
  Metadados لكي تقوم SDKs بتكوين دوائر SoraNet لحسابها الخاص
  (`metadata-only`). يا وكيل Padrao e `bridge`; استخدم `metadata-only` عندما
  تعمل محطة العمل على تصدير تدفقات إعادة الإرسال الواضحة.
- `prewarm_circuits`، `max_streams_per_circuit` و`circuit_ttl_hint_secs`
  عرض تلميحات إضافية للمتصفح حتى تتمكن من الحصول على تدفقات متوازية
  أدخل مقدارًا أو وكيلًا لإعادة استخدام الدوائر.
- `car_bridge` (اختياري) خيار لذاكرة التخزين المؤقت المحلية لملفات CAR. يا كامبو
  `extension` التحكم في الملحق عند حذف أو ارتفاع الدفق `*.car`;
  قم بتعريف `allow_zst = true` لخادم الحمولات `*.car.zst` المسبقة التركيب.
- `kaigi_bridge` (اختياري) لعرض التدوير في التخزين المؤقت للوكيل. يا كامبو
  `room_policy` أعلن عن جسر الأوبرا في الوضع `public` ou `authenticated`حتى يتمكن العملاء من تصفح التصنيفات الصحيحة لـ GAR مسبقًا.
- `sorafs_cli fetch` يتجاوز العرض `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`، يسمح لك بتغيير طريقة التشغيل أو
  Apontar para البكرات البديلة غير المعدلة إلى السياسة JSON.
- `downgrade_remediation` (اختياري) التكوين أو ربط الرجوع إلى إصدار سابق تلقائيًا.
  عندما يكون مؤهلًا، يقوم الأوركسترا بمراقبة قياس المرحلات عن بعد للرحلات
  الرجوع إلى إصدار أقدم، بعد تكوين `threshold` في `window_secs`،
  الوكيل المحلي لـ `target_mode` (padrao `metadata-only`). تخفيض نظام التشغيل Quando
  cessam، أو يعود الوكيل إلى `resume_mode` apos `cooldown_secs`. استخدم المصفوفة
  `modes` للحد من تشغيل وظائف التتابع المحددة (مرحلات التبديل
  دي المدخل).

عند استخدام الوكيل في الوضع الجسري، يتم تقديم خدمات التطبيق التالية:- **`norito`** – مستوى تدفق العميل والحل المتعلق به
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (sem traversal, sem caminhos
  مطلق)، وعندما يتم حفظ الملف بدون امتداد، أو يتم تكوين اللاحقة وتطبيقها
  يتم نقل الحمولة المسبقة إلى الملاح.
- **`car`** – يتم حل التدفق داخل `car_bridge.cache_dir`، هنا
  توسيع نطاق التكوين وإعادة تحميل الحمولات المضمنة على الأقل
  `allow_zst` مؤهل. الجسور bem-sucedidos تستجيب com `STREAM_ACK_OK`
  قبل نقل وحدات البايت إلى الملف حتى يتمكن العملاء من استخدام خط الأنابيب
  التحقق.

في جميع الحالات أو الوكيل الخاص بعلامة التخزين المؤقت HMAC (عندما يكون هناك أي شيء آخر
ذاكرة التخزين المؤقت للحماية أثناء المصافحة) وتسجيل رموز القياس عن بعد
`norito_*` / `car_*` لتميز لوحات المعلومات بالنجاح والملفات الأصلية
و falhas de sanitizacao بسرعة.

`Orchestrator::local_proxy().await` يعرض التعامل مع التنفيذ للمكالمات
الحصول على شهادة PEM أو البحث عن بيان التنقل أو المطالبة
Encerramento gracioso quando aplicacao Finaliza.

عند التمكن من ذلك، سيخدم الوكيل الآن السجلات **البيان v2**. عالم القيام
شهادة موجودة ومفتاح حماية ذاكرة التخزين المؤقت، إضافة الإصدار الثاني:- `alpn` (`"sorafs-proxy/1"`) ومصفوفة `capabilities` للعملاء
  أكد بروتوكول البث الذي يجب أن يحدث.
- Um `session_id` للمصافحة وكتلة `cache_tagging` للاشتقاق
  تحسينات الحراسة من خلال الجلسات والعلامات HMAC.
- تلميحات الدائرة والاختيار للحماية (`circuit`، `guard_selection`،
  `route_hints`) لتتمكن المتصفحات المتكاملة من عرض واجهة مستخدم أفضل مسبقًا
  تيارات مفتوحة.
- `telemetry_v2` مع مقابض التوسيع والخصوصية للأجهزة المحلية.
- كل `STREAM_ACK_OK` يتضمن `cache_tag_hex`. العملاء يتحدثون عن الشجاعة بدون رأس
  `x-sorafs-cache-tag` لإصدار متطلبات HTTP أو TCP لتحديد الحماية
  في ذاكرة التخزين المؤقت للتشفير بشكل دائم.

هذه المجالات جزء من المخطط الفعلي; يجب على العملاء استخدام المجموعة
تيارات كاملة للتفاوض.

## 2. دلالات الفلاسفة

يطبق المشرف قدرات التحقق والميزانيات السابقة لك
Unico بايت سيجا ترانسفيردو. كما هو الحال في ثلاث فئات:1. **Falhas de elegibilidade (ما قبل الرحلة).** مقدمو الخدمة بدون قدرة النطاق،
   الإعلانات التي انتهت صلاحيتها أو القياس عن بعد عفا عليها الزمن ولم يتم تسجيلها بشكل مصطنع
   لوحة النتائج وحذف جدول الأعمال. Resumos da CLI preenchem o array
   `ineligible_providers` مع الحلول التي يفحصها المشغلون
   سجلات Goveranca SEM RASPAR.
2. ** يتم الاعتماد عليه في وقت التشغيل. ** يتم إثبات كل خطأ متتابع. عندما
   `provider_failure_threshold` وتم إثباته وتسويقه مثل `disabled`
   بيلو ريستانتي دا سيساو. إذا تم نقل كافة الإجراءات إلى `disabled`،
   يا أوركيسترادور ريتورنا
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **إجهاض الحتمية.** يحد من القيود الصارمة التي تطرأ على أخطاء الإنشاءات:
   - `MultiSourceError::NoCompatibleProviders` — o بيان exige um range
     قطع أو جزء من أن المحافظين لا يحظون بالاحترام.
   - `MultiSourceError::ExhaustedRetries` — o ميزانية إعادة المحاولة لكل قطعة
     com.consumido.
   - `MultiSourceError::ObserverFailed` - مراقبون في اتجاه مجرى النهر (خطافات de
     البث) تم التحقق من صحتها.

كل خطأ في تضمين مؤشر القطعة يسبب مشكلة، وعندما يتم توفيره، يتم قطعه
النهائي دي فالها القيام بروبروف. عالج هذه الأخطاء مثل حواجز الإصدار —
إعادة المحاولات مع نفس المدخلات لإعادة إنتاجها بشكل صحيح أو إعلان أو قياس عن بعد
ou a saudi do providor subjacente mudem.

### 2.1 الإصرار على لوحة النتائجعند تكوين `persist_path`، يخرج الأوركسترا من لوحة النتائج النهائية
تشغيل apos cada. مستند JSON contem:

- `eligibility` (`eligible` أو `ineligible::<reason>`).
- `weight` (البيزو نورماليزادو أتريبويدو بارا هذا المدى).
- التعريفات `provider` (المعرف، نقاط النهاية، ميزانية المطابقة).

يمكنك حفظ لقطات من لوحة النتائج جنبًا إلى جنب مع أدوات الإصدار لاتخاذ القرار
الحظر والتشغيل الدائم للمراجعة.

## 3. القياس عن بعد والتشغيل

### 3.1 متريكاس Prometheus

يصدر الأوركسترا كمقاييس متتابعة عبر `iroha_telemetry`:| متريكا | التسميات | وصف |
|---------|-------|-----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`، `region` | مقياس جلب عمليات البحث. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`، `region` | رسم بياني مسجل لوقت الجلب من بونتا إلى بونتا. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`، `region`، `reason` | Contador de falhas terminais (إعادة المحاولة esgotados، sem provores، falha de observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`، `provider`، `reason` | محاسب إعادة المحاولة من قبل المثبت. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`، `provider`، `reason` | Contador de falhas de providor na sessao que levam a disability. |
| `sorafs_orchestrator_policy_events_total` | `region`، `stage`، `outcome`، `reason` | نتجت القرارات السياسية المجهولة (الوعد مقابل الانسحاب) من مرحلة الطرح والدافع الاحتياطي. |
| `sorafs_orchestrator_pq_ratio` | `region`، `stage` | الرسم البياني للمشاركة في مرحلات PQ مع مجموعة SoraNet المحددة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`، `stage` | الرسم البياني لنسب عرض التبديلات PQ لا يحتوي على لقطة على لوحة النتائج. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`، `stage` | الرسم البياني للعجز السياسي (فجوة بين الجميع ومشاركة PQ الحقيقية). |
| `sorafs_orchestrator_classical_ratio` | `region`، `stage` | الرسم البياني للمشاركة في التتابعات الكلاسيكية المستخدمة في كل جلسة. || `sorafs_orchestrator_classical_selected` | `region`، `stage` | رسم بياني لعدوى المرحلات الكلاسيكية المختارة خلال الجلسة. |

قم بدمج المقاييس في لوحات المعلومات المرحلية قبل تأهيل المقابض
producao. التخطيط الموصى به لتمثيل أو خطة مراقبة SF-6:

1. **جلب العناصر** — تنبيه إذا كنت تقيس مدى اكتمال المراسلات.
2. **إعادة المحاولة** — عندما تتجاوز المنافسين `retry` خطوط الأساس
   تاريخي.
3. **Falhas de profetor** — تنبيهات مختلفة عند عدم وجود جهاز النداء عند أي مقدم
   Cruza `session_failure > 0` في 15 دقيقة.

### 3.2 أهداف سجل التطوير

يقوم المنظم بنشر الأحداث التدريبية لتحديد الأهداف:

- `telemetry::sorafs.fetch.lifecycle` - ماركادوريس `start` و`complete` com
  عدوى القطع وإعادة المحاولة والإجمالي.
- `telemetry::sorafs.fetch.retry` — أحداث إعادة المحاولة (`provider`، `reason`،
  `attempts`) لدليل فرز المواد الغذائية.
- `telemetry::sorafs.fetch.provider_failure` - إثبات عدم القدرة على التحمل أ
  أخطاء متكررة.
- `telemetry::sorafs.fetch.error` — انتهى الأمر مع `reason` e
  metadados opcionais doprovider.قم بتدفق هذه التدفقات لخط أنابيب السجلات Norito الموجود لذلك
الرد على الأحداث يكون بمثابة مصدر واحد للخضرة. أحداث دورة الحياة
عرض مستورا PQ/classica عبر `anonymity_effective_policy`،
`anonymity_pq_ratio`، `anonymity_classical_ratio` وشركائك المرتبطين،
تورناندو تبسط لوحات المعلومات المتكاملة مع قياسات راسبار. دورانتي طرح دي
GA، قم بإصلاح مستوى السجل `info` لأحداث دورة الحياة/إعادة المحاولة والاستخدام
`warn` للنهاية النهائية.

### 3.3 السيرة الذاتية JSON

Tanto `sorafs_cli fetch` من حيث إعادة تطوير SDK Rust لاستكمال البناء التنافسي:

- `provider_reports` com contagens de sucesso/falha e se oprofetor foi
  desabilitado.
- `chunk_receipts` تفاصيل يدوية مؤهلة لكل قطعة.
- المصفوفات `retry_stats` و `ineligible_providers`.

أرشيف ملف السيرة الذاتية للتخلص من المشكلات — الإيصالات
خريطة مباشرة لامتدادات السجل الحالي.

## 4. قائمة المراجعة التشغيلية1. **إعداد تكوين CI.** تنفيذ `sorafs_fetch` مع التكوين
   ألفا، قم بتمرير `--scoreboard-out` للحصول على تأشيرة الأهلية
   قارن كوم أو الإصدار السابق. كل ما أثبته هو أنه غير مؤهل
   interrompe a promocao.
2. **التحقق من القياس عن بعد.** ضمان نشر مقاييس التصدير `sorafs.fetch.*`
   السجلات التي تم تصميمها مسبقًا للتأهيل تجلب أصولًا متعددة للمستخدمين. أ
   تشير زيادة المقاييس عادةً إلى أن المنسق لم يقم بذلك بعد
   استدعاء.
3. **تجاوزات المستندات.** يمكنك تطبيق `--deny-provider` أو `--boost-provider`
   الطوارئ، قم بإنشاء JSON (أو استدعاء CLI) بدون سجل التغيير. تم تطوير عمليات التراجع
   التراجع أو التجاوز والتقاط لقطة جديدة للوحة النتائج.
4. **إعادة تنفيذ اختبارات الدخان.** بعد تعديل ميزانيات إعادة المحاولة أو حدودها
   Pro, Refaça oجلب تركيبات Canonico
   (`fixtures/sorafs_manifest/ci_sample/`) والتحقق من إيصالات القطع
   محددات الدوام.

اتبع الخطوات التالية للحفاظ على سلوك المنسق التكاثري فيها
عمليات النشر من أجل Fase e Fornece a Telemetry ضرورية للاستجابة للحوادث.

### 4.1 تجاوزات السياسةيمكن للمشغلين إصلاح حالة النقل/المجهول دون تحريرها
قاعدة التكوين المحددة `policy_override.transport_policy` e
`policy_override.anonymity_policy` في JSON الخاص بك من `orchestrator` (أو ترجمته
`--transport-policy-override=` / `--anonymity-policy-override=` ao
`sorafs_cli fetch`). عندما يتجاوز هذا الحاضر، أو الأوركسترا
o التراجع الاحتياطي المعتاد: إذا كان مستوى PQ مطلوبًا فلا يمكن أن يكون مرضيًا، o
قم بإحضار falha com `no providers` في وقت واحد للتخلي عن صمتك. يا التراجع
من أجل السلوك والسلوك البسيط، بقدر ما يخفف مجال التجاوز.