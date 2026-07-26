---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-elastic-lane
titre : تجهيز voie المرن (NX-7)
sidebar_label : تجهيز lane المرن
description : Le bootstrap se manifeste pour le déploiement de la voie Nexus et du déploiement.
---

:::note المصدر الرسمي
Il s'agit de la référence `docs/source/nexus_elastic_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح الترجمة الى البوابة.
:::

# مجموعة ادوات تجهيز voie المرن (NX-7)

> **عنصر خارطة الطريق:** NX-7 - ادوات تجهيز lane المرن  
> **الحالة:** الادوات مكتملة - تولد manifestes, مقتطفات الكتالوج، حمولات Norito, اختبارات fumée,
> ومساعد bundle لاختبارات الحمل يجمع الان بوابة زمن الاستجابة لكل slot + manifestes الادلة كي تنشر اختبارات حمل المدققين
> دون سكربتات مخصصة.

Il s'agit d'un manifeste pour la voie et l'espace de données. Et déploiement. جديدة في Nexus (عامة او خاصة) دون تحرير عدة ملفات يدويا ودون اعادة اشتقاق هندسة الكتالوج يدويا.

## 1. المتطلبات المسبقة1. L'alias de la voie et de l'espace de données est utilisé pour le règlement.
2. قائمة نهائية بالمدققين (معرفات الحسابات) et les espaces de noms محمية.
3. وصول الى مستودع تهيئة العقد كي تتمكن من اضافة المقتطفات المولدة.
4. Les manifestes des manifestes sont sur la voie (`nexus.registry.manifest_directory` et `cache_directory`).
5. Si vous utilisez PagerDuty pour la voie, vous pouvez également utiliser la voie pour la voie.

## 2. توليد artefacts للـ lane

شغّل المساعد من جذر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

اهم الاعلام:

- `--lane-id` est disponible dans l'index de référence de `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` sont dans l'espace de données (l'identifiant de la voie est la voie).
- `--validator` est compatible avec `--validators-file`.
- `--route-instruction` / `--route-account` تصدر قواعد توجيه جاهزة للصق.
- `--metadata key=value` (`--telemetry-contact/channel/runbook`) تلتقط جهات اتصال الـ runbook لكي تعرض اللوحات اصحابها الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` sont des crochets de mise à niveau du runtime pour le manifeste et la voie de la voie pour la mise à niveau.
- `--encode-space-directory` et `cargo xtask space-directory encode`. استخدمه مع `--space-directory-out` اذا اردت ان يوضع ملف `.to` المشفر في مسار غير الافتراضي.

Les artefacts sont liés à l'encodage `--output-dir` (الافتراضي هو المجلد الحالي) :1. `<slug>.manifest.json` - manifeste pour la voie pour le quorum et les espaces de noms pour la mise à niveau du runtime du hook.
2. `<slug>.catalog.toml` - TOML est compatible avec `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` et est également disponible. Vous pouvez utiliser `fault_tolerance` dans l'espace de données pour le relais de voie (`3f+1`).
3. `<slug>.summary.json` - ملخص تدقيق يصف الهندسة (slug والقطاعات والبيانات) et déploiement du مطلوبة والامر الدقيق `cargo xtask space-directory encode` (voir `space_directory_encode.command`). Il s'agit d'une application d'intégration JSON.
4. `<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`؛ Il s'agit de `iroha app space-directory manifest publish` et Torii.

`--dry-run` pour JSON/المقتطفات pour les artefacts et `--force` pour les artefacts الموجودة.

## 3. تطبيق التغييرات1. Utiliser le manifeste JSON comme `nexus.registry.manifest_directory` (et le répertoire de cache dans le registre à distance). التزم بالملف اذا كانت manifeste تُدار بالنسخ في مستودع التهيئة.
2. Connectez-vous à `config/config.toml` (`config.d/*.toml` المناسب). Mettez le `nexus.lane_count` dans la voie `lane_id + 1` et `nexus.routing_policy.rules` dans la voie de circulation.
3. Il s'agit d'un manifeste (`--encode-space-directory`) et d'un manifeste dans Space Directory pour un résumé (`space_directory_encode.command`). Charge utile `.manifest.to` pour Torii et charge utile ارسله عبر `iroha app space-directory manifest publish`.
4. Utilisez `irohad --sora --config path/to/config.toml --trace-config` pour le suivi du déploiement de trace. Il s'agit de slug/قطاعات Kura المولدة.
5. اعد تشغيل المدققين المخصصين للlane بعد نشر تغييرات manifest/الكتالوج. Il s'agit d'un résumé JSON et d'une description détaillée.

## 4. بناء bundle لتوزيع السجل

Il existe des manifestes et des superpositions pour les configurations, ainsi que les voies et les configurations. Le bundler est livré avec des manifestes et une superposition de superposition pour `nexus.registry.cache_directory` et une archive tar. لنقل hors ligne :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

المخرجات:1. `manifests/<slug>.manifest.json` - انسخها الى `nexus.registry.manifest_directory` المهيأ.
2. `cache/governance_catalog.json` - par `nexus.registry.cache_directory`. Pour le `--module`, vous pouvez utiliser la superposition de la superposition (NX-2) Il s'agit d'un `config.toml`.
3. `summary.json` - Hachages et superposition de hachages et de superpositions.
4. Objet `registry_bundle.tar.*` - Objets SCP et S3 et objets artefacts.

زامن المجلد بالكامل (او الارشيف) لكل مدقق، وفكّه على hôtes معزولة، وانسخ manifests + overlay الكاش الى مسارات السجل Il s'agit de Torii.

## 5. اختبارات fume للمدققين

بعد اعادة تشغيل Torii, شغّل مساعد smoke الجديد للتحقق من ان lane يبلّغ `manifest_ready=true`, وان المقاييس Il y a des voies fermées et des voies scellées. يجب ان تعرض voies التي تتطلب manifestes قيمة `manifest_path` غير فارغة؛ Vous pouvez également consulter le manifeste NX-7 du manifeste :

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Le `--insecure` est un fichier auto-signé. Les voies de circulation sont fermées par voie scellée et scellées. Utilisez `--min-block-height` et `--max-finality-lag` et `--max-settlement-backlog` et `--max-headroom-events` pour la voie (démarrage). الكتلة/النهائية/backlog/headroom) ضمن حدود التشغيل، واربطها مع `--max-slot-p95` / `--max-slot-p99` (مع `--min-slot-samples`) لفرض Il s'agit d'un emplacement pour le NX-18 pour un emplacement idéal.Les paramètres air-gapped (CI) sont associés au point final Torii :

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Les luminaires `fixtures/nexus/lanes/` sont des artefacts utilisés par bootstrap pour les manifestes lint et les manifestes de lint. Utilisez CI pour `ci/check_nexus_lane_smoke.sh` et `ci/check_nexus_lane_registry_bundle.sh` (alias : `make check-nexus-lanes`) pour fumer de la fumée pour le NX-7. Il s'agit de la charge utile et des résumés/superpositions du bundle dans le cadre de la tâche.