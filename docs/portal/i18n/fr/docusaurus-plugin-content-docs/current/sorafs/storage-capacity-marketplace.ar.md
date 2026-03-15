---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : سوق سعة التخزين في SoraFS
sidebar_label : سوق السعة
description: Le SF-2c est un appareil photo connecté à Internet.
---

:::note المصدر المعتمد
Il s'agit de `docs/source/sorafs/storage_capacity_marketplace.md`. حافظوا على تزامن النسختين ما دامت الوثائق القديمة نشطة.
:::

# سوق سعة التخزين في SoraFS (مسودة SF-2c)

يقدّم بند خارطة الطريق SF-2c سوقا محكوما حيث يعلن مزودو التخزين عن سعة ملتزم بها، ويتلقون أوامر النسخ المتماثل، ويكسبون رسوما تتناسب مع التوافر المُسلَّم. يحدد هذا المستند نطاق المخرجات المطلوبة للإصدار الأول ويقسمها إلى مسارات قابلة للتنفيذ.

## الأهداف

- التعبير عن التزامات سعة المزودين (إجمالي البايتات، حدود لكل lane,، تاريخ الانتهاء) بصيغة قابلة للتحقق Il s'agit de SoraNet et Torii.
- توزيع pins عبر المزودين وفق السعة المعلنة et mise وقيود السياسة مع الحفاظ على سلوك حتمي.
- قياس تسليم التخزين (نجاح النسخ المتماثل، uptime, أدلة السلامة) et تصدير التليمترية لتوزيع الرسوم.
- توفير عمليات الإلغاء والنزاع حتى يمكن معاقبة المزودين غير الأمناء أو إزالتهم.

## مفاهيم النطاق

| المفهوم | الوصف | المخرج الأولي |
|---------|-------------|-----------|
| `CapacityDeclarationV1` | حمولة Norito تصف معرف المزود، دعم ملف تعريف chunker, GiB الملتزمة، حدود خاصة بـ lane, تلميحات Il s'agit d'un jalonnement et d'un jalonnement. | المخطط + المدقق في `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Il s'agit d'un manifeste pour le CID et d'un manifeste pour la sécurité des affaires. SLA. | Norito est associé à Torii + et contrat intelligent. |
| `CapacityLedger` | Il existe des liens entre les chaînes en chaîne et hors chaîne. | Il s'agit d'un contrat intelligent et d'un stub hors chaîne comme d'un instantané. |
| `MarketplacePolicy` | Il s'agit d'une mise en jeu et d'une mise en jeu. | config struct est `sorafs_manifest` + وثيقة حوكمة. |

### المخططات المنفذة (الحالة)

## تفصيل العمل

### 1. طبقة المخططات والسجل

| المهمة | Propriétaire(s) | ملاحظات |
|------|----------|-------|
| Voir `CapacityDeclarationV1` et `ReplicationOrderV1` et `CapacityTelemetryV1`. | Équipe Stockage / Gouvernance | استخدم Norito؛ وأدرج النسخ الدلالية ومراجع القدرات. |
| Utilisé parser + validateur pour `sorafs_manifest`. | Équipe de stockage | فرض IDs أحادية، حدود السعة، ومتطلبات mise. |
| Vous pouvez utiliser le chunker de métadonnées `min_capacity_gib` pour le faire fonctionner. | GT Outillage | يساعد العملاء على فرض الحد الأدنى من متطلبات العتاد لكل ملف تعريف. |
| صياغة وثيقة `MarketplacePolicy` التي تلتقط ضوابط القبول وجدول العقوبات. | Conseil de gouvernance | Voir la documentation sur les valeurs par défaut de la stratégie. |

