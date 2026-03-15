---
lang: fr
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-04T10:50:53.610533+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM Compliance Brief – Obligations de la loi chinoise sur la cryptographie
% Iroha Groupes de travail sur la conformité et la cryptographie
% 2026-02-12

# Invite

> Vous êtes LLM agissant en tant qu'analyste de conformité pour les équipes crypto et plateforme Hyperledger Iroha.  
> Contexte :  
> - Hyperledger Iroha est une blockchain autorisée basée sur Rust qui prend désormais en charge les primitives chinoises GM/T SM2 (signatures), SM3 (hachage) et SM4 (chiffrement par bloc).  
> - Les opérateurs de Chine continentale doivent se conformer à la loi de la RPC sur la cryptographie (2019), au système de protection à plusieurs niveaux (MLPS 2.0), aux règles de dépôt de la State Cryptography Administration (SCA) et aux contrôles d'importation/exportation supervisés par le ministère du Commerce (MOFCOM) et l'administration des douanes.  
> - Iroha distribue des logiciels open source à l'international. Certains opérateurs compileront des binaires compatibles SM au niveau national, tandis que d'autres pourront importer des artefacts pré-construits.  
> Analyse demandée : résumer les principales obligations légales déclenchées par l'envoi du support SM2/SM3/SM4 dans un logiciel blockchain open source, y compris : (a) la classification dans les catégories de cryptographie commerciale et de base/commune ; (b) les exigences de dépôt/approbation pour les logiciels qui mettent en œuvre la cryptographie commerciale d'État ; (c) les contrôles à l'exportation des fichiers binaires et des sources ; (d) les obligations opérationnelles des opérateurs de réseau (gestion des clés, journalisation, réponse aux incidents) dans le cadre du MLPS 2.0. Décrire des actions concrètes pour le projet Iroha (documentation, manifestes, déclarations de conformité) et pour les opérateurs déployant des nœuds compatibles SM en Chine.

# Résumé exécutif

- **Classification :** Les implémentations SM2/SM3/SM4 relèvent de la « cryptographie commerciale d'État » (商业密码) plutôt que de la cryptographie « de base » ou « commune », car il s'agit d'algorithmes publics publiés sanctionnés pour un usage civil/commercial. La distribution open source est autorisée mais soumise à dépôt lorsqu'elle est utilisée dans des produits ou services commerciaux proposés en Chine.
- **Obligations du projet :** Fournir la provenance de l'algorithme, des instructions de construction déterministes et une déclaration de conformité indiquant que les binaires implémentent la cryptographie commerciale d'État. Conservez les manifestes Norito qui signalent la capacité SM afin que les intégrateurs en aval puissent effectuer les dépôts.
- **Obligations des opérateurs :** Les opérateurs chinois doivent déposer leurs produits/services à l'aide d'algorithmes SM auprès du bureau provincial SCA, effectuer un enregistrement MLPS 2.0 (probablement niveau 3 pour les réseaux financiers), déployer des contrôles de gestion et de journalisation approuvés et garantir que les déclarations d'exportation/importation s'alignent sur les exemptions du catalogue du MOFCOM.

