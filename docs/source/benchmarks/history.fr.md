---
lang: fr
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2026-01-03T18:08:00.425813+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Historique de capture de référence GPU (FASTPQ WP5-B)

Ce fichier est généré par `python3 scripts/fastpq/update_benchmark_history.py`.
Il satisfait au livrable FASTPQ Stage 7 WP5-B en suivant chaque GPU enveloppé
artefact de référence, le manifeste du microbanc Poséidon et les balayages auxiliaires sous
`benchmarks/`. Mettez à jour les captures sous-jacentes et réexécutez le script chaque fois qu'un nouveau
les terres groupées ou la télémétrie ont besoin de nouvelles preuves.

## Portée et processus de mise à jour

- Produire ou envelopper de nouvelles captures GPU (via `scripts/fastpq/wrap_benchmark.py`),
  ajoutez-les à la matrice de capture et réexécutez ce générateur pour actualiser le
  tableaux.
- Lorsque les données du microbench Poséidon sont présentes, exportez-les avec
  `scripts/fastpq/export_poseidon_microbench.py` et reconstruisez le manifeste à l'aide de
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Enregistrez les balayages de seuil Merkle en stockant leurs sorties JSON sous
  `benchmarks/merkle_threshold/` ; ce générateur liste les fichiers connus donc audite
  peut comparer la disponibilité du CPU et du GPU.

## Benchmarks GPU FASTPQ Stage 7

| Paquet | Back-end | Mode | Moteur GPU | GPU disponible | Classe d'appareil | GPU | LDE ms (CPU/GPU/SU) | Poséidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|---------|----------------------|-------------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | GPU | cuda-sm80 | oui | xeon-rtx | NVIDIA RTX 6000 Ada | 1512,9/880,7/1,72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | métal | GPU | aucun | oui | pomme-m4 | GPU Apple 40 cœurs | 785,6/735,6/1,07 | 1803,8/1897,5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | métal | GPU | métal | oui | pomme-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | métal | GPU | métal | oui | pomme-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939,5/4083,3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | métal | GPU | métal | oui | pomme-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939,5/4083,3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | ouvrircl | GPU | ouvrircl | oui | néoverse-mi300 | AMD Instinct MI300A | 4518,5/688,9/6,56 | 2780.4/905.6/3.07 |

> Colonnes : `Backend` est dérivé du nom du bundle ; `Mode`/`GPU backend`/`GPU available`
> sont copiés à partir du bloc `benchmarks` encapsulé pour exposer les replis du processeur ou le GPU manquant
> découverte (par exemple, `gpu_backend=none` malgré `Mode=gpu`). SU = taux d'accélération (CPU/GPU).

## Instantanés du microbanc Poséidon

`benchmarks/poseidon/manifest.json` regroupe le Poséidon par défaut et scalaire
exécutions de microbench exportées à partir de chaque bundle Metal. Le tableau ci-dessous est actualisé par
le script du générateur, afin que les examens de CI et de gouvernance puissent différer les accélérations historiques
sans décompresser les rapports FASTPQ enveloppés.

| Résumé | Paquet | Horodatage | MS par défaut | MS scalaire | Accélération |
|---------|--------|-----------|------------|---------------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1h00 |

## Balayages de seuil MerkleCaptures de référence recueillies via
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
vivre sous `benchmarks/merkle_threshold/`. Les entrées de liste indiquent si l'hôte
Appareils métalliques exposés lors du balayage ; Les captures compatibles GPU doivent signaler
`metal_available=true`.

-`benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
-`benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
-`benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

La capture Apple Silicon (`takemiyacStudio.lan_25.0.0_arm64`) est la référence canonique du GPU utilisée dans `docs/source/benchmarks.md` ; les entrées macOS 14 restent comme références CPU uniquement pour les environnements qui ne peuvent pas exposer les appareils Metal.

## instantanés d'utilisation des lignes

Les décodages de témoins capturés via `scripts/fastpq/check_row_usage.py` prouvent le transfert
efficacité des lignes du gadget. Conservez les artefacts JSON sous `artifacts/fastpq_benchmarks/`
et ce générateur résumera les ratios de transfert enregistrés pour les auditeurs.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — lots = 2, rapport de transfert moyen = 0,629 (min = 0,625, max = 0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — lots = 2, rapport de transfert moyen = 0,619 (min = 0,613, max = 0,625)