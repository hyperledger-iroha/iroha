---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تكوين الأوركسترا
العنوان: تكوين orquestador de SoraFS
Sidebar_label: تكوين الباحث
الوصف: قم بتكوين أداة الجلب متعددة المصادر وتفسير السقوط وإزالة القياس عن بعد.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/orchestrator.md`. احتفظ بالنسخ المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

# أداة الجلب متعددة الأصول

أداة الجلب المتعددة الأصل SoraFS دفعة تنزيلات محددة و
موازية من مجموعة الموردين المنشورة والإعلانات التي تم الرد عليها
لا جوبيرنانزا. يتم شرح هذا الدليل كتكوين الأوركيستادور الذي يظهر
ترقب خلال عمليات الإطلاق وستؤدي تدفقات القياس عن بعد إلى إظهار المؤشرات
دي سالود.

## 1. استئناف التكوين

يجمع الأوركيستادور بين ثلاثة مصادر للتكوين:| فوينتي | اقتراح | نوتاس |
|--------|----------|-------|
| `OrchestratorConfig.scoreboard` | تطبيع أسعار الموردين، والتحقق من دقة القياس عن بعد، والحفاظ على لوحة النتائج JSON المستخدمة للمستمعين. | تم الرد على `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | تطبيق حدود في وقت التنفيذ (افتراضات إعادة السداد، حدود التزامن، تبديل التحقق). | قم بالتخطيط إلى `FetchOptions` و`crates/sorafs_car::multi_fetch`. |
| معلمات CLI / SDK | الحد من عدد الأقران ومناطق القياس عن بعد الإضافية وتوضيح سياسات الرفض/التعزيز. | `sorafs_cli fetch` يعرض هذه الأعلام مباشرة؛ يتم تمرير SDK عبر `OrchestratorConfig`. |

يتم تسلسل مساعدي JSON en `crates/sorafs_orchestrator::bindings`
التكوين الكامل في Norito JSON، ما الذي يجب فعله بين الارتباطات المحمولة
SDK والأتمتة.

### 1.1 نموذج تكوين JSON

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

استمر في حفظ الملف وسط الإضافة المعتادة لـ `iroha_config` (`defaults/`,
المستخدم، الفعلي) لكي تحدد الإضافات حدودًا لنفسها
دخول العقد. يتم توفير ملف احتياطي مباشر فقط مع الطرح
SNNet-5a، راجع `docs/examples/sorafs_direct_mode_policy.json` وراجع
المكملات الغذائية في `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 الإخلاصSNNet-9 يتكامل مع الولاء الموجه من قبل الإدارة والمنسق. الأمم المتحدة
الكائن الجديد `compliance` والتكوين Norito JSON يلتقط المنحوتات
كيفية جلب خط الأنابيب بطريقة مباشرة فقط:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` أعلن عن رموز ISO-3166 alpha-2 بعد أن تم تشغيلها
  مثيل orquestador. يتم تطبيع الرموز في بعض الأحيان خلال فترة زمنية
  بارسيو.
