---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : configuration de l'orchestrateur
titre : اعداد مُنسِّق SoraFS
sidebar_label : اعداد المُنسِّق
description: اضبط مُنسِّق الجلب متعدد المصادر، وفسّر الاخفاقات، وتتبع مخرجات التليمترية.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/developer/orchestrator.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

# دليل مُنسِّق الجلب متعدد المصادر

يقود مُنسِّق الجلب متعدد المصادر في SoraFS تنزيلات حتمية ومتوازية من مجموعة
المزوّدين المنشورة في annonces المدعومة بالحوكمة. يشرح هذا الدليل كيفية ضبط
المُنسِّق، وما هي إشارات الفشل المتوقعة أثناء عمليات الإطلاق، وأي تدفقات
التليمترية تُظهر مؤشرات الصحة.

## 1. نظرة عامة على الاعداد

يدمج المُنسِّق ثلاثة مصادر للاعداد:

| المصدر | الغرض | الملاحظات |
|--------|-------|---------------|
| `OrchestratorConfig.scoreboard` | Vous pouvez également utiliser JSON pour créer des liens. | مدعوم عبر `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | يطبّق حدود وقت التشغيل (ميزانيات إعادة المحاولة، حدود التوازي، مفاتيح التحقق). | Il s'agit de `FetchOptions` et `crates/sorafs_car::multi_fetch`. |
| Logiciels CLI / SDK | Vous pouvez utiliser la fonction deny/boost. | `sorafs_cli fetch` يعرّض هذه الأعلام مباشرةً؛ Il s'agit de `OrchestratorConfig` pour les SDK. |

Utiliser JSON pour `crates/sorafs_orchestrator::bindings` pour la version ultérieure
Le Norito JSON est également disponible pour le SDK.

### 1.1 Utiliser JSON

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

احفظ الملف عبر طبقات `iroha_config` المعتادة (`defaults/`, utilisateur, actuel) حتى
ترث عمليات النشر الحتمية الحدود نفسها على جميع العقد. لملف repli مباشر فقط
يتماشى مع rollout الخاص بـ SNNet-5a, راجع
`docs/examples/sorafs_direct_mode_policy.json` والإرشاد المكمل في
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Mises à jour

Utilisez SNNet-9 pour installer la fonction SNNet-9. كائن `compliance` جديد
Pour le Norito JSON, vous pouvez utiliser le mode direct uniquement :

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

- `operator_jurisdictions` pour la norme ISO‑3166 alpha‑2
  النسخة من المُنسِّق. تُحوّل الرموز إلى أحرف كبيرة أثناء التحليل.
- `jurisdiction_opt_outs` يعكس سجل الحوكمة. عندما تظهر أي ولاية تشغيلية ضمن
  القائمة، يفرض المُنسِّق `transport_policy=direct-only` ويُصدر سبب التراجع
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` digère les informations (CID aveugles en hexadécimal
  أحرف كبيرة). التطابقات تفرض أيضاً الجدولة direct-only et
  `compliance_blinded_cid_opt_out` pour la recherche.
- `audit_contacts` يسجل عناوين URI التي تتوقع الحوكمة أن ينشرها المشغلون في
  playbooks الخاصة بـ GAR.
- `attestations` يلتقط حزم الامتثال الموقّعة الداعمة للسياسة. كل إدخال يعرّف
  `jurisdiction` اختيارياً (ISO-3166 alpha-2) et `document_uri` et `digest_hex`
  القانوني بطول 64 حرفاً، وطابع الإصدار `issued_at_ms`, و`expires_at_ms`
  اختيارياً. تتدفق هذه الآثار إلى قائمة تدقيق المُنسِّق كي تربط أدوات الحوكمة
  بين التجاوزات والوثائق الموقعة.

