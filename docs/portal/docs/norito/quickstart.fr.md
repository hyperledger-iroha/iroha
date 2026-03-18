---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c9a8e27bb8f586eac6bf4925cc69357dfa6d3f94c3dcf9e032b916c27fadf21
source_last_modified: "2025-11-07T12:25:50.867928+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Demarrage Norito
description: Construisez, validez et deployez un contrat Kotodama avec l'outillage de release et le reseau mono-pair par defaut.
slug: /norito/quickstart
---

Ce walkthrough reprend le workflow que nous attendons des developpeurs lorsqu'ils decouvrent Norito et Kotodama pour la premiere fois : demarrer un reseau deterministe mono-pair, compiler un contrat, le dry-run localement, puis l'envoyer via Torii avec le CLI de reference.

Le contrat d'exemple ecrit une paire cle/valeur dans le compte de l'appelant afin que vous puissiez verifier l'effet de bord immediatement avec `iroha_cli`.

## Prerequis

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 active (utilise pour demarrer le pair d'exemple defini dans `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) pour construire les binaires auxiliaires si vous ne telechargez pas ceux publies.
- Binaires `koto_compile`, `ivm_run` et `iroha_cli`. Vous pouvez les construire depuis le checkout du workspace comme ci-dessous ou telecharger les artefacts de release correspondants :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires ci-dessus sont sans risque a installer avec le reste du workspace.
> Ils ne lient jamais `serde`/`serde_json` ; les codecs Norito sont appliques de bout en bout.

## 1. Demarrer un reseau dev mono-pair

Le depot inclut un bundle Docker Compose genere par `kagami swarm` (`defaults/docker-compose.single.yml`). Il connecte la genesis par defaut, la configuration client et les health probes afin que Torii soit joignable a `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur tourner (au premier plan ou en detache). Toutes les commandes CLI suivantes ciblent ce pair via `defaults/client.toml`.

## 2. Ecrire le contrat

Creez un repertoire de travail et enregistrez l'exemple Kotodama minimal :

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

> Preferer conserver les sources Kotodama en controle de version. Des exemples heberges sur le portail sont aussi disponibles dans la [galerie d'exemples Norito](./examples/) si vous voulez un point de depart plus riche.

## 3. Compiler et dry-run avec IVM

Compilez le contrat en bytecode IVM/Norito (`.to`) et executez-le localement pour confirmer que les syscalls du host reussissent avant de toucher le reseau :

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Le runner imprime le log `info("Hello from Kotodama")` et effectue le syscall `SET_ACCOUNT_DETAIL` contre le host simule. Si le binaire optionnel `ivm_tool` est disponible, `ivm_tool inspect target/quickstart/hello.to` affiche l'en-tete ABI, les bits de features et les entrypoints exportes.

## 4. Soumettre le bytecode via Torii

Le noeud etant toujours en cours d'execution, envoyez le bytecode compile a Torii avec le CLI. L'identite de developpement par defaut est derivee de la cle publique dans `defaults/client.toml`, donc l'ID de compte est
```
i105...
```

Utilisez le fichier de configuration pour fournir l'URL Torii, le chain ID et la cle de signature :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Le CLI encode la transaction avec Norito, la signe avec la cle de dev et l'envoie au pair en cours d'execution. Surveillez les logs Docker pour le syscall `set_account_detail` ou lisez la sortie du CLI pour le hash de transaction committed.

## 5. Verifier le changement d'etat

Utilisez le meme profil CLI pour recuperer l'account detail que le contrat a ecrit :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Vous devriez voir le payload JSON adosse a Norito :

```json
{
  "hello": "world"
}
```

Si la valeur est absente, verifiez que le service Docker compose tourne toujours et que le hash de transaction signale par `iroha` a atteint l'etat `Committed`.

## Etapes suivantes

- Explorez la [galerie d'exemples](./examples/) auto-generee pour voir
  comment des snippets Kotodama plus avances se mappent a des syscalls Norito.
- Lisez le [guide Norito getting started](./getting-started) pour une explication
  plus approfondie des outils compilateur/runner, du deploiement de manifests et des metadonnees IVM.
- Lorsque vous iterez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  workspace pour regenerer les snippets telechargeables afin que les docs du portail et les artefacts restent
  synchronises avec les sources sous `crates/ivm/docs/examples/`.
