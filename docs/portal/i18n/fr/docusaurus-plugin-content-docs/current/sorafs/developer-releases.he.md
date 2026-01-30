---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8dbae5f69eaada4d3853e196aa31eb966bca923d7f587a4043dab8f664c22c5f
source_last_modified: "2025-11-14T04:43:21.652947+00:00"
translation_last_reviewed: 2026-01-30
---

# Processus de release

Les binaires SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) et les crates SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sont livrés ensemble. Le pipeline
de release garde le CLI et les bibliothèques alignés, assure la couverture lint/test
et capture des artefacts pour les consommateurs downstream. Exécutez la checklist
ci-dessous pour chaque tag candidat.

## 0. Confirmer la validation de la revue sécurité

Avant d'exécuter le gate technique de release, capturez les derniers artefacts de
revue sécurité :

- Téléchargez le mémo de revue sécurité SF-6 le plus récent ([reports/sf6-security-review](./reports/sf6-security-review.md))
  et enregistrez son hash SHA256 dans le ticket de release.
- Joignez le lien du ticket de remédiation (par ex. `governance/tickets/SF6-SR-2026.md`) et notez
  les approbateurs de Security Engineering et du Tooling Working Group.
- Vérifiez que la checklist de remédiation du mémo est clôturée ; les éléments non résolus bloquent la release.
- Préparez l'upload des logs du harness de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  avec le bundle de manifest.
- Confirmez que la commande de signature que vous comptez exécuter inclut à la fois `--identity-token-provider` et
  un `--identity-token-audience=<aud>` explicite pour capturer le scope Fulcio dans les preuves de release.

Incluez ces artefacts lors de la notification à la gouvernance et de la publication.

## 1. Exécuter le gate de release/tests

