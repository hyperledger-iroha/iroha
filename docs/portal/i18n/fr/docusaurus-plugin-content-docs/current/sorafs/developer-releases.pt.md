---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Processus de publication
résumé : Exécutez la porte de publication du CLI/SDK, appliquez la politique de version partagée et les notes publiques des versions canoniques.
---

# Processus de publication

Les binaires de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) et les caisses du SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) Sao entregues juntos. Ô pipeline
de release mantem o CLI e as bibliotecas alinhados, garante cobertura de lint/test e
captura artefatos para consumidores en aval. Exécuter une liste de contrôle abaixo para cada
tag candidat.

## 0. Confirmer l'approbation de la révision de la sécurité

Avant d'exécuter la porte technique de libération, capturez les œuvres les plus récentes de
révision de la sécurité :

- Voici le mémo de révision de sécurité SF-6 le plus récent ([reports/sf6-security-review](./reports/sf6-security-review.md))
  et enregistrez votre hash SHA256 sans ticket de sortie.
- Anexe ou link do ticket de remediacao (par exemple, `governance/tickets/SF6-SR-2026.md`) et note
  Nous sommes les avenants de l'ingénierie de sécurité et du groupe de travail sur l'outillage.
- Vérifiez qu'une liste de contrôle de remediacao no memo esta fechada ; les éléments pendants bloquent la libération.
- Préparer ou télécharger les journaux dos pour exploiter la parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  avec le bundle de manifeste.
- Confirmez que le commandant d'assassinat qui va exécuter inclut `--identity-token-provider` et
  un `--identity-token-audience=<aud>` explicite pour capturer la sauvegarde de Fulcio dans les preuves de la libération.Inclut ces articles pour notifier la gouvernance et publier la libération.

## 1. Exécuter la porte de libération/testes

O helper `ci/check_sorafs_cli_release.sh` tige formatée, Clippy et testes nos caisses
CLI et SDK avec un répertoire cible local vers l'espace de travail (`.target`) pour éviter les conflits
de permissao ao executar dentro de containers CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script faz as seguintes verificacoes :

