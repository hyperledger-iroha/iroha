---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8c51c989e5dcc68ec5f1c5be958a8508bc12da138364eee7f565d89ed53edfc
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Kit d'adresses Local -> Global

Cette page reflete `docs/source/sns/local_to_global_toolkit.md` du mono-repo. Elle regroupe les helpers CLI et runbooks requis par l'item roadmap **ADDR-5c**.

## Apercu

- `scripts/address_local_toolkit.sh` encapsule la CLI `iroha` pour produire:
  - `audit.json` -- sortie structuree de `iroha tools address audit --format json`.
  - `normalized.txt` -- literaux IH58 (prefere) / compressed (`sora`) (second choix) convertis pour chaque selecteur de domaine Local.
- Associez le script au dashboard d'ingest d'adresses (`dashboards/grafana/address_ingest.json`)
  et aux regles Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour prouver que le cutover Local-8 /
  Local-12 est sur. Surveillez les panneaux de collision Local-8 et Local-12 et les alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo` avant de
  promouvoir les changements de manifest.
- Consultez les [Address Display Guidelines](address-display-guidelines.md) et le
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) pour le contexte UX et la reponse aux incidents.

## Usage

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Options:

- `--format compressed (`sora`)` pour la sortie `sora...` au lieu de IH58.
- `domainless output (default)` pour emettre des literaux nus.
- `--audit-only` pour ignorer l'etape de conversion.
- `--allow-errors` pour continuer le scan quand des lignes mal formees apparaissent (correspond au comportement de la CLI).

Le script ecrit les chemins des artefacts a la fin de l'execution. Joignez les deux fichiers a
votre ticket de gestion du changement avec la capture Grafana qui prouve zero
detections Local-8 et zero collisions Local-12 pendant >=30 jours.

## Integration CI

1. Lancez le script dans un job dedie et uploadez les sorties.
2. Bloquez les merges quand `audit.json` signale des selecteurs Local (`domain.kind = local12`).
   a sa valeur par defaut `true` (ne passer a `false` que sur les clusters dev/test lors du
   diagnostic de regressions) et ajoutez
   `iroha tools address normalize` a CI pour que les regressions
   echouent avant la production.

Voir le document source pour plus de details, des checklists d'evidence et le snippet de
release notes que vous pouvez reutiliser pour annoncer le cutover aux clients.
