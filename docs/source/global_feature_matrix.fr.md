---
lang: fr
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a406b7656a87bb1469444db1cc2d2d5922f16660b53cc7eaef5b838199127e8
source_last_modified: "2026-01-23T20:16:38.056405+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Matrice des fonctionnalités globales

Légende : `◉` entièrement implémenté · `○` en grande partie implémenté · `▲` partiellement implémenté · L'implémentation `△` vient de démarrer · `✖︎` n'a pas démarré

## Consensus et réseautage

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Prise en charge K/r multi-collecteurs et premiers certificats d'engagement gagnés | ◉ | Sélection déterministe des collecteurs, répartition redondante, paramètres K/r en chaîne et acceptation du premier certificat de validation valide livré avec les tests. | statut.md:255; statut.md:314 |
| Attente du stimulateur cardiaque, plancher RTT, gigue déterministe | ◉ | Minuteries configurables avec bande de gigue câblée via la configuration, la télémétrie et la documentation. | statut.md:251 |
| NEW_VIEW contrôle et suivi du contrôle qualité le plus élevé | ◉ | Le flux de contrôle porte NEW_VIEW/Evidence, le QC le plus élevé adopte de manière monotone, les empreintes digitales calculées par les gardes de poignée de main. | statut.md:210 |
| suivi des preuves de disponibilité (consultatif) | ◉ | Preuve de disponibilité émise et suivie ; la validation ne détermine pas la disponibilité dans la v1. | status.md:dernier |
| Diffusion fiable (transport de charge utile DA) | ◉ | Le flux de messages RBC (Init/Chunk/Ready/Deliver) est activé lorsque `da_enabled=true` comme chemin de transport/récupération ; les preuves de disponibilité sont suivies (consultatives) tandis que la validation se déroule de manière indépendante. | status.md:dernier |
| Valider la liaison racine-état QC | ◉ | Les QC de validation portent `parent_state_root`/`post_state_root` ; il n'y a pas de porte d'exécution-QC séparée. | status.md:dernier |
| Propagation des preuves et points finaux d'audit | ◉ | ControlFlow::Evidence, les paramètres de preuve Torii et les tests négatifs obtenus. | statut.md:176; statut.md:760-761 |
| Télémétrie RBC, état de préparation/mesures fournies | ◉ | Points de terminaison `/v1/sumeragi/rbc*` et compteurs/histogramme de télémétrie disponibles pour les opérateurs. | statut.md:283-284 ; statut.md:772 |
| Annonce des paramètres de consensus et vérification de la topologie | ◉ | Les nœuds diffusent `(collectors_k, redundant_send_r)` et valident l’égalité entre pairs. | statut.md:255 |
| Rotation autorisée basée sur le PRF | ◉ | La sélection de leader/collecteur autorisée utilise la graine PRF + la hauteur/vue sur la liste canonique ; La rotation du hachage précédent reste un assistant hérité. | status.md:dernier |

## Pipeline, Kura et État| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Quarantine lane caps & telemetry | ◉ | Boutons de configuration, gestion déterministe des débordements et compteurs de télémétrie implémentés. | statut.md:263 |
| Bouton de pool de travailleurs de pipeline | ◉ | `[pipeline].workers` enfilé via l'initialisation d'état avec des tests d'analyse d'environnement. | statut.md:264 |
| Voie de requête d'instantané (curseurs stockés/éphémères) | ◉ | Mode curseur stocké avec intégration Torii et blocage des pools de tâches. | statut.md:265; statut.md:371; statut.md:501 |
| Static DAG fingerprint recovery sidecars | ◉ | Sidecars stockés dans Kura, validés au démarrage, alertes émises en cas de non-concordance. | statut.md:106; statut.md:349 |
| Kura bloc magasin décodage de hachage durcissement | ◉ | Les lectures de hachage sont passées à la gestion brute de 32 octets avec des tests aller-retour indépendants de Norito. | statut.md:608; statut.md:668 |
| Norito télémétrie adaptative pour codecs | ◉ | Métriques de sélection AoS vs NCB ajoutées à Norito. | statut.md:156 |
| Requêtes WSV d'instantané via Torii | ◉ | La voie de requête d’instantané Torii utilise un pool de tâches bloquant et une sémantique déterministe. | statut.md:501 |
| Déclencher le chaînage d'exécution d'un appel | ◉ | Les données déclenchent la chaîne immédiatement après l’exécution de l’appel avec un ordre déterministe. | statut.md:668 |

