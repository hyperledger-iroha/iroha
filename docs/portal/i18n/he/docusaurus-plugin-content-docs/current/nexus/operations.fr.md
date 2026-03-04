---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operations
כותרת: Runbook operations Nexus
תיאור: קורות חיים מבצעים את השטח של מפעיל זרימת עבודה Nexus, חומר רפלט `docs/source/nexus_operations.md`.
---

Utilisez cette page comme compagnon de référence rapide de `docs/source/nexus_operations.md`. Elle condense la checklist operationnelle, les points de contrôle de gestion du changement et les exigences de couverture télémétrie que les opérateurs Nexus doivent suivre.

## רשימת רשימת מחזור

| Étape | פעולות | Preuves |
|-------|--------|--------|
| פרה-וול | בדוק את ה-hash/חתימות לשחרור, אישור `profile = "iroha3"` ומכינים את דגמי התצורה. | Sortie de `scripts/select_release_profile.py`, journal de checksum, bundle de manifestes signné. |
| Alignement du catalog | Mettre à jour le catalog `[nexus]`, la politique de routage et les seuils DA selon le manifeste émis par le conseil, puis capturer `--trace-config`. | Sortie `irohad --sora --config ... --trace-config` סטוק עם כרטיס כניסה למטוס. |
| עשן וחתך | Lancer `irohad --sora --config ... --trace-config`, מוציא לפועל את ה-CLI (`FindNetworkStatus`), תקף ליצוא של טלמטרים ודרישות לכניסה. | Log de smoke-test + אישור Alertmanager. |
| יציב משטר | לוחות מחוונים/התראות של סוררים, תוכניות הפעלה/סנכרון/ספרי הפעלה lorsque les manifestes changent. | Minutes de revue trimestrielle, לוכדת לוחות מחוונים, תעודות מזהות של כרטיסים לסיבוב. |

L'onboarding détaillé (replacement de clés, modèles de routage, étapes de profil de release) reste dans `docs/source/sora_nexus_operator_onboarding.md`.

## Gestion du change

1. **Mises à jour de release** - suivre les annonces dans `status.md`/`roadmap.md` ; joindre la checklist d'onboarding à chaque PR de release.
2. **Changements de manifestes de lane** - vérifier les bundles signnés du Space Directory et les archiver sous `docs/source/project_tracker/nexus_config_deltas/`.
3. ** הגדרות תצורה** - שינוי של `config/config.toml` נחוץ ללא כרטיס référencant lane/data-space. שומר אחד העתקת ביטול תצורה יעילה lors des des joins/upgrade de noeuds.
4. **Exercices de rollback** - répéter trimestriellement les procédures stop/restore/smoke ; consigner les résultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Appropations conformité** - les lanes privées/CBDC doivent obtenir un feu vert conformité avant de modifier la politique DA ou les knobs de redaction de télémétrie (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLOs- לוחות מחוונים: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, בתוספת מפרט SDK של des vues (בדוגמה `android_operator_console.json`).
- התראות: `dashboards/alerts/nexus_audit_rules.yml` et règles de transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- מטרות למעקב:
  - `nexus_lane_height{lane_id}` - alerter en cas d'absence de progression pendant trois חריצי.
  - `nexus_da_backlog_chunks{lane_id}` - alerter au-dessus des seuils par lane (par défaut 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - התראה quand le P99 dépasse 900 ms (ציבורי) או 1200 ms (פרטי).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerter si le ratio d'erreur à 5 דקות dépasse 2%.
  - `telemetry_redaction_override_total` - סיוו 2 מיד; assurer des tickets conformité pour les overrides.
- Exécuter la checklist de remédiation télémétrie dans le [plan de remédiation télémétrie Nexus](./nexus-telemetry-remediation) au moins trimestriellement et joindre le formulaire rempli aux notes de revue opérations.

## מטריצה לאירוע

| גרוויטה | הגדרה | תגובה |
|--------|------------|--------|
| סב 1 | מרחב נתונים של בריכת ד'בידוד, הסדר התיישבות >15 דקות, או שחיתות בהצבעה לממשל. | Alerter Nexus Primary + Release Engineering + Compliance, geler l'admission, collecter les artefacts, comms publiser <=60 min, RCA <=5 jours ouvrés. |
| סב' 2 | הפרה של SLA de backlog de lane, angle mort télémétrie>30 דקות, השקה של manifeste échoué. | Alerter Nexus Primary + SRE, atténuer <=4 h, déposer des suivis sous 2 jours ouvrés. |
| סוו 3 | Dérive non bloquante (מסמכים, התראות). | רשום dans le tracker, planifier le correctif dans le sprint. |

Les tickets d'incident doivent reregistrer les IDs de lane/data-space affectés, les hashes de manifeste, la chronologie, les métriques/logs de support et les tâches/propriétaires de suivi.

## Archive de preuves

- חבילות/גילויים/ייצוא סטוקר de télémétrie sous `artifacts/nexus/<lane>/<date>/`.
- Configs Conserver expurgées + sortie `--trace-config` pour chaque release.
- Joindre minutes du conseil + décisions signnées lorsque des changements de config ou manifeste sont appliqués.
- שומר תמונות Snapshot Prometheus hebdomadaires des métriques Nexus תליון 12 חודשים.
- Enregistrer les modifications du runbook dans `docs/source/project_tracker/nexus_config_deltas/README.md` pour que les auditeurs sachent quand les responsabilités ont changé.

## Matériel lié

- Vue d'ensemble: [סקירה כללית של Nexus](./nexus-overview)
- מפרט: [מפרט Nexus](./nexus-spec)
- Géométrie des lanes: [דגם נתיב Nexus](./nexus-lane-model)
- Transition et shims de routage: [Nexus הערות מעבר](./nexus-transition-notes)
- מפעיל הפעלה: [הכנסת מפעיל סורה Nexus](./nexus-operator-onboarding)
- טלמטריה לתיקון : [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation)