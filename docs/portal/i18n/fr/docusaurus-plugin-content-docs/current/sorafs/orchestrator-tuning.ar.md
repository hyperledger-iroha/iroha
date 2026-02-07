---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : réglage de l'orchestrateur
titre : إطلاق وضبط المُنسِّق
sidebar_label : ضبط المُنسِّق
description: قيم افتراضية عملية وإرشادات ضبط ونقاط تدقيق لمواءمة المُنسِّق متعدد المصادر مع GA.
---

:::note المصدر المعتمد
C'est `docs/source/sorafs/developer/orchestrator_tuning.md`. حافظ على تطابق النسختين إلى أن تُحال مجموعة التوثيق القديمة للتقاعد.
:::

# دليل إطلاق وضبط المُنسِّق

يبني هذا الدليل على [مرجع الإعداد](orchestrator-config.md) et
[دليل إطلاق متعدد المصادر](multi-source-rollout.md). يشرح
كيفية ضبط المُنسِّق لكل مرحلة إطلاق، وكيفية قراءة آثار لوحة النتائج، وما هي
إشارات التليمترية المطلوبة قبل توسيع الحركة. طبّق التوصيات بشكل متسق عبر
Les CLI et les SDK sont également utiles pour créer des liens avec les utilisateurs.

## 1. مجموعات المعلمات الأساسية

ابدأ من قالب إعداد مشترك واضبط مجموعة صغيرة من المقابض مع تقدم الإطلاق. يلتقط
الجدول أدناه القيم الموصى بها لأكثر المراحل شيوعًا؛ وما لم يُذكر يعود إلى
Description de `OrchestratorConfig::default()` et `FetchOptions::default()`.

| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | الملاحظات |
|--------------|-----------------|-------------------------------|----------------------------------------|-----------------------------|-----------------------------------------|----------------|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | سقف كمون ضيق ونافذة سماح قصيرة يكشفان التليمترية المزعجة سريعًا. خفّض المحاولات لكشف المانيفستات غير الصالحة مبكرًا. |
| **Mise en scène** | `4` | `3` | `3` | `4000` | `600` | يعكس قيم الإنتاج مع ترك فسحة للنظراء الاستكشافية. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ اضبط `telemetry_region` حتى تتمكن لوحات المتابعة من فصل حركة الكناري. |
| **الإتاحة العامة (GA)** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | ارفع حدود إعادة المحاولة والفشل لامتصاص الأعطال العابرة مع استمرار التدقيق في ضمان الحتمية. |

- يبقى `scoreboard.weight_scale` على القيمة الافتراضية `10_000` ما لم يتطلب نظام لاحق دقة عدية مختلفة. زيادة المقياس لا تغيّر ترتيب المزوّدين؛ بل تنتج توزيعًا أدق للرصيد.
- عند الانتقال بين المراحل، احفظ حزمة JSON et `--scoreboard-out` لتوثيق مجموعة المعلمات الدقيقة في مسار التدقيق.

## 2. نظافة لوحة النتائج

تجمع لوحة النتائج بين متطلبات المانيفست وإعلانات المزوّدين والتليمترية.
قبل المضي قدمًا:1. **تحقّق من حداثة التليمترية.** تأكد من أن اللقطات المشار إليها عبر
   `--telemetry-json` التقطت ضمن نافذة السماح. الإدخالات الأقدم من
   `telemetry_grace_secs` correspond à `TelemetryStale { last_updated }`.
   اعتبر ذلك توقفًا حتميًا وحدّث تصدير التليمترية قبل المتابعة.
2. **راجع أسباب الأهلية.** احفظ الآثار عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل إدخال
   يحمل كتلة `eligibility` بالسبب الدقيق للفشل. لا تتجاوز عدم توافق القدرات أو
   الإعلانات المنتهية؛ أصلح الحمولة المصدرية.
3. **راجع تغيّر الأوزان.** قارن حقل `normalised_weight` بالإصدار السابق. تغيّر
   أكبر من 10% يجب أن يرتبط بتغييرات مقصودة في الإعلانات أو التليمترية ويُسجّل
   في سجل الإطلاق.
4. **أرشف الآثار.** اضبط `scoreboard.persist_path` ليصدر كل تشغيل لقطة لوحة النتائج
   النهائية. أرفق الأثر بسجل الإصدار مع المانيفست وحزمة التليمترية.
