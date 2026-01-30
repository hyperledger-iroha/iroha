---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operator-onboarding
title: Integration des operateurs de data-space Sora Nexus
description: Miroir de `docs/source/sora_nexus_operator_onboarding.md`, suivant la checklist de release de bout en bout pour les operateurs Nexus.
---

:::note Source canonique
Cette page reflete `docs/source/sora_nexus_operator_onboarding.md`. Gardez les deux copies alignees jusqu'a l'arrivee des editions localisees sur le portal.
:::

# Integration des operateurs de data-space Sora Nexus

Ce guide capture le flux de bout en bout que les operateurs de data-space Sora Nexus doivent suivre une fois un release annonce. Il complete le runbook a double voie (`docs/source/release_dual_track_runbook.md`) et la note de selection d'artefacts (`docs/source/release_artifact_selection.md`) en decrivant comment aligner les bundles/images telecharges, les manifests et les templates de configuration avec les attentes globales de lanes avant de mettre un noeud en ligne.

## Audience et prerequis
- Vous avez ete approuve par le Programme Nexus et avez recu votre affectation de data-space (index de lane, data-space ID/alias et exigences de politique de routage).
- Vous pouvez acceder aux artefacts signes du release publies par Release Engineering (tarballs, images, manifests, signatures, cles publiques).
- Vous avez genere ou recu le materiel de cles de production pour votre role de validator/observer (identite de noeud Ed25519; cle de consensus BLS + PoP pour les validators; plus tout toggle de fonctionnalite confidentielle).
- Vous pouvez joindre les pairs Sora Nexus existants qui bootstrappent votre noeud.

## Etape 1 - Confirmer le profil de release
1. Identifiez l'alias de reseau ou le chain ID qui vous a ete donne.
2. Lancez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) sur un checkout de ce depot. Le helper consulte `release/network_profiles.toml` et imprime le profil a deployer. Pour Sora Nexus la reponse doit etre `iroha3`. Pour toute autre valeur, arretez et contactez Release Engineering.
3. Notez le tag de version reference par l'annonce du release (par exemple `iroha3-v3.2.0`); vous l'utiliserez pour recuperer les artefacts et manifests.

## Etape 2 - Recuperer et valider les artefacts
1. Telechargez le bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) et ses fichiers compagnons (`.sha256`, optionnel `.sig/.pub`, `<profile>-<version>-manifest.json`, et `<profile>-<version>-image.json` si vous deployez des conteneurs).
2. Validez l'integrite avant de decompresser:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` par le verificateur approuve par l'organisation si vous utilisez un KMS materiel.
3. Inspectez `PROFILE.toml` dans le tarball et les manifests JSON pour confirmer:
   - `profile = "iroha3"`
   - Les champs `version`, `commit` et `built_at` correspondent a l'annonce du release.
   - L'OS/architecture correspond a votre cible de deploiement.
4. Si vous utilisez l'image conteneur, repetez la verification hash/signature pour `<profile>-<version>-<os>-image.tar` et confirmez l'image ID enregistree dans `<profile>-<version>-image.json`.

## Etape 3 - Preparer la configuration a partir des templates
1. Extrayez le bundle et copiez `config/` vers l'emplacement ou le noeud lira sa configuration.
2. Traitez les fichiers sous `config/` comme des templates:
   - Remplacez `public_key`/`private_key` par vos cles Ed25519 de production. Supprimez les cles privees du disque si le noeud les source depuis un HSM; mettez a jour la configuration pour pointer vers le connecteur HSM.
   - Ajustez `trusted_peers`, `network.address` et `torii.address` afin qu'ils refletent vos interfaces accessibles et les pairs bootstrap assignes.
   - Mettez a jour `client.toml` avec l'endpoint Torii cote operateur (y compris la configuration TLS si applicable) et les identifiants provisionnes pour l'outillage operationnel.
3. Conservez le chain ID fourni dans le bundle sauf instruction explicite de Governance - la lane globale attend un identifiant de chaine canonique unique.
4. Planifiez le demarrage du noeud avec le flag de profil Sora: `irohad --sora --config <path>`. Le chargeur de configuration rejettera les parametres SoraFS ou multi-lane si le flag est absent.

