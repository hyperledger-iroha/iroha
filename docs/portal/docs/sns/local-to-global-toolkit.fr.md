---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20dd2db9bb7af3c40b35154b927df8000cb8c0d6c8e6190e71fb3e6491403149
source_last_modified: "2025-12-19T22:33:53.968499+00:00"
translation_last_reviewed: 2026-01-01
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
- `--no-append-domain` pour emettre des literaux nus.
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
   `iroha tools address normalize --fail-on-warning --only-local` a CI pour que les regressions
   echouent avant la production.

Voir le document source pour plus de details, des checklists d'evidence et le snippet de
release notes que vous pouvez reutiliser pour annoncer le cutover aux clients.
