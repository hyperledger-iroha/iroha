---
lang: fr
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Constantes partagées de Poséidon Metal

Les noyaux métalliques, les noyaux CUDA, le prouveur Rust et chaque appareil SDK doivent partager
exactement les mêmes paramètres de Poséidon2 afin de conserver l'accélération matérielle
hachage déterministe. Ce document enregistre l'instantané canonique, comment
le régénérer et comment les pipelines GPU sont censés ingérer les données.

## Manifeste d'instantané

Les paramètres sont publiés sous forme de document `PoseidonSnapshot` RON. Les copies sont
conservé sous contrôle de version afin que les chaînes d'outils GPU et les SDK ne dépendent pas du temps de construction
génération de code.

| Chemin | Objectif | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | Instantané canonique généré à partir de `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` ; source de vérité pour les builds GPU. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Reflète l'instantané canonique afin que les tests unitaires Swift et le faisceau de fumée XCFramework chargent les mêmes constantes attendues par les noyaux Metal. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Les appareils Android/Kotlin partagent le même manifeste pour les tests de parité et de sérialisation. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Chaque consommateur doit vérifier le hachage avant de câbler les constantes dans un GPU
canalisation. Lorsque le manifeste change (nouveau jeu de paramètres ou profil), le SHA et
les miroirs en aval doivent être mis à jour de manière synchronisée.

## Régénération

Le manifeste est généré à partir des sources Rust en exécutant le `xtask`
aide. La commande écrit à la fois le fichier canonique et les miroirs du SDK :

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Utilisez `--constants <path>`/`--vectors <path>` pour remplacer les destinations ou
`--no-sdk-mirror` lors de la régénération uniquement de l'instantané canonique. L'assistant va
refléter les artefacts dans les arbres Swift et Android lorsque le drapeau est omis,
qui maintient les hachages alignés pour CI.

## Alimentation des builds Metal/CUDA

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` et
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` doit être régénéré à partir du
  se manifeste chaque fois que la table change.
- Les constantes arrondies et MDS sont mises en scène dans `MTLBuffer`/`__constant` contigus
  segments qui correspondent à la disposition du manifeste : `round_constants[round][state_width]`
  suivi de la matrice MDS 3x3.
- `fastpq_prover::poseidon_manifest()` charge et valide l'instantané à
  d'exécution (pendant l'échauffement de Metal) afin que les outils de diagnostic puissent affirmer que le
  les constantes du shader correspondent au hachage publié via
  `fastpq_prover::poseidon_manifest_sha256()`.
- Lecteurs de luminaires SDK (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) et
  les outils hors ligne Norito s'appuient sur le même manifeste, ce qui empêche le GPU uniquement
  fourchettes de paramètres.

## Validation

1. Après avoir régénéré le manifeste, exécutez `cargo test -p xtask` pour exercer le
   Tests unitaires de génération de luminaires Poséidon.
2. Enregistrez le nouveau SHA-256 dans ce document et dans tous les tableaux de bord qui surveillent
   Artefacts GPU.
3. Analyses `cargo test -p fastpq_prover poseidon_manifest_consistency`
   `poseidon2.metal` et `fastpq_cuda.cu` au moment de la construction et affirme que leur
   les constantes sérialisées correspondent au manifeste, en conservant les tables CUDA/Metal et
   l'instantané canonique en mode verrouillé.Garder le manifeste à côté des instructions de construction du GPU donne le Metal/CUDA
workflows une poignée de main déterministe : les noyaux sont libres d'optimiser leur mémoire
mise en page tant qu'ils ingèrent le blob de constantes partagées et exposent le hachage dans
télémétrie pour les contrôles de parité.