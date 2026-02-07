---
lang: fr
direction: ltr
source: docs/source/crypto/attachments/sm_export_statement_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6742d2b87b8fbbc1493c5ae2704147b0f8d5d23af78004c2c9a112fe881efb11
source_last_modified: "2026-01-03T18:07:57.071790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Modèle de déclaration de contrôle des exportations SM2/SM3/SM4
% Hyperledger Iroha Groupe de travail sur la conformité
% 2026-05-06

# Utilisation

Intégrez cette déclaration dans les notes de version, les manifestes ou la correspondance juridique lorsque
distribuer des artefacts compatibles SM. Mettez à jour les espaces réservés pour qu'ils correspondent à la version,
juridiction et les exceptions de licence applicables. Conserver une copie signée avec le
liste de contrôle de publication.

# Déclaration

> **Produit :** Hyperledger Iroha {{ RELEASE_VERSION }} (`{{ ARTEFACT_ID }}`)
>
> **Algorithmes inclus :** Signature numérique SM2, hachage SM3, symétrique SM4
> cryptage (GCM/CCM)
>
> **Classification des exportations :** États-Unis EAR Catégorie 5, Partie 2 (5D002.c.1) ;
> Règlement de l'Union européenne 2021/821 Annexe 1, 5D002.
>
> **Exception(s) de licence :** {{ LICENSE_EXCEPTION }} (par exemple, ENC §740.17(b)(2),
> TSU §740.13 pour la distribution des sources).
>
> **Périmètre de distribution :** {{ DISTRIBUTION_SCOPE }} (par exemple, « Global, à l'exclusion
> territoires sous embargo répertoriés dans 15 CFR 746").
>
> **Obligations de l'opérateur :** Les destinataires doivent se conformer aux exigences d'exportation applicables,
> réglementations d'importation et d'utilisation. Déploiements en République populaire de
> La Chine exige des dépôts de produits et d'utilisation auprès de l'État de cryptographie
> Administration et respect des exigences de résidence des données sur le continent.
>
> **Contact :** {{ LEGAL_CONTACT_NAME }} — {{ LEGAL_CONTACT_EMAIL }} /
> {{ LEGAL_CONTACT_PHONE }}
>
> Cette déclaration accompagne la liste de contrôle de conformité de la cryptographie et le dépôt
> modèles fournis dans `docs/source/crypto/sm_compliance_brief.md`. Conservez ceci
> document et dépôts associés pendant au moins trois ans.

#Signature

- Représentant autorisé : ________________________
- Titre : ________________________
-Date : ________________________