- `jurisdiction_opt_outs` يعكس سجل الإدارة. عندما يكون هناك شيء آخر
  تظهر ولاية المشغل في القائمة، ويتم تطبيق المُنسق
  `transport_policy=direct-only` وإخراج السبب الاحتياطي
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` قائمة ملخصات البيان (CIDs cegados، codeificados en
  مايوسكولاس عرافة). تتزامن الشحنات أيضًا مع التخطيط
  المباشر فقط ويعرض الخيار الاحتياطي `compliance_blinded_cid_opt_out` على هذا النحو
  القياس عن بعد.
- `audit_contacts` يقوم بتسجيل URI الذي تتوقعه الإدارة من المشغلين
  Publiquen en sus playbooks GAR.
- `attestations` يلتقط حزم الولاء التي تستجيب لها
  سياسة. كل مدخل يحدد `jurisdiction` اختياري (codigo ISO-3166
  alpha-2)، و`document_uri`، و`digest_hex` كانون ذات 64 حرفًا، وel
  الطابع الزمني للإصدار `issued_at_ms` و `expires_at_ms` اختياري. إستوس
  تمد المصنوعات اليدوية قائمة مراجعة سمعية الأوركيستر لكي يتمكن
  يمكن لأدوات الإدارة أن تتغلب على الحذف بالوثائق
  Firmada.قم بتقسيم كتلة الامتثال وسط التطبيق المعتاد
التكوين بحيث يتلقى المشغلون إلغاءات محددة. ش
يتوافق تطبيق orquestador _despues_ مع تلميحات وضع الكتابة: بما في ذلك
تطلب مجموعة SDK `upload-pq-only`، الاستثناءات حسب الولاية القضائية أو البيان
siguen forzando Transporte direct-only and fast fast cuando غير موجود
proveedores aptos.

يتم تنشيط كتالوجات إلغاء الاشتراك الأساسية
`governance/compliance/soranet_opt_outs.json`; El Consejo de Gobernanza publica
تحديثات متوسطة لآداب النشرات. مثال كامل
التكوين (بما في ذلك الشهادات) متاح في
`docs/examples/sorafs_compliance_policy.json`، ويتم تنفيذ العملية
التقاط وإل
[دليل الولاء GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 ضبط CLI وSDK| العلم / كامبو | تأثير |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | عدد محدود من الموردين ينقذون مرشح لوحة النتائج. Ponlo en `None` لاستخدام جميع الموردين المؤهلين. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita los retentos por قطعة. فوق الأجناس المحدودة `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | احصل على لقطات من التأخير/السقوط في مُنشئ لوحة النتائج. علامة القياس عن بعد قديمة ولكنها `telemetry_grace_secs` تم تصنيفها على أنها غير مؤهلة. |
| `--scoreboard-out` | استمر في حساب لوحة النتائج (المزودون المؤهلون + غير المؤهلين) للفحص اللاحق. |
| `--scoreboard-now` | قم باستبدال الطابع الزمني للوحة النتائج (ثانيًا على نظام Unix) حتى يتم تحديد لقطات المباريات. |
| `--deny-provider` / خطاف النتيجة السياسية | استبعاد محدد لمقدمي التخطيط دون منع الإعلانات. استخدم لقوائم الرد السريع السوداء. |
| `--boost-provider=name:delta` | قم بتعديل قروض الجولة التأملية حتى يتمكن المورد من الحفاظ على أموال الحكومة سليمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | تم إنشاء مقاييس الإصدار والسجلات حتى تتمكن لوحات المعلومات من الترشيح حسب الموقع الجغرافي عند بدء التشغيل. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | بسبب الخلل هو `soranet-first` الآن، أصبح الأوركسترا متعدد الأصول هو القاعدة. الولايات المتحدة الأمريكية `direct-only` للتحضير للرجوع إلى إصدار سابق أو متابعة توجيه الولاء، وحجز `soranet-strict` للطيارين PQ فقط؛ Las anulaciones de الامتثال siguen siendo el techo duro. |

SoraNet-first الآن هي القيمة بسبب الخلل، وعمليات التراجع يجب أن تكون قوية
مراسلة حظر SNNet. عبر التخرج SNNet-4/5/5a/5b/6a/7/8/12/13,
تتحمل الحاكم الوضعية المطلوبة (hacia `soranet-strict`)؛ hasta
وبالتالي، فإن الإلغاءات التحفيزية للحوادث لها الأولوية فقط
`direct-only`، ويجب عليك التسجيل في سجل التشغيل.

جميع الأعلام الأمامية مقبولة بأسلوب سينتاكس `--` tanto en
`sorafs_cli fetch` في الاتجاه الثنائي `sorafs_fetch`
com.desarrolladores. يعرض SDK الخيارات المختلفة بين أنواع المنشئين.

### 1.4 إدارة ذاكرة التخزين المؤقت