5. **وثّق دليل خليط المزوّدين.** يجب أن تعرض ميتاداتا `scoreboard.json` و`summary.json`
   `provider_count` و`gateway_provider_count` والتسمية `provider_mix` لإثبات
   Il s'agit du modèle `direct-only` et `gateway-only` et `mixed`. يجب أن تُظهر لقطات
   البوابة `provider_count=0` و`provider_mix="gateway-only"`, بينما تتطلب التشغيلات المختلطة
   أعدادًا غير صفرية للمصدرين. يفرض `cargo xtask sorafs-adoption-check` dans la version `cargo xtask sorafs-adoption-check`
   (ويفشل إذا لم تتطابق العدادات/التسميات)، لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` أو نص التقاط لديك لإنتاج حزمة الأدلة
   `adoption_report.json`. إشراك بوابات Torii, احفظ `gateway_manifest_id`/
   `gateway_manifest_cid` ضمن ميتاداتا لوحة النتائج حتى يتمكن حاجز التبني من
   ربط مغلف المانيفست بخليط المزوّدين الملتقط.

للتعريفات التفصيلية للحقول راجع
`crates/sorafs_car/src/scoreboard.rs` وبنية ملخص CLI التي يخرجها
`sorafs_cli fetch --json-out`.

## مرجع أعلام CLI et SDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`) et
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) Mise à jour
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالية عند التقاط أدلة الإطلاق أو
Liste des rencontres à venir:

