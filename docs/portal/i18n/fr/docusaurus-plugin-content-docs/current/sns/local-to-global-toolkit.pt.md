---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit d'enderecos Local -> Global

Cette page correspond à `docs/source/sns/local_to_global_toolkit.md` du mono-repo. Il a ajouté les aides de CLI et les runbooks requis pour l'élément de feuille de route **ADDR-5c**.

## Visa général

- `scripts/address_local_toolkit.sh` encapsule la CLI `iroha` pour produire :
  - `audit.json` -- dit estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- littéraire I105 (préféré) / compressé (`sora`) (seconde meilleure option) converti pour chaque sélecteur de domaine local.
- Combiner le script avec le tableau de bord d'ingest de enderecos (`dashboards/grafana/address_ingest.json`)
  et comme base d'Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour vérifier que le basculement Local-8 /
  Local-12 et sécurité. Observez les paineis de colisao Local-8 e Local-12 et les alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo` avant
  Promoteur mudancas de manifest.
- Consultez comme [Directives d'affichage des adresses](address-display-guidelines.md) e o
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) pour le contexte UX et la réponse aux incidents.

## Utilisation

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opcoes :

- `--format I105` pour `sora...` à la place de I105.
- `domainless output (default)` pour émettre des lettres sans domicile.
- `--audit-only` pour l'étape de conversation.
- `--allow-errors` pour continuer la réparation lorsque des lignes malformées apparaissent (même avec le comportement de la CLI).Le script écrit les chemins des artistes jusqu'à l'exécution finale. Anexe os dois arquivos ao
votre ticket de gestao de mudancas avec la capture d'écran de Grafana qui comprend zéro
détecte Local-8 et zéro colis Local-12 pour >=30 dias.

## Intégration CI

1. Rode o script em um job dédié et envie comme dit.
2. Bloqueie fusionne quando `audit.json` reporte les sélecteurs locaux (`domain.kind = local12`).
   aucune valeur n'est attribuée à `true` (donc à modifier pour `false` dans les clusters de développement/test pour le diagnostic
   régressions) et ajout
   `iroha tools address normalize` à CI pour les régressions
   falhem avant de commencer à produire.

Voir le document contenant plus de détails, les listes de contrôle des preuves et l'extrait de
notes de version qui vous permettent de les réutiliser pour annoncer la transition pour les clients.