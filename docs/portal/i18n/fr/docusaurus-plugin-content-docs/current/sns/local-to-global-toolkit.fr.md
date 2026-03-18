---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit d'adresses Local -> Global

Cette page reflète `docs/source/sns/local_to_global_toolkit.md` du mono-repo. Elle regroupe les helpers CLI et les runbooks requis par l'item roadmap **ADDR-5c**.

## Apercu

- `scripts/address_local_toolkit.sh` encapsule la CLI `iroha` pour produire :
  - `audit.json` -- sortie structurée de `iroha tools address audit --format json`.
  - `normalized.txt` -- literaux I105 (préférer) / compressé (`sora`) (deuxième choix) convertis pour chaque sélecteur de domaine Local.
- Associez le script au tableau de bord d'ingest d'adresses (`dashboards/grafana/address_ingest.json`)
  et aux règles Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour prouver que le basculement Local-8 /
  Local-12 est sur. Surveillez les panneaux de collision Local-8 et Local-12 et les alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo` avant de
  promouvoir les changements de manifeste.
- Consultez les [Directives d'affichage des adresses](address-display-guidelines.md) et le
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) pour le contexte UX et la réponse aux incidents.

## Utilisation

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Possibilités :

- `--format I105` pour la sortie `sora...` au lieu de I105.
- `domainless output (default)` pour emettre des littéraux nus.
- `--audit-only` pour ignorer l'étape de conversion.
- `--allow-errors` pour continuer le scan lorsque des lignes mal formées apparaissent (correspondent au comportement de la CLI).Le script écrit les chemins des artefacts à la fin de l'exécution. Joignez les deux fichiers a
votre ticket de gestion du changement avec la capture Grafana qui prouve zéro
détections Local-8 et zéro collisions Local-12 pendant >=30 jours.

## Intégration CI

1. Lancez le script dans un travail dédié et téléchargez les sorties.
2. Bloquez les fusions lorsque `audit.json` signale des sélecteurs Local (`domain.kind = local12`).
   a sa valeur par défaut `true` (ne passer a `false` que sur les clusters dev/test lors du
   diagnostic de régressions) et ajouter
   `iroha tools address normalize` a CI pour que les régressions
   écho avant la production.

Voir le document source pour plus de détails, des checklists d'evidence et le snippet de
release notes que vous pouvez réutiliser pour annoncer le basculement aux clients.