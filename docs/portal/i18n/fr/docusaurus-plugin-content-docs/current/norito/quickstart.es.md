---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Début rapide de Norito
description : Créez, validez et exécutez un contrat Kotodama avec les outils de publication et le rouge prédéterminé par un pair solo.
limace : /norito/quickstart
---

Cet enregistrement reflète le flux que nous espérons demander aux développeurs d'apprendre Norito et Kotodama pour la première fois : organiser un déterministe rouge d'un pair solo, compiler un contrat, faire un essai local et l'envoyer par Torii avec le CLI de référence.

Le contrat d'exemple décrit une clé/valeur dans le compte de l'appel afin que vous puissiez vérifier l'effet latéral immédiat avec `iroha_cli`.

## Conditions préalables

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 habilité (à utiliser pour lancer le pair de valeur défini dans `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) pour construire les binaires auxiliaires sans télécharger les publications.
- Binaires `koto_compile`, `ivm_run` et `iroha_cli`. Vous pouvez construire à partir de la caisse de l'espace de travail comme il faut descendre ou télécharger les artefacts de la version correspondante :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires antérieurs sont sécurisés par l'installation avec le reste de l'espace de travail.
> Nunca enlazan con `serde`/`serde_json` ; les codecs Norito sont appliqués de bout en bout.

## 1. Lancez un développeur rouge par un pair soloLe référentiel comprend un bundle de Docker Compose généré par `kagami swarm` (`defaults/docker-compose.single.yml`). Connectez la genèse par défaut, la configuration du client et les sondes de santé pour que Torii soit accessible en `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur corriendo (en primer plano ou desacoplado). Toutes les appels postérieurs de la CLI s'appliquent à ce pair intermédiaire `defaults/client.toml`.

## 2. Rédiger le contrat

Créez un répertoire de travail et gardez l'exemple minimal de Kotodama :

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

> Préférer maintenir les sources de Kotodama dans le contrôle des versions. Les exemples trouvés sur le portail sont également disponibles dans la [galerie d'exemples Norito](./examples/) si vous souhaitez un point de départ plus complet.

## 3. Compila et effectue un essai à sec avec IVM

Compilez le contrat avec le bytecode IVM/Norito (`.to`) et exécutez-le localement pour confirmer que les appels système de l'hôte fonctionnent avant de lancer le rouge :

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Le coureur imprime le journal `info("Hello from Kotodama")` et exécute l'appel système `SET_ACCOUNT_DETAIL` contre l'hôte simulé. Si le binaire optionnel `ivm_tool` est disponible, `ivm_tool inspect target/quickstart/hello.to` affiche l'ABI enfiché, les bits de fonctionnalités et les points d'entrée exportés.

## 4. Envoyez le bytecode via ToriiAvec le nœud également, envoyez le bytecode compilé à Torii à l'aide de la CLI. L'identité de développement par défaut est dérivée de la clé publique en `defaults/client.toml`, car l'ID de compte est
```
<katakana-i105-account-id>
```

Utilisez le fichier de configuration pour spécifier l'URL de Torii, l'ID de chaîne et la clé de l'entreprise :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

La CLI codifie la transaction avec Norito, la société avec la clé de développement et l'envoi par les pairs pour l'exécution. Observez les journaux de Docker pour l'appel système `set_account_detail` ou surveillez la sortie de la CLI pour le hachage de transaction compromis.

## 5. Vérifier le changement d'état

Utilisez le même profil de la CLI pour obtenir les détails du compte qui écrivent le contrat :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

Deberias ver el payload JSON répondu par Norito :

```json
{
  "hello": "world"
}
```

Si la valeur est mauvaise, confirmez que le service de Docker compose alors lors de l'exécution et que le hachage de transaction signalé par `iroha` arrive à l'état `Committed`.

## Suivant pasos- Explorez la [galerie d'exemples](./examples/) automatiquement générée pour la visualisation
  comme les fragments Kotodama mais avancés, ils sont mappés avec les appels système Norito.
- Lisez le [guia de inicio de Norito](./getting-started) pour une explication
  plus profond de l'outillage du compilateur/runner, de l'affichage des manifestes et des métadonnées de IVM.
- Lorsque vous iteres dans vos propres contrats, usa `npm run sync-norito-snippets` en el
  espace de travail pour régénérer les extraits téléchargeables de manière à ce que les documents du portail et les artefacts soient téléchargés
  se mantengan sincronizados con las fuentes en `crates/ivm/docs/examples/`.