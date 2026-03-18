---
lang: fr
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2026-01-03T18:07:57.038484+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Paysage des fournisseurs d'audit SM
% Groupe de travail sur la cryptographie Iroha
% 2026-02-12

# Aperçu

Le groupe de travail Crypto a besoin d'un groupe permanent d'examinateurs indépendants qui
comprendre à la fois la cryptographie Rust et les normes chinoises GM/T (SM2/SM3/SM4).
Cette note répertorie les cabinets avec les références pertinentes et résume l'audit
portée que nous demandons généralement afin que les cycles de demande de proposition (RFP) restent rapides et
cohérent.

# Entreprises candidates

## Trail of Bits (Pratique de cryptographie CN)

- Engagements documentés : examen de sécurité 2023 de Tongsuo d'Ant Group
  (distribution OpenSSL compatible SM) et audits répétés des applications basées sur Rust
  blockchains telles que Diem/Libra, Sui et Aptos.
- Points forts : équipe dédiée à la cryptographie Rust, temps constant automatisé
  outils d'analyse, expérience en validation d'exécution déterministe et de matériel
  politiques d’expédition.
- Convient pour Iroha : peut étendre le SOW d'audit SM actuel ou effectuer des travaux indépendants
  re-tests ; fonctionnement confortable avec les appareils Norito et l'appel système IVM
  surfaces.

## Groupe NCC (Services de cryptographie APAC)

- Engagements documentés : examens de code gm/T (SM) pour paiement régional
  fournisseurs de réseaux et de HSM ; critiques antérieures de Rust pour Parity Substrate, Polkadot,
  et les composants Balance.
- Points forts : large banc APAC avec reporting bilingue, capacité à combiner
  vérifications de processus de type conformité avec examen approfondi du code.
- Adapté pour Iroha : idéal pour les évaluations de deuxième avis ou axées sur la gouvernance
  validation parallèlement aux résultats de Trail of Bits.

## SECBIT Labs (Pékin)

- Engagements documentés : mainteneurs de la caisse open-source `libsm` Rust
  utilisé par Nervos CKB et CITA ; audit de l'activation de Guomi pour Nervos, Muta et
  Composants FISCO BCOS Rust avec livrables bilingues.
- Points forts : ingénieurs qui expédient activement des primitives SM dans Rust, forts
  capacités de test de propriété, connaissance approfondie de la conformité nationale
  exigences.
- Convient pour Iroha : utile lorsque nous avons besoin de réviseurs capables de fournir des comparaisons
  vecteurs de test et conseils de mise en œuvre parallèlement aux résultats.

## Sécurité SlowMist (Chengdu)

- Engagements documentés : examens de sécurité du substrat/Polkadot Rust, y compris
  Fourches Guomi pour les opérateurs chinois ; évaluations de routine du portefeuille SM2/SM3/SM4
  et le code de pont utilisé par les échanges.
- Points forts : pratique d'audit axée sur la blockchain, réponse intégrée aux incidents,
  des conseils qui couvrent le code de protocole de base et les outils de l’opérateur.
- Adapté pour Iroha : utile pour valider la parité du SDK et les points de contact opérationnels
  en plus des caisses de base.

## Chaitin Tech (Laboratoire de sécurité QAX 404)- Engagements documentés : contributeurs au durcissement GmSSL/Tongsuo et SM2/SM3/
  Conseils de mise en œuvre du SM4 pour les institutions financières nationales ; établi
  Pratique d'audit Rust couvrant les piles TLS et les bibliothèques cryptographiques.
- Points forts : expérience approfondie en cryptanalyse, capacité à coupler une vérification formelle
  artefacts avec examen manuel, relations de longue date avec les régulateurs.
- Convient pour Iroha : convient en cas d'approbation réglementaire ou d'artefacts de preuve formelle
  doivent accompagner le rapport standard de révision du code.

# Portée et livrables typiques de l'audit

- **Conformité aux spécifications :** valider le calcul SM2 ZA, signature
  canonisation, remplissage/compression SM3 et planification des clés SM4 et gestion IV
  contre GM/T 0003-2012, GM/T 0004-2012 et GM/T 0002-2012.
- **Déterminisme et comportement en temps constant :** examen des branchements, recherche
  tables et portes de fonctionnalités matérielles (par exemple, instructions NEON, SM4) pour garantir
  La répartition Rust et FFI reste déterministe sur le matériel pris en charge.
- **Intégration FFI et fournisseur :** examinez les liaisons OpenSSL/Tongsuo,
  Adaptateurs PKCS#11/HSM et chemins de propagation des erreurs pour une sécurité consensuelle.
- **Couverture des tests et des luminaires :** évalue les harnais de fuzz, les allers-retours Norito,
  des tests de fumée déterministes et recommander des tests différentiels là où les lacunes
  apparaître.
- **Examen des dépendances et de la chaîne d'approvisionnement :** confirmer la provenance de la construction et le fournisseur
  politiques de correctifs, précision du SBOM et instructions de construction reproductibles.
- **Documentation et opérations :** valider les runbooks des opérateurs, la conformité
  les mémoires, les paramètres par défaut et les procédures de restauration.
- **Attentes en matière de reporting :** résumé avec évaluation du risque, détaillé
  résultats avec références de code et conseils de remédiation, plan de retest et
  attestations couvrant les garanties de déterminisme.

# Prochaines étapes

- Utiliser cette liste de fournisseurs pendant les cycles de RFQ ; ajuster la liste de contrôle de la portée ci-dessus pour
  correspondre au jalon SM actif avant de lancer une demande de propositions.
- Enregistrer les résultats de l'engagement dans `docs/source/crypto/sm_audit_brief.md` et
  les mises à jour de l'état de surface dans `status.md` une fois les contrats exécutés.