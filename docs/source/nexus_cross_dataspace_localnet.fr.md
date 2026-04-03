<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Preuve Localnet multi-espace de données

Ce runbook exécute la preuve d'intégration Nexus qui :

- démarre un réseau local à 4 homologues avec deux espaces de données privés restreints (`ds1`, `ds2`),
- achemine le trafic du compte vers chaque espace de données,
- crée un actif dans chaque espace de données,
- exécute le règlement des échanges atomiques dans les espaces de données dans les deux sens,
- prouve la sémantique de restauration en soumettant une jambe sous-financée et en vérifiant que les soldes restent inchangés.

Le test canonique est le suivant :
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Exécution rapide

Utilisez le script wrapper de la racine du référentiel :

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Comportement par défaut :

- exécute uniquement le test de preuve inter-espaces de données,
- ensembles `NORITO_SKIP_BINDINGS_SYNC=1`,
- ensembles `IROHA_TEST_SKIP_BUILD=1`,
- utilise `--test-threads=1`,
- passe `--nocapture`.

## Options utiles

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` conserve des répertoires de pairs temporaires (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) à des fins médico-légales.
- `--all-nexus` exécute `mod nexus::` (sous-ensemble d'intégration complet Nexus), pas seulement le test de validation.

## Porte CI

Assistant CI :

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Fixer une cible :

```bash
make check-nexus-cross-dataspace
```

Cette porte exécute le wrapper de preuve déterministe et fait échouer le travail si l'attaque atomique inter-espaces de données
le scénario d’échange régresse.

## Commandes équivalentes manuelles

Test de preuve ciblé :

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Sous-ensemble Nexus complet :

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Signaux de preuve attendus- Le test réussit.
- Un avertissement attendu apparaît pour la jambe de règlement sous-financée intentionnellement défaillante :
  `settlement leg requires 10000 but only ... is available`.
- Les assertions de solde final réussissent après :
  - swap à terme réussi,
  - reverse swap réussi,
  - échec du swap sous-financé (annulation des soldes inchangés).

## Instantané de validation actuel

Au **19 février 2026**, ce flux de travail s'est déroulé avec :

- test ciblé : `1 passed; 0 failed`,
- sous-ensemble Nexus complet : `24 passed; 0 failed`.