مرجع أعلام متعدد المصادر (أبقِ مساعدة CLI والوثائق متزامنة بتعديل هذا الملف فقط) :- `--max-peers=<count>` يحدّ عدد المزوّدين المؤهلين الذين ينجون من فلتر لوحة النتائج. اتركه فارغًا لتدفق جميع المزوّدين المؤهلين، واضبطه على `1` فقط عند اختبار الرجوع لمصدر واحد عمدًا. Utilisez `maxPeers` pour les SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` est un morceau de morceau `FetchOptions`. استخدم جدول الإطلاق في دليل الضبط للقيم الموصى بها؛ Vous pouvez également utiliser la CLI pour installer le SDK sur votre ordinateur.
- `--telemetry-region=<label>` pour Prometheus `sorafs_orchestrator_*` (OTLP) pour les applications/applications Il s'agit d'une mise en scène et d'une mise en scène de GA.
- `--telemetry-json=<path>` يحقن لقطة التليمترية المشار إليها في لوحة النتائج. JSON est utilisé pour créer des liens avec les utilisateurs (et les `cargo xtask sorafs-adoption-check --require-telemetry` pour OTLP) غذّى الالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) pour les crochets. Vous pouvez également utiliser les chunks pour Proxy Norito/Kaigi et les caches de garde et Kaigi. نفس الإيصالات التي يصدرها Rust.
- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) يحفظ لقطة الأهلية للمدققين. Utilisez JSON pour créer des liens avec les utilisateurs.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يطبّقان تعديلات حتمية فوق ميتاداتا الإعلانات. استخدم هذه الأعلام للتجارب فقط؛ Vous avez besoin d'artefacts pour créer des objets précieux.
- `--provider-metrics-out` / `--chunk-receipts-out` pour les morceaux et les morceaux أرفق كلا الأثرين عند تقديم أدلة التبني.

مثال (باستخدام الـ luminaire المنشور):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Les SDK sont disponibles pour `SorafsGatewayFetchOptions` pour Rust
(`crates/iroha/src/client.rs`) par JS
(`javascript/iroha_js/src/sorafs.js`) et SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
La CLI est également disponible en ligne avec la CLI.
طبقات ترجمة مخصصة.

## 3. ضبط سياسة الجلب

يتحكم `FetchOptions` في إعادة المحاولة والتوازي والتحقق. عند الضبط:

- **إعادة المحاولة:** رفع `per_chunk_retry_limit` et `4` يزيد زمن الاستعادة لكنه
  قد يخفي أعطال المزوّدين. يفضّل إبقاء السقف عند `4` والاعتماد على تدوير المزوّدين
  لإظهار الأداء الضعيف.
- **عتبة الفشل :** تحدد `provider_failure_threshold` متى يتم تعطيل المزوّد لبقية الجلسة.
  يجب أن تتوافق هذه القيمة مع سياسة إعادة المحاولة: عتبة أقل من ميزانية المحاولة
  تُخرج المزوّد قبل استنفاد جميع المحاولات.
- **التوازي:** اترك `global_parallel_limit` غير مضبوط (`None`) ما لم تكن بيئة محددة
  غير قادرة على تشبع النطاقات المُعلنة. عند ضبطه، تأكد أن القيمة ≤ مجموع ميزانيات
  التدفق للمزوّدين لتجنب التجويع.
- **مفاتيح التحقق:** يجب إبقاء `verify_lengths` و`verify_digests` مفعّلين في الإنتاج.
  فهي تضمن الحتمية عندما تعمل أساطيل مزوّدين مختلطة؛ عطّلها فقط في بيئات fuzzing المعزولة.

## 4. مراحل النقل والخصوصية

استخدم حقول `rollout_phase` و`anonymity_policy` و`transport_policy` وضع الخصوصية:- Le `rollout_phase="snnet-5"` est connecté à SNNet-5. استعمل
  `anonymity_policy_override` فقط عندما تصدر الحوكمة توجيهًا موقّعًا.
- `transport_policy="soranet-first"` est compatible avec SNNet-4/5/5a/5b/6a/7/8/12/13 pour 🈺
  (راجع `roadmap.md`). استخدم `transport_policy="direct-only"` فقط لخفض موثق أو تدريبات امتثال،
  وانتظر مراجعة تغطية PQ قبل الترقية إلى `transport_policy="soranet-strict"`—هذا المستوى يفشل سريعًا
  إذا بقيت ترحيلات كلاسيكية فقط.
- Pour télécharger `write_mode="pq-only"`, vous devez utiliser le SDK (SDK)
  تلبية متطلبات PQ. أثناء الإطلاق، أبقِ `write_mode="allow-downgrade"` حتى يمكن لخطط الطوارئ
  الاعتماد على المسارات المباشرة بينما تُعلِّم التليمترية حالات التخفيض.
- يعتمد اختيار الحراس وتجهيز الدوائر على دليل SoraNet. زوّد لقطة `relay_directory` الموقعة
  Le cache `guard_set` est un élément de votre système d'exploitation. بصمة الكاش
  Le `sorafs_cli fetch` est utilisé pour le nettoyage.

## 5. crochets التخفيض والامتثال

يساعد نظامان فرعيان في المُنسِّق على فرض السياسة دون تدخل يدوي:

- **معالجة التخفيض** (`downgrade_remediation`): تراقب أحداث `handshake_downgrade_total`, وبعد تجاوز
  `threshold` est utilisé pour `window_secs` comme proxy pour `target_mode` (métadonnées uniquement).
  احتفظ بالقيم الافتراضية (`threshold=3`, `window=300`, `cooldown=900`) pour تظهر مراجعات الحوادث
  نمطًا مختلفًا. وثّق أي override في سجل الإطلاق وتأكد أن لوحات المتابعة تراقب
  `sorafs_proxy_downgrade_state`.
- **سياسة الامتثال** (`compliance`) : تمر استثناءات الولاية القضائية والمانيفست عبر قوائم opt-out التي
  تديرها الحوكمة. لا تُدرِج remplace عشوائية في حزمة الإعداد؛ اطلب تحديثًا موقعًا
  Pour `governance/compliance/soranet_opt_outs.json`, vous utilisez JSON.

بالنسبة لكلا النظامين، احفظ حزمة الإعداد الناتجة وأرفقها بأدلة الإصدار حتى يتمكن
المدققون من تتبع كيفية تفعيل التخفيضات.

## 6. Questions et réponses

قبل توسيع الإطلاق، تأكد أن الإشارات التالية نشطة في البيئة المستهدفة:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن تكون صفرًا بعد اكتمال الكناري.
- `sorafs_orchestrator_retries_total` et
  `sorafs_orchestrator_retry_ratio` — يجب أن تستقر تحت 10% أثناء الكناري وتبقى
  5% sur GA.
- `sorafs_orchestrator_policy_events_total` — يتحقق من أن مرحلة الإطلاق المطلوبة
  Il s'agit d'une coupure de courant (étiquette `stage`) et d'une baisse de tension sur `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — تتبع عرض ترحيلات PQ مقابل توقعات السياسة.