## Norito Sérialisation et outillage

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|----------|
| Norito JSON migration (workspace) | ◉ | Serde retiré de la production ; inventaire + garde-corps conservent l'espace de travail Norito uniquement. | statut.md:112; statut.md:124 |
| Liste de refus Serde et garde-corps CI | ◉ | Les workflows/scripts de protection empêchent une nouvelle utilisation directe de Serde dans l'espace de travail. | statut.md:218 |
| Codecs Norito et tests AoS/NCB | ◉ | Ajout des Goldens AoS/NCB, des tests de troncature et de la synchronisation des documents. | statut.md:140-147 ; statut.md:149-150 ; statut.md:332; statut.md:666 |
| Norito outillage de matrice de fonctionnalités | ◉ | `scripts/run_norito_feature_matrix.sh` prend en charge les tests de fumée en aval ; CI couvre les combos packed-seq/struct. | statut.md:146; statut.md:152 |
| Norito language bindings (Python/Java) | ◉ | Codecs Python et Java Norito maintenus avec des scripts de synchronisation. | statut.md:74; statut.md:81 |
| Norito Classificateurs structurels SIMD étape 1 | ◉ | Classificateurs NEON/AVX2 étape 1 avec Goldens cross-arch et tests de corpus randomisés. | statut.md:241 |

## Governance & Runtime Upgrades| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Runtime upgrade admission (ABI gating) | ◉ | Ensemble ABI actif appliqué à l'admission avec des erreurs et des tests structurés. | statut.md:196 |
| Protected namespace deploy gating | ▲ | Deploy metadata requirements and gating wired; politique/UX toujours en évolution. | statut.md:171 |
| Torii governance read endpoints | ◉ | `/v1/gov/*` lit les API acheminées avec des tests de routeur. | statut.md:212 |
| Verifying-key registry lifecycle & events | ◉ | Enregistrement/mise à jour/obsolète VK, événements, filtres CLI et sémantique de rétention implémentés. | statut.md:236-239 ; statut.md:595; statut.md:603 |

## Infrastructure sans connaissance

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Attachment storage APIs | ◉ | Points de terminaison de pièce jointe `POST/GET/LIST/DELETE` avec identifiants et tests déterministes. | statut.md:231 |
| Background prover worker & report TTL | ▲ | Prover stub behind feature flag; TTL GC and config knobs wired; pipeline complet en attente. | statut.md:212; statut.md:233 |
| Liaison de hachage d'enveloppe dans CoreHost | ◉ | Vérifiez les hachages d'enveloppe liés via CoreHost et exposés via des impulsions d'audit. | statut.md:250 |
| Shielded root history gating | ◉ | Instantanés racine intégrés à CoreHost avec un historique limité et une configuration racine vide. | statut.md:303 |
| Exécution du scrutin ZK et verrouillages de gouvernance | ○ | Dérivation d'annuleur, mises à jour de verrouillage, bascules de vérification mises en œuvre ; cycle de vie complet encore en cours de maturation. | statut.md:126-128 ; statut.md:194-195 |
| Pré-vérification et déduplication des pièces jointes de preuve | ◉ | La cohérence des balises back-end, la déduplication et les enregistrements de preuve ont persisté avant l'exécution. | statut.md:348; statut.md:602 |
| ZK Torii point de terminaison de récupération de preuve | ◉ | `/v1/zk/proof/{backend}/{hash}` expose les enregistrements de preuve (statut, hauteur, vk_ref/commitment). | statut.md:94 |