#### تعريفات المخطط (منفذة)- Le `CapacityDeclarationV1` fournit des fonctionnalités pour gérer les capacités du chunker et les capacités du chunker. Il y a des métadonnées dans Lane Lane. Le gestionnaire de pieux gère les alias de la voie et GiB. Lire.【crates/sorafs_manifest/src/capacity.rs:28】
- Le fichier `ReplicationOrderV1` manifeste des manifestes pour les SLA et les affectations. Les validators handles chunker sont disponibles pour la date limite et la date limite pour le Torii et la date limite. الأمر.【crates/sorafs_manifest/src/capacity.rs:301】
- يعبر `CapacityTelemetryV1` pour les instantanés (GiB المعلنة مقابل المستخدمة، عدادات النسخ، نسب uptime/PoR) الرسوم. La valeur de la valeur de référence est de 0 à 100 %.【crates/sorafs_manifest/src/capacity.rs:476】
- Les assistants de la voie (`CapacityMetadataEntry` et `PricingScheduleV1` pour la voie/l'affectation/SLA) sont également utilisés pour les aider. Pour CI et les outils en aval, consultez la page.【crates/sorafs_manifest/src/capacity.rs:230】
- يعرض `PinProviderRegistry` instantané de l'instantané, puis `/v2/sorafs/capacity/state`, pour le grand livre des frais. Norito JSON حتمي.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Les poignées des poignées sont celles de la voie et des voies. نطاق التليمترية حتى تظهر الانحدارات فوراً في CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Spécifications techniques : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` spécifications techniques pour les charges utiles Norito pour les blobs base64 et JSON. يتمكن المشغلون من تجهيز luminaires لـ `/v2/sorafs/capacity/declare` و`/v2/sorafs/capacity/telemetry` وأوامر النسخ المتماثل مع تحقق محلي.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 توجد Appareils de référence pour `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et توليدها عبر `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. تكامل طبقة التحكم

| المهمة | Propriétaire(s) | ملاحظات |
|------|----------|-------|
| Utilisez Torii pour `/v2/sorafs/capacity/declare` et `/v2/sorafs/capacity/telemetry` et `/v2/sorafs/capacity/orders` pour Norito JSON. | Équipe Torii | محاكاة منطق التحقق؛ J'utilise Norito Helpers JSON. |
| Les instantanés sont utilisés pour `CapacityDeclarationV1`, les métadonnées et l'orchestrateur et la passerelle. | GT Outillage / Equipe Orchestrateur | Le `provider_metadata` est utilisé pour marquer la voie de la voie. |
| Il s'agit d'un orchestrateur/passerelle pour les affectations et le basculement. | Équipe Réseautage TL / Gateway | يستهلك Constructeur de tableau de bord أوامر النسخ الموقعة من الحوكمة. |
| Dans CLI : remplacez `sorafs_cli` par `capacity declare` et `capacity telemetry` et `capacity orders import`. | GT Outillage | توفير JSON حتمي + مخرجات tableau de bord. |

### 3. سياسة السوق والحوكمة| المهمة | Propriétaire(s) | ملاحظات |
|------|----------|-------|
| إقرار `MarketplacePolicy` (pour la mise en jeu, مضاعفات العقوبة، وتواتر التدقيق). | Conseil de gouvernance | نشرها في docs وتسجيل سجل المراجعات. |
| Il y a aussi les déclarations du Parlement et les déclarations. | Conseil de gouvernance / équipe Smart Contract | استخدام Événements Norito + ingestion de manifeste. |
| تنفيذ جدول العقوبات (تخفيض الرسوم، slashing للبوند) المرتبط بانتهاكات SLA المقاسة عن بعد. | Conseil de Gouvernance / Trésorerie | مواءمة ذلك مع مخرجات التسوية في `DealEngine`. |
| توثيق عملية النزاع ومصفوفة التصعيد. | Documents / Gouvernance | Il existe un runbook de contestation + des assistants CLI. |

### 4. الميترينغ وتوزيع الرسوم

| المهمة | Propriétaire(s) | ملاحظات |
|------|----------|-------|
| Vous pouvez ingérer des informations sur Torii pour `CapacityTelemetryV1`. | Équipe Torii | Il s'agit de GiB-heure, de PoR et de disponibilité. |
| Utilisez le lien `sorafs_node` pour obtenir un contrat SLA + un contrat SLA. | Équipe de stockage | Il s'agit d'un gestionnaire de chunker. |
| خط أنابيب التسوية: تحويل التليمترية + بيانات النسخ إلى paiements مقومة byـ XOR, وإنتاج ملخصات جاهزة للحوكمة، وتسجيل حالة grand livre. | Équipe Trésorerie / Stockage | التوصيل إلى Deal Engine / Exportations du Trésor. |
| Utiliser des tableaux de bord/alertes pour l'ingestion du backlog (ingestion du backlog). | Observabilité | Utilisez le Grafana pour le SF-6/SF-7. |

- Utilisez Torii pour `/v2/sorafs/capacity/telemetry` et `/v2/sorafs/capacity/state` (JSON + Norito) pour créer des instantanés. لكل حقبة ويمكن للمراجعين استرجاع ledger الحتمي للتدقيق أو تغليف الأدلة.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- يضمن تكامل `PinProviderRegistry` إمكانية الوصول إلى أوامر النسخ المتماثل عبر نفس endpoint؛ Utilisez la CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) pour le hachage et l'alias.
- Captures d'écran instantanées `CapacityTelemetrySnapshot` et capture instantanée `metering` et exportations Prometheus et Grafana L'analyseur de données `docs/source/grafana_sorafs_metering.json` est responsable de la gestion des données GiB-heure et nano-SORA Les informations relatives au SLA sont prises en charge par les utilisateurs.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Permet de lisser les mesures et de créer un instantané `smoothed_gib_hours` et `smoothed_por_success_bps` pour créer un instantané avec l'EMA. Vous pouvez utiliser les paiements pour les paiements.【crates/sorafs_node/src/metering.rs:401】

