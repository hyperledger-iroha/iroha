---
lang: fr
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2026-01-03T18:07:57.078074+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Registre des risques du programme SM pour l'activation SM2/SM3/SM4.

# Registre des risques du programme SM

Dernière mise à jour : 2025-03-12.

Ce registre développe le résumé `sm_program.md`, en associant chaque risque à
la propriété, les déclencheurs de surveillance et l’état actuel de l’atténuation. Le GT Crypto
et les responsables de la plate-forme principale examinent ce registre à la cadence SM hebdomadaire ; changements
se reflètent ici et dans la feuille de route publique.

## Résumé des risques

| ID | Risque | Catégorie | Probabilité | Impact | Gravité | Propriétaire | Atténuation | Statut | Déclencheurs |
|----|------|----------|-------------|--------|--------------|-------|------------|--------|----------|
| R1 | Audit externe des caisses RustCrypto SM non exécuté avant la signature du validateur GA | Chaîne d'approvisionnement | Moyen | Élevé | Élevé | GT sur la cryptographie | Contract Trail of Bits/NCC Group, conserver la position de vérification uniquement jusqu'à ce que le rapport soit accepté | Atténuation en cours | L'EDT d'audit n'a pas été signé avant le 2025-04-15 ou le rapport d'audit a été retardé après le 2025-06-01 |
| R2 | Régressions déterministes occasionnelles dans les SDK | Mise en œuvre | Moyen | Élevé | Élevé | Responsables du programme SDK | Partagez des appareils sur le SDK CI, appliquez le codage canonique r∥s, ajoutez des tests de falsification inter-SDK | Surveillance | Dérive des luminaires détectée dans la version CI ou SDK sans luminaires SM |
| R3 | Bogues spécifiques à ISA dans les intrinsèques (NEON/SIMD) | Performances | Faible | Moyen | Moyen | GT sur les performances | Les intrinsèques de porte derrière les indicateurs de fonctionnalité, nécessitent une couverture CI sur ARM, maintiennent le repli scalaire | Atténuation en cours | Les bancs NEON échouent ou une régression matérielle découverte dans la matrice de performances SM |
| R4 | L'ambiguïté de la conformité retarde l'adoption du SM | Gouvernance | Moyen | Moyen | Moyen | Documents et liaison juridique | Publier un dossier de conformité, une liste de contrôle pour l'opérateur, une liaison avec le conseiller juridique avant l'AG | Atténuation en cours | Examen juridique en suspens après le 01/05/2025 ou mises à jour manquantes de la liste de contrôle |
| R5 | Dérive du backend FFI avec les mises à jour du fournisseur | Intégration | Moyen | Moyen | Moyen | Opérations de plateforme | Épingler les versions du fournisseur, ajouter des tests de parité, conserver l'opt-in de prévisualisation OpenSSL/Tongsuo | Surveillance | Mise à jour du package fusionnée sans exécution de parité ni aperçu activé en dehors de la portée du pilote |

## Cadence de révision

- Synchronisation hebdomadaire du Crypto WG (point permanent de l'ordre du jour).
- Examen conjoint mensuel avec Platform Ops et Docs pour confirmer la conformité.
- Point de contrôle avant la publication : gel du registre des risques et attestation fournie avec GA
  des artefacts.

## Signature

| Rôle | Représentant | Dates | Remarques |
|------|------|------|-------|
| Responsable du GT Crypto | (signature au dossier) | 2025-03-12 | Approuvé pour publication et partagé avec le backlog du groupe de travail. |
| Responsable de la plateforme principale | (signature au dossier) | 2025-03-12 | Atténuations acceptées et cadence de surveillance. |

Pour l'historique des approbations et les procès-verbaux des réunions, voir `docs/source/crypto/sm_program.md`.
(`Communication Plan`) et l'archive de l'agenda SM liée au Crypto WG
espace de travail.