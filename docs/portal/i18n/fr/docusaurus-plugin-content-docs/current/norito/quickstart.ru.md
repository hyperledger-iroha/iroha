---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Быстрый старт Norito
description : Récupérez, vérifiez et exécutez le contrat Kotodama avec les instruments de travail et l'ensemble de travaux prévus à cet effet.
limace : /norito/quickstart
---

Cette procédure pas à pas explique le processus qui vous permet de configurer les robots avant de passer à Norito et Kotodama : Déterminez la configuration du contrat, configurez le contrat, effectuez un fonctionnement à sec local, puis activez-le à partir de Torii avec l'élément CLI.

Le premier contrat prévoit un engagement/un engagement dans votre compte, ce que vous pouvez faire pour vérifier cet effet avec votre investissement. `iroha_cli`.

## Требования

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 (utilisé pour le démarrage d'un échantillon homologue, en relation avec `defaults/docker-compose.single.yml`).
- Chaîne d'outils Rust (1.76+) pour les ordinateurs de bureau, si vous ne les recherchez pas.
- Binaires `koto_compile`, `ivm_run` et `iroha_cli`. Vous pouvez utiliser l'espace de travail de paiement, comme vous le souhaitez, ou télécharger les artefacts de version suivants :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Ces binaires peuvent facilement être utilisés dans l'espace de travail.
> Ce n'est pas un lien avec `serde`/`serde_json` ; les codes Norito sont utilisés de bout en bout.

## 1. Запустите одноузловую dev сетьDans le référentiel, il s'agit du bundle Compose Docker, généré par `kagami swarm` (`defaults/docker-compose.single.yml`). Lors du déploiement de Genesis, la configuration des sondes client et de santé, Torii est téléchargée sur `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Ouvrir le conteneur (dans le téléphone ou au premier plan). Ensuite, vous pouvez utiliser la CLI pour ce peer à partir de `defaults/client.toml`.

## 2. Rédiger un contrat

Sélectionnez le directeur du travail et sélectionnez le modèle mini Kotodama :

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

> Veuillez installer l'appareil Kotodama dans la version de contrôle du système. Les exemples, les paramètres du portail, doivent être téléchargés dans [les exemples de la galerie Norito](./examples/), si vous n'avez pas encore de démarrage rapide. набор.

## 3. Compilation et fonctionnement à sec avec IVM

Compilez le contrat avec le code de batterie IVM/Norito (`.to`) et installez-le localement pour effectuer ces appels système. хоста проходят до обращения к сети:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner envoie le journal `info("Hello from Kotodama")` et active l'appel système `SET_ACCOUNT_DETAIL` pour le modèle d'hôte. Si vous installez le binaire optionnel `ivm_tool`, la commande `ivm_tool inspect target/quickstart/hello.to` sélectionne l'en-tête ABI, les bits de fonctionnalité et l'exportation des points d'entrée.

## 4. Ouvrir le clavier à partir de ToriiSi vous utilisez le robot, ouvrez le code de batterie de compilation dans Torii à partir de la CLI. L'identification du développeur s'effectue depuis la clé publique `defaults/client.toml`, avec l'ID de compte :
```
soraカタカナ...
```

Utilisez le fichier de configuration pour télécharger l'URL Torii, l'ID de chaîne et le bouton ci-dessous :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

La CLI code la transition Norito, permettant d'accéder au bouton de développement et à l'activation du peer de travail. Sélectionnez les journaux Docker pour l'appel système `set_account_detail` ou surveillez la CLI pour votre transmission validée.

## 5. Vérifiez la configuration de votre service

Utilisez la CLI du profil pour afficher les détails du compte, en cliquant sur le contrat :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Vous pouvez visualiser la charge utile JSON sur la base Norito :

```json
{
  "hello": "world"
}
```

Si vous décidez d'ouvrir votre service, assurez-vous que le service Docker compose tout votre travail et votre transfert, que vous avez effectué `iroha`, application `Committed`.

## Следующие шаги- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  Vous devez également produire des extraits de code Kotodama mappés sur les appels système Norito.
- Téléchargez le [Guide de démarrage Norito](./getting-started) pour tout le groupe
  объяснения инструментов компилятора/раннера, деплоя manifestes и метаданных IVM.
- Pour démarrer votre contrat, utilisez `npm run sync-norito-snippets` dans
  espace de travail, pour régénérer les captures d'écran et gérer les documents du portail et des articles
  synchronisation avec les appareils `crates/ivm/docs/examples/`.