- `cargo fmt --all -- --check` (espace de travail)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (avec une fonctionnalité `cli`),
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` pour ces caisses mesmos

Si vous faites un faux pas, corrigez la régression avant de commencer. Construit le développement de la version
être continu avec le principal ; nao faca cherry-pick de correcoes em branches de release. Ô
gate tambem verifica se os flags de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) foram fornecidos quando aplicavel; arguments erronés
fazem a execucao falhar.

## 2. Appliquer la politique de version

Tous les crates CLI/SDK de SoraFS avec SemVer :- `MAJOR` : Introduction pour la première version 1.0. Antes de 1.0 ou augmentation mineure
  `0.y` **indica mudancas quebradoras** à la surface du CLI ou nos esquemas Norito.
- `MINOR` : Trabalho de Features compativel para tras (nouvelles commandes/drapeaux, nouvelles
  champs Norito protégés par la politique facultative, adicoes de télémétrie).
- `PATCH` : corrections de bugs, versions partielles de la documentation et actualisations de
  les dépendances qui ne changent pas le comportement d'observation.

Mantenha semper `sorafs_car`, `sorafs_manifest` et `sorafs_chunker` à l'autre
pour que les utilisateurs du SDK en aval dépendent d'une chaîne unique
versao alinhada. Ao actualizar verso:

1. Actualisez les champs `version =` à chaque `Cargo.toml`.
2. Régénérer o `Cargo.lock` via `cargo update -p <crate>@<new-version>` (o espace de travail
   exige verso explicitas).
3. Montez nouvellement la porte de déverrouillage pour garantir que nao restem artefatos desatualizados.

## 3. Préparer les notes de version

Chaque version doit publier un journal des modifications dans le markdown qui remplace les CLI, SDK et
impact de gouvernance. Utilisez le modèle em `docs/examples/sorafs_release_notes.md`
(copie-o para votre diretorio de artefatos de release e preencha as secoes com detalhes
concrets).

Compte minimo :- **Points forts** : fonctionnalités disponibles pour les utilisateurs de CLI et SDK.
- **Compatibilité** : mudancas quebradoras, mises à niveau de politique, requisitos minimos
  de gateway/nodo.
- **Pass de mise à niveau** : commandes TL;DR pour actualiser les dépendances de chargement et de recharge
  luminaires déterministes.
- **Vérification** : hachages de ladite ou enveloppes et révision exata de
  `ci/check_sorafs_cli_release.sh` exécuté.

Anexe as notas of release preenchidas ao tag (par exemple, corps de release do GitHub) et
gardez-les ensemble avec les artefatos, en prenant une forme déterministe.

## 4. Exécuter les hooks de release

Rode `scripts/release_sorafs_cli.sh` pour gérer le paquet d'Assinatura et le CV
verificacao que accompagne cada release. La compilation wrapper ou CLI lorsque cela est nécessaire,
chama `sorafs_cli manifest sign` et immédiatement réexécuté `manifest verify-signature`
para que falhas aparecam avant de faire le marquage. Exemple :

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

Dicas :- Registre des entrées de version (charge utile, plans, résumés, hachage attendu du jeton)
  pas de dépôt ou de configuration de déploiement pour la reproduction du script. Ô
  le bundle de luminaires dans `fixtures/sorafs_manifest/ci_sample/` montre la disposition canonique.
- Baseie a automação de CI em `.github/workflows/sorafs-cli-release.yml` ; ele roda o
  gate de release, chama o script acima e arquiva bundles/assinaturas como artefatos
  faire le flux de travail. Mantenha a mesma ordem de comandos (gate de release -> assinatura ->
  vérification) dans d'autres systèmes CI pour que les journaux d'auditoire batam avec les hachages
  gerados.
- Mantenha `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` et
  Ensembles `manifest.verify.summary.json` ; ils forment un paquet de référence
  notification de gouvernance.
- Quando o release atualizar luminaires canonicos, copie o manifest atualizado, o
  plan de fragments et résumés du système pour `fixtures/sorafs_manifest/ci_sample/` (et mise à jour
  `docs/examples/sorafs_ci_sample/manifest.template.json`) avant de commencer. Opérateurs
  en aval dépend des appareils engagés pour reproduire le bundle de version.
- Capturer le journal d'exécution de la vérification des canaux limités
  `sorafs_cli proof stream` et l'annexe à la publication pour démontrer que
  salvaguardas de proof streaming continuam ativas.
- Registre o `--identity-token-audience` exato usado durante a assinatura nas
  notes de sortie ; une gouvernance cruza o audience com a politica de Fulcio antes de
  approuver une publication.Utilisez `scripts/sorafs_gateway_self_cert.sh` lorsque la version est également incluse dans le déploiement
de la passerelle. Aponte para o mesmo bundle de manifest para prouver qu'une attestation
correspond au candidat artefato :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag et publication

Après que les contrôles soient passés et que les crochets soient terminés :1. Rode `sorafs_cli --version` et `sorafs_fetch --version` pour confirmer que les binaires
   reportam a nova versao.
2. Préparez une configuration de version dans la version `sorafs_release.toml` (préférée)
   ou un autre archivage de configuration rastreado pour votre dépôt de déploiement. Éviter les personnes à charge
   de variables ambiantes ad hoc ; passe les chemins pour la CLI avec `--config` (ou
   équivalent) pour que les entrées soient publiées de manière explicite et reproductible.
3. Criez un tag assassiné (préféré) ou annoté :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Faca upload dos artefatos (bundles CAR, manifestes, resumos de proofs, notas de release,
   sorties d'attestation) pour le registre du projet en suivant une liste de contrôle de gouvernance
   non [guia de déploiement](./developer-deployment.md). Se o sortir les luminaires Gerou Novas,
   envie-as pour le dépôt de luminaires, le partage ou le magasin d'objets pour l'automatisation
   de auditoria consiga comparer le bundle publié avec le contrôle de code.
5. Notifier le canal de gouvernance avec les liens pour le tag assassiné, les notes de sortie, les hachages
   faire bundle/assinaturas do manifest, résumés arquivados de `manifest.sign/verify` e
   quaisquer des enveloppes d'attestation. Inclut une URL pour le travail de CI (ou l'archivage des journaux)
   que rodou `ci/check_sorafs_cli_release.sh` et `scripts/release_sorafs_cli.sh`. Actualiser
   o ticket de gouvernance para que os audites possam rastrear aprovacoes ate os artefatos;
   quand le travail `.github/workflows/sorafs-cli-release.yml` publier des notifications, lienLes hachages sont enregistrés sous la forme de CV ad hoc.

## 6. Pos-release

- Garantie qu'une documentation est disponible pour une nouvelle version (démarrages rapides, modèles de CI)
  esteja atualizada ou confirme que nenhuma mudanca e necessaria.
- Enregistrez les entrées sans feuille de route pour le travail nécessaire postérieur (par exemple, drapeaux
- Arquive os logs do gate de release para auditoria - guarde-os ao lado dos artefatos
  assassinés.

Suivez ce pipeline en utilisant la CLI, le SDK crates et le matériel de gouvernance alinhados
à chaque cycle de sortie.