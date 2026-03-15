---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: storage-capacity-marketplace
כותרת: Marketplace de capacité de stockage SoraFS
sidebar_label: Marketplace de capacité
תיאור: תוכנית SF-2c pour le marketplace de capacité, les ordres de réplication, la télémétrie et les hooks de governance.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/storage_capacity_marketplace.md`. Gardez les deux emplacements alignés tant que la documentation héritée reste active.
:::

# Marketplace de capacité de stockage SoraFS (Brouillon SF-2c)

L'item SF-2c de la introduit un marketplace governé où les providers de
מלאי הצהרת une capacité engagée, reçoivent des ordres de réplication et
perçoivent des fees proportionnels à la disponibilité livrée. Ce document cadre
les livrables requis pour la première release et les décline en pistes actionnables.

## אובייקטים

- Expprimer les engagements de capacité (בתים טוטאו, מגבלות מסלול, תפוגה)
  ניתנת לאימות מתכלה לפי שלטון, תחבורה SoraNet et Torii.
- Allouer les pins entre providers selon la capacité déclarée, le stake et les
  contraintes de politique tout en Maintenant un comportement déterministe.
- Mesurer la livraison de stockage (הצלחות של השכפול, זמן פעולה, preuves d'intégrité)
  et exporter la télémétrie pour la distribution des fees.
- Fournir des processus de révocation et de dispute afin que les providers malhonnêtes
  soient pénalisés או פורש.

## Concepts de domaine

| קונספט | תיאור | ראשי תיבות סביר |
|--------|----------------|----------------|
| `CapacityDeclarationV1` | מטען Norito לפי תעודת הזהות של הספק, התמיכה בפרופיל chunker, ה-GiB מעורבים, מגבלות המסלול, רמזים לתמחור, התקשרות ותפוגה. | Schema + validateur dans `sorafs_manifest::capacity`. |
| `ReplicationOrder` | הוראה זו נכתבה על ידי נציג הממשל ו-CID de manifest à un ou plusieurs ספקים, כולל Le niveau de redondance et les métriques SLA. | Schema Norito partagé avec Torii + API של חוזה חכם. |
| `CapacityLedger` | הרישום בשרשרת/מחוץ לשרשרת מתאים להצהרות היכולות הפעילות, לסדרה דה réplication, למטרות ביצועים ולצבירה של עמלות. | מודול חוזה חכם או תעודת שירות מחוץ לשרשרת עם תמונת מצב דטרמיניסטית. |
| `MarketplacePolicy` | הפוליטיקה הממשלתית דפיינית למינימום ההימור, Les exigences d'audit et les courbes de pénalité. | Config Struct dans `sorafs_manifest` + מסמך ניהול. |

### תכניות יישום (חוק)

## Découpage du travail

### 1. ספה ומערכת רישום| טאצ'ה | בעלים | הערות |
|------|----------------|-------|
| Définir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | צוות אחסון / ממשל | Utiliser Norito ; כלול גרסה סמנטיקה ומקורות כושר. |
| Implémenter les modules parser + validateur dans `sorafs_manifest`. | צוות אחסון | תעודות זהות מונוטוניות, מגבלות קיבולת, דרישות עניין. |
| מידע על מטא נתונים של chunker registry עם פרופיל `min_capacity_gib`. | Tooling WG | עזר ללקוחות מטפחים דרישות מינימליות של חומרה לפרופיל. |
| Rediger le document `MarketplacePolicy` לוכד les guards d'admission et le calendrier de pénalités. | מועצת ממשל | מוציא לאור של מסמכים עם ברירת מחדל של פוליטיקה. |

#### Definitions de schéma (יישום)- `CapacityDeclarationV1` ללכוד את התקשרויות היכולות של ספק, כולל ידיות chunker canoniques, רפרנסים של קיבולת, caps optionnels par lane, רמזים לתמחור, הגדרות תקפות ומטא נתונים. La validation assure un stake non null, des handles canoniques, des aliases dédupliqués, des caps par lane dans le total déclaré et une comptabilité GiB monotone.【ארגזים/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` associe des manifests à des assignments émis par la governance avec objectifs de redondance, seuils SLA et garanties par assignment ; les validateurs imposent des handles canoniques, des providers uniques et des contraintes de deadline avant ingestion par Torii ou le registry.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` אקספריים של צילומי מצב לעתיד (GiB déclarés vs utilisés, compteurs de réplication, pourcentages d'uptime/PoR) qui alimentent the distribution des fees. Les bornes maintiennent l'utilisation dans les déclarations et les pourcentages dans 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Les helpers partagés (`CapacityMetadataEntry`, `PricingScheduleV1`, נתיב אימות/מקצה/SLA) fournissent une validation déterministe des clés et un reporting d'erreur réutilisable par CI et le tooling במורד הזרם.【ארגזים/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` לחשוף את תמונת המצב על השרשרת דרך `/v1/sorafs/capacity/state`, ב-`/v1/sorafs/capacity/state`, הצהרות משולבות של ספקים וכניסות ל-Fee Ledger Derrière un Norito JSON déterministe.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La couverture de validation exerce l'application des handles canoniques, la détection de doublons, les bornes par lane, les guards d'assignation de réplication et les checks de plage de télémétrie pour que les régressions apparaissent immmédiatement en CI.【ארגזים/sorafs_manifest/src/capacity.rs:792】
- מפעיל כלי עבודה: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` המרת מפרט טכני ומטענים Norito canoniques, blobs base64 ו-JSON קורות חיים, אבל מתכננים מתקנים Prometheus, Prometheus, des ordres de réplication avec validation locale.【ארגזים/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Les fixtures de référence vivent dans `fixtures/sorafs_manifest/replication_order/` (Prometheus, son Prometheus, son Prometheus, générées via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Intégration du plan de contrôle| טאצ'ה | בעלים | הערות |
|------|----------------|-------|
| Ajouter des handlers Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` avec מטענים Norito JSON. | צוות Torii | Répliquer la logique de validation; réutiliser les helpers Norito JSON. |
| הצג תמונות מצב `CapacityDeclarationV1` לעומת מטא נתונים של לוח התוצאות של מתזמר ותוכניות לשער הבאות. | צוות Tooling WG / תזמורת | Étendre `provider_metadata` עם נקודות זכות לניקוד מרובות מקורות בכבוד לגבולות הרצועה. |
| מזמינים את הסדרים של רפליקציית לקוחות מתזמר/שער עבור משימות טייסים ורמזים לכשל. | Networking TL / Gateway צוות | Le Builder de scoreboard consomme des ordres signnés par la governance. |
| כלי עבודה CLI: étendre `sorafs_cli` avec `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Fournir JSON déterministe + לוח תוצאות מיונים. |

### 3. שוק פוליטי וממשל

| טאצ'ה | בעלים | הערות |
|------|----------------|-------|
| מאשר `MarketplacePolicy` (מינימום הימור, מכפילי עונשין, קצב ביקורת). | מועצת ממשל | Publier dans les docs, capturer l'historique des révisions. |
| Ajouter des hooks de governance pour que le Parlement approuve, renouvelle et révoque les déclarations. | מועצת ממשל / צוות חוזה חכם | Utiliser les événements Norito + בליעה של מניפסטים. |
| Implémenter le calendrier de pénalités (הפחתת עמלות, חיתוך אגרות חוב) לעזרת הפרות SLA télémétrées. | מועצת ממשל / האוצר | S'aligner avec les outputs de settlement du `DealEngine`. |
| מתעד את התהליכים המחלוקת ו-la matrice d'escalade. | מסמכים / ממשל | Lier au runbook de dispute + helpers CLI. |

### 4. מדידה והפצה של עמלות

| טאצ'ה | בעלים | הערות |
|------|----------------|-------|
| Étendre l'ingestion de metering Torii לשפוך מקבל `CapacityTelemetryV1`. | צוות Torii | Valider GiB-hour, Succès PoR, uptime. |
| Mettre à jour le pipeline de meterering `sorafs_node` עבור ניצול כתב לפי הזמנה + נתונים סטטיסטיים SLA. | צוות אחסון | Aligner avec les ordres de réplication ומטפל ב-chunker. |
| Pipeline de Settlement: convertir télémétrie + réplication en payouts dénommés en XOR, produire des résumés prêts pour governance, and enregistrer l'état du ledger. | צוות אוצר / אחסנה | Connecter à Deal Engine / יצוא משרד האוצר. |
| לוחות מחוונים/התראות של יצואנים pour la santé du meterering (פיגור בבליעה, télémétrie stale). | צפייה | Étendre le pack Grafana référencé par SF-6/SF-7. |- Torii חשוף désormais `/v1/sorafs/capacity/telemetry` et `/v1/sorafs/capacity/state` (JSON + Norito) afin que les opérateurs soumettent des snapshots de télémétrie par cup lesépoiqueur ledger audit ou packaging de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` garantit que les ordres de réplication sont accessibles via le même endpoint; les helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) תקף ופרסום תקף לתקשורת טלמית של אוטומציות עם חישוף דטרמיניסט ופתרון ד'כינוי.
- Les snapshots de metering produisent des entrées `CapacityTelemetrySnapshot` fixées au snapshot `metering`, et les exports Prometheus alimentent le board Grafana prêt à importer dans `docs/source/grafana_sorafs_metering.json` afin que les équipes de facturation suivent l'accumulation de GiB-hour, les fees nano-SORA projetés et la conformité SLA en temps réel.【crates/iroha_torii/src/routing.rs:5143】】【anadocs.son/source:
- Lorsque le smoothing de metering est active, the snapshot inclut `smoothed_gib_hours` et `smoothed_por_success_bps` pour comparer les valeurs EMA aux compteurs bruts utilisés par la governance pour les payouts.【crates/sorafs/4metering/s.rs

