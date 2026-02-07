---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Processus de publication
résumé : CLI/SDK release gate Voir la politique de versioning et les notes de version canoniques ici
---

# Processus de publication

Binaires SoraFS (`sorafs_cli`, `sorafs_fetch`, assistants) et caisses SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) Navire en ligne ہوتے ہیں۔ Libération
pipeline CLI et bibliothèques alignées pour la couverture lint/test
Les consommateurs en aval et les artefacts capturent les choses ہر tag candidat کے لیے
Liste de contrôle pour votre liste de contrôle

## 0. Approbation de l'examen de sécurité ici

La porte de publication technique et les artefacts d'examen de sécurité capturent les éléments :

- J'ai lu le mémo d'examen de sécurité SF-6 en cours de lecture ([reports/sf6-security-review](./reports/sf6-security-review.md))
  Il s'agit d'un ticket de libération de hachage SHA256 pour plus de détails.
- Lien du ticket de correction (مثلاً `governance/tickets/SF6-SR-2026.md`) منسلک کریں اور
  Ingénierie de sécurité et Groupe de travail sur l'outillage et les approbateurs de signature
- Un mémo et une liste de contrôle de remédiation sont disponibles. les éléments non résolus sont libérés et bloqués
- Journaux de faisceau de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) ici
  manifeste bundle کے ساتھ upload کرنے کے لیے تیار رہیں۔
- Il s'agit d'une commande de signature `--identity-token-provider`.
  واضح `--identity-token-audience=<aud>` شامل ہو تاکہ Fulcio scope release proof میں capture ہو۔

Gouvernance et publication des artefacts et des artefacts## 1. Libérer/tester la porte

`ci/check_sorafs_cli_release.sh` helper CLI pour les caisses SDK pour le formatage, Clippy et les tests
Il s'agit du répertoire cible local de l'espace de travail (`.target`) et du répertoire cible local de l'espace de travail (`.target`).
conteneurs et conflits d'autorisations

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

یہ script درج ذیل assertions کرتا ہے :

- `cargo fmt --all -- --check` (espace de travail)
- `cargo clippy --locked --all-targets` `sorafs_car` pour (fonction `cli` pour)
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` pour les caisses

Il s'agit d'un échec ou d'un marquage ou d'une régression. Release builds et main
کے ساتھ continu رہنا چاہیے؛ publier des branches et corriger des erreurs de sélection cerise Porte
Drapeaux de signature sans clé (`--identity-token-issuer`, `--identity-token-audience`)
جہاں ضروری ہوں فراہم کیے گئے ہوں؛ les arguments manquants s'exécutent ou échouent

## 2. Politique de gestion des versions par exemple

Caisses CLI/SDK SoraFS de SemVer pour les utilisateurs :

- `MAJOR` : la version 1.0 est présentée ici. 1.0 سے پہلے `0.y` bosse mineure
  **modifications avec rupture** Les schémas Norito sont également disponibles sur la surface CLI.
- `PATCH` : corrections de bogues, versions de documentation uniquement, mises à jour des dépendances et comportement observable pour les utilisateurs.

`sorafs_car`, `sorafs_manifest` et `sorafs_chunker` version en version anglaise
Consommateurs en aval du SDK avec chaîne de version alignée et dépendance Version bump کرتے وقت:1. Pour la caisse `Cargo.toml` et les champs `version =` pour les champs
2. `cargo update -p <crate>@<new-version>` کے ذریعے `Cargo.lock` régénérer کریں (les versions explicites de l'espace de travail appliquent کرتا ہے)۔
3. Libérer la porte pour les artefacts périmés et les objets périmés

## 3. Notes de version en cours

La publication et le journal des modifications Markdown ainsi que les changements ayant un impact sur la gouvernance
کو mettre en évidence کرے۔ `docs/examples/sorafs_release_notes.md` modèle de modèle de modèle (version officielle)
répertoire d'objets et détails concrets des sections et des détails concrets

Contenu minimum :

- **Points forts** : Clients CLI et SDK et titres des fonctionnalités
- **Étapes de mise à niveau** : les dépendances de chargement sont modifiées et les appareils déterministes réexécutent les commandes TL;DR۔
- **Vérification** : hachages de sortie de commande et enveloppes et `ci/check_sorafs_cli_release.sh` et révision exacte et exécution et

Voici les notes de version et la balise et attachez le fichier (corps de la version GitHub) et ajoutez-le de manière déterministe
artefacts générés کے ساتھ محفوظ کریں۔

## 4. Libérer les crochets

`scripts/release_sorafs_cli.sh` Un ensemble de signatures et un résumé de vérification génèrent une version et une version
کے ساتھ navire ہوتے ہیں۔ Wrapper est basé sur la version CLI build pour `sorafs_cli manifest sign` et est également disponible
`manifest verify-signature` replay et marquage des échecs et des échecs مثال:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Conseils :- Entrées de version (charge utile, plans, résumés, hachage de jeton attendu) et configuration du dépôt et du déploiement et suivi du script reproductible. `fixtures/sorafs_manifest/ci_sample/` et disposition canonique du bundle de luminaires
- Automatisation CI pour `.github/workflows/sorafs-cli-release.yml` pour base de données Release Gate et les scripts et les bundles/signatures et les artefacts de flux de travail et les archives et les archives. Les systèmes CI utilisent l'ordre de commande (libérer la porte -> signer -> vérifier) ​​les journaux d'audit générés par les hachages et les aligner automatiquement
- `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`, et `manifest.verify.summary.json` pour votre appareil photo یہی notification de gouvernance des paquets میں se référer à ہوتا ہے۔
- La publication des appareils canoniques comprend un manifeste actualisé, un plan de fragments et des résumés selon `fixtures/sorafs_manifest/ci_sample/` (voir `docs/examples/sorafs_ci_sample/manifest.template.json`). کریں) marquage سے پہلے۔ Les opérateurs en aval ont engagé des montages qui dépendent des versions du bundle de versions reproduites.
- Vérification du canal limité `sorafs_cli proof stream` pour la capture du journal d'exécution et le paquet de publication et l'attachement des garanties de diffusion en continu des preuves pour le transfert de données
- Signature des notes de version exactes `--identity-token-audience` et des notes de version exactes `--identity-token-audience`. gouvernance Fulcio politique کے خلاف audience کو recoupement کرتی ہے۔

Cette version est le déploiement de la passerelle pour le projet `scripts/sorafs_gateway_self_cert.sh`. Le lot de manifestes contient un point de référence pour l'artefact du candidat à l'attestation et correspond à :```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag اور publier

