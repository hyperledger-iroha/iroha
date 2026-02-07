---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
titre : Incorporation des opérateurs d'espace de données de Sora Nexus
description : en particulier de `docs/source/sora_nexus_operator_onboarding.md`, qui suit la liste de contrôle de publication de bout en bout pour les opérateurs de Nexus.
---

:::note Fuente canonica
Cette page reflète `docs/source/sora_nexus_operator_onboarding.md`. Manten ambas copias alineadas hasta que las ediciones localizadas lleguen al portal.
:::

# Incorporation des opérateurs d'espace de données de Sora Nexus

Cette guía capture le flux de bout en bout qui doit suivre les opérateurs de l'espace de données de Sora Nexus une fois qu'ils annoncent une version. Complétez le runbook de double via (`docs/source/release_dual_track_runbook.md`) et la note de sélection des artefacts (`docs/source/release_artifact_selection.md`) pour décrire les paquets/images linéaires téléchargés, les manifestes et les plantes de configuration avec les attentes de voies globales avant de créer un nœud en ligne.## Audience et prérequis
- A été approuvé par le programme Nexus et reçoit votre attribution d'espace de données (indice de voie, ID/alias d'espace de données et conditions de politique de routage).
- Vous pouvez accéder aux artefacts firmados del release publiés par Release Engineering (archives tar, images, manifestes, firmas, clés publiques).
- A généré ou reçu du matériel de clés de production pour votre rôle de validateur/observateur (identité de nœud Ed25519 ; clé de consensus BLS + PoP pour validateurs ; mais cualquier toggle de funciones confidentielles).
- Vous pouvez contacter les pairs existants de Sora Nexus qui bootstrapent votre nœud.

## Paso 1 - Confirmer le profil de sortie
1. Identifiez l'alias de rouge ou l'ID de chaîne qui vous apparaîtra.
2. Exécutez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) pour effectuer une extraction de ce référentiel. L'assistant consulte `release/network_profiles.toml` et imprime le profil à télécharger. Pour Sora Nexus, la réponse doit être `iroha3`. Pour toute autre valeur, veuillez contacter Release Engineering.
3. Notez l'étiquette de version qui fait référence à l'annonce de la version (par exemple `iroha3-v3.2.0`) ; je l'utilise pour télécharger des artefacts et des manifestes.## Étape 2 - Récupérer et valider les artefacts
1. Téléchargez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et vos archives d'entreprise (`.sha256`, `.sig/.pub` en option, `<profile>-<version>-manifest.json` et `<profile>-<version>-image.json` si vous téléchargez des conteneurs).
2. Validez l'intégrité avant de décomprimer :
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` par le vérificateur approuvé par l'organisation si vous utilisez un KMS avec réponse au matériel.
3. Inspectez `PROFILE.toml` dans l'archive tar et les manifestes JSON pour confirmer :
   -`profile = "iroha3"`
   - Les champs `version`, `commit` et `built_at` coïncident avec l'annonce de la sortie.
   - El OS/arquitectura coïncide con tu objetivo de despliegue.
4. Si vous utilisez l'image du conteneur, répétez la vérification du hachage/de la société pour `<profile>-<version>-<os>-image.tar` et confirmez l'ID de l'image enregistrée en `<profile>-<version>-image.json`.## Étape 3 - Préparer la configuration des plantes
1. Extrayez le bundle et copiez `config/` à l'emplacement où le nœud lira votre configuration.
2. Trata los archivos baso `config/` como plantillas :
   - Remplacez `public_key`/`private_key` avec vos clés Ed25519 de production. Éliminez les clés privées de la discothèque si vous obtenez un HSM ; actualisez la configuration pour pointer le connecteur HSM.
   - Ajustez `trusted_peers`, `network.address` et `torii.address` pour refléter vos interfaces accessibles et les pairs de bootstrap assignés.
   - Mise à jour `client.toml` avec le point de terminaison Torii de la part de l'opérateur (y compris la configuration TLS si application) et les informations d'identification fournies pour l'opérateur de l'outillage.
3. Maintenir l'ID de chaîne à condition que le bundle soit moins que Governance l'indique explicitement : la voie globale attend un identifiant de chaîne canonique unique.
4. Plan initier le nœud avec le drapeau de profil Sora : `irohad --sora --config <path>`. Le chargeur de configuration rechazara ajuste le SoraFS ou le multivoie si le drapeau est affiché.## Étape 4 - Métadonnées linéaires de l'espace de données et du routage
1. Edita `config/config.toml` pour que la section `[nexus]` coïncide avec le catalogue d'espaces de données fourni par le Conseil Nexus :
   - `lane_count` doit être le même que le total des voies autorisées à l'époque actuelle.
   - Chaque entrée dans `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` doit contenir un `index`/`id` unique et l'alias correspondant. No élimine les entrées globales existantes; agrégez vos alias délégués si le conseiller attribue des espaces de données supplémentaires.
   - Assurez-vous que chaque entrée de l'espace de données inclut `fault_tolerance (f)` ; Les comités Lane-Relay sont dimensionnés en `3f+1`.
2. Actualiser `[[nexus.routing_policy.rules]]` pour capturer la politique que vous avez désignée. La plante par défaut contient des instructions de gestion sur la voie `1` et des plis de contrats sur la voie `2` ; ajoutez ou modifiez les règles pour que le trafic destiné à votre espace de données aille dans la voie et l'alias corrects. Coordonnez-vous avec Release Engineering avant de modifier l’ordre des règles.
3. Révisez les parapluies de `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. J'espère que les opérateurs conserveront les valeurs approuvées par le consommateur ; ajustalos solo si se ratifico unea politica actualizada.4. Enregistrez la configuration finale dans votre tracker d'opérations. Le runbook de publication de double via nécessite l'ajout du `config.toml` efficace (avec des secrets rédigés) au ticket d'intégration.

