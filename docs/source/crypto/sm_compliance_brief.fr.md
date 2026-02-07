---
lang: fr
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2026-01-03T18:07:57.080817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Brief de conformité et d'exportation SM2/SM3/SM4

Cette note complète les notes d'architecture dans `docs/source/crypto/sm_program.md`
et fournit des conseils pratiques aux équipes d'ingénierie, d'exploitation et juridiques en tant que
La famille d’algorithmes GM/T passe d’un aperçu de vérification uniquement à une activation plus large.

## Résumé
- **Base réglementaire :** la *Loi chinoise sur la cryptographie* (2019), la *Loi sur la cybersécurité* et
  *La loi sur la sécurité des données* classe les SM2/SM3/SM4 comme « cryptographie commerciale » une fois déployés.
  à terre. Les opérateurs doivent déposer des rapports d'utilisation et certains secteurs nécessitent une accréditation
  tests avant utilisation en production.
- **Contrôles internationaux :** En dehors de la Chine, les algorithmes relèvent de la catégorie US EAR.
  5 Partie 2, UE 2021/821, annexe 1 (5D002), et régimes nationaux similaires. Source ouverte
  la publication est généralement admissible aux exceptions de licence (ENC/TSU), mais les fichiers binaires
  expédiés vers des régions sous embargo restent des exportations contrôlées.
- **Politique du projet :** Les fonctionnalités SM restent désactivées par défaut. Fonctionnalité de signature
  ne sera activé qu'après la clôture de l'audit externe, performances/télémétrie déterministes
  le contrôle et la documentation de l'opérateur (ce dossier) atterrissent.

## Actions requises par fonction
| Équipe | Responsabilités | Artefacts | Propriétaires |
|------|--------|-----------|--------|
| GT sur la cryptographie | Suivez les mises à jour des spécifications GM/T, coordonnez les audits tiers, maintenez une politique déterministe (dérivation occasionnelle, r∥s canoniques). | `sm_program.md`, rapports d'audit, lots de luminaires. | Responsable du GT Crypto |
| Ingénierie des versions | Fonctionnalités Gate SM derrière une configuration explicite, maintien par défaut de vérification uniquement, gestion de la liste de contrôle de déploiement des fonctionnalités. | `release_dual_track_runbook.md`, manifestes de version, ticket de déploiement. | Libérer TL |
| Opérations / SRE | Fournir une liste de contrôle d'activation SM, des tableaux de bord de télémétrie (utilisation, taux d'erreur), un plan de réponse aux incidents. | Runbooks, tableaux de bord Grafana, tickets d'intégration. | Opérations/SRE |
| Liaison juridique | Déposer des rapports de développement/d'utilisation du PRC lorsque les nœuds s'exécutent en Chine continentale ; examinez la posture d’exportation pour chaque offre groupée. | Modèles de dépôt, déclarations d'exportation. | Contact juridique |
| Programme SDK | Prise en charge cohérente de l'algorithme Surface SM, application d'un comportement déterministe, propagation des notes de conformité aux documents du SDK. | Notes de version du SDK, documents, contrôle CI. | Pistes du SDK |## Exigences en matière de documentation et de dépôt (Chine)
1. **Dépôt de produit (开发备案) :** Pour le développement à terre, soumettez la description du produit,
   déclaration de disponibilité de la source, liste de dépendances et étapes de construction déterministes pour
   l'administration provinciale de cryptographie avant sa diffusion.
2. **Fichier de ventes/utilisation (销售/使用备案) :** Les opérateurs exécutant des nœuds compatibles SM doivent
   enregistrer la portée d'utilisation, la gestion des clés et la collecte de télémétrie avec le même
   autorité. Fournissez les coordonnées et les SLA de réponse aux incidents.
3. **Certification (检测/认证) :** Les opérateurs d'infrastructures critiques peuvent exiger
   tests accrédités. Fournir des scripts de build reproductibles, des SBOM et des rapports de test
   afin que les intégrateurs en aval puissent compléter la certification sans modifier le code.
4. **Tenue de dossiers :** Archivez les dépôts et les approbations dans le système de suivi de la conformité.
   Mettez à jour `status.md` lorsque de nouvelles régions ou opérateurs terminent le processus.

## Liste de contrôle de conformité

### Avant d'activer les fonctionnalités SM
- [ ] Confirmez que le conseiller juridique a examiné les régions de déploiement cibles.
- [ ] Capturez les instructions de construction déterministes, les manifestes de dépendance et le SBOM
      exportations à inclure dans les dépôts.
-[ ] Alignez `crypto.allowed_signing`, `crypto.default_hash` et admission
      la politique se manifeste avec le ticket de déploiement.
- [ ] Produire des communications opérateur décrivant la portée des fonctionnalités SM,
      conditions préalables à l’activation et plans de secours en cas de désactivation.
-[ ] Exporter les tableaux de bord de télémétrie couvrant les compteurs de vérification/signature SM,
      taux d'erreur et métriques de performances (`sm3`, `sm4`, synchronisation des appels système).
- [ ] Préparer les contacts de réponse aux incidents et les voies d'escalade pour les opérations à terre.
      opérateurs et le Crypto WG.

### Préparation au dépôt et à l'audit
- [ ] Sélectionnez le modèle de classement approprié (produit vs ventes/utilisation) et remplissez
      dans les métadonnées de la version avant la soumission.
- [ ] Joignez les archives SBOM, les transcriptions de tests déterministes et les hachages manifestes.
- [ ] Assurez-vous que la déclaration de contrôle des exportations reflète les artefacts exacts étant
      délivré et cite les exceptions de licence invoquées (ENC/TSU).
