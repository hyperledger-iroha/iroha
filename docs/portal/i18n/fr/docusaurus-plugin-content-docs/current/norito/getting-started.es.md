---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Premières étapes avec Norito

Ce guide rapide montre le flux minimum pour compiler un contrat Kotodama, inspecter le bytecode Norito généré, l'exécuter localement et le supprimer sur un nœud Iroha.

## Conditions préalables

1. Installez la chaîne d'outils de Rust (1.76 ou plus récente) et clonez ce référentiel.
2. Créez ou téléchargez les binaires du support :
   - `koto_compile` - compilateur Kotodama qui émet le bytecode IVM/Norito
   - `ivm_run` et `ivm_tool` - utilitaires d'exécution locale et d'inspection
   - `iroha_cli` - à utiliser pour l'exécution des contrats via Torii

   Le Makefile du référentiel espère ces binaires en `PATH`. Vous pouvez télécharger des artefacts précompilés ou des compilations à partir du code source. Si vous compilez la chaîne d'outils localement, ajoutez les aides du Makefile aux binaires :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Assurez-vous qu'un nœud de Iroha est en exécution lorsque vous passez au pas de déchargement. Les exemples d'abajo supposent que Torii est accessible dans l'URL configurée dans votre profil de `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler un contrat Kotodama

Le référentiel comprend un contrat minimum "hello world" en `examples/hello/hello.ko`. Compilez un bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Options clés :- `--abi 1` fixe le contrat à la version ABI 1 (l'unique supportée au moment de l'écriture).
- `--max-cycles 0` demande d'éjection sans limites ; établissez un numéro positif pour acotar el padding de ciclos para pruebas de conocimiento cero.

## 2. Inspecciona el artefacto Norito (facultatif)

Utilisez `ivm_tool` pour vérifier la tête et les métadonnées incrustées :

```sh
ivm_tool inspect target/examples/hello.to
```

Deberias ver la version ABI, les drapeaux habilités et les points d'entrée exportés. Il s’agit d’une transaction rapide avant le début de l’opération.

## 3. Exécuter le contrat localement

Exécutez le bytecode avec `ivm_run` pour confirmer le comportement sans cliquer sur un nœud :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` enregistre un salut et émet un appel système `SET_ACCOUNT_DETAIL`. Exécuter localement est utile pendant plusieurs itérations dans la logique du contrat avant de le publier en chaîne.

## 4. Despliega via `iroha_cli`

Lorsque vous êtes satisfait du contrat, envoyez-le à un nœud en utilisant la CLI. Proportionne un compte d'autorité, votre clé d'entreprise et un fichier `.to` ou charge utile Base64 :

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande envoie un bundle de manifeste Norito + bytecode par Torii et indique l'état de la transaction résultante. Une fois confirmé, le hachage du code affiché dans la réponse peut être utilisé pour récupérer les manifestes ou lister les instances :

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```## 5. Exécution contre Torii

Avec le bytecode enregistré, vous pouvez appeler en envoyant une instruction qui fait référence au code enregistré (par exemple, au milieu de `iroha_cli ledger transaction submit` ou de votre client d'application). Assurez-vous que les autorisations du compte autorisent les appels système souhaités (`set_account_detail`, `transfer_asset`, etc.).

## Conseils et solutions aux problèmes

- Utilisez `make examples-run` pour compiler et exécuter les exemples en une seule étape. Résumé des variables d'entrée `KOTO`/`IVM` si les binaires ne sont pas en `PATH`.
- Si `koto_compile` recherche la version ABI, vérifiez que le compilateur et le noeud correspondant à ABI v1 (exécutez `koto_compile --abi` sans arguments pour lister le support).
- La CLI accepte les clés d'entreprise en hexadécimal ou Base64. Pour les essais, vous pouvez utiliser les touches émises par `iroha_cli tools crypto keypair`.
- Pour supprimer les charges utiles Norito, la sous-commande `ivm_tool disassemble` aide à corréler les instructions avec le code source Kotodama.

Ceci reflète les étapes utilisées en CI et dans les essais d'intégration. Pour une analyse plus approfondie de la gramatique de Kotodama, des mappages d'appels système et des éléments internes de Norito, consultez :

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`