مرّر كتلة الامتثال عبر طبقات الاعداد المعتادة كي يحصل المشغلون على تجاوزات
حتمية. يطبق المُنسِّق الامتثال _بعد_ تلميحات write-mode : حتى لو طلب SDK
`upload-pq-only`, est une option de connexion directe uniquement
وتفشل سريعاً عندما لا يوجد مزوّدون متوافقون.توجد كتالوجات opt-out المعتمدة في
`governance/compliance/soranet_opt_outs.json`؛ وينشر مجلس الحوكمة التحديثات عبر
إصدارات موسومة. يتوفر مثال كامل للاعداد (بما في ذلك attestations) في
`docs/examples/sorafs_compliance_policy.json`, c'est la raison pour laquelle
[playbook امتثال GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Version CLI et SDK

| العلم / الحقل | الاثر |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | يحد عدد المزوّدين الذين يمرون من فلتر لوحة النتائج. اضبطه على `None` لاستخدام كل المزوّدين المؤهلين. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | يحد إعادة المحاولة لكل شريحة. تجاوز الحد يرفع `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Il s'agit d'une latence/échec ou d'un problème de latence/échec. La description de l'article `telemetry_grace_secs` est conforme à la description. |
| `--scoreboard-out` | يحفظ لوحة النتائج المحسوبة (مزوّدون مؤهلون + غير مؤهلين) للفحص بعد التشغيل. |
| `--scoreboard-now` | Il s'agit d'un système d'exploitation (Unix) et de luminaires. |
| `--deny-provider` / crochet score | يستبعد المزوّدين بشكل حتمي دون حذف annonces. مفيد للمنع السريع. |
| `--boost-provider=name:delta` | يضبط أرصدة round-robin الموزونة لمزوّد مع الإبقاء على أوزان الحوكمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | يوسم المقاييس والسجلات المهيكلة لتمكين لوحات المتابعة من التجميع حسب الجغرافيا أو موجة الإطلاق. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | الافتراضي `soranet-first` الآن بعد أن أصبح المُنسِّق متعدد المصادر أساسياً. Utilisez `direct-only` pour la connexion avec PQ uniquement et `soranet-strict`. تظل تجاوزات الامتثال سقفاً صارماً. |

SoraNet-first est la solution idéale pour SNNet
المعني. Pour SNNet-4/5/5a/5b/6a/7/8/12/13, utilisez la fonction SNNet-4/5/5a/5b/6a/7/8/12/13.
(نحو `soranet-strict`) ، وحتى ذلك الحين يجب أن تقتصر تجاوزات `direct-only` على
الحالات المدفوعة بالحوادث مع تسجيلها في سجل الإطلاق.

كل الأعلام أعلاه تقبل صيغة `--` في كل من `sorafs_cli fetch` والثنائي
`sorafs_fetch`. Les SDK sont également disponibles pour les constructeurs.

### 1.4 Mise en cache du cache

Utilisez CLI pour connecter SoraNet à vos relais
Il s'agit du déploiement de SNNet-5. ثلاثة أعلام جديدة تتحكم
Parسير:

| العلم | الغرض |
|------|-------|
| `--guard-directory <PATH>` | JSON est également utilisé pour les relais (ou JSON). تمرير الدليل يحدّث cache الحراس قبل الجلب. |
| `--guard-cache <PATH>` | يحفظ `GuardSet` المشفر بنوريتو. عمليات التشغيل التالية تعيد استخدام cache حتى بدون دليل جديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | تجاوزات اختيارية لعدد حراس الدخول المثبتين (الافتراضي 3) وفترة الاحتفاظ (الافتراضي 30 يوماً). |
| `--guard-cache-key <HEX>` | Si vous avez 32 ans pour MAC Blake3, vous devez utiliser le cache pour obtenir plus de détails. |

تستخدم حمولة دليل الحراس مخططاً مدمجاً:

يتوقع علم `--guard-directory` الآن حمولة `GuardDirectorySnapshotV2` مشفرة بنوريتو.
تحتوي اللقطة الثنائية على:- `version` — Numéro de téléphone (`2`).
-`directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  بيانات توافق يجب أن تطابق كل شهادة مضمنة.
- `validation_phase` — بوابة سياسة الشهادات (`1` = السماح بتوقيع Ed25519 واحد،
  `2` = تفضيل توقيعات مزدوجة، `3` = اشتراط التوقيعات المزدوجة).
- `issuers` — Fonctions de référence telles que `fingerprint`, `ed25519_public`,
  `mldsa65_public`. تُحسب البصمات كالتالي:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — Compatible avec SRCv2 (voir `RelayCertificateBundleV2::to_cbor()`). تحمل
  Pour les relais et les relais ML-KEM et Ed25519/ML-DSA-65
  المزدوجة.

تتحقق CLI من كل حزمة مقابل مفاتيح المصدر المعلنة قبل دمج الدليل مع cache
الحراس. لم تعد الرسومات JSON القديمة مقبولة؛ مطلوب لقطات SRCv2.

Utilisez la CLI pour `--guard-directory` pour créer un lien vers le cache.
يحافظ المحدد على الحراس المثبتين ضمن نافذة الاحتفاظ والمؤهلين في الدليل؛
وتستبدل relais الجديدة الإدخالات المنتهية. بعد نجاح الجلب تُكتب cache
Il s'agit d'un `--guard-cache` qui correspond à la situation actuelle.
Utilisez le SDK pour installer le SDK
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` et
`GuardSet` est compatible avec `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` Déploiement PQ pour le déploiement
SNNet-5. تعمل مفاتيح المراحل (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) الآن على تخفيض relays الكلاسيكية تلقائياً: عندما يتوفر حارس
PQ يقوم المحدد بإسقاط الدبابيس الكلاسيكية الزائدة لتفضيل المصافحات الهجينة.
Utilisez la CLI/SDK pour `anonymity_status`/`anonymity_reason`,
`anonymity_effective_policy`, `anonymity_pq_selected`, `anonymity_classical_selected`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` et `anonymity_classical_ratio`
Il y a aussi des baisses de tension et des replis.

Vous pouvez utiliser SRCv2 pour `certificate_base64`. يفك
المُنسِّق كل حزمة ويعيد التحقق من تواقيع Ed25519/ML-DSA ويحتفظ بالشهادة
المحللة بجانب cache الحراس. عند وجود شهادة تصبح المصدر المعتمد لمفاتيح PQ
وتفضيلات المصافحة والترجيح؛ وتُهمل الشهادات المنتهية ويعود المحدد إلى حقول
الوصف القديمة. تنتشر الشهادات عبر إدارة دورة حياة الدوائر وتُعرض عبر
`telemetry::sorafs.guard` و`telemetry::sorafs.circuit` Éléments de base
وحزم المصافحة وما إذا لوحظت توقيعات مزدوجة لكل حارس.

استخدم مساعدات CLI للحفاظ على تزامن اللقطات مع الناشرين :

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

ينزّل `fetch` اللقطة SRCv2 ويتحقق منها قبل الكتابة على القرص، بينما يعيد
`verify` Utiliser la fonction JSON pour utiliser JSON
Utilisez la CLI/SDK.

### 1.5 مدير دورة حياة الدوائر

عندما يتوفر دليل relais وcache الحراس معاً، يقوم المُنسِّق بتفعيل مدير دورة
L'application SoraNet est également compatible avec SoraNet. يقع الاعداد ضمن
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) pour votre recherche :