يقوم CLI الآن بدمج محدد حراس SoraNet للمشغلين
يمكنك تشغيل مرحلات إدخال شكل محدد قبل اكتمال الطرح
SNNet-5 للنقل. ثلاثة أعلام جديدة تتحكم في التدفق:| علم | اقتراح |
|------|-----------|
| `--guard-directory <PATH>` | Apunta a un archive JSON الذي يصف توافق التتابعات الأحدث (إذا كان يُظهر اقترانًا فرعيًا آخر). قم بتمرير الدليل إلى تحديث ذاكرة التخزين المؤقت قبل تشغيل الجلب. |
| `--guard-cache <PATH>` | استمر في حفظ `GuardSet` المشفر في Norito. تعمل عمليات التشغيل اللاحقة على إعادة استخدام ذاكرة التخزين المؤقت المتضمنة عندما لا يتم عرض دليل جديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | يتجاوز الخيارات الاختيارية لعدد واقيات الدخول إلى النافذة (للعيب 3) ونافذة التثبيت (للعيب 30 يومًا). |
| `--guard-cache-key <HEX>` | مفتاح اختياري يبلغ 32 بايت يستخدم لحذف ذاكرات التخزين المؤقت باستخدام MAC Blake3 حتى يتمكن الأرشيف من التحقق قبل إعادة استخدامه. |

تستخدم حمولات دليل الحراسة نوعًا مضغوطًا:

العلم `--guard-directory` الآن على أمل الحمولة `GuardDirectorySnapshotV2`
مشفر في Norito. تحتوي اللقطة الثنائية على:- `version` - إصدار التحدي (`2` فعليًا).
- `directory_hash`، `published_at_unix`، `valid_after_unix`، `valid_until_unix` —
  البيانات الوصفية المتفق عليها يجب أن تتزامن مع كل شهادة مدمجة.
- `validation_phase` — كمبيوتر سياسي معتمد (`1` = تصريح
  شركة واحدة Ed25519، `2` = الشركات المزدوجة المفضلة، `3` = الشركات المطلوبة
  زوجي).
- `issuers` — باعثات الإدارة مع `fingerprint`, `ed25519_public` y
  `mldsa65_public`. يتم حساب بصمات الأصابع بنفس الطريقة
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — قائمة حزم SRCv2 (خرجت من
  `RelayCertificateBundleV2::to_cbor()`). تتضمن كل حزمة الواصف
  مرحل وأعلام القدرة والسياسة ML-KEM والشركات المزدوجة Ed25519/ML-DSA-65.

