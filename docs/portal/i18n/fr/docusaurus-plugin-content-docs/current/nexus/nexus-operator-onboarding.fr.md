---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : intégration de l'opérateur Nexus
titre : Intégration des opérateurs de data-space Sora Nexus
description : Miroir de `docs/source/sora_nexus_operator_onboarding.md`, suivant la checklist de release de bout en bout pour les opérateurs Nexus.
---

:::note Source canonique
Cette page reflète `docs/source/sora_nexus_operator_onboarding.md`. Gardez les deux exemplaires alignés jusqu'à l'arrivée des éditions localisées sur le portail.
:::

# Intégration des opérateurs de data-space Sora Nexus

Ce guide capture le flux de bout en bout que les opérateurs de data-space Sora Nexus doivent suivre une fois une annonce de sortie. Il complète le runbook à double voie (`docs/source/release_dual_track_runbook.md`) et la note de sélection d'artefacts (`docs/source/release_artifact_selection.md`) en décrivant comment aligner les bundles/images téléchargés, les manifestes et les modèles de configuration avec les attentes globales de voies avant de mettre un nœud en ligne.## Public et prérequis
- Vous avez été approuvé par le Programme Nexus et avez reçu votre affectation de data-space (index de lane, data-space ID/alias et exigences de politique de routage).
- Vous pouvez accéder aux artefacts signes du release publies par Release Engineering (archives tar, images, manifestes, signatures, clés publiques).
- Vous avez généré ou récupéré le matériel de clés de production pour votre rôle de validateur/observateur (identité de noeud Ed25519; clé de consensus BLS + PoP pour les validateurs; plus tout toggle de fonctionnalité confidentielle).
- Vous pouvez joindre les paires Sora Nexus existantes qui bootstrappent votre noeud.

## Etape 1 - Confirmer le profil de release
1. Identifiez l'alias de réseau ou le chain ID qui vous a été donnée.
2. Lancez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) sur une caisse de ce dépôt. L'assistant consulte `release/network_profiles.toml` et imprime le profil du déployeur. Pour Sora Nexus la réponse doit être `iroha3`. Pour toute autre valeur, arrêtez et contactez Release Engineering.
3. Notez le tag de version référence par l'annonce du release (par exemple `iroha3-v3.2.0`) ; vous l'utiliserez pour récupérer les artefacts et manifestes.## Etape 2 - Récupérer et valider les artefacts
1. Téléchargez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et ses fichiers compagnons (`.sha256`, optionnel `.sig/.pub`, `<profile>-<version>-manifest.json`, et `<profile>-<version>-image.json` si vous déployez des conteneurs).
2. Validez l'intégrité avant de décompresser :
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` par le vérificateur approuve par l'organisation si vous utilisez un matériel KMS.
3. Inspectez `PROFILE.toml` dans le tarball et les manifestes JSON pour confirmer :
   -`profile = "iroha3"`
   - Les champs `version`, `commit` et `built_at` correspondant à l'annonce du release.
   - L'OS/architecture correspond à votre cible de déploiement.
4. Si vous utilisez l'image conteneur, répétez la vérification hash/signature pour `<profile>-<version>-<os>-image.tar` et confirmez l'image ID enregistrée dans `<profile>-<version>-image.json`.## Etape 3 - Préparer la configuration à partir des templates
1. Extrayez le bundle et copiez `config/` vers l'emplacement ou le noeud lire dans la configuration.
2. Traitez les fichiers sous `config/` comme des templates :
   - Remplacez `public_key`/`private_key` par vos clés Ed25519 de production. Supprimez les clés privées du disque si le noeud les sources depuis un HSM; mettez à jour la configuration pour pointer vers le connecteur HSM.
   - Ajustez `trusted_peers`, `network.address` et `torii.address` afin qu'ils reflètent vos interfaces accessibles et les paires bootstrap assignées.
   - Mettez à jour `client.toml` avec l'endpoint Torii côté opérateur (y compris la configuration TLS si applicable) et les identifiants fournis pour l'outillage opérationnel.
3. Conservez le chain ID fourni dans le bundle sauf instruction explicite de Governance - la voie globale attend un identifiant de chaine canonique unique.
4. Planifiez le démarrage du noeud avec le drapeau de profil Sora : `irohad --sora --config <path>`. Le chargeur de configuration rejettera les paramètres SoraFS ou multi-voies si le drapeau est absent.## Etape 4 - Aligner les métadonnées de data-space et le routage
1. Editez `config/config.toml` pour que la section `[nexus]` corresponde au catalogue de data-space fourni par le Conseil Nexus :
   - `lane_count` doit égaler le total des voies actives dans l'époque courante.
   - Chaque entrée dans `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` doit contenir un `index`/`id` unique et les alias convenus. Ne supprimez pas les entrées globales existantes ; ajoutez vos alias délégués si le conseil a attribué des espaces de données additionnels.
   - Assurez-vous que chaque entrée de dataspace inclut `fault_tolerance (f)` ; les comités voie-relais sont dimensionnés à `3f+1`.
2. Mettez à jour `[[nexus.routing_policy.rules]]` pour capturer la politique qui vous a été attribuée. Le modèle par défaut route les instructions de gouvernance vers la voie `1` et les déploiements de contrats vers la voie `2` ; ajoutez ou modifiez des règles pour que le trafic destiné à votre espace de données soit dirigé vers la voie et l'alias corrects. Coordonnez avec Release Engineering avant de changer l'ordre des règles.
3. Revoyez les seuils `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Les opérateurs sont censés conserver les valeurs approuvées par le conseil; Ajustez-les uniquement si une politique mise à jour à été ratifiée.4. Enregistrez la configuration finale dans votre tracker d'opérations. Le runbook de release a double voie exige d'attacher le `config.toml` effectif (secrets rediges) au ticket d'onboarding.