## Etape 4 - Aligner la metadata de data-space et le routage
1. Editez `config/config.toml` pour que la section `[nexus]` corresponde au catalogue de data-space fourni par le Nexus Council:
   - `lane_count` doit egaler le total des lanes activees dans l'epoque courante.
   - Chaque entree dans `[[nexus.lane_catalog]]` et `[[nexus.dataspace_catalog]]` doit contenir un `index`/`id` unique et les alias convenus. Ne supprimez pas les entrees globales existantes; ajoutez vos alias delegues si le conseil a attribue des data-spaces additionnels.
   - Assurez-vous que chaque entree de dataspace inclut `fault_tolerance (f)`; les comites lane-relay sont dimensionnes a `3f+1`.
2. Mettez a jour `[[nexus.routing_policy.rules]]` pour capturer la politique qui vous a ete attribuee. Le template par defaut route les instructions de gouvernance vers la lane `1` et les deploiements de contrats vers la lane `2`; ajoutez ou modifiez des regles pour que le trafic destine a votre data-space soit dirige vers la lane et l'alias corrects. Coordonnez avec Release Engineering avant de changer l'ordre des regles.
3. Revoyez les seuils `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Les operateurs sont censes conserver les valeurs approuvees par le conseil; ajustez-les uniquement si une politique mise a jour a ete ratifiee.
4. Enregistrez la configuration finale dans votre tracker d'operations. Le runbook de release a double voie exige d'attacher le `config.toml` effectif (secrets rediges) au ticket d'onboarding.

## Etape 5 - Validation pre-flight
1. Executez le validateur de configuration integre avant de rejoindre le reseau:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela imprime la configuration resolue et echoue tot si les entrees catalogue/routage sont incoherentes ou si genesis et config divergent.
2. Si vous deployez des conteneurs, executez la meme commande dans l'image apres l'avoir chargee avec `docker load -i <profile>-<version>-<os>-image.tar` (pensez a inclure `--sora`).
3. Verifiez les logs pour des avertissements sur des identifiants placeholder de lane/data-space. Si besoin, revenez a l'etape 4 - les deploiements production ne doivent pas dependre des IDs placeholder livres avec les templates.
4. Executez votre procedure smoke locale (p. ex. soumettre une requete `FindNetworkStatus` avec `iroha_cli`, confirmer que les endpoints telemetrie exposent `nexus_lane_state_total`, et verifier que les cles de streaming sont tournees ou importees selon les exigences).

## Etape 6 - Cutover et hand-off
1. Stockez le `manifest.json` verifie et les artefacts de signature dans le ticket de release pour que les auditeurs puissent reproduire vos verifications.
2. Informez Nexus Operations que le noeud est pret a etre introduit; incluez:
   - Identite du noeud (peer ID, hostnames, endpoint Torii).
   - Valeurs effectives du catalogue lane/data-space et politique de routage.
   - Hashes des binaires/images verifies.
3. Coordonnez l'admission finale des pairs (gossip seeds et affectation de lane) avec `@nexus-core`. Ne rejoignez pas le reseau avant d'avoir recu l'approbation; Sora Nexus applique une occupation deterministe des lanes et requiert un manifest d'admissions mis a jour.
4. Apres la mise en ligne du noeud, mettez a jour vos runbooks avec les overrides introduits et notez le tag de release pour que la prochaine iteration parte de cette baseline.

## Checklist de reference
- [ ] Profil de release valide comme `iroha3`.
- [ ] Hashes et signatures du bundle/image verifies.
- [ ] Cles, adresses de pairs et endpoints Torii mis a jour en valeurs production.
- [ ] Catalogue lanes/dataspace et politique de routage Nexus correspondent a l'affectation du conseil.
- [ ] Validateur de configuration (`irohad --sora --config ... --trace-config`) passe sans avertissements.
- [ ] Manifests/signatures archives dans le ticket d'onboarding et Ops notifie.

Pour un contexte plus large sur les phases de migration Nexus et les attentes de telemetrie, consultez [Nexus transition notes](./nexus-transition-notes).
