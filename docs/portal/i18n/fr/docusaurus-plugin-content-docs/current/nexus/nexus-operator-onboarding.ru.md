---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
titre : Organisation des opérateurs d'espace de données Sora Nexus
description : Le commutateur `docs/source/sora_nexus_operator_onboarding.md`, fournit une réponse de bout en bout pour les opérateurs Nexus.
---

:::note Канонический источник
Cette page correspond à `docs/source/sora_nexus_operator_onboarding.md`. Il est possible de copier des copies de synchronisation si la localisation n'est pas disponible sur le portail.
:::

# Intégration des opérateurs de l'espace de données Sora Nexus

Il s'agit d'une solution de bout en bout qui permet aux opérateurs d'espace de données Sora Nexus de prendre connaissance de la décision. En ajoutant un runbook double piste (`docs/source/release_dual_track_runbook.md`) et en installant des objets d'art (`docs/source/release_artifact_selection.md`), vous découvrirez comment télécharger des bundles/images, des manifestes et Les configurations globales des voies pour votre utilisation en ligne.

## Auditeurs et prédicteurs
- Vous avez créé le programme Nexus et avez défini l'espace de données (index de voie, ID/alias de l'espace de données et stratégie de routage).
- Vous êtes en mesure de télécharger des artefacts de version de Release Engineering (archives tar, images, manifestes, signatures, clés publiques).
- Vous avez sélectionné ou acheté du matériel clé pour le rôle de validateur/observateur (Ed25519 identifiant votre utilisateur ; BLS консенсусный ключ + PoP pour les validateurs ; plus любые bascule la fonction confidentielle).
- Vous pouvez télécharger les pairs Sora Nexus en lien avec votre bootstrap.## Partie 1 - Modifier la version du profil
1. Choisissez l'alias ou l'ID de chaîne que vous avez choisi.
2. Choisissez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) dans ce dépôt de caisse. Helper utilise `release/network_profiles.toml` et vous permet de déployer le profil. Pour Sora Nexus, vous devez utiliser `iroha3`. Pour trouver la solution, vous devez vous connecter à Release Engineering.
3. Sélectionnez la version de la balise correspondant à la version finale (par exemple `iroha3-v3.2.0`) ; он понадобится для загрузки artefacts et manifestes.

## Partie 2 - Rechercher et vérifier les œuvres d'art
1. Téléchargez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et votre téléchargement (`.sha256`, optionnel `.sig/.pub`, `<profile>-<version>-manifest.json` et `<profile>-<version>-image.json`, si vous ne déployez pas de conteneurs).
2. Vérifiez la vitesse avant de procéder :
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Veuillez contacter `openssl` pour le vérificateur de l'organisation en question, si vous n'utilisez pas KMS approprié.
3. Ajoutez `PROFILE.toml` à l'archive tar et aux manifestes JSON pour obtenir :
   -`profile = "iroha3"`
   - Pour `version`, `commit`, `built_at`, ils sont associés à une réponse anonyme.
   - OS/ARCHITECTURA соответствуют цели деплоя.
4. Si vous utilisez une image de conteneur, vous devez fournir le hachage/signature pour `<profile>-<version>-<os>-image.tar` et modifier l'ID de l'image à partir de `<profile>-<version>-image.json`.## Partie 3 - Configuration des paramètres des blocs
1. Téléchargez le bundle et copiez `config/` maintenant, puis utilisez la configuration.
2. Ouvrez le module `config/` du câble :
   - Entrez `public_key`/`private_key` sur le bouton Ed25519. Choisissez votre clé privée sur le disque si vous l'utilisez avec HSM ; Vérifiez la configuration pour ouvrir le connecteur HSM.
   - Connectez-vous à `trusted_peers`, `network.address` et `torii.address` pour que vous puissiez télécharger vos pairs et vos pairs bootstrap.
   - Connectez le point de terminaison `client.toml` avec le point de terminaison Torii côté opérateur (avec configuration TLS en cas de besoin) et les données disponibles pour outillage d'exploitation.