## Etape 5 - Pré-vol de validation
1. Exécutez le validateur de configuration intégré avant de rejoindre le réseau :
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela imprime la configuration résolue et échoue si les entrées catalogue/routage sont incohérentes ou si la genèse et la configuration sont divergentes.
2. Si vous déployez des conteneurs, exécutez la même commande dans l'image après l'avoir chargé avec `docker load -i <profile>-<version>-<os>-image.tar` (pensez à inclure `--sora`).
3. Vérifiez les logs pour des avertissements sur des identifiants placeholder de lane/data-space. Si besoin, revenez à l'étape 4 - les déploiements de production ne doivent pas dépendre des livres d'espace réservé IDs avec les modèles.
4. Exécutez votre procédure smoke locale (p. ex. soumettre une requête `FindNetworkStatus` avec `iroha_cli`, confirmer que les endpoints telemetrie exposent `nexus_lane_state_total`, et vérifier que les clés de streaming sont tournées ou importées selon les exigences).## Etape 6 - Cutover et hand-off
1. Stockez le `manifest.json` vérifier et les artefacts de signature dans le ticket de release pour que les auditeurs puissent reproduire vos vérifications.
2. Informez Nexus Operations que le nœud est prêt à être introduit ; incluez:
   - Identité du noeud (peer ID, noms d'hôtes, endpoint Torii).
   - Valeurs effectives du catalogue lane/data-space et politique de routage.
   - Les hachages des binaires/images vérifient.
3. Coordonnez l'admission finale des paires (gossip seeds et affectation de lane) avec `@nexus-core`. Ne rejoignez pas le réseau avant d'avoir reçu l'approbation ; Sora Nexus applique une occupation déterministe des voies et nécessite un manifeste d'admission mis à jour.
4. Après la mise en ligne du noeud, mettez à jour vos runbooks avec les overrides introduites et notez le tag de release pour que la prochaine itération fasse partie de cette ligne de base.## Checklist de référence
- [ ] Profil de release valide comme `iroha3`.
- [ ] Hashes et signatures du bundle/image vérifie.
- [ ] Cles, adresses de paires et endpoints Torii mis à jour en production de valeurs.
- [ ] Catalogue lanes/dataspace et politique de routage Nexus correspondant à l'affectation du conseil.
- [ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) passe sans avertissements.
- [ ] Archives des manifestes/signatures dans le ticket d'onboarding et Ops notifie.

Pour un contexte plus large sur les phases de migration Nexus et les attentes de télémétrie, consultez [Nexus transition notes](./nexus-transition-notes).