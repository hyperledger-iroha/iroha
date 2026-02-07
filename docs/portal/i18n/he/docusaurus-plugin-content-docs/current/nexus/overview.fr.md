---
lang: he
direction: rtl
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-overview
כותרת: Apercu de Sora Nexus
תיאור: קורות חיים על רמה גבוהה של ארכיטקטורה Iroha 3 (Sora Nexus) עם נקודות נקודתיות versus les docs canoniques du mono-repo.
---

Nexus (Iroha 3) etend Iroha 2 עם ביצוע רב-נתיבי, des espaces de donnees cadres par la governance et des outils partages sur chaque SDK. Cette page reflete le nouveau brief `docs/source/nexus_overview.md` dans le mono-repo afin que les lecteurs du portail comprennent rapidement comment les pieces de l'architecture s'emboitent.

## Lignes de version

- **Iroha 2** - deploiements auto-heberges pour consortiums ou reseaux prives.
- **Iroha 3 / Sora Nexus** - רשמי ציבורי רב-נתיבי או למפעילי רישום של ספייס דה דונה (DS) et heritent d'outils partages de governance, reglement et observabilite.
- סביבת העבודה של Les deux lignes depuis le meme (IVM + כלי כלים Kotodama), SDK לתיקון, les mises a jour ABI et les fixtures Norito מכשירי restent. Les operators telechargent l'archive `iroha3-<version>-<os>.tar.zst` pour rejoindre Nexus ; reportez-vous a `docs/source/sora_nexus_operator_onboarding.md` pour la checklist plein ecran.

## גושי בנייה| קומפוזיטור | קורות חיים | Points du portail |
|--------|--------|----------------|
| Espace de donnees (DS) | Domaine d'execution/stockage defini par la governance qui possede une ou plusieurs lanes, declare des ensembles de validateurs, la classe de confidentialite et la politique de frais + DA. | Voir [Nexus spec](./nexus-spec) pour le schema du manifeste. |
| ליין | Shard deterministe d'execution; emet des engagements que l'anneau NPoS global ordonne. Les classes de lane כוללים `default_public`, `public_custom`, `private_permissioned` ו-`hybrid_confidential`. | Le [modele de lane](./nexus-lane-model) ללכוד את הגיאומטריה, את הקידומות של ה-stockage ואת השמירה. |
| תוכנית מעבר | זיהויים של מציין מיקום, שלבי מסלול ואריזה פרופיל כפול הערה מפורטת לפריצות חד-נתיב evolution vers Nexus. | Les [notes de transition](./nexus-transition-notes) תיעוד של שלב ההגירה. |
| ספריית החלל | ניגוד הרישום qui stocke les manifestes + גרסאות DS. Les operators concilient les entrees du catalogue avec ce ce repertoire avant de rejoindre. | Le suivi des diffs de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. |
| קטלוג הנתיבים | La section de configuration `[nexus]` map les IDs de lane vers des alias, politiques de routage et seuils DA. `irohad --sora --config ... --trace-config` מחליטים לקטלוג החלטה על ביקורת. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour le parcours CLI. |
| נתיב תקנון | תזמורת העברות XOR qui connecte des lanes CBDC privates aux lanes de liquidite publiques. | `docs/source/cbdc_lane_playbook.md` פרטים נוספים על הפוליטיקה והטלמטריה. |
| Telemetrie/SLOs | Tableaux de bord + alertes sous `dashboards/grafana/nexus_*.json` capturent la hauteur des lanes, le backlog DA, la latence de reglement et la profondeur de la file de governance. | Le [plan de remediation de telemetrie](./nexus-telemetry-remediation) detaille les tableaux de bord, alertes et preuves d'audit. |

## פריסה מיידית| שלב | פוקוס | קריטריונים דה מיון |
|-------|-------|-------------|
| N0 - Beta fermee | רשם gere par le conseil (`.sora`), מדריך הפעלה למטוס, קטלוג הנתיבים הסטטי. | מניב סימני DS + פסציות דה שלטון חוזרות. |
| N1 - Lancement public | Ajoute les suffixes `.nexus`, les encheres, un registrar in libre-service, le cablage de reglement XOR. | מבחנים של פותר/שער סנכרון, טבלאות של פיוס דה-פקטורציה, תרגילי משפט. |
| N2 - הרחבה | Introduit `.dao`, APIs revendeurs, analytique, portail de litiges, scorecards de stewards. | חפצי אומנות של גרסאות תואמות, ערכת כלים של חבר מושבעים של פוליטיקה בשגרה, יחסי שקיפות של הטרסור. |
| Porte NX-12/13/14 | Le moteur de conformite, les dashboards de telemetrie ו-la documentation doivent sortir ensemble avant les pilotes partenaires. | [סקירה כללית של Nexus](./nexus-overview) + [Nexus פעולות](./nexus-operations) מפרסמים, כבלים של לוחות מחוונים, מוטur de politiques fusionne. |

## אחריות מפעילים

1. **היגיינה של תצורה** - gardez `config/config.toml` סינכרון עם קטלוג מפרסמים של נתיבים ומרחבי נתונים ; archivez la sortie `--trace-config` avec chaque ticket de release.
2. **Suivi des manifestes** - conciliez les entrees du catalog avec le dernier bundle Space Directory avant de rejoindre ou de mettre a niveau les noeuds.
3. **Couverture telemetrie** - חשיפת לוחות מחוונים `nexus_lanes.json`, `nexus_settlement.json` et ceux lies aux SDK ; cablez les alertes a PagerDuty et realisez des revues trimestrielles selon le plan de remediation de telemetrie.
4. **Signalement d'incidents** - suivez la matrice de severite dans [Nexus פעולות](./nexus-operations) et deposez les RCAs sous cinq jours ouvrables.
5. **הכנה לממשל** - אסיסטיז aux votes du conseil Nexus impactant vos lanes et repetez les הוראות ה-rollback chaque trimestre (suivi via `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- Apercu canonique : `docs/source/nexus_overview.md`
- פירוט מפרט: [./nexus-spec](./nexus-spec)
- Geometrie des lanes: [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediation de telemetrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- פעולות Runbook : [./nexus-operations](./nexus-operations)
- מדריך עלייה למטוס: `docs/source/sora_nexus_operator_onboarding.md`