يتحقق CLI من كل حزمة ضد مفاتيح الباعث المعلن عنها مسبقًا
دمج الدليل مع ذاكرة التخزين المؤقت. هناك أصناف JSON موجودة
لا حد ذاته مقبول. إذا كانت هناك حاجة إلى لقطات SRCv2.استدعاء CLI مع `--guard-directory` لدمج التوافق الأحدث مع
ذاكرة التخزين المؤقت موجودة. يقوم المحدد بحفظ الحراس الموجودين بداخلهم
نافذة الاحتفاظ وهي مؤهلة في الدليل؛ المرحلات الجديدة
استبدال مداخل انتهاء الصلاحية. أثناء عملية الجلب، يتم تحديث ذاكرة التخزين المؤقت
اكتب الجديد في المسار المشار إليه لـ `--guard-cache`، مع الاحتفاظ بالجلسات
الحتمية اللاحقة. يمكن لـ SDK إعادة إنتاج نفس المواصفات تمامًا
لامار `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
وابحث عن `GuardSet` الناتج عن `SorafsGatewayFetchOptions`.

يسمح `ml_kem_public_hex` بحماية أولوية المحدد بسعة PQ
عند تدمير SNNet-5. مفاتيح التبديل (`anon-guard-pq`،
`anon-majority-pq`, `anon-strict-pq`) الآن يتم تحويل المرحلات تلقائيًا
الكلاسيكو: عندما يكون هناك حارس PQ متاح لإزالة المحدد من الصنوبر
الكلاسيكو سوبرانتيس لكي تكون الجلسات اللاحقة مفضلة للمصافحة
الهبريدوس. تعرض السيرة الذاتية لـ CLI/SDK المزيج الناتج عبر
`anonymity_status`/`anonymity_reason`، `anonymity_effective_policy`،
`anonymity_pq_selected`، `anonymity_classical_selected`، `anonymity_pq_ratio`،
`anonymity_classical_ratio` والمجالات التكميلية للمرشحين/العجز/
تباين العرض، وتكوين التخفيضات الواضحة والكلاسيكية الاحتياطية.يمكن الآن أن تتضمن أدلة الحراسة حزمة SRCv2 كاملة عبر
`certificate_base64`. يقوم الأوركيستادور بفك تشفير كل حزمة، ويعيد التحقق من صحة الشركات
Ed25519/ML-DSA والحفاظ على الشهادة التي تم تحليلها إلى جانب ذاكرة التخزين المؤقت.
عندما يتم تحويل الشهادة المقدمة إلى المصدر الكنسي
مفاتيح PQ وتفضيلات المصافحة والتأمل؛ انتهت صلاحية الشهادات
إذا تم حذفه وسيظهر المحدد في الحقول الموروثة من الواصف. لوس
تعمل الشهادات على الترويج لإدارة دورة حياة الدوائر ونفسها
الأس عبر `telemetry::sorafs.guard` و`telemetry::sorafs.circuit`، الذي قمت بالتسجيل فيه
نافذة الصلاحية، وأجنحة المصافحة، وإذا كنت تراقب الشركات المزدوجة
حارس بارا كادا.

استخدام مساعدي CLI للحفاظ على اللقطات المتزامنة معهم
الناشرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` قم بتنزيل والتحقق من لقطة SRCv2 قبل الكتابة على القرص،
بينما يقوم `verify` بتكرار مسار التحقق من صحة المصنوعات اليدوية التي تم الحصول عليها
من خلال معدات أخرى، قم بإصدار استئناف JSON يعكس خروج المحدد
حراس على CLI/SDK.

### 1.5 مؤشر دورة حياة الدائرةعندما يتم توفير دليل المرحلات مثل مخبأ الحراسة
يقوم الأوركستادور بتنشيط مدير دورة الحياة من أجل البناء المسبق و
تجديد دوائر SoraNet قبل كل شيء. التكوين حي أون
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) عبر دوس كامبوس
جديد:

- `relay_directory`: التقاط لقطة من دليل SNNet-3 للقفزات
  يمكن تحديد الشكل المحدد للوسط/الخروج.
- `circuit_manager`: التكوين الاختياري (التأهيل بسبب العيب)
  التحكم في دائرة TTL.

Norito JSON الآن يقبل كتلة `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

تقوم مجموعة SDK بإعادة تجميع بيانات الدليل عبر
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)، ويتصل سطر الأوامر تلقائيًا دائمًا
هذا هو ملخص `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

يقوم المدير بإعادة تشغيل الدوائر عند تغيير بيانات الحماية (نقطة النهاية،
اضغط على PQ أو الطابع الزمني المحدد) أو عندما يتم تشغيل TTL. المساعد `refresh_circuits`
استدعاء مسبق للجلب (`crates/sorafs_orchestrator/src/lib.rs:1346`)
قم بإصدار سجلات `CircuitEvent` حتى يتمكن المشغلون من اتخاذ قراراتهم
سيكلو دي فيدا. اختبار النقع
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) ديمويسترا لاتنسيا إيستابل أ
اجتياز ثلاث حواجز حماية؛ استشارة التقرير أون
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 وكيل QUIC محلييمكن للأوركيستادور بدء وكيل QUIC محلي اختياريًا لذلك
لا تحتاج ملحقات المتصفح ومحولات SDK إلى إدارتها
شهادات أو مفاتيح حماية ذاكرة التخزين المؤقت. يتم إرسال الوكيل إلى اتجاه واحد
الاسترجاع، إنهاء الاتصالات QUIC وإعادة بيان Norito الذي يصف El
شهادة ومفتاح حماية ذاكرة التخزين المؤقت اختياري للعميل. الأحداث دي
نقل المراسلات من خلال الوكيل حتى تتم متابعته عبر
`sorafs_orchestrator_transport_events_total`.

تمكّن الوكيل من خلال الكتلة الجديدة `local_proxy` في JSON del
أوركيستادور:

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
```- `bind_addr` كونترولا دوندي escucha el proxy (الولايات المتحدة الأمريكية ميناء `0` للاستدعاء
  الأمم المتحدة بويرتو efimero).
- يقوم `telemetry_label` بنشر المقاييس حتى تتمكن لوحات المعلومات من العمل
  تمييز وكلاء جلسات الجلب.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض نفس ذاكرة التخزين المؤقت
  أدوات الحماية الأساسية التي تستخدم CLI/SDK، وتحافظ على امتدادات الامتدادات
  ديل نافيغادور.
- `emit_browser_manifest` بديل إذا كانت المصافحة ستتحول إلى بيان كما هو الحال
  يمكن للملحقات أن تقوم بالتحديث والصلاحية.
- `proxy_mode` يتم تحديده إذا كان الوكيل يمكنه المرور محليًا (`bridge`) o
  قم فقط بإصدار البيانات التعريفية حتى تتمكن SDK من فتح دوائر SoraNet من خلال حسابها
  (`metadata-only`). الوكيل المعيب هو `bridge`; إستابليس `metadata-only`
  عندما تقوم محطة عمل بتوضيح التدفقات الواضحة.
- `prewarm_circuits`، `max_streams_per_circuit` و`circuit_ttl_hint_secs`
  قم بشرح تلميحات إضافية للمتصفح حتى تتمكن من افتراض التدفقات المسبقة
  متوازيان ويدركان أن الوكيل يعيد استخدام الدوائر بشكل صارخ.
- `car_bridge` (اختياري) يفتح ذاكرة التخزين المؤقت المحلية لملفات CAR. الكامبو
  `extension` التحكم في الصوفي المتوافق عند حذف هدف الدفق
  `*.car`; تحديد `allow_zst = true` لخادم الحمولات `*.car.zst`
  precomprimidos مباشرة.
- `kaigi_bridge` (اختياري) يعرض rutas Kaigi والتخزين المؤقت للوكيل. الكامبو`room_policy` الإعلان عن أوبرا الجسر بطريقة `public` أو `authenticated`
  لكي يتمكن عملاء المتصفح من تحديد علامات GAR مسبقًا
  تصحيح.
- `sorafs_cli fetch` تجاوزات العرض `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`، يسمح لك بتغيير الوضع
  قم بتشغيل أو تشغيل البكرات البديلة دون تعديل السياسة JSON.
- `downgrade_remediation` (اختياري) لتكوين خطاف الرجوع إلى إصدار سابق تلقائيًا.
  عندما يتم تدريب الأوركيستادور على مراقبة المرحلات عن بعد
  Detector rafagas de downgrade y، بعد تكوين `threshold` داخل
  `window_secs`، اضغط على الوكيل المحلي على `target_mode` (بسبب العيب
  `metadata-only`). بمجرد انتهاء التخفيضات، سيتحول الوكيل إلى
  `resume_mode` بعد `cooldown_secs`. الولايات المتحدة الأمريكية `modes` للحد
  يؤدي ذلك إلى تشغيل أدوار التتابع المحددة (من خلال مرحلات الإدخال المعيبة).

عند تشغيل الوكيل في وضع الجسر الذي يقدم خدمات التطبيق:- **`norito`** – يتم إرسال هدف تدفق العميل ذات الصلة
  `norito_bridge.spool_dir`. يتم تعقيم الأهداف (بدون اجتيازها أو اجتيازها).
  مطلق) و، عندما لا يكون للملف امتداد، يتم تطبيقه على الصوفي
  تم تكوينه قبل إرسال الحمولة إلى المتصفح حرفيًا.
- **`car`** – يتم حل أهداف البث في الداخل
  `car_bridge.cache_dir`، هناك امتداد بسبب خلل في التكوين
  يتم تجميع الحمولات النافعة بشكل أقل مما تم تنشيطه `allow_zst`. لوس
  الجسور الخارجة تستجيب مع `STREAM_ACK_OK` قبل نقل البايتات
  الأرشيف حتى يتمكن العملاء من التحقق من صحتها.

في العديد من الحالات، يقوم الوكيل بإدخال علامة التخزين المؤقت لـ HMAC (عند وجود مفتاح رئيسي
ذاكرة التخزين المؤقت للحماية أثناء المصافحة) وتسجيل رموز لسبب القياس عن بعد
`norito_*` / `car_*` لتمييز لوحات المعلومات عن المخرجات والأرشيفات
أخطاء وسقوط التعقيم في مشهد.

`Orchestrator::local_proxy().await` يشرح المقبض وينفذه
يمكن للمتصلين قراءة PEM للشهادة والحصول على البيان
المتصفح أو طلب الضغطة التي تم ترتيبها عند الانتهاء من التطبيق.

عندما يكون قادرًا على ذلك، الوكيل الآن لديه سجلات **البيان v2**. أديماس ديل
شهادة موجودة ومفتاح حماية ذاكرة التخزين المؤقت، الإصدار 2 المنضم:- `alpn` (`"sorafs-proxy/1"`) وتم توصيل `capabilities` للعملاء
  تأكيد بروتوكول البث الذي يجب استخدامه.
- `session_id` للمصافحة وكتلة `cache_tagging` لاستخراجها
  تحسينات الحماية للجلسة والعلامات HMAC.
- تلميحات الدائرة واختيار الحماية (`circuit`، `guard_selection`،
  `route_hints`) لكي تعمل تكاملات المتصفح على توسيع واجهة المستخدم
  تيارات ريكا قبل الافتتاح.
- `telemetry_v2` مع مقابض الموسيقى والخصوصية للأجهزة المحلية.
- كل `STREAM_ACK_OK` يتضمن `cache_tag_hex`. العملاء يستمتعون بالشجاعة
  الرأس `x-sorafs-cache-tag` لإصدار طلبات HTTP أو TCP لذلك
  اختيارات الحراسة والتخزين المؤقت للشفرات الدائمة في مكانها.

هذه الحقول تحافظ على التنسيق السابق؛ يمكن أن يتجاهل العملاء القديمون
المفاتيح الجديدة والاستمرار في استخدام الرابط الفرعي v1.

## 2. دلالات السقوط

يطبق الأوركسترا اختبارات مقيدة للقدرة والافتراضات المسبقة
سيتم نقل بايت واحد فقط. يمكن أن تكون السقوط ضمن ثلاث فئات:1. **Fallos de elegibilidad (ما قبل الرحلة).** مقدمو الخدمة بدون قدرة على المدى،
   الإعلانات التي انتهت صلاحيتها أو القياس عن بعد عفا عليها الزمن عندما يتم تسجيلها في قطعة أثرية
   لوحة النتائج ويتم حذفها من التخطيط. السيرة الذاتية لـ CLI متجددة
   الشبكة `ineligible_providers` لها أسباب تمكن المشغلين من العمل
   فحص الانجراف من خلال سجلات راسبار.
2. **التمتع بوقت القذف.** كل مورد يسجل السقوط
   متتالية. مرة واحدة يتم ضبط `provider_failure_threshold`
   تم تكوينه، ويتم تحديد المورد على أنه `disabled` من خلال بقية الجلسة.
   إذا انتقل جميع الموردين إلى `disabled`، فإن الأوركيستادور يتطور
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **إلغاء التحديدات.** تظهر الحدود المقيدة كأخطاء
   البنية التحتية:
   - `MultiSourceError::NoCompatibleProviders` — البيان الذي يتطلب نقلًا
     قطع أو قطع لا يمكن للموردين إكمالها.
   - `MultiSourceError::ExhaustedRetries` — يتم استهلاك هذا الافتراض
     retentos por قطعة.
   - `MultiSourceError::ObserverFailed` - المراقبون في اتجاه مجرى النهر (الخطافات de
     الدفق) rechazaron un Chunk Verificado.

يتضمن كل خطأ مؤشر القطعة الإشكالية، وعندما يكون هذا متاحًا،
La razon Final de Fallo del Proveedor. اجعل هذه السقطات بمثابة حواجز
الإصدار: يتم إعادة إنتاج Los Rententos مع نفس الشيء الذي تم إدخاله، حتى يتسنى لك ذلك
قم بتغيير الإعلان أو القياس عن بعد أو صحة المورد الفرعي.### 2.1 ثبات لوحة النتائج

عندما يتم تكوين `persist_path`، يقوم المنسق بتسجيل لوحة النتائج النهائية
من خلال القذف كليًا. يحتوي المستند JSON على:

- `eligibility` (`eligible` أو `ineligible::<reason>`).
- `weight` (العملة الطبيعية المخصصة لهذا التنفيذ).
- بيانات تعريف `provider` (المعرف ونقاط النهاية والافتراضي
  التزامن).

يتم أرشفة لقطات لوحة النتائج جنبًا إلى جنب مع العناصر التي يتم إصدارها لذلك
ستظل قرارات القائمة السوداء والطرح قابلة للتدقيق.

## 3. القياس عن بعد والتطهير

### 3.1 متريكاس Prometheus

يقوم الأوركيستادور بإصدار المقاييس التالية عبر `iroha_telemetry`:| متريكا | التسميات | الوصف |
|---------|-------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`، `region` | مقياس جلب الأغراض بالطائرة. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`، `region` | رسم بياني يسجل زمن الجلب من أقصى إلى أقصى. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`، `region`، `reason` | Contador de Fallos Terminales (reintentosagotados، sin profeedores، Fallo del observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`، `provider`، `reason` | موازنة نية الاحتفاظ من قبل المورّد. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`، `provider`، `reason` | مكافح السقوط على مستوى الجلسة الذي يخفف من تأهيل الموردين. |
| `sorafs_orchestrator_policy_events_total` | `region`، `stage`، `outcome`، `reason` | تم تجميع القرارات السياسية المجهولة (الكاملة مقابل التخفيضات) من خلال البدء في الطرح والحل الاحتياطي. |
| `sorafs_orchestrator_pq_ratio` | `region`، `stage` | رسم بياني لقطع المرحلات PQ داخل مجموعة SoraNet المحددة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`، `stage` | الرسم البياني لنسب عرض التبديلات PQ في لقطة لوحة النتائج. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`، `stage` | رسم بياني للعجز السياسي (يتقاطع بين الهدف والجزء الحقيقي من PQ). || `sorafs_orchestrator_classical_ratio` | `region`، `stage` | رسم بياني لقطع التتابعات الكلاسيكية المستخدمة في كل جلسة. |
| `sorafs_orchestrator_classical_selected` | `region`، `stage` | رسم بياني لبيانات التتابع الكلاسيكية المختارة للجلسة. |

قم بدمج المقاييس في لوحات المعلومات المرحلية قبل تنشيط المقابض
إنتاج. يشير التخطيط الموصى به إلى خطة المراقبة SF-6:

1. **جلب الأنشطة** — تنبيه إذا كان المقياس مكافئًا للإكمالات الفرعية.
2. **نسبة إعادة التدوير** — تنبيه عندما تكون المقابض `retry` فائقة
   خطوط الأساس التاريخية.
3. **فشل المورد** — تنبيهات مختلفة عند النداء عند أي مزود آخر
   supera `session_failure > 0` في 15 دقيقة.

### 3.2 أهداف سجلات الإنشاء

ينشر الأوركيستادور الأحداث الهيكلية والأهداف المحددة:

- `telemetry::sorafs.fetch.lifecycle` — علامة حلقة الحياة `start` y
  `complete` مع محتوى القطع والتجديد والمتانة الإجمالية.
- `telemetry::sorafs.fetch.retry` — أحداث إعادة التشغيل (`provider`, `reason`,
  `attempts`) لتغذية دليل الفرز.
- `telemetry::sorafs.fetch.provider_failure` - الموردين المجهزين
  أخطاء متكررة.
- `telemetry::sorafs.fetch.error` - المحطات الطرفية المتبقية مع `reason` y
  البيانات الوصفية الاختيارية للمورد.أرسل هذه التدفقات إلى خط أنابيب السجلات Norito الموجود حتى يتم الرد
الأحداث لها مصدر واحد حقيقي. أحداث دورة الحياة
قم بتوضيح المزيج PQ/classica عبر `anonymity_effective_policy`،
`anonymity_pq_ratio`، `anonymity_classical_ratio` والشركاء المرتبطون بهم،
ما الذي يسهل استخدام لوحات المعلومات الكبلية بدون مقاييس دقيقة. عمليات طرح Durante لـ GA،
مستوى السجلات في `info` لأحداث دورة الحياة/الاستئناف والولايات المتحدة الأمريكية
`warn` لمحطات السقوط.

### 3.3 السيرة الذاتية JSON

Tanto `sorafs_cli fetch` كـ SDK de Rust يقوم ببناء السيرة الذاتية
ماذا تحتوي على:

- `provider_reports` مع مستندات الخروج/الكسر وإذا كان لديك مستورد
  com.dehabilitado.
- `chunk_receipts` يوضح كيفية حل كل قطعة.
- تم تسجيله `retry_stats` و`ineligible_providers`.

أرشفة ملف السيرة الذاتية للتخلص من المشكلات: الإيصالات
يتم عرضه مباشرة على بيانات التعريف للسجلات السابقة.

## 4. قائمة المراجعة التشغيلية1. **إعداد التكوين في CI.** قم بتشغيل `sorafs_fetch` مع
   تكوين الهدف، انتقل إلى `--scoreboard-out` لالتقاط المشهد
   الأهلية والمقارنة مع الإصدار السابق. مُثبت Cualquier أو غير مؤهل
   لا تنتهي من الترويج.
2. **التحقق من القياس عن بعد.** تأكد من متابعة مقاييس التصدير
   `sorafs.fetch.*` وسجلات الإنشاءات السابقة للتأهيل تجلب مصادر متعددة
   للمستخدمين. تشير قاعدة القياسات الصحيحة إلى الواجهة
   orquestador no fue invocado.
3. **تجاوزات التوثيق.** تطبيق ضبط الطوارئ `--deny-provider`
   o `--boost-provider`، قم بتأكيد JSON (استدعاء CLI) في سجل التغيير الخاص بك.
   يجب أن تؤدي عمليات التراجع إلى إرجاع التجاوز والتقاط لقطة جديدة
   لوحة النتائج.
4. **تكرار اختبارات الدخان.** من أجل تعديل متطلبات إعادة الاستخدام أو الحدود
   من قبل الموردين، قم بالتمرير لجلب التثبيت الكنسي
   (`fixtures/sorafs_manifest/ci_sample/`) والتحقق من الإيصالات
   قطع sigan siendo deterministas.

اتبع الخطوات السابقة للحفاظ على صحة المنسق
قابلة للتكرار والطرح من خلال المراحل وتوفير القياس عن بعد اللازم لذلك
لا الرد على الحوادث.

### 4.1 الإلغاءات السياسيةيمكن للمشغلين رؤية عملية النقل النشطة/المجهولة بدون تحرير
تم تعيين قاعدة التكوين `policy_override.transport_policy` y
`policy_override.anonymity_policy` في JSON من `orchestrator` (الموجز
`--transport-policy-override=` / `--anonymity-policy-override=` أ
`sorafs_cli fetch`). عندما يكون هناك أي تجاوزات موجودة
يقوم المشغل بحذف البديل الاحتياطي المعتاد: إذا لم يتم طلب المستوى PQ
يمكن أن تكون مرضيًا، والجلب يفشل مع `no providers` في مكان degradar en
صمت. إن تغيير المعامل الخاطئ هو أمر بسيط للغاية لتطهيره
مجال التجاوز.