- [ ] Vérifiez que les rapports d'audit, le suivi des mesures correctives et les runbooks des opérateurs
      sont liés à partir du dossier de dépôt.
- [ ] Stocker les dépôts, approbations et correspondances signés dans le respect
      tracker avec références versionnées.

### Opérations post-approbation
- [ ] Mettre à jour `status.md` et le ticket de déploiement une fois le dépôt accepté.
- [ ] Réexécutez la validation de la télémétrie pour confirmer les correspondances de couverture d'observabilité
      les entrées de dépôt.
- [ ] Planifier un examen périodique (au moins une fois par an) des dossiers, des rapports d'audit,
      et exportez les déclarations pour capturer les mises à jour des spécifications/réglementations.
- [ ] Déclencher des addendums de dépôt à chaque fois que la configuration, la portée des fonctionnalités ou l'hébergement
      l’empreinte change sensiblement.## Conseils d'exportation et de distribution
- Inclure une courte déclaration d'exportation dans les notes de version/manifestes faisant référence à la confiance
  sur ENC/TSU. Exemple :
  > "Cette version contient des implémentations SM2/SM3/SM4. La distribution suit ENC
  > (15 CFR Part 742) / UE 2021/821 Annexe 1 5D002. Les opérateurs doivent assurer la conformité
  > avec les lois locales sur l'exportation/l'importation.
- Pour les builds hébergés en Chine, coordonnez-vous avec Ops pour publier des artefacts depuis
  infrastructures terrestres ; éviter le transfert transfrontalier de binaires compatibles SM, sauf si
  les licences appropriées sont en place.
- Lors de la mise en miroir vers des référentiels de packages, enregistrez quels artefacts incluent des fonctionnalités SM
  pour simplifier les rapports de conformité.

## Liste de contrôle de l'opérateur
-[ ] Confirmez le profil de version (`scripts/select_release_profile.py`) + indicateur de fonctionnalité SM.
-[ ] Consultez `sm_program.md` et ce mémoire ; s’assurer que les dossiers légaux sont enregistrés.
- [ ] Activez les fonctionnalités SM en compilant avec `sm`, en mettant à jour `crypto.allowed_signing` pour inclure `sm2` et en basculant `crypto.default_hash` vers `sm3-256` uniquement une fois que les garanties de déterminisme sont en place et que l'état d'audit est vert.
- [ ] Mettre à jour les tableaux de bord/alertes de télémétrie pour inclure les compteurs SM (échecs de vérification,
      demandes de signature, métriques de performances).
- [ ] Conservez les manifestes, les preuves de hachage/signature et les confirmations de dépôt joints à
      le ticket de déploiement.

## Exemples de modèles de dépôt

Les modèles se trouvent sous `docs/source/crypto/attachments/` pour une inclusion facile dans
dépôt de dossiers. Copiez le modèle Markdown pertinent dans le changement de l'opérateur
enregistrez-le ou exportez-le au format PDF selon les exigences des autorités locales.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) —
  Formulaire provincial de dépôt de produit (开发备案) capturant les métadonnées de la version, les algorithmes,
  Références SBOM et contacts support.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  dossier de ventes/utilisation de l'opérateur (销售/使用备案) décrivant l'empreinte du déploiement,
  procédures de gestion des clés, de télémétrie et de réponse aux incidents.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) —
  déclaration de contrôle des exportations adaptée aux notes de sortie, aux manifestes ou aux documents légaux
  correspondance s’appuyant sur les exceptions de licence ENC/TSU.## Normes et citations
- **GM/T 0002-2012 / GB/T 32907-2016** — Chiffrement par bloc SM4 et paramètres AEAD (ECB/GCM/CCM). Correspond aux vecteurs capturés dans `docs/source/crypto/sm_vectors.md`.
- **GM/T 0003-2012 / GB/T 32918.x-2016** — Cryptographie à clé publique SM2, paramètres de courbe, processus de signature/vérification et tests à réponse connue de l'Annexe D.
- **GM/T 0004-2012 / GB/T 32905-2016** — Spécification de la fonction de hachage SM3 et vecteurs de conformité.
- **RFC 8998** — Échange de clés SM2 et utilisation de signatures dans TLS ; citer lors de la documentation de l'interopérabilité avec OpenSSL/Tongsuo.
- **Loi sur la cryptographie de la République populaire de Chine (2019)**, **Loi sur la cybersécurité (2017)**, **Loi sur la sécurité des données (2021)** — Base juridique du flux de travail de dépôt mentionné ci-dessus.
- **US EAR Catégorie 5 Part 2** et **Règlement UE 2021/821 Annexe 1 (5D002)** – Régimes de contrôle des exportations régissant les binaires compatibles SM.
- **Artefacts Iroha :** `scripts/sm_interop_matrix.sh` et `scripts/sm_openssl_smoke.sh` fournissent des transcriptions d'interopérabilité déterministes que les auditeurs peuvent relire avant de signer les rapports de conformité.

## Références
- `docs/source/crypto/sm_program.md` — architecture technique et politique.
- `docs/source/release_dual_track_runbook.md` — processus de validation et de déploiement.
- `docs/source/sora_nexus_operator_onboarding.md` — exemple de flux d'intégration des opérateurs.
-GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, série GB/T 32918, RFC 8998.

Des questions ? Contactez le Crypto WG ou la liaison juridique via le tracker de déploiement SM.