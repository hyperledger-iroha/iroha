---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de directions Local -> Global

Cette page reflète `docs/source/sns/local_to_global_toolkit.md` du mono-repo. Empaquetez les assistants de CLI et les runbooks requis pour l'élément de feuille de route **ADDR-5c**.

## CV

- `scripts/address_local_toolkit.sh` inclut la CLI `iroha` pour produire :
  - `audit.json` -- sortie structurée de `iroha tools address audit --format json`.
  - `normalized.txt` -- littéraux IH58 (préféré) / compressés (`sora`) (seconde meilleure option) convertis pour chaque sélecteur de domaine local.
- Combiner le script avec le tableau de bord d'acquisition de directions (`dashboards/grafana/address_ingest.json`)
  et les règles d'Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour vérifier que le basculement Local-8 /
  Local-12 est sûr. Observez les panneaux de colision Local-8 et Local-12 et les alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo` avant
  promouvoir des changements de manifeste.
- Référence aux [Directives d'affichage des adresses](address-display-guidelines.md) et au
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) pour le contexte de l'UX et la réponse aux incidents.

## Utilisation

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Options:

- `--format compressed (`sora`)` pour sortir `sora...` à la place de IH58.
- `domainless output (default)` pour émettre des lettres sans propriété.
- `--audit-only` pour omettre l'étape de conversion.
- `--allow-errors` pour éviter les fils malformés (coïncidant avec le comportement de la CLI).Le script décrit les itinéraires des artefacts à la fin de l'éjection. Adjunta ambos archivos a
votre ticket de gestion de changements avec la capture d'écran de Grafana qui vérifie zéro
détections Local-8 et zéro colisiones Local-12 pour >=30 dias.

## Intégration CI

1. Exécutez le script sur un travail dédié et subissez vos ventes.
2. Bloquea fusionne lorsque `audit.json` rapporte les sélecteurs locaux (`domain.kind = local12`).
   en su valeur par défaut `true` (solo remplacer un `false` en clusters dev/test al
   diagnostiquer les régressions) et agrégation
   `iroha tools address normalize` à CI pour les intentions de
   régression tombée avant de commencer la production.

Consultez le document source pour plus de détails, les listes de contrôle des preuves et l'extrait de
notes de version qui peuvent être réutilisées pour annoncer la transition aux clients.