---
lang: fr
direction: ltr
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2026-01-03T18:07:57.068055+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Modèle de classement des ventes et de l'utilisation % SM2/SM3/SM4 (销售/使用备案)
% Hyperledger Iroha Groupe de travail sur la conformité
% 2026-05-06

# Instructions

Utilisez ce modèle lors du dépôt de l'utilisation du déploiement auprès d'un bureau SCA pour les opérations à terre.
opérateurs. Fournissez une soumission par cluster de déploiement ou espace de données. Mise à jour
les espaces réservés avec les détails spécifiques à l'opérateur et joignez les preuves répertoriées
dans la liste de contrôle.

# 1. Résumé de l'opérateur et du déploiement

| Champ | Valeur |
|-------|-------|
| Nom de l'opérateur | {{ OPERATOR_NAME }} |
| Numéro d'enregistrement de l'entreprise | {{ REG_ID }} |
| Adresse enregistrée | {{ ADRESSE }} |
| Contact principal (nom/titre/e-mail/téléphone) | {{ CONTACT }} |
| Identifiant de déploiement | {{ DEPLOYMENT_ID }} |
| Lieu(x) de déploiement | {{ EMPLACEMENTS }} |
| Type de dépôt | Ventes / Utilisation (销售/使用备案) |
| Date de dépôt | {{ AAAA-MM-JJ }} |

# 2. Détails du déploiement

- ID/hachage de build du logiciel : `{{ BUILD_HASH }}`
- Source de build : {{ BUILD_SOURCE }} (par exemple, construit par l'opérateur à partir de la source, binaire fourni par le fournisseur).
- Date d'activation : {{ ACTIVATION_DATE }}
- Fenêtres de maintenance planifiées : {{ MAINTENANCE_CADENCE }}
- Rôles des nœuds participant à la signature SM :
  | Nœud | Rôle | Fonctionnalités SM activées | Emplacement du coffre-fort à clés |
  |------|------|-----------|-------------------|
  | {{ NODE_ID }} | {{ RÔLE }} | {{ CARACTÉRISTIQUES }} | {{ COFFRE }} |

# 3. Contrôles cryptographiques

- Algorithmes autorisés : {{ ALGORITHMS }} (assurez-vous que l'ensemble SM correspond à la configuration).
- Résumé clé du cycle de vie :
  | Scène | Descriptif |
  |-------|-------------|
  | Génération | {{ KEY_GENERATION }} |
  | Stockage | {{ KEY_STORAGE }} |
  | Rotation | {{ KEY_ROTATION }} |
  | Révocation | {{ KEY_REVOCATION }} |
- Politique d'identité distincte (`distid`) : {{ DISTID_POLICY }}
- Extrait de configuration (section `crypto`) : fournit un instantané Norito/JSON avec des hachages.

# 4. Télémétrie et pistes d'audit

- Surveillance des points de terminaison : {{ METRICS_ENDPOINTS }} (`/metrics`, tableaux de bord).
- Métriques enregistrées : `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  histogrammes de latence, compteurs d'erreurs.
- Politique de conservation des journaux : {{ LOG_RETENTION }} (≥ trois ans recommandés).
- Emplacement de stockage du journal d'audit : {{ AUDIT_STORAGE }}

# 5. Réponse aux incidents et contacts

| Rôle | Nom | Téléphone | Courriel | ANS |
|------|------|-------|-------|-----|
| Responsable des opérations de sécurité | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} | {{ SLA }} |
| Crypto de garde | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} | {{ SLA }} |
| Juridique / Conformité | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} | {{ SLA }} |
| Assistance fournisseur (le cas échéant) | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} | {{ SLA }} |

# 6. Liste de contrôle des pièces jointes- [ ] Instantané de configuration (Norito + JSON) avec hachages.
- [ ] Preuve de build déterministe (hashes, SBOM, notes de reproductibilité).
- [ ] Exportations du tableau de bord de télémétrie et définitions d'alertes.
- [ ] Plan de réponse aux incidents et document de rotation d'astreinte.
- [ ] Accusé de réception de la formation de l'opérateur ou reçu du runbook.
- [ ] Déclaration de contrôle des exportations reflétant les artefacts livrés.
- [ ] Copies des accords contractuels ou des renonciations aux politiques pertinents.

# 7. Déclaration de l'opérateur

> Nous confirmons que le déploiement listé ci-dessus est conforme aux exigences commerciales de la RPC
> les réglementations de cryptographie, que les services compatibles SM suivent les
> les politiques de réponse aux incidents et de télémétrie, et que les artefacts d'audit seront
> conservé pendant au moins trois ans.

- Signataire autorisé : ________________________
-Date : ________________________