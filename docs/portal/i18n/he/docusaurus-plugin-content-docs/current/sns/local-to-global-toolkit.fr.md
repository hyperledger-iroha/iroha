---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ערכת כתובות מקומית -> גלובלית

Cette page reflete `docs/source/sns/local_to_global_toolkit.md` du mono-repo. כדי לקבץ מחדש את העוזרים CLI ואת ספרי ההפעלה דרושים מפת הדרכים של הפריט **ADDR-5c**.

## אפרקו

- `scripts/address_local_toolkit.sh` encapsule la CLI `iroha` עבור מוצרים:
  - `audit.json` -- מיון מבנה של `iroha tools address audit --format json`.
  - `normalized.txt` -- literaux i105 (עדיף) / דחוס (`sora`) (בחירה שנייה) convertis pour chaque selecteur de domaine Local.
- Associez le script au dashboard d'ingest d'adresses (`dashboards/grafana/address_ingest.json`)
  et aux regles Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour prouver que le cutover Local-8 /
  Local-12 est sur. Surveillez les panneaux de collision Local-8 et Local-12 et les alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo` avant de
  promouvoir les changements de manifest.
- Consultez les [הנחיות להצגת כתובת](address-display-guidelines.md) et le
  [פנקס מניפסט כתובות](../../../source/runbooks/address_manifest_ops.md) pour le contexte UX et la response aux incidents.

## שימוש

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

אפשרויות:

- `--format i105` pour la sortie `sora...` au lieu de i105.
- `domainless output (default)` pour emettre des literaux nus.
- `--audit-only` pour ignorer l'etape de conversion.
- `--allow-errors` pour continuer le scan quand des lignes mal formees apparaissent (correspond au comportement de la CLI).

התסריט ecrit les chemins des artefacts a la fin de l'execution. Joignez les deux fichiers א
votre ticket de gestion du changement avec la capture Grafana qui prouve zero
זיהוי התנגשויות מקומיות-8 ואפסות תליון מקומי-12 >=30 יו"ר.

## אינטגרציה CI

1. Lancez le script ב-un job dedie and uploadez lesies.
2. Bloquez les merges quand `audit.json` signale des selecteurs Local (`domain.kind = local12`).
   a sa valeur par defaut `true` (ne passer a `false` que sur les clusters dev/test lors du
   diagnostic de regressions) et ajoutez
   `iroha tools address normalize` a CI pour que les regressions
   echouent avant la production.

Voir le document source pour plus de details, des checklists d'evidence et le snippet de
הערות שחרור que vous pouvez reutiliser pour annoncer le cutover aux clients.