---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Process реLISа
Résumé : Téléchargez la CLI/SDK, indiquez la version politique et ouvrez les notes canoniques.
---

# Processus de publication

Binaires SoraFS (`sorafs_cli`, `sorafs_fetch`, assistants) et caisses SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sélectionnez-le. Réel
pipeline fournit la CLI et les bibliothèques de logiciels, qui utilisent la fonction lint/test
et des articles de réparation pour les utilisateurs en aval. Consultez la liste de contrôle pour
каждого étiquette de candidat.

## 0. Mettre à jour la signature en cas de problème

Avant d'utiliser la porte de déverrouillage technique, vous devez examiner l'examen de sécurité de certains objets :

- Téléchargez le dernier mémorandum SF-6 en utilisant les informations supplémentaires ([reports/sf6-security-review](./reports/sf6-security-review.md))
  et téléchargez votre hachage SHA256 dans le ticket de sortie.
- Sélectionnez le ticket de remédiation (par exemple, `governance/tickets/SF6-SR-2026.md`) et supprimez-le.
  approuver le groupe de travail sur l'ingénierie de sécurité et l'outillage.
- Vérifiez la liste de contrôle de remédiation dans mon mémoire ; незакрытые пункты bloкируют реLIS.
- Подготовьте загрузку логов harnais de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  вместе с bundle manifeste.
- Assurez-vous que la commande soit en mesure de planifier votre achat, en cliquant sur `--identity-token-provider`, et
  явный `--identity-token-audience=<aud>`, чтобы scope Fulcio был зафиксирован в релизных preuves.

Découvrez ces articles dans le cadre de la gouvernance et des décisions publiques.## 1. Ouvrir la porte de libération/test

L'assistant `ci/check_sorafs_cli_release.sh` fournit des formats, Clippy et tests
Dans les caisses CLI et SDK du répertoire cible local de l'espace de travail (`.target`), vous devez utiliser
Les conflits se produisent dans les conteneurs CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Le script contient les preuves suivantes :