### 5. Gestion des disputes et révocations

| טאצ'ה | בעלים | הערות |
|------|----------------|-------|
| Définir le payload `CapacityDisputeV1` (מיוחס, preuve, ספק ciblé). | מועצת ממשל | Schema Norito + תוקף. |
| תמכו ב-CLI pour déposer des disputes et répondre (avec pièces de preuve). | Tooling WG | Assurer un hashing déterministe du bundle de preuves. |
| Ajouter des checks automatiques pour הפרות SLA répétées (auto-escalade en dispute). | צפייה | Seuils d'alertes et hooks de governance. |
| Documenter le playbook de révocation (תקופת החסד, évacuation des données pin). | צוות מסמכים / אחסון | Lier au doc ​​de politique et au runbook opérateur. |

## דרישות בדיקות ו-CI

- Tests unitaires pour tous les nouveaux validateurs de schéma (`sorafs_manifest`).
- מבחנים סימולנט של אינטגרציה: הצהרה → הזמנה דה réplication → מדידה → תשלום.
- זרימת עבודה CI pour régénérer des declarations/telemetrie de capacité et assurer la synchronization des signatures (étendre `ci/check_sorafs_fixtures.sh`).
- בדיקות לטעינה לרישום API (הדמה של 10,000 ספקים, 100,000 הזמנות).

