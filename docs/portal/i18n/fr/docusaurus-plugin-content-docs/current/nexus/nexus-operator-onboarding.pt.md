---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
titre : Intégration des opérateurs d'espace de données Sora Nexus
description : Espelho de `docs/source/sora_nexus_operator_onboarding.md`, accompagnant une liste de contrôle de version de bout en bout pour les opérateurs Nexus.
---

:::note Fonte canonica
Cette page reflète `docs/source/sora_nexus_operator_onboarding.md`. Mantenha as duas copias alinhadas ate que as edicoes localizadas cheguem ao portal.
:::

# Intégration des opérateurs d'espace de données Sora Nexus

Cette guide capture le flux de bout en bout que les opérateurs de l'espace de données Sora Nexus doivent suivre lors de la publication et de l'annonce. Il complète le runbook de duplication via (`docs/source/release_dual_track_runbook.md`) et la note de sélection des artéfacts (`docs/source/release_artifact_selection.md`) décrits comme des bundles/images disponibles, des manifestes et des modèles de configuration comme attendus des voies globales avant de les placer dans la production.## Audience et prérequis
- Votre voix a été approuvée par le programme Nexus et a reçu vos attributs d'espace de données (indice de voie, ID/alias d'espace de données et exigences politiques de routage).
- Voce consegue acessar os artefatos assinados do release publicados pelo Release Engineering (archives tar, images, manifestes, assassinats, chaves publiques).
- Vous pouvez ou recevoir du matériel de clés de production pour votre papier de validateur/observateur (identité de non Ed25519 ; clé de consensus BLS + PoP pour validateurs ; mais quaisquer les options de ressources confidentielles).
- Voce consegue alcancar os peers Sora Nexus existent que farao bootstrap do your no.

## Étape 1 - Confirmer le profil de sortie
1. Identifiez l'alias de rede ou l'ID de chaîne fourni.
2. Exécutez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) pour extraire ce référentiel. L'assistant consulte `release/network_profiles.toml` et imprime le profil pour le déploiement. Pour Sora Nexus, la réponse doit être `iroha3`. Pour toute valeur, consultez Release Engineering.
3. Notez l'étiquette du verso référencée dans l'annonce de la version (par exemple `iroha3-v3.2.0`) ; Voce o usara para buscar artefatos e manifestes.## Étape 2 - Récupérer et valider les objets
1. Utilisez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et vos archives associées (`.sha256`, `.sig/.pub` en option, `<profile>-<version>-manifest.json` et `<profile>-<version>-image.json` pour le déploiement du générateur de voix avec les contenants).
2. Validez l'intégrité avant de décompacter :
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` par le vérificateur agréé par l'organisation qui utilise le matériel KMS.
3. Inspectez `PROFILE.toml` dans l'archive tar et le système d'exploitation manifeste JSON pour confirmer :
   -`profile = "iroha3"`
   - Les champs `version`, `commit` et `built_at` correspondent à l'annonce de la sortie.
   - Le système d'exploitation/architecture correspond à ce déploiement.
4. Lorsque vous utilisez l'image du conteneur, répétez la vérification du hachage/assistance pour `<profile>-<version>-<os>-image.tar` et confirmez l'ID de l'image enregistrée dans `<profile>-<version>-image.json`.## Étape 3 - Préparer la configuration à partir de deux modèles
1. Extraia o bundle e copie `config/` para o local onde o ne vai ler sua configuracao.
2. Traitez vos archives sur `config/` comme modèles :
   - Remplacer `public_key`/`private_key` pour votre produit Ed25519. Supprimer les éléments privés de la discothèque ou ne pas obtenir d'objets à partir d'un HSM ; actualisez une configuration pour ouvrir le connecteur HSM.
   - Ajustez `trusted_peers`, `network.address` et `torii.address` pour refléter vos interfaces alcancavées et les pairs de bootstrap que le foram attribue.
   - Configurez `client.toml` avec le point de terminaison Torii pour les opérateurs (y compris la configuration TLS avec application) et comme informations d'identification fournies pour l'outillage opérationnel.
3. Mantenha o chain ID fornecido no bundle a menos que Governance instrua explicitement - a lane global espéra un unico identificador canonico de cadeia.
4. Planifiez votre lancement avec le drapeau de profil Sora : `irohad --sora --config <path>`. Le chargeur de configuration rejette les configurations SoraFS ou multivoie avec le drapeau activé.## Etapa 4 - Alinhar métadonnées de l'espace de données et du routage
1. Editer `config/config.toml` pour que la section `[nexus]` corresponde au catalogue de l'espace de données fourni par le Conseil Nexus :
   - `lane_count` doit être identique à l'ensemble des voies autorisées à l'époque actuelle.
   - Chaque entrée dans `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` doit être vérifiée par un `index`/`id` unique et les alias correspondants. Nao apague as entradas globais existentes; Ajoutez vos alias aux délégués ou au conseiller attribuant des espaces de données supplémentaires.
   - Garanta que chaque entrée de l'espace de données inclut `fault_tolerance (f)` ; comites lane-relay sao dimensionados em `3f+1`.
2. Actualisez `[[nexus.routing_policy.rules]]` pour capturer la politique qui a foi en Dieu. Le modèle de commande de la direction générale de la voie `1` et le déploiement des contrats pour la voie `2` ; Ajoutez ou modifiez les règles pour que le trafic destiné à votre espace de données soit encaminhado pour une voie et un alias corretos. Coordonnez-vous avec Release Engineering avant de modifier l'ordre des tâches.
3. Révisez les limites de `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Espérons que les opérateurs conservent leurs valeurs approuvées par le conseil ; Ajustez simplement une politique actualisée ayant été ratifiée.4. Enregistrez une configuration finale dans votre tracker d'opérations. Le runbook de sortie de duplication via l'exigence annexe de l'effet `config.toml` (avec droits réservés) au ticket d'intégration.