- أهداف سجلات `telemetry::sorafs.fetch.*` — يجب بثها إلى مجمّع السجلات المشترك مع
  عمليات بحث محفوظة لـ `status=failed`.

حمّل لوحة Grafana القياسية من
`dashboards/grafana/sorafs_fetch_observability.json` (مصدَّرة في البوابة تحت
**SoraFS → Récupérer l'observabilité**)
Les problèmes liés au burn-in SRE sont également importants.
Utiliser Alertmanager pour `dashboards/alerts/sorafs_fetch_rules.yml` et votre application
صياغة Prometheus à `scripts/telemetry/test_sorafs_fetch_alerts.sh` (يشغّل المساعد
`promtool test rules` entre Docker). تتطلب عمليات تسليم التنبيه نفس كتلة
التوجيه التي يطبعها السكربت حتى يتمكن المشغلون من إرفاق الأدلة بتذكرة الإطلاق.

### تدفق burn-in للتليمتريةيتطلب بند خارطة الطريق **SF-6e** فترة burn-in للتليمترية لمدة 30 يومًا قبل تحويل
المُنسِّق متعدد المصادر إلى افتراضاته GA. استخدم سكربتات المستودع لالتقاط حزمة
آثار قابلة لإعادة الإنتاج لكل يوم في النافذة:

1. Utilisez `ci/check_sorafs_orchestrator_adoption.sh` pour le burn-in. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   يعيد المساعد تشغيل `fixtures/sorafs_orchestrator/multi_peer_parity_v1`, ويكتب
   `scoreboard.json` et `summary.json` et `provider_metrics.json` et `chunk_receipts.json`
   و`adoption_report.json` comme `artifacts/sorafs_orchestrator/<timestamp>/`,
   ويفرض حدًا أدنى من المزوّدين المؤهلين عبر `cargo xtask sorafs-adoption-check`.
2. Utilisez le burn-in pour le processus de gravure `burn_in_note.json`.
   وفهرس اليوم، ومعرّف المانيفست، ومصدر التليمترية، وملخصات الآثار. Utiliser JSON
   بسجل الإطلاق لتوضيح أي لقطة استوفت كل يوم ضمن نافذة 30 يومًا.
3. Prise en charge Grafana (`dashboards/grafana/sorafs_fetch_observability.json`)
   Il s'agit de la mise en scène/production, ainsi que du burn-in, ainsi que de la mise en scène et de la production.
   للمانيفست/المنطقة قيد الاختبار.
4. Voir `scripts/telemetry/test_sorafs_fetch_alerts.sh` (voir `promtool test rules …`)
   عند تغيير `dashboards/alerts/sorafs_fetch_rules.yml` لتوثيق أن توجيه التنبيهات
   Il s'agit d'un burn-in.
5. أرشف لقطة لوحة المتابعة، ومخرجات اختبار التنبيهات، وذيل السجلات لعمليات البحث
   `telemetry::sorafs.fetch.*` مع آثار المُنسِّق حتى تتمكن الحوكمة من إعادة إنتاج
   الأدلة دون الاعتماد على أنظمة حية.

## 7. قائمة تحقق الإطلاق

1. Utilisez les tableaux de bord pour CI pour les besoins des utilisateurs.
2. شغّل جلب luminaires الحتمي في كل بيئة (المختبر، mise en scène، الكناري، الإنتاج) وأرفق
   Utilisez `--scoreboard-out` et `--json-out` pour le nettoyage.
3. راجع لوحات التليمترية مع مهندس المناوبة، وتأكد أن كل المقاييس أعلاه لديها عينات حية.
4. Utilisez la méthode de validation (consultez `iroha_config`) et commit git pour la gestion de la communauté.
   للإعلانات والامتثال.
5. Utilisez le SDK pour créer des liens avec les utilisateurs.

اتباع هذا الدليل يحافظ على إطلاقات المُنسِّق حتمية وقابلة للتدقيق، مع توفير حلقات
تغذية راجعة واضحة لضبط ميزانيات إعادة المحاولة، وسعة المزوّدين، ووضع الخصوصية.