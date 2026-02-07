---
lang: fr
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2026-01-03T18:07:57.096028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% d'intrinsèques ARMv8 SM3/SM4 par rapport aux implémentations Pure Rust
% Groupe de travail sur la cryptographie Iroha
% 2026-02-12

# Invite

> Vous êtes LLM agissant à titre d'expert-conseil auprès de l'équipe crypto Hyperledger Iroha.  
> Contexte :  
> - Hyperledger Iroha est une blockchain autorisée basée sur Rust où chaque validateur doit s'exécuter de manière déterministe afin que le consensus ne puisse pas diverger.  
> - Iroha utilise les primitives cryptographiques chinoises GM/T SM2 (signatures), SM3 (hachage) et SM4 (chiffrement par bloc) pour certains déploiements réglementaires.  
> - L'équipe livre deux implémentations SM3/SM4 dans la pile du validateur :  
> 1. Code scalaire pur Rust, découpé en bits et à temps constant qui s'exécute sur n'importe quel processeur.  
> 2. Noyaux accélérés ARMv8 NEON qui s'appuient sur les instructions facultatives `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` et `SM4EKEY` exposées sur les nouveaux serveurs Apple M-series et Arm. Processeurs.  
> - Le code accéléré est à l'origine de la détection des fonctionnalités d'exécution à l'aide des intrinsèques `core::arch::aarch64` ; le système doit éviter un comportement non déterministe lorsque les threads migrent entre les cœurs big.LITTLE ou lorsque les répliques sont construites avec différents indicateurs du compilateur.  
> Analyse demandée :  
> Comparez les implémentations intrinsèques d'ARMv8 avec les solutions de repli pures de Rust pour la vérification déterministe de la blockchain. Discutez des gains de débit/latence, des pièges du déterminisme (détection de fonctionnalités, cœurs hétérogènes, risque SIGILL, alignement, mélange des chemins d'exécution), des propriétés à temps constant et des garanties opérationnelles (tests, manifestes, télémétrie, documentation de l'opérateur) nécessaires pour maintenir tous les validateurs synchronisés même lorsque certains matériels prennent en charge les instructions et d'autres non.

# Résumé

Périphériques ARMv8-A qui exposent les options `SM3` (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) et `SM4` (`SM4E`, `SM4EKEY`) peuvent accélérer considérablement le hachage GM/T et les primitives de chiffrement par bloc. Cependant, l’exécution déterministe de la blockchain exige un contrôle strict sur la détection des fonctionnalités, la parité de secours et le comportement en temps constant. Les instructions suivantes expliquent comment les deux stratégies de mise en œuvre se comparent et ce que la pile Iroha doit appliquer.

# Comparaison des implémentations| Aspects | Intrinsèques ARMv8 (AArch64 Inline ASM/`core::arch::aarch64`) | Pure Rust (tranché en bits / sans table) |
|--------|-------------------------------------------------------------|--------------------------------------|
| Débit | Hachage SM3 3 à 5 fois plus rapide et SM4 ECB/CTR jusqu'à 8 fois plus rapide par cœur sur Apple série M et Neoverse V1 ; les gains diminuent lorsque la mémoire est limitée. | Débit de base lié par ALU scalaire et rotation ; bénéficie parfois des extensions SHA `aarch64` (via la vectorisation automatique du compilateur) mais est généralement en retard sur NEON d'un écart similaire de 3 à 8 ×. |
| Latence | Latence monobloc ~ 30 à 40 ns sur M2 avec éléments intrinsèques ; convient au hachage de messages courts et au cryptage de petits blocs dans les appels système. | 90 à 120 ns par bloc ; peut nécessiter un déroulement pour rester compétitif, ce qui augmente la pression du cache d'instructions. |
| Taille du code | Nécessite deux chemins de code (intrinsèques + scalaires) et un contrôle d'exécution ; chemin intrinsèque compact si vous utilisez `cfg(target_feature)`. | Chemin unique ; légèrement plus grand en raison de tables de planification manuelles mais pas de logique de déclenchement. |
| Déterminisme | Doit verrouiller la répartition du temps d'exécution sur un résultat déterministe, éviter les courses de sondage de fonctionnalités entre threads et épingler l'affinité du processeur si les cœurs hétérogènes diffèrent (par exemple, big.LITTLE). | Déterministe par défaut ; aucune détection de fonctionnalité d'exécution. |
| Posture à temps constant | L'unité matérielle est à temps constant pour les tours de base, mais le wrapper doit éviter la sélection dépendante du secret lors du repli ou du mélange des tables. | Entièrement contrôlé dans Rust ; temps constant assuré par construction (bit-slicing) s'il est codé correctement. |
| Portabilité | Nécessite `aarch64` + fonctionnalités optionnelles ; x86_64 et RISC-V reviennent automatiquement. | Fonctionne partout ; les performances dépendent des optimisations du compilateur. |

# Pièges de répartition lors de l'exécution

1. **Sondage de fonctionnalités non déterministe**
   - Problème : tester `is_aarch64_feature_detected!("sm4")` sur des SoC big.LITTLE hétérogènes peut donner des réponses différentes par cœur, et le vol de travail entre threads peut mélanger les chemins à l'intérieur d'un même bloc.
   - Atténuation : capturez la capacité matérielle exactement une fois lors de l'initialisation du nœud, diffusez via `OnceLock` et associez-la à l'affinité du processeur lors de l'exécution de noyaux accélérés à l'intérieur de la VM ou des caisses de chiffrement. Ne branchez jamais sur les indicateurs de fonctionnalités après le début du travail critique par consensus.

2. **Précision mitigée entre les répliques**
   - Problème : les nœuds construits avec différents compilateurs peuvent être en désaccord sur la disponibilité intrinsèque (activation à la compilation `target_feature=+sm4` vs détection à l'exécution). Si l'exécution passe par différents chemins de code, la synchronisation micro-architecturale peut s'infiltrer dans des délais d'attente basés sur la puissance ou dans des limiteurs de débit.
   - Atténuation : distribuez des profils de construction canoniques avec `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE` explicite, exigez un ordre de secours déterministe (par exemple, préférez scalaire à moins que la configuration n'active le matériel) et incluez un hachage de configuration dans les manifestes pour l'attestation.3. **Disponibilité des instructions sur Apple vs Linux**
   - Problème : Apple expose les instructions SM4 uniquement dans les dernières versions du silicium et du système d'exploitation ; Les distributions Linux peuvent corriger les noyaux pour les masquer en attendant les approbations d'exportation. S'appuyer sur des intrinsèques sans garde provoque des SIGILL.
   - Atténuation : portez via `std::arch::is_aarch64_feature_detected!`, détectez `SIGILL` lors des tests de fumée et traitez les intrinsèques manquants comme une solution de secours attendue (toujours déterministe).

4. **Découpage parallèle et classement de la mémoire**
   - Problème : les noyaux accélérés traitent souvent plusieurs blocs par itération ; l'utilisation de charges/stockages NEON avec des entrées non alignées peut provoquer des défauts ou nécessiter des correctifs d'alignement explicites lorsqu'ils sont alimentés par des tampons désérialisés Norito.
   - Atténuation : conservez les allocations alignées sur les blocs (par exemple, les multiples `SM4_BLOCK_SIZE` via les wrappers `aligned_alloc`), validez l'alignement dans les versions de débogage et revenez au scalaire en cas de mauvais alignement.

5. **Attaques d'empoisonnement du cache d'instructions**
   - Problème : les adversaires du consensus peuvent créer des charges de travail qui écrasent de minuscules lignes de cache I sur des cœurs plus faibles, élargissant ainsi la différence de latence entre les chemins accélérés et scalaires.
   - Atténuation : correction de la planification sur des tailles de fragments déterministes, remplissage des boucles pour éviter les branchements imprévisibles et inclusion de tests de régression sur microbench pour garantir que la gigue reste dans les fenêtres de tolérance.

# Recommandations de déploiement déterministe

- **Politique de compilation :** conservez le code accéléré derrière un indicateur de fonctionnalité (par exemple, `sm_accel_neon`) activé par défaut dans les versions, mais exigez un consentement explicite dans les configurations pour les réseaux de test jusqu'à ce que la couverture de parité soit mature.
- **Tests de parité de repli :** conservent des vecteurs dorés qui exécutent des chemins accélérés et scalaires dos à dos (workflow `sm_neon_check` actuel) ; étendre pour couvrir les modes SM3/SM4 GCM une fois que le support du fournisseur sera disponible.
- **Attestation de manifeste :** incluez la politique d'accélération (`hardware=sm-neon|scalar`) dans le manifeste Norito du nœud pour rendre la divergence détectable lors de l'admission des pairs.
- **Télémétrie :** émet des métriques comparant la latence par appel sur les deux chemins ; alerte si la divergence dépasse des seuils prédéterminés (par exemple, gigue > 5 %), signalant une éventuelle dérive matérielle.
- **Documentation :** gardez le guide de l'opérateur (`sm_operator_rollout.md`) à jour avec les instructions d'activation/désactivation des intrinsèques, et notez que le comportement déterministe est préservé quel que soit le chemin.

# Références

- `crates/iroha_crypto/src/sm.rs` — Hooks d'implémentation NEON vs scalaires.
- `.github/workflows/sm-neon-check.yml` — voie CI à NEON forcé assurant la parité.
- `docs/source/crypto/sm_program.md` — garde-corps à dégagement et portails performants.
- Manuel de référence de l'architecture Arm, Armv8-A, Section D13 (instructions SM3/SM4).
- GM/T 0002-2012, GM/T 0003-2012 — spécifications officielles SM3/SM4 pour les tests de comparaison.

## Invite autonome (Copier/Coller)> Vous êtes LLM agissant à titre d'expert-conseil auprès de l'équipe crypto Hyperledger Iroha.  
> Contexte : Hyperledger Iroha est une blockchain autorisée basée sur Rust qui nécessite une exécution déterministe entre les validateurs. La plateforme prend en charge la suite cryptographique chinoise GM/T SM2/SM3/SM4. Pour SM3 et SM4, la base de code fournit deux implémentations : (a) du code scalaire à temps constant découpé en bits Rust pur qui s'exécute partout, et (b) des noyaux accélérés ARMv8 NEON qui dépendent des instructions facultatives `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` et `SM4EKEY`. Les chemins accélérés sont activés via la détection des fonctionnalités d'exécution à l'aide de `core::arch::aarch64` ; ils ne doivent pas introduire de non-déterminisme lorsque les threads migrent à travers des cœurs big.LITTLE hétérogènes ou lorsque les réplicas sont construits avec différents indicateurs `target_feature`.  
> Tâche : comparer les implémentations intrinsèques avec les solutions de repli scalaires pour la vérification déterministe de la blockchain. Détaillez les différences de débit et de latence, énumérez les risques liés au déterminisme (détection de fonctionnalités, cœurs hétérogènes, comportement SIGILL, alignement, chemins d'exécution mixtes), commentez la posture en temps constant et recommandez des mesures de protection (stratégie de test, champs de manifeste/attestation, télémétrie, documentation de l'opérateur) qui garantissent que tous les validateurs restent synchronisés même si les capacités matérielles diffèrent.