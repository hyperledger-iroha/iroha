---
lang: fr
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2026-01-03T18:07:57.113286+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Note d'audit externe SM2/SM3/SM4
% Groupe de travail sur la cryptographie Iroha
% 2026-01-30

# Aperçu

Ce dossier présente le contexte d'ingénierie et de conformité requis pour un
examen indépendant de l’activation SM2/SM3/SM4 du Iroha. Il s'adresse aux équipes d'audit
avec une expérience en cryptographie Rust et une familiarité avec Chinese National
Normes de cryptographie. Le résultat attendu est un rapport écrit couvrant
risques de mise en œuvre, écarts de conformité et conseils de remédiation prioritaires
avant le déploiement de SM, passant de la prévisualisation à la production.

# Aperçu du programme

- **Portée de la version :** Base de code partagée Iroha 2/3, vérification déterministe
  chemins à travers les nœuds et les SDK, signature disponible derrière la protection de configuration.
- **Phase actuelle :** SM-P3.2 (intégration backend OpenSSL/Tongsuo) avec Rust
  implémentations déjà livrées pour vérification et cas d'utilisation symétriques.
- **Date de décision cible :** 2026-04-30 (les résultats de l'audit indiquent le bon/non pour
  activant la signature SM dans les builds du validateur).
- **Principaux risques suivis :** pedigree de dépendance à des tiers, déterministe
  comportement sous matériel mixte, préparation à la conformité de l'opérateur.

# Références de codes et de luminaires

- `crates/iroha_crypto/src/sm.rs` — Implémentations Rust et OpenSSL en option
  liaisons (fonctionnalité `sm-ffi-openssl`).
- `crates/ivm/tests/sm_syscalls.rs` — Couverture des appels système IVM pour le hachage,
  vérification et modes symétriques.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Charge utile Norito
  allers-retours pour les artefacts SM.
- `docs/source/crypto/sm_program.md` — historique du programme, audit des dépendances et
  garde-corps de déploiement.
- `docs/source/crypto/sm_operator_rollout.md` — activation face à l'opérateur et
  procédures de restauration.
- `docs/source/crypto/sm_compliance_brief.md` — synthèse réglementaire et export
  considérations.
