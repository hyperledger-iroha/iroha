---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Début rapide du Norito
description : Construire, valider et faciliter le déploiement d'un contrat Kotodama avec les outils de version et la refonte d'un homologue unique.
limace : /norito/quickstart
---

Cette étape à pas est celle où nous espérons que les développeurs seront impliqués dans l'apprentissage de Norito et Kotodama pour la première fois : lancez un processus déterministe d'un homologue unique, compilez un contrat, faites un essai à sec localement et ensuite envoyez-le via Torii avec la CLI de référence.

Le contrat d'exemple grave avec chave/valor auprès du chamador pour que vous puissiez vérifier l'effet collatéral immédiatement avec `iroha_cli`.

## Pré-requis

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 habilité (utilisé pour démarrer ou un homologue défini dans l'exemple `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) pour compiler les binaires auxiliaires afin de les rendre publics.
- Binaires `koto_compile`, `ivm_run` et `iroha_cli`. Vous pouvez compiler les éléments à partir de la caisse de l'espace de travail pour afficher ou afficher les artefacts des correspondants de la version :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires sont très sûrs pour installer ensemble le reste de l'espace de travail.
> Eles nunca fazem link com `serde`/`serde_json` ; les codecs Norito sont appliqués de pont à pont.

## 1. Démarrer un développeur rede d'un pair uniqueLe référentiel comprend un bundle Docker Composé généré par `kagami swarm` (`defaults/docker-compose.single.yml`). Il est connecté au clavier Genesis, à la configuration du client et des sondes de santé pour que Torii soit activé dans `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o conteneur rodando (em primeiro plano ou détaché). Toutes les commandes de CLI postérieures sont disponibles pour cela via `defaults/client.toml`.

## 2. Écrire le contrat

Criez un répertoire de travail et un exemple minimal de Kotodama :

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

> Préférez manter les polices Kotodama dans le contrôle de versao. Les exemples d'hôtels sur le portail sont également disponibles dans la [galerie d'exemples Norito](./examples/) pour vouloir un point de départ plus riche.

## 3. Compiler et exécuter à sec avec IVM

Compilez le contrat pour le bytecode IVM/Norito (`.to`) et exécutez-le localement pour confirmer que les appels système fonctionnent bien avant de démarrer le processus :

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Le coureur imprime le journal `info("Hello from Kotodama")` et exécute l'appel système `SET_ACCOUNT_DETAIL` contre le mockado de l'hôte. Si le binaire optionnel `ivm_tool` est disponible, `ivm_tool inspect target/quickstart/hello.to` affiche l'en-tête ABI, les bits de fonctionnalité et les points d'entrée exportés.

## 4. Envie du bytecode via ToriiComme ce n'est pas le cas, j'aimerais le bytecode compilé pour Torii en utilisant la CLI. L'identité du responsable du développement et dérivée de la personne publique sur `defaults/client.toml`, portant l'ID du compte
```
ih58...
```

Utilisez le fichier de configuration pour fournir une URL de Torii, un ID de chaîne et une clé d'Assurance :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

La CLI codifie la transaction avec Norito, est associée à la demande de développement et est envoyée à un homologue lors de son exécution. Observez les journaux du système d'exploitation Docker pour l'appel système `set_account_detail` ou surveillez la CLI pour le hachage de la transaction validée.

## 5. Vérifier la situation

Utilisez votre profil CLI pour rechercher les détails du compte correspondant au contrat :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Voix développer la charge utile JSON prise en charge par Norito :

```json
{
  "hello": "world"
}
```

Si la valeur est ausente, confirmez que le service Docker compose bien esta rodando et que le hachage de la transaction a été signalé par `iroha` vers l'état `Committed`.

## Passons à proximité- Explorez une [galerie d'exemples](./examples/) automatiquement créée pour voir
  comme les extraits Kotodama plus avancés sont mapeiam pour les appels système Norito.
- Leia ou [guia de inicio do Norito](./getting-started) pour une explication
  plus profond dans l'outillage du compilateur/runner, dans le déploiement des manifestes et des métadonnées IVM.
- Pour parcourir nos contrats, utilisez `npm run sync-norito-snippets` sans espace de travail pour
  régénérer les extraits de code disponibles et gérer les documents du portail et les artefacts synchronisés avec les polices `crates/ivm/docs/examples/`.