Le helper `ci/check_sorafs_cli_release.sh` exécute le formatage, Clippy et les tests
sur les crates CLI et SDK avec un répertoire target local au workspace (`.target`)
pour éviter les conflits de permissions lors de l'exécution dans des conteneurs CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Le script effectue les assertions suivantes :

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (avec la feature `cli`),
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` pour ces mêmes crates

Si une étape échoue, corrigez la régression avant de tagger. Les builds de release
doivent être continus avec main ; ne cherry-pickez pas de correctifs dans des branches
release. Le gate vérifie aussi que les flags de signature keyless (`--identity-token-issuer`,
`--identity-token-audience`) sont fournis quand requis ; les arguments manquants font
échouer l'exécution.

## 2. Appliquer la politique de versioning

Tous les crates CLI/SDK SoraFS utilisent SemVer :

- `MAJOR` : Introduit pour la première release 1.0. Avant 1.0, le bump mineur `0.y`
  **indique des changements cassants** dans la surface du CLI ou les schémas Norito.
- `MINOR` : Nouvelles fonctionnalités (nouveaux commandes/flags, nouveaux champs Norito
  derrière une politique optionnelle, ajouts de télémétrie).
- `PATCH` : Corrections de bugs, releases uniquement documentation et mises à jour de
  dépendances qui ne modifient pas le comportement observable.

Gardez toujours `sorafs_car`, `sorafs_manifest` et `sorafs_chunker` à la même version
pour que les consommateurs SDK downstream puissent dépendre d'une seule chaîne de version
alignée. Lors des bumps de version :

1. Mettez à jour les champs `version =` dans chaque `Cargo.toml`.
2. Régénérez le `Cargo.lock` via `cargo update -p <crate>@<new-version>` (le workspace
   impose des versions explicites).
3. Relancez le gate de release afin d'éviter les artefacts périmés.

## 3. Préparer les notes de release

Chaque release doit publier un changelog en markdown mettant en avant les changements
impactant le CLI, le SDK et la gouvernance. Utilisez le template dans
`docs/examples/sorafs_release_notes.md` (copiez-le dans votre répertoire d'artefacts
release et remplissez les sections avec des détails concrets).

Contenu minimal :

- **Highlights** : titres de fonctionnalités pour les consommateurs CLI et SDK.
- **Compatibilité** : changements cassants, upgrades de politiques, exigences minimales
  gateway/nœud.
- **Étapes d'upgrade** : commandes TL;DR pour mettre à jour les dépendances cargo et
  relancer les fixtures déterministes.
- **Vérification** : hashes de sortie ou enveloppes et révision exacte de
  `ci/check_sorafs_cli_release.sh` exécutée.

Joignez les notes de release remplies au tag (par ex. corps de la release GitHub) et
stockez-les à côté des artefacts générés de façon déterministe.

## 4. Exécuter les hooks de release

Exécutez `scripts/release_sorafs_cli.sh` pour générer le bundle de signatures et le
résumé de vérification livrés avec chaque release. Le wrapper construit le CLI si
nécessaire, appelle `sorafs_cli manifest sign` et rejoue immédiatement
`manifest verify-signature` pour faire remonter les échecs avant le tag. Exemple :

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

Tips :

- Suivez les inputs de release (payload, plans, summaries, hash de token attendu)
  dans votre repo ou config de déploiement afin de garder le script reproductible.
  Le bundle CI sous `fixtures/sorafs_manifest/ci_sample/` montre le layout canonique.
- Basez l'automatisation CI sur `.github/workflows/sorafs-cli-release.yml` ; elle exécute
  le gate de release, invoque le script ci-dessus et archive bundles/signatures comme
  artefacts de workflow. Reproduisez le même ordre de commandes (gate → signature →
  vérification) dans d'autres systèmes CI pour aligner les logs d'audit avec les hashes.
- Gardez `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` et
  `manifest.verify.summary.json` ensemble : ils forment le paquet référencé dans la
  notification de gouvernance.
- Lorsque la release met à jour des fixtures canoniques, copiez le manifest rafraîchi,
  le chunk plan et les summaries dans `fixtures/sorafs_manifest/ci_sample/` (et mettez
  à jour `docs/examples/sorafs_ci_sample/manifest.template.json`) avant le tag. Les
  opérateurs downstream dépendent des fixtures commités pour reproduire le bundle.
- Capturez le log d'exécution de la vérification des bounded-channels de
  `sorafs_cli proof stream` et joignez-le au paquet de release pour démontrer que les
  garde-fous de proof streaming restent actifs.
- Notez l'`--identity-token-audience` exact utilisé lors de la signature dans les notes
  de release ; la gouvernance recoupe l'audience avec la politique Fulcio avant approbation.

Utilisez `scripts/sorafs_gateway_self_cert.sh` quand la release inclut aussi un rollout
gateway. Pointez-le sur le même bundle de manifest pour prouver que l'attestation
correspond à l'artefact candidat :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tagger et publier

Après le passage des checks et la fin des hooks :

1. Exécutez `sorafs_cli --version` et `sorafs_fetch --version` pour confirmer que les binaires
   reportent la nouvelle version.
2. Préparez la configuration de release dans un `sorafs_release.toml` versionné (préféré)
   ou un autre fichier de config suivi par votre repo de déploiement. Évitez de dépendre
   de variables d'environnement ad-hoc ; passez les chemins au CLI avec `--config` (ou
   équivalent) afin que les inputs soient explicites et reproductibles.
3. Créez un tag signé (préféré) ou un tag annoté :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Uploadez les artefacts (bundles CAR, manifests, résumés de proofs, notes de release,
   outputs d'attestation) vers le registry du projet selon la checklist de gouvernance
   dans le [guide de déploiement](./developer-deployment.md). Si la release a produit
   de nouvelles fixtures, poussez-les vers le repo de fixtures partagé ou l'object store
   afin que l'automatisation d'audit puisse comparer le bundle publié au source control.
5. Notifiez le canal de gouvernance avec les liens vers le tag signé, les notes de release,
   les hashes du bundle/signatures du manifest, les résumés archivés `manifest.sign/verify`
   et tout enveloppe d'attestation. Incluez l'URL du job CI (ou l'archive de logs) qui a
   exécuté `ci/check_sorafs_cli_release.sh` et `scripts/release_sorafs_cli.sh`. Mettez à
   jour le ticket de gouvernance pour que les auditeurs puissent relier les approbations
   aux artefacts ; lorsque `.github/workflows/sorafs-cli-release.yml` envoie des notifications,
   liez les hashes enregistrés au lieu de coller des résumés ad-hoc.

## 6. Suivi post-release

- Assurez-vous que la documentation pointant vers la nouvelle version (quickstarts, templates CI)
  est à jour ou confirmez qu'aucun changement n'est requis.
- Créez des entrées de roadmap si un travail de suivi est nécessaire (par ex. flags de migration,
- Archivez les logs de sortie du gate de release pour les auditeurs : stockez-les à côté des
  artefacts signés.

Suivre ce pipeline maintient le CLI, les crates SDK et les éléments de gouvernance
alignés à chaque cycle de release.