- `relay_directory` : prend en charge SNNet-3 pour les sauts intermédiaires/de sortie
  حتمي.
- `circuit_manager` : اعداد اختياري (ممكّن افتراضياً) يتحكم في TTL للدوائر.

Utiliser Norito JSON pour `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Utiliser les SDK pour créer des liens
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) et la CLI est en cours de réalisation
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).يجدد المدير الدوائر كلما تغيرت بيانات الحارس (endpoint أو مفتاح PQ أو الطابع
(زمني للتثبيت) et TTL. يقوم المساعد `refresh_circuits` المستدعى
قبل كل جلب (`crates/sorafs_orchestrator/src/lib.rs:1346`) بإصدار سجلات
`CircuitEvent` لتمكين المشغلين من تتبع قرارات دورة الحياة. اختبار tremper
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) يثبت استقرار الكمون عبر ثلاث
دورات تبديل للحراس؛ راجع التقرير في
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 par QUIC محلي

يمكن للمُنسِّق اختيارياً تشغيل وكيل QUIC محلي بحيث لا تضطر إضافات المتصفح
Le SDK fournit également des fonctionnalités de cache et de cache. يرتبط الوكيل بعنوان
loopback, ainsi que le manifeste QUIC et le manifeste Norito, sont également disponibles
cache الاختياري إلى العميل. تُعد أحداث النقل التي يصدرها الوكيل ضمن
`sorafs_orchestrator_transport_events_total`.

L'application `local_proxy` est basée sur la version JSON :

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
```