# Paysage réglementaire| Réglementation | Portée | Impact sur la prise en charge Iroha SM |
|------------|-------|----------------------------|
| **Loi sur la cryptographie de la RPC (2019)** | Définit la cryptographie de base/commune/commerciale, le système de gestion des mandats, le classement et la certification. | SM2/SM3/SM4 sont des « cryptographies commerciales » et doivent suivre les règles de dépôt/certification lorsqu'ils sont fournis en tant que produits/services en Chine. |
| **Mesures administratives SCA pour les produits de cryptographie commerciale** | Régit la production, la vente et la prestation de services ; nécessite le dépôt ou la certification du produit. | Les logiciels open source qui implémentent les algorithmes SM nécessitent les dépôts des opérateurs lorsqu'ils sont utilisés dans des offres commerciales ; les développeurs doivent fournir de la documentation pour faciliter les dépôts. |
| **MLPS 2.0 (Loi sur la cybersécurité + réglementation MLPS)** | Oblige les opérateurs à classer les systèmes d’information et à mettre en œuvre des contrôles de sécurité ; Le niveau 3 ou supérieur nécessite une preuve de conformité en matière de cryptographie. | Les nœuds blockchain gérant les données financières/d'identité s'enregistrent généralement au niveau MLPS 3 ; les opérateurs doivent documenter l’utilisation de SM, la gestion des clés, la journalisation et la gestion des incidents. |
| **Catalogue de contrôle des exportations du MOFCOM et règles douanières d'importation** | Contrôle l'exportation de produits cryptographiques, nécessite des autorisations pour certains algorithmes/matériels. | La publication du code source est généralement exemptée en vertu des dispositions du « domaine public », mais l'exportation de fichiers binaires compilés dotés de la capacité SM peut déclencher le catalogue à moins qu'ils ne soient expédiés à des destinataires approuvés ; les importateurs doivent déclarer la cryptographie commerciale d'État. |

# Obligations clés

## 1. Dépôt de produits et de services (Administration nationale de la cryptographie)

- **Qui dépose :** L'entité fournissant le produit/service en Chine (par exemple, opérateur, fournisseur SaaS). Les responsables de l'open source ne sont pas tenus de déposer, mais les directives d'empaquetage doivent permettre les dépôts en aval.
- **Livrables :** Description de l'algorithme, documents de conception de sécurité, preuves de tests, provenance de la chaîne d'approvisionnement et coordonnées.
- **Action Iroha :** Publiez une « déclaration de cryptographie SM » comprenant la couverture de l'algorithme, les étapes de construction déterministes, les hachages de dépendances et le contact pour les demandes de sécurité.

## 2. Certification et tests

- Certains secteurs (finance, télécommunications, infrastructures critiques) peuvent nécessiter des tests ou une certification en laboratoire accrédité (par exemple, certification CC-Grade/OSCCA).
- Inclure des artefacts de tests de régression démontrant la conformité aux spécifications GM/T.

## 3. Contrôles opérationnels MLPS 2.0

Les opérateurs doivent :1. **Enregistrez le système blockchain** auprès du Bureau de la sécurité publique, y compris les résumés d'utilisation de la cryptographie.
2. **Mettre en œuvre des politiques de gestion des clés** : génération, distribution, rotation et destruction de clés alignées sur les exigences SM2/SM4 ; consigner les événements clés du cycle de vie.
3. **Activer l'audit de sécurité** : capturez les journaux de transactions compatibles SM, les événements d'opérations cryptographiques et la détection d'anomalies ; conserver les journaux ≥6 mois.
4. **Réponse aux incidents :** maintenez des plans d'intervention documentés qui incluent des procédures de compromission de la cryptographie et des délais de reporting.
5. **Gestion des fournisseurs :** garantit que les fournisseurs de logiciels en amont (projet Iroha) peuvent fournir des notifications de vulnérabilité et des correctifs.

## 4. Considérations relatives à l'importation/exportation

- **Code source open source :** Généralement exempté en vertu d'une exception du domaine public, mais les responsables doivent héberger les téléchargements sur des serveurs qui suivent les journaux d'accès et inclure une licence/une clause de non-responsabilité faisant référence à la cryptographie commerciale de l'État.
- **Binaires prédéfinis :** Les exportateurs expédiant des binaires compatibles SM vers/hors de Chine doivent confirmer si l'article est couvert par le « Catalogue de contrôle des exportations de cryptographie commerciale ». Pour les logiciels à usage général sans matériel spécialisé, une simple déclaration de double usage peut suffire ; les responsables ne devraient pas distribuer de fichiers binaires provenant de juridictions avec des contrôles plus stricts, à moins que le conseil local ne l'approuve.
- **Importation par l'opérateur :** Les entités apportant des fichiers binaires en Chine doivent déclarer leur utilisation de la cryptographie. Fournissez des manifestes de hachage et du SBOM pour simplifier l’inspection douanière.

