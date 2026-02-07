---
lang: fr
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Flux de travail de performances GOST

Cette note documente la manière dont nous suivons et appliquons l'enveloppe de performance pour le
Backend de signature TC26 GOST.

## Exécuté localement

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

En coulisses, les deux cibles appellent `scripts/gost_bench.sh`, qui :

1. Exécute `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. Exécute `gost_perf_check` par rapport à `target/criterion`, en vérifiant les médianes par rapport au
   référence enregistrée (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Injecte le résumé Markdown dans `$GITHUB_STEP_SUMMARY` lorsqu'il est disponible.

Pour actualiser la ligne de base après avoir approuvé une régression/amélioration, exécutez :

```bash
make gost-bench-update
```

ou directement :

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` exécute le banc + vérificateur, écrase le JSON de base et imprime
les nouvelles médianes. Validez toujours le JSON mis à jour avec l'enregistrement de décision dans
`crates/iroha_crypto/docs/gost_backend.md`.

### Médianes de référence actuelles

| Algorithme | Médiane (µs) |
|----------------------|-------------|
| ed25519 | 69,67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160,53 |

##CI

`.github/workflows/gost-perf.yml` utilise le même script et exécute également le garde de synchronisation duduct.
CI échoue lorsque la médiane mesurée dépasse la ligne de base de plus que la tolérance configurée
(20% par défaut) ou lorsque le chronomètre détecte une fuite, les régressions sont donc automatiquement captées.

## Sortie récapitulative

`gost_perf_check` imprime le tableau de comparaison localement et ajoute le même contenu à
`$GITHUB_STEP_SUMMARY`, donc les journaux de tâches CI et les résumés d'exécution partagent les mêmes numéros.