- `cargo fmt --all -- --check` (espace de travail)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (avec fonctionnalité `cli`),
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` pour ces caisses

Si vous êtes prêt à le faire, utilisez la régression pour le marquage. Relisnye сборки должны
идти непрерывно от principal; ne tardez pas à choisir les correctifs dans les versions de sortie. Porte
Vérifiez les drapeaux de signature sans clé (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; Les arguments avancés valent la peine.

## 2. Proposer une version politique

Les caisses CLI/SDK SoraFS utilisent SemVer :

- `MAJOR` : correspond à la version 1.0. Vers la version 1.0 mineure `0.y`
  **Définit la rupture** dans la surface CLI ou dans le schéma Norito.
  за опциональной политикой, добавления телеметрии).
- `PATCH` : Mise à jour des informations sur les informations relatives à la documentation uniquement et sur les mises à jour,
  не меняющие наблюдаемое поведение.

Sélectionnez `sorafs_car`, `sorafs_manifest` et `sorafs_chunker` dans votre version actuelle.
Le SDK en aval peut être utilisé sur la chaîne de version actuelle. Selon la version disponible :1. Connectez-vous au `version =` dans le cas `Cargo.toml`.
2. Sélectionnez `Cargo.lock` vers `cargo update -p <crate>@<new-version>` (espace de travail
   требует явных версий).
3. Veuillez ouvrir la porte de déverrouillage afin de ne pas installer les objets d'art usagés.

## 3. Ajouter les notes de version

Il est temps de publier le journal des modifications de Markdown avec l'accent sur la CLI et le SDK
и la gouvernance. Utilisez le sabot `docs/examples/sorafs_release_notes.md` (utilisez le
его в директорию релизных артефактов и заполните секции конкретикой).

Quantité minimale :

- **Points forts** : fiches techniques pour la CLI et le SDK disponibles.
- **Étapes de mise à niveau** : TL;DR команды для обновления cargo зависимостей и перезапуска
  детерминированных luminaires.
- **Vérification** : vos enveloppes ou vos enveloppes correspondent à votre commande et à votre révision.
  `ci/check_sorafs_cli_release.sh`, cela correspond à votre choix.

Téléchargez les notes de version ici (par exemple, dans la version GitHub) et
храните рядом с детерминированно сгенерированными артефактами.

## 4. Déverrouillez les crochets de déverrouillage

Utilisez `scripts/release_sorafs_cli.sh` pour générer un bundle de signatures et
résumé de vérification, которые отгружаются с каждым релизом. Wrapper pour les besoins
En utilisant CLI, vous pouvez utiliser `sorafs_cli manifest sign` et régler le problème.
`manifest verify-signature`, vous devez effectuer le marquage. Exemple :

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

Podcasts :- Ouvrir les entrées de réponse (charge utile, plans, résumés, hachage de jeton attendu)
  Dans les dépôts ou les configurations de déploiement, les scripts sont disponibles. CI
  bundle под `fixtures/sorafs_manifest/ci_sample/` propose une disposition canonique.
- Placez l'automatisation CI sur `.github/workflows/sorafs-cli-release.yml` ; sur выполняет
  release gate, vous pouvez créer un script et archiver des bundles/signatures pour les éléments de workflow.
  Повторяйте тот же порядок команд (libérer la porte → signer → vérifier) dans les systèmes CI,
  Ces journaux d'audit sont basés sur les hachages générés.
- Prenez `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` et
  `manifest.verify.summary.json` est un paquet pour la notification de gouvernance envoyée.
- Si vous consultez des appareils canoniques mis à jour, copiez le manifeste actuel, le plan de fragments et
  résumés dans `fixtures/sorafs_manifest/ci_sample/` (et обновите
  `docs/examples/sorafs_ci_sample/manifest.template.json`) pour le marquage. Opérateurs en aval
  зависят от закоммиченных luminaires для воспроизводимости release bundle.
- Enregistrez le journal pour vérifier les canaux limités pour `sorafs_cli proof stream` et
  J'utilise moi-même un paquet fiable pour découvrir les activités de sauvegarde des preuves de streaming.
- Téléchargez le `--identity-token-audience`, utilisé dans les notes de version ;
  la gouvernance сверяет public с политикой Fulcio перед одобрением публикации.Utilisez `scripts/sorafs_gateway_self_cert.sh` pour activer la passerelle de déploiement.
Téléchargez le lot de manifestes pour que l'attestation soit fournie avec l'article du candidat :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Gestion et publication

Après la vérification des contrôles et la vérification des crochets :1. Sélectionnez `sorafs_cli --version` et `sorafs_fetch --version` pour savoir ce qui se passe.
   Les binaires sont disponibles dans une nouvelle version.
2. Ajoutez la configuration de version dans `sorafs_release.toml` selon la version de contrôle (prévue)
   ou dans votre configuration de configuration, vous pouvez également définir votre référentiel de déploiement. Избегайте
   переменных окружения ad hoc; Veuillez vous connecter à la CLI à partir de `--config` (ou analogique)
   чтобы release inputs были явными и воспроизводимыми.
3. Sélectionnez le thème approprié (précédemment) ou le thème annoté :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Ajouter des articles (lots CAR, manifestes, résumés de preuves, notes de version,
   résultats de l'attestation) dans le registre du projet, ainsi que dans la liste de contrôle de gouvernance et
   [guide de déploiement](./developer-deployment.md). Si vous découvrez de nouveaux luminaires,
   Vous pouvez ouvrir un dépôt de luminaires ou un magasin d'objets, ce qui permet d'automatiser l'audit.
   сравнить опубликованный bundle с контролем версий.
5. Découvrez les canaux de gouvernance concernant les éléments suivants, les notes de version et les hachages
   bundle/подписей manifest, архивированные `manifest.sign/verify` résumés и любые
   enveloppes d'attestation. Utiliser l'URL CI job (ou les archives des logos), ce que vous avez sélectionné
   `ci/check_sorafs_cli_release.sh` et `scripts/release_sorafs_cli.sh`. Обновите gouvernance
   ticket, les auditeurs peuvent obtenir les approbations des articles ; когда
   `.github/workflows/sorafs-cli-release.yml` publiez des informations, indiquez-les
   зафиксированные hachages вместо résumés ad hoc.

## 6. Le plaisir post-réel- Ajouter la documentation disponible pour la nouvelle version (démarrages rapides, modèles CI),
  обновлена, либо подтвердите отсутствие изменений.
- Consultez la feuille de route si vous avez besoin d'un travail ultérieur (par exemple, les drapeaux de migration,
- Enregistrez le journal de votre porte de déverrouillage pour les auditeurs - ouvrez votre porte avec les appareils
  artéfacts.

La gestion de ce pipeline comprend les CLI, les caisses SDK et les matériaux de gouvernance synchronisés
Dans chaque cycle réel.