---
lang: fr
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Ensemble de 3 bancs

La suite de bancs Iroha 3 multiplie par deux les chemins chauds sur lesquels nous comptons pendant le jalonnement, frais
facturation, vérification des preuves, planification et points finaux de preuve. Il fonctionne comme un
Commande `xtask` avec fixations déterministes (graines fixes, matériel de clé fixe,
et charges utiles de requêtes stables) afin que les résultats soient reproductibles sur tous les hôtes.

## Exécuter la suite

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Drapeaux :

- `--iterations` contrôle les itérations par exemple de scénario (par défaut : 64).
- `--sample-count` répète chaque scénario pour calculer la médiane (par défaut : 5).
- `--json-out|--csv-out|--markdown-out` choisit les artefacts de sortie (tous facultatifs).
- `--threshold` compare les médianes aux limites de la ligne de base (définir `--no-threshold`
  sauter).
- `--flamegraph-hint` annote le rapport Markdown avec le `cargo flamegraph`
  commande pour profiler un scénario.

La colle CI réside dans `ci/i3_bench_suite.sh` et utilise par défaut les chemins ci-dessus ; ensemble
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` pour régler le temps d'exécution dans les soirées.

## Scénarios

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — débit du payeur ou du sponsor
  et le rejet du manque à gagner.
- `staking_bond` / `staking_slash` — file d'attente de liaison/déliaison avec et sans
  coupant.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  vérification de la signature sur les certificats de validation, les attestations JDG et le pont
  charges utiles de preuve.
- `commit_cert_assembly` — assemblage de résumé pour les certificats de validation.
- `access_scheduler` — planification d'ensembles d'accès prenant en compte les conflits.
- `torii_proof_endpoint` — Analyse du point final de preuve Axum + vérification aller-retour.

Chaque scénario enregistre les nanosecondes médianes par itération, le débit et un
compteur d'allocation déterministe pour des régressions rapides. Les seuils vivent dans
`benchmarks/i3/thresholds.json` ; il y a des limites lorsque le matériel change et
valider le nouvel artefact avec un rapport.

## Dépannage

- Épinglez la fréquence/le gouverneur du processeur lors de la collecte de preuves pour éviter les régressions bruyantes.
- Utilisez `--no-threshold` pour les exécutions exploratoires, puis réactivez-le une fois la ligne de base établie.
  rafraîchi.
- Pour profiler un seul scénario, définissez `--iterations 1` et réexécutez sous
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.