### 5. معالجة النزاعات والإلغاء| المهمة | Propriétaire(s) | ملاحظات |
|------|----------|-------|
| تعريف حمولة `CapacityDisputeV1` (preuves, صاحب الشكوى، المزود المستهدف). | Conseil de gouvernance | Utilisez Norito + مدقق. |
| دعم CLI لتقديم conteste والرد عليها (مع مرفقات preuve). | GT Outillage | Le hachage est une preuve. |
| Il s'agit d'un litige relatif au SLA (conflit). | Observabilité | عتبات تنبيه وخطافات حوكمة. |
| توثيق playbook الإلغاء (فترة سماح، إخلاء بيانات épinglé). | Équipe Documents/Stockage | Il s'agit du runbook de l'opérateur. |

## متطلبات الاختبار و CI

- اختبارات وحدات لكل مدققات المخطط الجديدة (`sorafs_manifest`).
- اختبارات تكامل تحاكي: إعلان → أمر نسخ → comptage → paiement.
- workflow avec CI لإعادة توليد إعلانات/تليمترية السعة وضمان تزامن التواقيع (توسيع `ci/check_sorafs_fixtures.sh`).
- Les API de registre sont disponibles (10 000 à 100 000).

## التليمترية ولوحات المعلومات

- Tableau de bord لوحات :
  - السعة المعلنة مقابل المستخدمة لكل مزود.
  - arriéré أوامر النسخ ومتوسط ​​تأخير التعيين.
  - SLA (temps de disponibilité et PoR).
  - تراكم الرسوم والغرامات لكل حقبة.
- Alertes :
  - مزود أقل من الحد الأدنى للسعة الملتزم بها.
  - Il s'agit d'un problème lié au SLA.
  - إخفاقات خط أنابيب الميترينغ.

## مخرجات التوثيق

- دليل المشغل لإعلان السعة وتجديد الالتزامات ومراقبة الاستخدام.
- دليل الحوكمة للموافقة على الإعلانات وإصدار الأوامر ومعالجة النزاعات.
- مرجع API لنقاط نهاية السعة وتنسيق أوامر النسخ المتماثل.
- أسئلة شائعة للـ marché موجهة للمطورين.

## قائمة تحقق جاهزية GA

بند خارطة الطريق **SF-2c** يبوّب إطلاق الإنتاج على أدلة ملموسة عبر المحاسبة ومعالجة النزاعات والالتحاق. استخدموا الأدوات أدناه لإبقاء معايير القبول متزامنة مع التنفيذ.

### Comptabilité nocturne et rapprochement XOR
- L'instantané est pris en charge et le grand livre XOR est également disponible :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ينهي المساعد التنفيذ بكود غير صفري عند وجود تسويات أو غرامات مفقودة/زائدة، ويصدر ملخصا بصيغة Fichier texte Prometheus.
- Modèle `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml`)
  يطلق عندما تشير مقاييس réconciliation إلى فجوات؛ لوحات المعلومات موجودة تحت
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Utiliser JSON et les hachages pour `docs/examples/sorafs_capacity_marketplace_validation/`
  بجانب paquets de gouvernance.

### Contestation et réduction des preuves
- قدّم litiges عبر `sorafs_manifest_stub capacity dispute` (اختبارات :
  `cargo test -p sorafs_car --test capacity_cli`) حتى تبقى payloads قياسية.
- شغّل `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et عقوبات
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) Il s'agit de litiges et de barres obliques.
- اتبع `docs/source/sorafs/dispute_revocation_runbook.md` لالتقاط الأدلة والتصعيد؛ اربط موافقات strike في تقرير التحقق.

### Tests de fumée d'intégration et de sortie des fournisseurs
- Vous pouvez utiliser les artefacts pour l'analyse/analyse des artefacts `sorafs_manifest_stub capacity ...` et l'interface CLI pour l'interface utilisateur (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Remplacez Torii (`/v2/sorafs/capacity/declare`) par `/v2/sorafs/capacity/state` pour Grafana. اتبع مسار الخروج في `docs/source/sorafs/capacity_onboarding_runbook.md`.
- أرشِف artefacts الموقعة ومخرجات réconciliation داخل `docs/examples/sorafs_capacity_marketplace_validation/`.

## الاعتماديات والتسلسل1. إكمال SF-2b (politique d'admission) - يعتمد market على مزودين مدققين.
2. تنفيذ طبقة المخطط + السجل (هذا المستند) قبل تكامل Torii.
3. Installer le pipeline de comptage et le pipeline de comptage.
4. Fonctionnalités : Utiliser le comptage pour la mise en scène.

Il s'agit d'une feuille de route et d'une feuille de route. La feuille de route est complétée par le plan de contrôle (plan de contrôle, mesure, mesures) et la fonctionnalité est complète.