## Télémétrie & לוחות מחוונים

- לוח המחוונים Panneaux de:
  - Capacité déclarée לעומת ספק שימושי.
  - Backlog des ordres de réplication et délai moyen d'assignation.
  - SLA Conformité (זמן פעילות, %, taux de succès PoR).
  - הצטברות עמלות et pénalités par époque.
- התראות:
  - ספק sous la capacité minimale engagée.
  - Ordre de réplication bloqué > SLA.
  - Échecs du pipeline de meterering.

## Livrables de documentation- מדריך מפעיל להכריז על יכולת, חידוש התקשרויות ומעקב אחר שימוש.
- Guide de governance pour approuver les déclarations, émettre des ordres et gérer les disputes.
- API של Référence pour les pointpoints de capacité et le format d'ordre de réplication.
- שאלות נפוצות למפתחים בשוק.

## רשימת רשימת מוכנות GA

L'item de roadmap **SF-2c** conditionne le rollout production à des preuves concrètes
sur la comptabilité, la gestion des disputes et l'onboarding. Utilisez les artefacts
ci-dessous pour garder les critères d'acceptation alignés avec l'implémentation.

### Comptabilité nocturne et réconciliation XOR
- יצואנית תמונת מצב של קיבולת et l'export du ledger XOR pour la même fenêtre, puis lancer :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Le helper sort avec un code non null en cas de settlements/penalties manquants ou excessifs et émet un résumé Prometheus au format textfile.
- L'alerte `SoraFSCapacityReconciliationMismatch` (dans `dashboards/alerts/sorafs_capacity_rules.yml`) se déclenche lorsque les métriques de réconciliation signalent des écarts ; לוחות מחוונים ב-`dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiver le résumé JSON et les hashes sous `docs/examples/sorafs_capacity_marketplace_validation/` עם חבילות ניהול.

### Preuve de dispute & slashing
- Deposer des disputes באמצעות `sorafs_manifest_stub capacity dispute` (בדיקות:
  `cargo test -p sorafs_car --test capacity_cli`) pour garder des payloads canoniques.
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) pour prouver que disputes et slashes rejouent de manière déterministe.
- Suivre `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture de preuves et l'escalade ; lier les approbations de strike au rapport de validation.

### כניסה של ספקים ובדיקות עשן למיון
- Regénérer les artefacts de déclaration/télémétrie avec `sorafs_manifest_stub capacity ...` et rejouer les tests CLI avant soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Soumettre via Torii (`/v1/sorafs/capacity/declare`) puis capturer `/v1/sorafs/capacity/state` et des captures Grafana. Suivre le flux de sortie dans `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiver les artefacts signnés et les outputs de réconciliation dans
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## תלות וסידור

1. Termins SF-2b (פוליטיק ד'הודטה) - השוק תלוי בתוקף של ספקים.
2. Implémenter la couche schéma + registry (ce doc) avant l'intégration Torii.
3. מסיים את צינור המדידה של תשלומים.
4. סיום הסיום: הפצה פעילה של אגרות פיילוט על פי ניהול יחידות חוץ למסגרות המדידות בהנחיה.

Le progrès doit être suivi dans la roadmap avec des références à ce document. Mettre à jour la מפת הדרכים une fois que chaque section majeure (סכימה, תוכנית שליטה, אינטגרציה, מדידה, תנועה של מחלוקות) תכונה הושלמה לחוק.