## Intégration IVM et Kotodama| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Appel système CoreHost → Pont ISI | ○ | Décodage TLV du pointeur et mise en file d'attente des appels système opérationnels ; écarts de couverture/tests de parité prévus. | statut.md:299-307 ; statut.md:477-486 |
| Constructeurs de pointeurs et intégrés au domaine | ◉ | Les composants intégrés Kotodama émettent des TLV et SCALL typés Norito, avec des tests et des documents IR/e2e. | statut.md:299-301 |
| Validation stricte Pointer-ABI et synchronisation des documents | ◉ | Politique TLV appliquée sur l'hôte/IVM avec des tests dorés et des documents générés. | statut.md:227; statut.md:317; statut.md:344; statut.md:366; statut.md:527 |
| Contrôle des appels système ZK via CoreHost | ◉ | Les files d'attente per-op contrôlent les enveloppes vérifiées et appliquent la correspondance de hachage avant l'exécution d'ISI. | crates/iroha_core/src/smartcontracts/ivm/host.rs:213; crates/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama pointeur-ABI docs & grammaire | ◉ | Grammaire/docs synchronisés avec les constructeurs en direct et les mappages SCALL. | statut.md:299-301 |
| Moteur basé sur un schéma ISO 20022 et pont Torii | ◉ | Schémas canoniques ISO 20022 intégrés, analyse XML déterministe et API `/v1/iso20022/status/{MsgId}` exposée. | statut.md:65-70 |

## Accélération matérielle

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Tests de parité queue/désalignement SIMD | ◉ | Les tests de parité randomisés garantissent que les opérations vectorielles SIMD correspondent à la sémantique scalaire pour un alignement arbitraire. | statut.md:243 |
| Repli et autotests Metal/CUDA | ◉ | Les backends GPU exécutent des auto-tests en or et reviennent au scalaire/SIMD en cas de non-concordance ; les suites de parité couvrent SHA-256/Keccak/AES. | statut.md:244-246 |

## Temps de réseau et modes de consensus

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Service de temps réseau (NTS) | ✖︎ | Le design existe dans `new_pipeline.md` ; la mise en œuvre n’a pas encore été suivie dans les mises à jour de statut. | nouveau_pipeline.md |
| Mode consensus PoS nominé | ✖︎ | Les documents de conception Nexus sont les modes fermés et NPoS ; mise en œuvre de base en attente. | nouveau_pipeline.md; lien.md |

## Nexus Feuille de route du grand livre| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Space Directory contract scaffold | ✖︎ | Le contrat de registre mondial pour les manifestes/gouvernance DS n'est pas encore mis en œuvre. | lien.md |
| Data Space manifest format & lifecycle | ✖︎ | Le schéma manifeste Norito, la gestion des versions et le flux de gouvernance restent sur la feuille de route. | lien.md |
| DS governance & validator rotation | ✖︎ | Les procédures en chaîne pour l'adhésion/rotation DS sont encore en phase de conception. | lien.md |
| Ancrage Cross-DS & composition du bloc Nexus | ✖︎ | Couche de composition et engagements d’ancrage définis mais non mis en œuvre. | lien.md |
| Kura/WSV erasure-coded storage | ✖︎ | Stockage d'objets blob/instantanés codés avec effacement pour DS public/privé non encore construit. | lien.md |
| ZK/optimistic proof policy per DS | ✖︎ | Les exigences de preuve Per-DS et leur application ne sont pas suivies dans le code. | lien.md |
| Fee/quota isolation per Data Space | ✖︎ | Les quotas spécifiques à DS et les mécanismes de politique tarifaire restent des travaux futurs. | lien.md |

## Chaos & Fault Injection

| Fonctionnalité | Statut | Remarques | Preuve |
|---------|--------|-------|--------------|
| Izanami chaosnet orchestration | ○ | La charge de travail Izanami gère désormais les recettes de définition d'actifs, de métadonnées, de NFT et de répétition de déclenchement avec une couverture unitaire pour les nouveaux chemins. | crates/izanami/src/instructions.rs; crates/izanami/src/instructions.rs#tests |