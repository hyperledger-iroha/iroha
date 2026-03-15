---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-rollout-checklist
כותרת: רשימת רשימת השקה של Registre chunker SoraFS
sidebar_label: נתח השקת רשימת תשובות
תיאור: תוכנית הגלישה pas à pas pour les mises à jour du registre chunker.
---

:::הערה מקור קנוניק
Reflète `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Gardez les deux copies Syncées jusqu'à la retraite complète du set Sphinx hérité.
:::

# רשימת רשימת השקה של רישום SoraFS

Cette checklist détaille les étapes nécessaires pour promouvoir un nouveau profil
chunker ou un bundle d'admission fournisseur de la revue à la production after
אשרור תעודת הממשלה.

> **Portée :** S'applique à toutes les releases qui modifient
> `sorafs_manifest::chunker_registry`, les envelopes d'admission fournisseurs ou
> les bundles de fixtures canoniques (`fixtures/sorafs_chunker/*`).

## 1. אימות קדם

1. Regénérez les fixtures et vérifiez le déterminisme :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmez que les hashes de déterminisme dans
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le rapport de profil
   רלוונטי) correspondent aux artefacts régénérés.
3. Assurez-vous que `sorafs_manifest::chunker_registry` הידור avec
   `ensure_charter_compliance()` באנגלית:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Mettez à jour le dossier de proposition:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrée de minutes du conseil sous `docs/source/sorafs/council_minutes_*.md`
   - Rapport de déterminisme

## 2. ניהול אימות

1. Presentez le rapport du Tooling Working Group et le digest de la proposition
   פאנל תשתיות הפרלמנט של au Sora.
2. Enregistrez les détails d'approvation dans
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publiez l'envelope signnée par le Parlement à côté des fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Vérifiez que l'envelope est נגיש דרך le helper de fetch de gouvernance:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. שלב השקה

Référez-vous au [בימוי מניפסט של ספר הפעלה](./staging-manifest-playbook) pour une
פרוצדורה מפורטת.

1. Déployez Torii avec Discovery `torii.sorafs` פעיל et l'application de
   l'admission activeé (`enforce_admission = true`).
2. Poussez les envelopes d'admission fournisseurs approuvées dans le répertoire
   de registre staging référencé par `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifiez que les פרסומות של ספקיות הן פרואגנט דרך l'API Discovery:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. תרגיל נקודות קצה מניפסט/תוכנית עם כותרות ניהול:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirmez que les Dashboards de télémétrie (`torii_sorafs_*`) et les règles
   דובר חדש ללא שגיאות.

## 4. השקת ייצור

1. Répétez les étapes de staging sur les nœuds Torii de production.
2. Annoncez la fenêtre d'activation (תאריך/היור, תקופת החסד, תוכנית החזרה לאחור)
   aux canaux opérateurs et SDK.
3. מיזוג תוכן יחסי ציבור לשחרור:
   - מתקנים et envelope mises à jour
   - שינויים בתיעוד (Reférences à la Charte, Rapport de déterminisme)
   - רענון מפת דרכים/סטטוס
4. Taguez la release et archivez les artefacts signnés pour la provenance.

## 5. ביקורת לאחר ההשקה1. משחקי הגמר של Capturez les métriques (מחץ גילוי, אחזור של הצלחה,
   histogrammes d'erreurs) 24 שעות לפני ההשקה.
2. Mettez à jour `status.md` avec un bref résumé et un lien vers le rapport de déterminisme.
3. Consignez les tâches de suivi (לדוגמה Guidance d'authoring de profils) dans
   `roadmap.md`.