# Actions recommandées pour le projet

1. **Documentations**
   - Ajouter une annexe de conformité à `docs/source/crypto/sm_program.md` notant le statut de la cryptographie commerciale de l'État, les attentes en matière de dépôt et les points de contact.
   - Publier un champ manifeste Norito (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) que les opérateurs peuvent utiliser lors de la préparation des dépôts.
   - Assurez-vous que la publicité Torii `/v1/node/capabilities` (et l'alias CLI `iroha runtime capabilities`) est livrée avec chaque version afin que les opérateurs puissent capturer l'instantané du manifeste `crypto.sm` pour les preuves MLPS/密评.
   - Fournir un démarrage rapide de la conformité bilingue (EN/ZH) résumant les obligations.
2. **Libérer les artefacts**
   - Expédiez les fichiers SBOM/CycloneDX pour les versions compatibles SM.
   - Incluez des scripts de construction déterministes et des fichiers Docker reproductibles.
3. **Dépôts des opérateurs de support**
   - Proposer des modèles de lettres attestant la conformité des algorithmes (par exemple, références GM/T, couverture des tests).
   - Tenir à jour une liste de diffusion d'avis de sécurité pour satisfaire aux exigences de notification des fournisseurs.
4. **Gouvernance interne**
   - Suivre les points de contrôle de conformité SM dans la liste de contrôle de version (audit terminé, documentation mise à jour, champs de manifeste en place).

# éléments d'action de l'opérateur (Chine)1. Déterminez si le déploiement constitue un « produit/service de cryptographie commercial » (c'est le cas de la plupart des réseaux d'entreprise).
2. Déposer le produit/service auprès du bureau provincial de la SCA ; joindre la déclaration de conformité Iroha, le SBOM et les rapports de test.
3. Enregistrez le système blockchain sous MLPS 2.0, ciblez les contrôles de niveau 3 ; intégrer les journaux Iroha dans la surveillance de la sécurité.
4. Établir des procédures de cycle de vie des clés SM (utiliser le KMS/HSM approuvé si nécessaire).
5. Inclure des scénarios de compromission de la cryptographie dans les exercices de réponse aux incidents ; définir des contacts d'escalade avec les responsables Iroha.
6. Pour le flux de données transfrontalier, confirmez les dépôts supplémentaires CAC (Cyberspace Administration) si les données personnelles sont exportées.

# Invite autonome (Copier/Coller)

> Vous êtes LLM agissant en tant qu'analyste de conformité pour les équipes crypto et plateforme Hyperledger Iroha.  
> Contexte : Hyperledger Iroha est une blockchain autorisée basée sur Rust qui prend désormais en charge les primitives chinoises GM/T SM2 (signatures), SM3 (hachage) et SM4 (chiffrement par bloc). Les opérateurs de Chine continentale doivent se conformer à la loi de la RPC sur la cryptographie (2019), au système de protection à plusieurs niveaux (MLPS 2.0), aux règles de dépôt de l'administration nationale de la cryptographie (SCA) et aux contrôles d'importation/exportation supervisés par le MOFCOM et l'administration des douanes. Le projet Iroha distribue des logiciels open source compatibles SM à l'échelle internationale ; certains opérateurs compilent des binaires au niveau national, tandis que d'autres importent des artefacts pré-construits.  
> Tâche : Résumer les obligations légales déclenchées par l'envoi du support SM2/SM3/SM4 dans un logiciel blockchain open source. Couvre la classification de ces algorithmes (cryptographie commerciale ou de base/commune), les dépôts ou certifications requis pour les produits logiciels, les contrôles d'exportation/importation relatifs à la source et aux binaires, ainsi que les tâches opérationnelles des opérateurs de réseau sous MLPS 2.0 (gestion des clés, journalisation, réponse aux incidents). Fournir des actions concrètes pour le projet Iroha (documentation, manifestes, déclarations de conformité) et pour les opérateurs déployant des nœuds compatibles SM en Chine.