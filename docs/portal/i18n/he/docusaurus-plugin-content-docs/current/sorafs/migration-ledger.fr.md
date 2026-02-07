---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Registre de migration SoraFS
תיאור: Journal des changements canonique qui suit chaque jalon de migration, les responsables et les suivis requis.
---

> Adapte de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Registre de migration SoraFS

הרשם נציג את יומן ההגירות לכידת ב-RFC d'architecture
SoraFS. Les entrees sont groupees par jalon et inindiquent la fenetre יעיל,
les equipes impactees et les actions requises. Les mises a jour du plan de
הגירה DOIVENT משנה cette page et le RFC
(`docs/source/sorafs_architecture_rfc.md`) pour garder les consommateurs en aval
מתיישר.

| ג'לון | Fentre יעיל | קורות חיים לשינוי | מכשירי השפעה | פעולות | סטטוט |
|-------|----------------|------------------------|----------------|--------|--------|
| M1 | Semaines 7–12 | Le CI להטיל את מכשירי הקבע; les preuves d'alias sont disponibles in staging; le tooling expose des flags d'attente explicites. | מסמכים, אחסון, ממשל | S'assurer que les fixtures נחתם חתומים, registrer les alias dans le registry de staging, mettre a jour les checklists de release with l'exigence `--car-digest/--root-cid`. | ⏳ En attente |

Les minutes du plan de control de governance qui referencent ces jalons vivent sous
`docs/source/sorafs/`. Les equipes doivent ajouter des puces datees sous chaque ligne
lorsque des evenements notables surviennent (לדוגמה: nouveaux enregistrements d'alias,
retrospectives d'incidents du registry) afin de fournir une trace auditable.

## מתגעגע ליום האחרון

- 2025-11-01 - Diffusion de `migration_roadmap.md` au conseil de gouvernance et aux lists
  מפעילים יוצקים עדכון; en attente de validation lors de la prochaine session du
  קונסיל (ר': suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 - L'ISI d'enregistrement du Pin Registry applique desormais la validation
  partagee chunker/politique via les helpers `sorafs_manifest`, gardant les chemins
  על השרשרת מיישרת עם בדיקות Torii.
- 2026-02-13 - Ajout des phases de rollout de provider advert (R0–R3) au registre et
  פרסום של לוחות מחוונים et de la guidance operator associations
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).