3. Si vous utilisez l'ID de chaîne du bundle, si la gouvernance n'est pas disponible - la voie globale affiche l'identifiant de chaîne canonique.
4. Planifiez votre utilisation avec le profil du drapeau Sora : `irohad --sora --config <path>`. Le chargeur de configuration ouvre la configuration SoraFS ou multivoie, si le drapeau est ouvert.## Partie 4 - Installer l'espace de données et le routage métadonnées
1. Ouvrez `config/config.toml`, pour le `[nexus]`, le catalogue de données de l'espace, en passant par le Conseil Nexus :
   - `lane_count` должен равняться общему числу voies, включенных в текущей эпохе.
   - Il est possible d'ouvrir le `[[nexus.lane_catalog]]` et le `[[nexus.dataspace_catalog]]` uniquement pour le `index`/`id` et le système. pseudonymes. Ne vous souciez pas des pages globales ; créez des alias délégués si vous souhaitez créer des espaces de données supplémentaires.
   - Убедитесь, что каждая запись dataspace включает `fault_tolerance (f)` ; комитеты voie-relais имеют размер `3f+1`.
2. Ouvrez `[[nexus.routing_policy.rules]]` pour ouvrir votre politique. Shablon doit établir des instructions de gouvernance sur la voie `1` et déployer des contrats sur la voie `2` ; Essayez ou modifiez le trafic de votre espace de données dans la voie principale et l'alias. Connectez-vous à Release Engineering avant de procéder à la planification.
3. Vérifiez les numéros `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Ожидается, что операторы сохраняют значения, одобренные советом; изменяйте их только при ратификации обновленной politique.
4. Configurez la configuration finale en fonction du fonctionnement du voyage. Le runbook double piste permet d'utiliser efficacement `config.toml` (avec la rédaction des secrets) pour le billet d'entrée.## Partie 5 - Validation préalable
1. Sélectionnez la configuration du validateur avant de procéder à l'installation :
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   La commande vous permet de modifier la configuration et de régler le problème, si vous ouvrez un catalogue/une mise en marché incohérente ou que Genesis et la configuration sont terminées.
2. Si vous déployez le conteneur, utilisez votre commande pour ouvrir après avoir enregistré `docker load -i <profile>-<version>-<os>-image.tar` (ne pas utiliser `--sora`).
3. Vérifiez les journaux de préparation de l'espace réservé pour l'identification de la voie/de l'espace de données. Si c'est le cas, vérifiez le Chapitre 4 - vous ne pourrez pas déployer les identifiants d'espace réservé dans les fichiers.
4. Vérifiez le détecteur de fumée local (par exemple, ouvrez `FindNetworkStatus` à partir de `iroha_cli`, vérifiez que les points finaux de télémétrie sont publiés `nexus_lane_state_total`, et vous devez savoir si les clés de streaming sont transférées ou importées).## Partie 6 – Cutover et transfert
1. Connectez-vous au `manifest.json` et aux objets de signature en temps réel pour que les auditeurs puissent vous fournir des preuves.
2. Effectuez les opérations Nexus pour utiliser les accès à votre ordinateur ; включите:
   - Identifiez l'utilisateur (ID d'homologue, noms d'hôte, point de terminaison Torii).
   - Effets de la voie/de l'espace de données et de la politique de routage du catalogue.
   - Ce sont des binarniks/образов éprouvés.
3. Coordonnez vos derniers pairs (gossip seeds et назначение lane) avec `@nexus-core`. Ne vous intéressez pas à ce sujet, mais vous ne pouvez pas le faire ; Sora Nexus s'occupe de la détermination des voies d'occupation et du manifeste d'admission обновленного.
4. Une fois que vous avez utilisé les runbooks avec vos remplacements et ajouté la version de balise, vous avez démarré l'itération avec cette ligne de base.

## Справочный чеклист
- [ ] La version du profil est passée à `iroha3`.
- [ ] Ce bundle/image est vérifié.
- [ ] Clés, adresses des pairs et points de terminaison Torii mis à jour pour le produit.
- [ ] Catalogue voies/espace de données et politique de routage Nexus соответствуют назначению совета.
- [ ] La configuration du validateur (`irohad --sora --config ... --trace-config`) est effectuée sans préavis.
- [ ] Manifestes/signatures заархивированы в тикете онбординга и Ops уведомлен.

Pour plus de contexte de migration de sites Nexus et d'installations télémétriques. [Notes de transition Nexus] (./nexus-transition-notes).