Vérifie les crochets et les crochets:

1. `sorafs_cli --version` et `sorafs_fetch --version` rapports de version binaires plus récents ici
2. Version de configuration et enregistrement `sorafs_release.toml` (préféré) et fichier de configuration et suivi du référentiel de déploiement. Variables d'environnement ad hoc CLI et `--config` (équivalent en ligne) pour les chemins d'accès et les entrées de libération et les fonctions reproductibles
3. Balise signée (de préférence) ou balise annotée par exemple :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artefacts (lots CAR, manifestes, résumés de preuves, notes de version, résultats d'attestation) et le registre du projet doit télécharger la liste de contrôle de gouvernance (guide de déploiement : [guide de déploiement] (./developer-deployment.md)) suivre Il s'agit d'une version publiée de luminaires, d'un dépôt de luminaires partagés et d'un magasin d'objets, ainsi que d'un push pour l'automatisation de l'audit, d'un bundle publié pour le contrôle de source, d'un diff et d'un diff.
5. Canal de gouvernance – balise signée, notes de version, ensembles de manifestes/hachages de signature, résumés `manifest.sign/verify` archivés, enveloppes d'attestation, liens et liens vers des liens URL de la tâche CI (archive du journal) se trouve entre `ci/check_sorafs_cli_release.sh` et `scripts/release_sorafs_cli.sh`. Ticket de gouvernance pour les approbations des auditeurs et les artefacts et les traces Voir la publication de notifications d'emploi `.github/workflows/sorafs-cli-release.yml` et les résumés ad hoc ainsi que le lien de hachage enregistré.

## 6. Suivi post-sortie- Dernière version de la documentation et de la documentation (démarrages rapides, modèles CI) درکار نہیں۔
- Libérer les journaux de sortie de la porte et les auditeurs et les archives - Ajouter les artefacts signés pour les archives

Il s'agit d'un pipeline et d'un cycle de publication, ainsi que de caisses CLI SDK et de garanties de gouvernance, ainsi que d'étapes de verrouillage.