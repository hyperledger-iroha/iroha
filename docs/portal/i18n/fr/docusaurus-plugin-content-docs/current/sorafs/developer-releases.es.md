---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Procès de lancement
résumé : Exécuter la porte de lancement de CLI/SDK, appliquer la politique de version partagée et publier des notes de lancement canoniques.
---

# Processus de lancement

Les binaires de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) et les caisses du SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sont des juntos publicains. El pipeline
de lancement pour maintenir la CLI et les bibliothèques alignées, assurer la couverture de
peluches/tests et capture d'artefacts pour les consommateurs en aval. Éjecter la liste de
vérification de l'abajo pour chaque tag candidat.

## 0. Confirmer l'approbation de la révision de sécurité

Avant d'exécuter le portail technique de lancement, capturez les objets plus
récents de la révision de la sécurité :- Téléchargez le mémo le plus récent de révision de sécurité SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  et enregistrez votre hash SHA256 sur le ticket de lancement.
- Ajouter l'enlacement du ticket de remédiation (par exemple, `governance/tickets/SF6-SR-2026.md`) et note
  les arobateurs du groupe de travail sur l'ingénierie de sécurité et l'outillage.
- Vérifier que la liste de remédiation du mémo est fermée ; Les éléments sans résolveur bloquent le lancement.
- Préparer pour subir les bûches du harnais de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  avec le bundle du manifeste.
- Confirma que le commando de firma que planas ejecutar incluya tanto `--identity-token-provider` como
  un `--identity-token-audience=<aud>` explique clairement que l'alcance de Fulcio est capturée dans la preuve de sa libération.

Inclut ces artefacts pour notifier la gouvernance et publier le lancement.

## 1. Exécuter la porte de lancement/pruebas

L'assistant `ci/check_sorafs_cli_release.sh` exécute le format, Clippy et tests
sur les caisses de CLI et SDK avec un répertoire cible local dans l'espace de travail (`.target`)
pour éviter les conflits de permis lors de l'exécution à l'intérieur des conteneurs CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Le script réalise les implémentations suivantes :

- `cargo fmt --all -- --check` (espace de travail)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (avec la fonctionnalité `cli`),
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` pour ces caisses mismosSi quelque chose n'arrive pas, corrigez la régression avant l'étiquette. Les builds de sortie
deben estar continue avec main; il n'est pas nécessaire de sélectionner les correctifs dans les dernières versions.
El gate aussi comprueba que los flags de firma sin claves (`--identity-token-issuer`,
`--identity-token-audience`) se proporcionen donde corresponda; les arguments
des erreurs surviennent lors de l'exécution.

## 2. Appliquer la politique de version

Tous les crates de CLI/SDK de SoraFS utilisant SemVer :

- `MAJOR` : présenté pour la version 1.0 du primaire. Avant la version 1.0 de l'incrément
  mineur `0.y` **indica change avec rupture** en surface del CLI ou en los
  esquemas Norito.
- `MINOR` : Trabajo de funciones sin ruptura (nuevos comandos/flags, nuevos
  champs Norito protégés par la politique optionnelle, ajouts de télémétrie).
- `PATCH` : Correcciones de bugs, releases solo de documentation and actualizaciones de
  dépendances qui ne modifient pas le comportement observable.

Gardez toujours `sorafs_car`, `sorafs_manifest` et `sorafs_chunker` dans la même version
pour que les utilisateurs du SDK en aval puissent dépendre d'une chaîne unique
alineada. Toutes les versions incrémentielles :1. Actualisez les champs `version =` et chaque `Cargo.toml`.
2. Régénérez le `Cargo.lock` via `cargo update -p <crate>@<new-version>` (l'espace de travail
   exige des versions explicites).
3. Exécutez la porte de lancement une autre fois pour vous assurer qu'aucun objet ne doit être détruit
   obsolètes.

## 3. Préparer les notes de lancement

Chaque publication doit publier un journal des modifications et un markdown qui résout les changements que
impact sur CLI, SDK et Gobernanza. Utiliser la plante fr
`docs/examples/sorafs_release_notes.md` (copie du répertoire des artefacts de
libérer et compléter les sections avec les détails concrets).

Contenu minimum :

- **Destacados** : titulaires de fonctions pour les utilisateurs de CLI et SDK.
- **Impact** : changements avec rupture, mises à niveau politiques, exigences
  minimes de gateway/nodo.
- **Pasos de actualización** : commandes TL;DR pour actualiser les dépendances du fret et
  reejecutar luminaires déterministas.
- **Vérification** : hachages ou nombres de sorties de commandes et révision exacte
  de `ci/check_sorafs_cli_release.sh` exécuté.

Ajouter les notes de lancement complètes à l'étiquette (par exemple, le corps du déclencheur
sur GitHub) et protégés conjointement avec les artefacts générés de forme déterministe.

## 4. Exécuter les crochets de libérationProjet `scripts/release_sorafs_cli.sh` pour générer le bundle de sociétés et le
curriculum vitae de vérification qui est envoyé avec chaque version. Le wrapper compile la CLI
quand c'est nécessaire, appelez le `sorafs_cli manifest sign` et reproduisez immédiatement
`manifest verify-signature` pour que les chutes apparaissent avant l'étiquette.
Exemple :

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

Conséquences :- Enregistrer les entrées de la version (charge utile, plans, résumés, hachage du jeton attendu)
  dans votre dépôt ou configuration de déploiement pour que le script soit reproductible. Le paquet
  de luminaires en `fixtures/sorafs_manifest/ci_sample/` montre la disposition canonique.
- Basa l'automatisation de CI en `.github/workflows/sorafs-cli-release.yml` ; éjecté
  el gate de release, invoca el script anterior y archiva bundles/firmas como
  artefacts du flux de travail. Refleja el mismo orden de comandos (gate de release → firmar
  → vérifier) dans d'autres systèmes CI pour que les journaux de l'auditoire coïncident avec les
  hachages générés.
- Mantén juntos `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  et `manifest.verify.summary.json` ; former le paquet référencé dans la notification
  de gobernanza.