-`scripts/sm_openssl_smoke.sh`/`crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — harnais de fumée déterministe pour les flux soutenus par OpenSSL.
- Corpus `fuzz/sm_*` — Graines de fuzz basées sur RustCrypto couvrant les primitives SM3/SM4.

# Portée de l'audit demandée1. **Conformité aux spécifications**
   - Valider la vérification de la signature SM2, le calcul ZA et le canonique
     comportement d’encodage.
   - Confirmez que les primitives SM3/SM4 suivent GM/T 0002-2012 et GM/T 0007-2012,
     y compris les invariants du mode compteur et la gestion IV.
2. **Déterminisme et garanties à temps constant**
   - Examiner les branchements, les recherches de tables et la répartition du matériel afin d'exécuter les nœuds
     reste déterministe dans toutes les familles de processeurs.
   - Évaluer les réclamations en temps constant pour les opérations de clé privée et confirmer le
     Les chemins OpenSSL/Tongsuo conservent une sémantique à temps constant.
3. **Analyse des canaux secondaires et des défauts**
   - Inspecter les risques liés au timing, au cache et aux canaux secondaires d'alimentation dans Rust et
     Chemins de code soutenus par FFI.
   - Évaluer la gestion des erreurs et la propagation des erreurs pour la vérification de la signature et
     échecs de chiffrement authentifiés.
4. **Examen de la construction, des dépendances et de la chaîne d'approvisionnement**
   - Confirmer les versions reproductibles et la provenance des artefacts OpenSSL/Tongsuo.
   - Examiner les licences de l'arbre de dépendance et la couverture des audits.
5. **Critique du harnais de test et de vérification**
   - Évaluer les tests de fumée déterministes, les harnais de fuzz et les luminaires Norito.
   - Recommander une couverture supplémentaire (par exemple, tests différentiels,
     épreuves) si des lacunes subsistent.
6. **Validation de la conformité et des conseils de l'opérateur**
   - Vérifier la documentation expédiée par rapport aux exigences légales et attendues
     commandes de l'opérateur.

# Livrables & Logistique

- **Coup d'envoi :** 2026-02-24 (virtuel, 90 minutes).
- **Entretiens :** Crypto WG, mainteneurs IVM, opérations de plateforme (si nécessaire).
- **Accès aux artefacts :** miroir du référentiel en lecture seule, journaux de pipeline CI, luminaire
  sorties et SBOM de dépendances (CycloneDX).
- **Mises à jour intermédiaires :** statut écrit hebdomadaire + alertes sur les risques.
- **Livrables finaux (attendus le 2026-04-15) :**
  - Résumé exécutif avec évaluation des risques.
  - Résultats détaillés (par problématique : impact, probabilité, références de code,
    conseils de remédiation).
  - Plan de re-test/vérification.
  - Déclaration sur le déterminisme, la posture en temps constant et l'alignement de la conformité.

## Statut de l'engagement

| Vendeur | Statut | Coup d'envoi | Fenêtre de champ | Remarques |
|--------|--------|----------|--------------|-------|
| Trail of Bits (pratique du CN) | Énoncé des travaux exécutés 2026-02-21 | 2026-02-24 | 2026-02-24–2026-03-22 | Livraison prévue le 2026-04-15 ; Hui Zhang dirige l'engagement avec Alexey M. comme homologue en ingénierie. Appel de statut hebdomadaire le mercredi à 09h00 UTC. |
| Groupe CNC (APAC) | Créneau de contingence réservé | N/A (en attente) | Provisoire 2026-05-06–2026-05-31 | Activation uniquement si les résultats à haut risque nécessitent un deuxième passage ; état de préparation confirmé par Priya N. (Sécurité) et le bureau d'engagement du groupe NCC 2026-02-22. |

# Pièces jointes incluses dans le package de sensibilisation-`docs/source/crypto/sm_program.md`
-`docs/source/crypto/sm_operator_rollout.md`
-`docs/source/crypto/sm_compliance_brief.md`
-`docs/source/crypto/sm_lock_refresh_plan.md`
-`docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — Instantané `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"`.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — Exportation `cargo metadata` pour la caisse `iroha_crypto` (graphe de dépendances verrouillé).
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — dernière exécution `scripts/sm_openssl_smoke.sh` (ignore les chemins SM2/SM4 lorsque la prise en charge du fournisseur est manquante).
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — provenance de la boîte à outils locale (notes de version pkg-config/OpenSSL).
- Manifeste du corpus Fuzz (`fuzz/sm_corpus_manifest.json`).

> **Mise en garde concernant l'environnement :** L'instantané de développement actuel utilise la chaîne d'outils OpenSSL 3.x fournie (fonctionnalité de caisse `openssl` `vendored`), mais macOS ne dispose pas des composants intrinsèques du processeur SM3/SM4 et le fournisseur par défaut n'expose pas SM4-GCM, de sorte que le faisceau de fumée OpenSSL ignore toujours la couverture SM4 et l'analyse de l'exemple d'annexe SM2. Un cycle de dépendance de l'espace de travail (`sorafs_manifest ↔ sorafs_car`) force également le script d'assistance à ignorer l'exécution après avoir émis l'échec `cargo check`. Réexécutez le bundle dans l'environnement de construction de la version Linux (OpenSSL/Tongsuo avec SM4 activé et sans le cycle) pour capturer la parité complète avant l'audit externe.

# Partenaires d'audit candidats et portée