- `bind_addr` يتحكم بمكان استماع الوكيل (استخدم المنفذ `0` لطلب منفذ مؤقت).
- `telemetry_label` يمرر إلى المقاييس لتمييز الوكلاء عن جلسات الجلب.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض cache الحراس بالمفتاح ذاته
  Vous pouvez également utiliser les CLI/SDK pour créer des liens.
- `emit_browser_manifest` يبدّل ما إذا كان المصافحة تعيد manifeste يمكن
  للامتدادات حفظه والتحقق منه.
- `proxy_mode` يحدد ما إذا كان الوكيل يجسر الحركة محلياً (`bridge`) ou يكتفي
  Utilisez les kits de développement logiciel (`metadata-only`) pour les SDK de SoraNet.
  الافتراضي `bridge`; استخدم `metadata-only` عندما تريد إظهار manifeste فقط.
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs`
  تلميحات إضافية للمتصفح لتقدير التوازي وفهم إعادة استخدام الدوائر.
- `car_bridge` (اختياري) يشير إلى cache محلي لأرشيفات CAR. يتحكم حقل `extension`
  في اللاحقة المضافة عند غياب `*.car`؛ واضبط `allow_zst = true` pour
  `*.car.zst` المضغوطة مباشرة.
- `kaigi_bridge` (اختياري) يعرّض مسارات Kaigi للوكيل. يعلن `room_policy` pour
  Utilisez le modèle `public` et `authenticated` pour GAR
  الصحيحة مسبقاً.
- `sorafs_cli fetch` et `--local-proxy-mode=bridge|metadata-only` et
  `--local-proxy-norito-spool=PATH` Bobines de bobines
  Vous utilisez JSON.
- `downgrade_remediation` (اختياري) يضبط خطاف خفض المستوى التلقائي. عند التفعيل
  يراقب المُنسِّق تليمترية relay لرصد اندفاعات downgrades, وبعد تجاوز
  `threshold` pour `window_secs` pour `target_mode`
  (افتراضي `metadata-only`). وبعد توقف downgrades يعود الوكيل إلى `resume_mode`
  Par `cooldown_secs`. Module `modes` pour relais de relais
  (افتراضياً relais الدخول).

عندما يعمل الوكيل بوضع bridge يخدم خدمتين للتطبيق:

- **`norito`** — يتم حل هدف البث الخاص بالعميل نسبةً إلى
  `norito_bridge.spool_dir`. تتم تنقية الأهداف (لا traversal ولا مسارات مطلقة)،
  وعندما لا يحتوي الملف على امتداد يتم تطبيق اللاحقة المحددة قبل بث الحمولة.
- **`car`** — تُحل أهداف البث داخل `car_bridge.cache_dir`, وترث الامتداد
  La description de l'article est la suivante: `allow_zst`. الجسور
  La recherche pour `STREAM_ACK_OK` s'effectue dans le cadre d'un projet de recherche
  التحقق بالأنابيب.Dans le cas de HMAC, vous devez utiliser la balise de cache (vous devez également utiliser le cache pour
المصافحة) ويسجل رموز سبب التليمترية `norito_*` / `car_*` حتى تميز لوحات
المتابعة بين النجاح، وفقدان الملفات، وفشل التنقية بسرعة.

`Orchestrator::local_proxy().await` يعرّض المقبض التشغيلي حتى يتمكن المستدعون
من قراءة شهادة PEM, أو جلب manifeste للمتصفح، أو طلب إيقاف لطيف عند خروج
التطبيق.

عند التفعيل، يقدم الوكيل الآن سجلات **manifest v2**. إضافة إلى الشهادة ومفتاح
cache الحراس، تضيف v2 ما يلي:

- `alpn` (`"sorafs-proxy/1"`) et `capabilities` pour le démarrage.
- `session_id` pour l'affinité et l'affinité `cache_tagging` pour l'affinité
  HMAC est également disponible.
- تلميحات الدوائر واختيار الحراس (`circuit`, `guard_selection`, `route_hints`)
  واجهة أغنى قبل فتح البث.
- `telemetry_v2` مع مفاتيح أخذ العينات والخصوصية للأدوات المحلية.
- Pour `STREAM_ACK_OK` et `cache_tag_hex`. يعكس العميل القيمة في ترويسة
  `x-sorafs-cache-tag` est compatible avec HTTP et TCP pour les connexions Internet
  مشفرة عند التخزين.

هذه الحقول متوافقة للخلف — يمكن للعملاء الأقدم تجاهل المفاتيح الجديدة
والاعتماد على مجموعة v1.

## 2. دلالات الفشل

يفرض المُنسِّق تحققاً صارماً من القدرات والميزانيات قبل نقل أي بايت. تقع
الإخفاقات في ثلاث فئات:

1. **إخفاقات الأهلية (قبل التنفيذ).** المزوّدون الذين يفتقرون لقدرة النطاق، أو
   annonces منتهية، أو تليمترية قديمة يتم تسجيلهم في لوحة النتائج ويُستبعدون من
   الجدولة. Utilisez la CLI pour `ineligible_providers` pour votre ordinateur
   المشغلون من فحص انحراف الحوكمة دون كشط السجلات.
2. **استنزاف أثناء التشغيل.** يتتبع كل مزوّد الإخفاقات المتتالية. عند بلوغ
   `provider_failure_threshold` يتم وسم المزوّد `disabled` لبقية الجلسة. إذا
   انتقل جميع المزوّدين إلى `disabled` يعيد المُنسِّق
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **إجهاضات حتمية.** تظهر الحدود الصارمة كأخطاء مهيكلة:
   - `MultiSourceError::NoCompatibleProviders` — يتطلب المانيفست مدى شرائح أو
     محاذاة لا يمكن للمزوّدين الباقين تلبيتها.
   - `MultiSourceError::ExhaustedRetries` — تم استهلاك ميزانية إعادة المحاولة لكل
     شريحة.
   - `MultiSourceError::ObserverFailed` — رفض المراقبون aval (خطاطيف البث)
     شريحة تم التحقق منها.

تتضمن كل أخطاء فهرس الشريحة المخالفة وعند توفره سبب فشل المزوّد النهائي. تعامل
مع هذه الأخطاء كمعوّقات إصدار — إعادة المحاولة بذات المدخلات ستعيد الفشل حتى
يتغير advert أو التليمترية أو صحة المزوّد الأساسي.

### 2.1 حفظ لوحة النتائج

عند ضبط `persist_path` يكتب المُنسِّق لوحة النتائج النهائية بعد كل تشغيل. يحتوي
Utiliser JSON pour :

- `eligibility` (`eligible` et `ineligible::<reason>`).
- `weight` (الوزن المطبع المعين لهذا التشغيل).
- Paramètres `provider` (points de terminaison pour les points de terminaison).

أرشِف لقطات لوحة النتائج مع آثار الإطلاق حتى تبقى قرارات الحظر والrollout
قابلة للتدقيق.

## 3. Questions et réponses

### 3.1 par Prometheus

يصدر المُنسِّق المقاييس التالية عبر `iroha_telemetry` :| المقياس | Étiquettes | الوصف |
|---------|--------|-------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | مقياس عدّاد للجلب الجاري. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | هيستوغرام يسجل زمن الجلب من طرف إلى طرف. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | عدّاد للإخفاقات النهائية (استنزاف إعادة المحاولة، عدم وجود مزوّدين، فشل المراقب). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | عدّاد لمحاولات إعادة المحاولة لكل مزوّد. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | عدّاد لإخفاقات المزوّد على مستوى الجلسة المؤدية للتعطيل. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | عدد قرارات سياسة الإخفاء (تحقق/تدهور) بحسب مرحلة rollout وسبب التراجع. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Les relais PQ sont également compatibles avec SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Le joueur relaie PQ sur le tableau de bord. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | هيستوغرام لعجز السياسة (الفجوة بين الهدف والحصة الفعلية). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | هيستوغرام لحصة relais الكلاسيكية في كل جلسة. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | هيستوغرام لعدد relays الكلاسيكية المختارة في كل جلسة. |

Il s'agit d'une mise en scène et d'une mise en scène. التخطيط الموصى به
Concernant l'observabilité pour SF-6 :

1. **الجلب النشط** — تنبيه إذا ارتفع القياس دون اكتمالات مقابلة.
2. **نسبة إعادة المحاولة** — تحذير عند تجاوز عدادات `retry` للخطوط الاساسية.
3. **إخفاقات المزوّد** — تشغيل pager عند تجاوز أي مزوّد `session_failure > 0`
   خلال 15 دقيقة.

### 3.2 اهداف السجل المهيكل

ينشر المُنسِّق أحداثاً مهيكلة إلى اهداف حتمية:

- `telemetry::sorafs.fetch.lifecycle` — `start` et `complete` pour votre appareil
  والمحاولات والمدة الاجمالية.
- `telemetry::sorafs.fetch.retry` — احداث إعادة المحاولة (`provider`, `reason`,
  `attempts`) pour le triage يدوي.
- `telemetry::sorafs.fetch.provider_failure` — مزوّدون تم تعطيلهم بسبب تكرار
  الاخطاء.
- `telemetry::sorafs.fetch.error` — Prise en charge par `reason` et mise à jour
  اختيارية.

وجّه هذه التدفقات إلى مسار السجلات Norito الحالي لكي تمتلك الاستجابة للحوادث
مصدر حقيقة واحد. تُظهر أحداث دورة الحياة مزيج PQ/الكلاسيكي عبر
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
ومعاداتها المرافقة، ما يجعل ربط لوحات المتابعة سهلاً دون كشط المقاييس. أثناء
rollouts الخاصة بـ GA، ثبّت مستوى السجلات على `info` لأحداث دورة الحياة/إعادة
L'article `warn` est utilisé pour l'installation.

### 3.3 Utilisation de JSON

Il s'agit de `sorafs_cli fetch` et du SDK Rust qui sont compatibles :

- `provider_reports` مع اعداد النجاح/الفشل وما إذا تم تعطيل المزوّد.
- `chunk_receipts` توضح المزوّد الذي لبّى كل شريحة.
- Modèles `retry_stats` et `ineligible_providers`.

أرشِف ملف الملخص عند تتبع مزوّدين سيئين — تتطابق الإيصالات مباشرة مع بيانات
السجل أعلاه.

## 4. قائمة تشغيل تشغيلية1. **حضّر الاعداد في CI.** شغّل `sorafs_fetch` byالاعداد المستهدف، ومرر
   `--scoreboard-out` لالتقاط عرض الأهلية، وقارن مع الإصدار السابق. أي مزوّد
   غير مؤهل بشكل غير متوقع يوقف الترقية.
2. **تحقق من التليمترية.** تأكد من أن النشر يصدر مقاييس `sorafs.fetch.*` وسجلات
   مهيكلة قبل تمكين الجلب متعدد المصادر للمستخدمين. غياب المقاييس يشير عادةً
   إلى أن واجهة المُنسِّق لم تُستدعَ.
3. **وثّق التجاوزات.** عند تطبيق إعدادات طارئة `--deny-provider` أو
   `--boost-provider`, JSON (et CLI) pour les applications. يجب أن تعكس
   عمليات الرجوع إزالة التجاوز والتقاط لقطة scoreboard جديدة.
4. **أعد تشغيل اختبارات smoke.** بعد تعديل ميزانيات إعادة المحاولة أو حدود
   المزوّدين، أعد جلب luminaire القياسي (`fixtures/sorafs_manifest/ci_sample/`) وتأكد
   أن إيصالات الشرائح ما زالت حتمية.

اتباع الخطوات أعلاه يحافظ على قابلية إعادة الإنتاج عبر déploiements المرحلية ويوفر
التليمترية اللازمة للاستجابة للحوادث.

### 4.1 Mises en garde

يمكن للمشغلين تثبيت مرحلة النقل/الاخفاء النشطة دون تعديل الاعداد الاساسي عبر
Pour `policy_override.transport_policy` et `policy_override.anonymity_policy` pour
JSON est utilisé pour `orchestrator` (et est `--transport-policy-override=` /
`--anonymity-policy-override=` ou `sorafs_cli fetch`). عند وجود أي override،
يتجاوز المُنسِّق fallback brownout المعتاد: إذا تعذر تحقيق طبقة PQ المطلوبة،
يفشل الجلب مع `no providers` بدلاً من التراجع الصامت. الرجوع للسلوك الافتراضي
Il s'agit d'une substitution.