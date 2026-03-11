---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Démarrage Norito
description : Construisez, validez et déployez un contrat Kotodama avec l'outillage de release et le réseau mono-paire par défaut.
limace : /norito/quickstart
---

Ce walkthrough reprend le workflow que nous attendons des développeurs lorsqu'ils découvrent Norito et Kotodama pour la première fois : demarrer un réseau déterministe mono-pair, compiler un contrat, le dry-run localement, puis l'envoyer via Torii avec la CLI de référence.

Le contrat d'exemple écrit une paire clé/valeur dans le compte de l'appelant afin que vous puissiez vérifier l'effet de bord immédiatement avec `iroha_cli`.

## Prérequis

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 actif (utiliser pour démarrer la paire d'exemple défini dans `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) pour construire les binaires auxiliaires si vous ne téléchargez pas ceux publies.
- Binaires `koto_compile`, `ivm_run` et `iroha_cli`. Vous pouvez les construire depuis le checkout du workspace comme ci-dessous ou télécharger les artefacts de release correspondants :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires ci-dessus sont sans risque à installer avec le reste de l'espace de travail.
> Ils ne lient jamais `serde`/`serde_json` ; les codecs Norito sont appliqués de bout en bout.

## 1. Démarrer une mono-paire de développement de réseauLe dépôt inclut un bundle Docker Compose générique par `kagami swarm` (`defaults/docker-compose.single.yml`). Il connecte la Genesis par défaut, la configuration client et les sondes de santé afin que Torii soit joignable à `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur tourner (au premier plan ou en détacher). Toutes les commandes CLI suivantes ciblent ce couple via `defaults/client.toml`.

## 2. Écrire le contrat

Créez un répertoire de travail et enregistrez l'exemple Kotodama minimal :

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Préférer conserver les sources Kotodama en contrôle de version. Des exemples hébergés sur le portail sont également disponibles dans la [galerie d'exemples Norito](./examples/) si vous voulez un point de départ plus riche.

## 3. Compilateur et dry-run avec IVM

Compilez le contrat en bytecode IVM/Norito (`.to`) et exécutez-le localement pour confirmer que les appels système de l'hôte réussissent avant de toucher le réseau :

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Le runner imprime le log `info("Hello from Kotodama")` et effectue le syscall `SET_ACCOUNT_DETAIL` contre le host simule. Si le binaire optionnel `ivm_tool` est disponible, `ivm_tool inspect target/quickstart/hello.to` affiche l'ABI en tête, les bits de fonctionnalités et les points d'entrée exportés.

## 4. Soumettre le bytecode via ToriiLe noeud étant toujours en cours d'exécution, envoyez le bytecode compile a Torii avec le CLI. L'identité de développement par défaut est dérivée de la clé publique dans `defaults/client.toml`, donc l'ID de compte est
```
i105...
```

Utilisez le fichier de configuration pour fournir l'URL Torii, le chain ID et la clé de signature :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Le CLI encode la transaction avec Norito, la signature avec la clé de développement et l'envoie au pair en cours d'exécution. Surveillez les logs Docker pour le syscall `set_account_detail` ou lisez la sortie du CLI pour le hash de transaction validé.

## 5. Vérifier le changement d'état

Utilisez le meme profil CLI pour récupérer le détail du compte que le contrat a écrit :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Vous devriez voir le payload JSON associer à Norito :

```json
{
  "hello": "world"
}
```

Si la valeur est absente, vérifiez que le service Docker compose tourne toujours et que le hash de transaction signale par `iroha` a atteint l'état `Committed`.

## Étapes suivantes- Explorez la [galerie d'exemples](./examples/) auto-générée pour voir
  comment des snippets Kotodama plus les avancées se mappent aux syscalls Norito.
- Lisez le [guide Norito Getting Started](./getting-started) pour une explication
  plus approfondi des outils compilateur/runner, du déploiement de manifestes et des métadonnées IVM.
- Lorsque vous itérez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  workspace pour régénérer les snippets téléchargeables afin que les docs du portail et les artefacts restent
  se synchronise avec les sources sous `crates/ivm/docs/examples/`.