## Etapa 5 - Pré-vol Validacao
1. Exécutez le validateur de configuration sélectionné avant l'entrée dans le réseau :
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Il s'agit donc d'une configuration résolue et falha cedo se car les entrées de catalogue/routage sont incohérentes ou se genèse et configuration divergente.
2. Lorsque vous souhaitez déployer des conteneurs, exécutez la même commande dans l'image posée sur `docker load -i <profile>-<version>-<os>-image.tar` (le membre inclut `--sora`).
3. Vérifiez les journaux pour avis sur les identifiants de l'espace réservé de la voie/de l'espace de données. Si vous le souhaitez, revenez à Etapa 4 - déploie la production en fonction des espaces réservés ID qui accompagnent les modèles.
4. Exécutez votre procédure locale de fumée (par exemple, envoyez une requête `FindNetworkStatus` avec `iroha_cli`, confirmez que les points de terminaison de l'exposition de télémétrie `nexus_lane_state_total` et vérifiez que les paramètres du forum de diffusion en continu ont été tournés ou importés conformément aux exigences).## Etapa 6 - Transition et transfert
1. Garder le `manifest.json` vérifié et les artefatos de assinatura aucun ticket de libération pour que les auditeurs puissent reproduire leurs vérifications.
2. Notifique Nexus Operations de que o esta pronto para être introduzido ; comprend :
   - Identidade ne pas (ID d'homologue, noms d'hôte, point final Torii).
   - Valeurs efficaces du catalogue de voie/espace de données et politique de routage.
   - Hachages des fichiers binaires/images vérifiés.
3. Coordonner l'admission finale des pairs (gossip seed et attribution de lane) avec `@nexus-core`. Nao entre na rede mangé receber aprovacao; Sora Nexus s'applique à l'occupation déterministe des voies et exige un manifeste d'admission actualisé.
4. Une fois que vous n'êtes pas actif, actualisez vos runbooks pour que les remplacements que vous avez introduits et notés la balise de publication afin que la prochaine itération vienne à partir de cette ligne de base.

## Checklist de référence
- [ ] Profil de version validé comme `iroha3`.
- [ ] Les hachages et les éliminations sont effectués par bundle/image vérifiés.
- [ ] Chaves, enderecos de peers et endpoints Torii actualisés pour les valeurs de production.
- [ ] Le catalogue des voies/espaces de données et la politique de routage du Nexus correspondent à l'attribution du conseil.
- [ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) passa sem avisos.
- [ ] Manifestes/assinaturas archivados no ticket de onboarding e Ops notificado.Pour plus d'informations sur les phases de migration vers Nexus et les attentes de télémétrie, révisez les [notes de transition Nexus] (./nexus-transition-notes).