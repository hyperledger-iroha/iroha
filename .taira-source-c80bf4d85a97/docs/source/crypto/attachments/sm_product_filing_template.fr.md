---
lang: fr
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2026-01-03T18:07:57.069144+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Modèle de dépôt de produit SM2/SM3/SM4 (开发备案)
% Hyperledger Iroha Groupe de travail sur la conformité
% 2026-05-06

# Instructions

Utilisez ce modèle lorsque vous soumettez un *dépôt de développement de produits* à un organisme provincial.
ou au bureau municipal de l'Administration de la cryptographie de l'État (SCA) avant de distribuer
Binaires compatibles SM ou artefacts sources provenant de Chine continentale. Remplacez le
espaces réservés avec des détails spécifiques au projet, exportez le formulaire complété au format PDF si
requis et joignez les artefacts référencés dans la liste de contrôle.

# 1. Résumé du demandeur et du produit

| Champ | Valeur |
|-------|-------|
| Nom de l'organisation | {{ ORGANISATION }} |
| Adresse enregistrée | {{ ADRESSE }} |
| Représentant légal | {{ LEGAL_REP }} |
| Contact principal (nom/titre/e-mail/téléphone) | {{ CONTACT }} |
| Nom du produit | Hyperledger Iroha {{ RELEASE_NAME }} |
| Version du produit/ID de build | {{ VERSION }} |
| Type de dépôt | Développement de produits (开发备案) |
| Date de dépôt | {{ AAAA-MM-JJ }} |

# 2. Aperçu de l'utilisation de la cryptographie

- Algorithmes pris en charge : `SM2`, `SM3`, `SM4` (fournir la matrice d'utilisation ci-dessous).
- Contexte d'utilisation :
  | Algorithme | Composant | Objectif | Garanties déterministes |
  |-----------|-----------|---------|------------------------------|
  | SM2 | {{ COMPOSANT }} | {{ OBJECTIF }} | RFC6979 + application canonique r∥s |
  | SM3 | {{ COMPOSANT }} | {{ OBJECTIF }} | Hachage déterministe via `Sm3Digest` |
  | SM4 | {{ COMPOSANT }} | {{ OBJECTIF }} | AEAD (GCM/CCM) avec politique de nonce appliquée |
- Algorithmes non SM en cours de construction : {{ OTHER_ALGORITHMS }} (par souci d'exhaustivité).

# 3. Contrôles de développement et de chaîne d'approvisionnement

- Dépôt de code source : {{ REPOSITORY_URL }}
- Instructions de construction déterministes :
  1.`git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (ajuster si nécessaire).
  3. SBOM généré via `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`).
- Résumé de l'environnement d'intégration continue :
  | Article | Valeur |
  |------|-------|
  | Construire le système d'exploitation/la version | {{ CONSTRUIRE_OS }} |
  | Chaîne d'outils du compilateur | {{ CHAÎNE D'OUTILS }} |
  | Source OpenSSL/Tongsuo | {{ OPENSSL_SOURCE }} |
  | Somme de contrôle de reproductibilité | {{ SOMME DE CHECK }} |

# 4. Gestion des clés et sécurité

- Fonctionnalités SM activées par défaut : {{ DEFAULTS }} (par exemple, vérification uniquement).
- Indicateurs de configuration requis pour la signature : {{ CONFIG_FLAGS }}.
- Approche de conservation des clés :
  | Article | Détails |
  |------|--------------|
  | Outil de génération de clés | {{ KEY_TOOL }} |
  | Support de stockage | {{ STORAGE_MEDIUM }} |
  | Politique de sauvegarde | {{ BACKUP_POLICY }} |
  | Contrôles d'accès | {{ ACCESS_CONTROLS }} |
- Contacts réponse aux incidents (24h/24 et 7j/7) :
  | Rôle | Nom | Téléphone | Courriel |
  |------|------|-------|-------|
  | Chef de file crypto | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} |
  | Opérations de plateforme | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} |
  | Liaison juridique | {{ NOM }} | {{ TÉLÉPHONE }} | {{ EMAIL }} |

# 5. Liste de contrôle des pièces jointes- [ ] Instantané du code source (`{{ SOURCE_ARCHIVE }}`) et hachage.
- [ ] Script de construction déterministe / notes de reproductibilité.
- [ ] SBOM (`{{ SBOM_PATH }}`) et manifeste de dépendance (empreinte digitale `Cargo.lock`).
-[ ] Transcriptions de tests déterministes (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [ ] Exportation du tableau de bord de télémétrie démontrant l'observabilité SM.
- [ ] Déclaration de contrôle des exportations (voir modèle séparé).
- [ ] Rapports d'audit ou évaluations de tiers (si déjà complétés).

# 6. Déclaration du demandeur

> Je confirme que les informations ci-dessus sont exactes, que les informations divulguées
> la fonctionnalité cryptographique est conforme aux lois et réglementations applicables de la RPC,
> et que l'organisation conservera les artefacts soumis pendant au moins
> trois ans.

- Signature (représentant légal) : ________________________
-Date : ________________________