## Étape 5 - Validation préalable
1. Exécutez le validateur de configuration intégré avant de démarrer sur le rouge :
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Ceci imprime le résultat de la configuration et tombe en température si les entrées de catalogue/routage sont incohérentes ou si la genèse et la configuration ne coïncident pas.
2. Si vous dépliez les conteneurs, exécutez la même commande à l'intérieur de l'image après le chargement avec `docker load -i <profile>-<version>-<os>-image.tar` (recuerda incluant `--sora`).
3. Réviser les journaux pour les publicités sur les identifiants d'espace réservé de la voie/de l'espace de données. Si cela apparaît, notez l'étape 4 : les tâches de production ne doivent pas dépendre des espaces réservés ID qui viennent avec les plantes.
4. Exécutez votre procédure locale de fumée (par exemple, envoyez une consultation `FindNetworkStatus` avec `iroha_cli`, confirmez que les points de terminaison de télémétrie exposent `nexus_lane_state_total` et vérifiez que les clés de streaming tournent ou importent en conséquence).## Étape 6 - Transition et transfert
1. Gardez le `manifest.json` vérifié et les artefacts de l'entreprise sur le ticket de sortie afin que les auditeurs puissent reproduire vos vérifications.
2. Notifier les opérations Nexus que le nœud est dans la liste pour être introduit ; comprend :
   - Identité du nœud (ID d'homologue, noms d'hôte, point de terminaison Torii).
   - Valeurs effectives du catalogue de voie/espace de données et politique de routage.
   - Hachages des binaires/images à vérifier.
3. Coordonner l'admission finale des pairs (gossip seed et attribution de voie) avec `@nexus-core`. Ne vous laissez pas aller à la rouge jusqu'à ce que vous receviez une approbation ; Sora Nexus s'applique à l'occupation déterministe des voies et nécessite un manifeste d'admission actualisé.
4. Une fois que le nœud est là, actualisez vos runbooks avec tout remplacement qui est introduit et annotez la balise de publication pour que l'itération suivante s'effectue à partir de cette ligne de base.## Checklist de référence
- [ ] Profil de version validé comme `iroha3`.
- [ ] Hachages et entreprises du bundle/image vérifiés.
- [ ] Clés, directions de pairs et points de terminaison Torii actualisés en valeurs de production.
- [ ] Le catalogue des voies/espaces de données et la politique de routage de Nexus coïncident avec l'attribution du consejo.
- [ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) pasa sin advertencias.
- [ ] Manifestes/entreprises archivées sur le ticket d'intégration et les opérations notifiées.

Pour le contexte supplémentaire des phases de migration de Nexus et des attentes de télémétrie, révisez les [notes de transition Nexus] (./nexus-transition-notes).