- Lorsque la version est actualisée, les appareils canoniques, copia le manifeste actualisé,
  el chunk plan et les résumés en `fixtures/sorafs_manifest/ci_sample/` (et actualisation
  `docs/examples/sorafs_ci_sample/manifest.template.json`) avant l'étiquette.
  Les opérateurs en aval dépendent des appareils versionados para reproducir
  le bundle de release.
- Capturer le journal d'exécution de la vérification des canaux associés de
  `sorafs_cli proof stream` et ajouté au paquet de version pour démontrer que
  les sauvegardes de preuve en streaming sont activées.
- Enregistrez le `--identity-token-audience` exactement utilisé au cours de l'entreprise.
  notes de lancement; gobernanza vérifie l'audience contre la politique de Fulcio
  avant d'approuver la publication.Usa `scripts/sorafs_gateway_self_cert.sh` lorsque la version inclut également un
déploiement de la passerelle. Apunta al mismo bundle de manifest para probar que la
l'attestation coïncide avec l'artefact candidat :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Étiquette et publication

Après que les contrôles aient été effectués et que les crochets soient terminés :1. Exécutez `sorafs_cli --version` et `sorafs_fetch --version` pour confirmer les binaires
   rapporter la nouvelle version.
2. Préparez la configuration de la version dans une version `sorafs_release.toml` (préférée)
   ou un autre fichier de configuration rastré par votre dépôt de téléchargement. Evita dépend de
   variables d'entreprise ad hoc ; passer à la CLI avec `--config` (ou équivalent) pour cela
   les entrées de la version sont explicites et reproductibles.
3. Créez une balise ferme (préférée) ou une balise annotée :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Sube los artefactos (bundles CAR, manifestes, resúmenes de proofs, notas de release,
   sorties d'attestation) au registre du projet en suivant la liste de contrôle de la gouvernance
   en la [guía de despliegue](./developer-deployment.md). Si la version a généré de nouveaux
   luminaires, sous-jacents au référentiel de luminaires partagé ou au magasin d'objets pour que le
   L'automatisation de l'auditoire peut comparer le paquet publié avec le contrôle de
   versions.
5. Notifier le canal de gouvernement avec des liens avec l'étiquette ferme, les notes de sortie, les hachages
   du bundle/firmas del manifest, résumés archivés de `manifest.sign/verify` et
   cualquier envoltorio de atestación. Incluez l'URL du travail CI (ou l'archive des journaux) que
   a été exécuté `ci/check_sorafs_cli_release.sh` et `scripts/release_sorafs_cli.sh`. Actualiser
   le ticket de gouvernement pour que les auditeurs puissent effectuer les autorisations à los
   artefacts; lorsque le travail `.github/workflows/sorafs-cli-release.yml` publiquenotifications, ajoutez les hachages enregistrés à la place des résultats ad hoc.

## 6. Suite à la libération postérieure

- Assurez-vous que la documentation apparaissant dans la nouvelle version (démarrages rapides, fiches de CI)
  esté actualizada ou confirme que aucun changement n’est requis.
- Enregistrer les entrées de la feuille de route si elles nécessitent un travail ultérieur (par exemple, les drapeaux de
- Archiva los logs de salida del gate de release para auditoría: guárdalos junto a los
  artefactos firmados.

Suivre ce pipeline pour maintenir la CLI, les caisses du SDK et le matériel de gestion
alineados para cada cycle de release.