| Entreprise | Expérience pertinente | Portée et livrables typiques | Remarques |
|------|-----------|------------------------------|-------|
| Trail of Bits (pratique de cryptographie du CN) | Examens des codes Rust (`ring`, zkVMs), évaluations GM/T antérieures pour les piles de paiement mobile. | Différence de conformité aux spécifications (GM/T 0002/3/4), examen en temps constant des chemins Rust + OpenSSL, fuzzing différentiel, examen de la chaîne d'approvisionnement, feuille de route de remédiation. | Déjà fiancé; tableau conservé par souci d’exhaustivité lors de la planification des futurs cycles d’actualisation. |
| Groupe NCC APAC | Équipes rouges de matériel/SOC + cryptographie Rust, revues publiées des primitives RustCrypto et des ponts HSM de paiement. | Évaluation holistique des liaisons Rust + JNI/FFI, validation des politiques déterministes, examen des portes de performances/télémétrie, présentation pas à pas du manuel de jeu de l'opérateur. | Réservé comme éventualité ; peut également fournir des rapports bilingues aux régulateurs chinois. |
| Kudelski Security (équipe Blockchain & crypto) | Audits de Halo2, Mina, zkSync, schémas de signature personnalisés implémentés dans Rust. | Concentrez-vous sur l'exactitude des courbes elliptiques, l'intégrité des transcriptions, la modélisation des menaces pour l'accélération matérielle et les preuves de CI/déploiement. | Utile pour les deuxièmes avis sur l'accélération matérielle (SM-5a) et les interactions FASTPQ vers SM. |
| Moins d'autorité | Audits de protocoles cryptographiques pour les blockchains basées sur Rust (Filecoin, Polkadot), conseil en builds reproductibles. | Vérification déterministe de la construction, vérification du codec Norito, vérification croisée des preuves de conformité, examen de la communication avec l'opérateur. | Bien adapté aux livrables de transparence/rapport d’audit lorsque les régulateurs demandent une vérification indépendante au-delà de l’examen du code. |

Toutes les missions nécessitent le même ensemble d'artefacts énuméré ci-dessus, ainsi que les modules complémentaires facultatifs suivants en fonction de l'entreprise :- **Conformité aux spécifications et comportement déterministe :** Vérification ligne par ligne de la dérivation SM2 ZA, du remplissage SM3, des fonctions rondes SM4 et de la porte de répartition d'exécution `sm_accel` pour garantir que l'accélération ne modifie jamais la sémantique.
- **Examen des canaux secondaires et FFI :** Inspection des réclamations en temps constant, des blocs de code non sécurisés et des couches de pontage OpenSSL/Tongsuo, y compris des tests de comparaison avec le chemin Rust.
- **Validation CI/chaîne d'approvisionnement :** Reproduction des harnais `sm_interop_matrix`, `sm_openssl_smoke` et `sm_perf` avec les attestations SBOM/SLSA afin que les résultats de l'audit puissent être directement liés aux preuves publiées.
- **Garantie destinée à l'opérateur :** Vérification croisée de `sm_operator_rollout.md`, des modèles de dépôt de conformité et des tableaux de bord de télémétrie pour confirmer que les atténuations promises dans la documentation sont techniquement applicables.

Lors de la définition de la portée des audits futurs, réutilisez ce tableau pour aligner les points forts des fournisseurs sur l'étape spécifique de la feuille de route (par exemple, privilégiez Kudelski pour les versions lourdes en matériel/perf, Trail of Bits pour l'exactitude du langage/d'exécution et Least Authority pour les garanties de construction reproductibles).

# Points de contact

- **Propriétaire technique :** Responsable du groupe de travail Crypto (Alexey M., `alexey@iroha.tech`)
- **Chef de programme :** Coordonnatrice des opérations de plateforme (Sarah K.,
  `sarah@iroha.tech`)
- **Liaison sécurité :** Ingénierie de sécurité (Priya N., `security@iroha.tech`)
- **Liaison documentation :** Responsable Docs/DevRel (Jamila R.,
  `docs@iroha.tech`)