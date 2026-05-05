---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Premiers pass avec Norito

Cette guía montre rapidement le flux minimum pour compiler un contrat Kotodama, inspecte le bytecode Norito, exécute-le localement et déploie-le facilement sur un nœud Iroha.

## Pré-requis

1. Installez une chaîne d'outils Rust (1.76 ou plus récente) et extrayez ce référentiel.
2. Compilez ou utilisez les binaires de support :
   - `koto_compile` - compilateur Kotodama qui émet le bytecode IVM/Norito
   - `ivm_run` et `ivm_tool` - utilitaires d'exécution locale et d'inspection
   - `iroha_cli` - utilisé pour déployer des contrats via Torii

   Le Makefile du référentiel espère ces binaires no `PATH`. Vous pouvez baisser les artefatos précompilés ou compiler à partir du codigo fonte. Pour compiler une chaîne d'outils localement, utilisez les helpers pour créer un Makefile pour les binaires :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Garantissez qu'un noeud Iroha est utilisé lorsque vous effectuez l'étape de déploiement. Les exemples suivants supposent que Torii est acessivement na URL configurada no profile `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler un contrat Kotodama

Le référentiel comprend un contrat minimum "hello world" dans `examples/hello/hello.ko`. Compile-o pour le bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Les drapeaux portent :- `--abi 1` fixe le contrat vers ABI 1 (uniquement pris en charge à ce moment-là).
- `--max-cycles 0` demande d'exécution sem limite ; définir un nombre positif pour limiter le rembourrage des cycles pour prouver la conhecimento zéro.

## 2. Inspecione o artefato Norito (facultatif)

Utilisez `ivm_tool` pour vérifier le câble et les métadonnées embutidos :

```sh
ivm_tool inspect target/examples/hello.to
```

Vous devez consulter l'ABI, les drapeaux autorisés et les points d'entrée exportés. Il y a un contrôle rapide avant le déploiement.

## 3. Exécuter le contrat localement

Exécutez le bytecode avec `ivm_run` pour confirmer le comportement à un nœud :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` enregistre un message audio et émet un appel système `SET_ACCOUNT_DETAIL`. Exécuter localement et utiliser pendant cette itération dans la logique du contrat avant de le publier en chaîne.

## 4. Déploiement Faca via `iroha_cli`

Lorsque vous êtes satisfait du contrat, vous devez les déployer à un nœud en utilisant la CLI. Indiquez un numéro d'autorisation, votre clé d'assassinat et un fichier `.to` ou charge utile Base64 :

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande envoie un bundle de manifeste Norito + bytecode via Torii et imprime le statut de la transaction résultante. Une fois la transaction effectuée, le hachage du code affiché dans la réponse peut être utilisé pour récupérer des manifestes ou répertorier des instances :

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Exécuter la contra ToriiAvec le bytecode enregistré, vous pouvez invoquer une instruction de référence au code armé (par exemple, via `iroha_cli ledger transaction submit` ou par le client de votre application). Garanta que as permissoes da conta permitam os syscalls desejados (`set_account_detail`, `transfer_asset`, etc.).

## Dicas et solutions aux problèmes

- Utilisez `make examples-run` pour compiler et exécuter les exemples d'une fois. Remplacez les variables d'ambiance `KOTO`/`IVM` par les binaires qui ne fonctionnent pas avec `PATH`.
- Si `koto_compile` refuse le vers ABI, vérifiez le compilateur et le nœud miram ABI v1 (utilisez `koto_compile --abi` avec des arguments pour lister ou prendre en charge).
- La CLI a des chaves d'assinatura em hex ou Base64. Pour les testicules, vous pouvez utiliser les chaves émis par `iroha_cli tools crypto keypair`.
- Pour supprimer les charges utiles Norito, la sous-commande `ivm_tool disassemble` ajuste les instructions corrélatives avec la police de code Kotodama.

Ce flux espère les passes utilisées par CI et les tests d'intégration. Pour une fusion plus profonde de la gramatique Kotodama, nos mappages d'appels système et nos composants internes de Norito, voir :

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`