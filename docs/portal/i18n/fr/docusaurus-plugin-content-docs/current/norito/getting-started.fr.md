---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Prise en main de Norito

Ce guide rapide présente le workflow minimal pour compiler un contrat Kotodama, inspecter le bytecode Norito généré, l'exécuter localement et le déployer sur un nu Iroha.

## Prérequis

1. Installez la toolchain Rust (1.76 ou plus) et récupérez ce dépôt.
2. Construisez ou téléchargez les binaires de support :
   - `koto_compile` - compilateur Kotodama qui emet du bytecode IVM/Norito
   - `ivm_run` et `ivm_tool` - utilitaires d'exécution locale et d'inspection
   - `iroha_cli` - utiliser pour le déploiement de contrat via Torii

   Le Makefile du dépôt attend ces binaires dans `PATH`. Vous pouvez télécharger des artefacts précompilés ou les construire depuis les sources. Si vous compilez la toolchain localement, pointez les helpers du Makefile vers les binaires :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Assurez-vous qu'un nud Iroha est en cours d'exécution lorsque vous atteignez l'étape de déploiement. Les exemples ci-dessous supposent que Torii est accessible à l'URL configurée dans votre profil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler un contrat Kotodama

Le dépôt fournit un contrat minimal "hello world" dans `examples/hello/hello.ko`. Compilez le bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Clés d'options :- `--abi 1` verrouille le contrat sur la version ABI 1 (la seule supportée au moment de la rédaction).
- `--max-cycles 0` demande une exécution sans limite ; fixez un nombre positif pour borner le padding de cycles pour les preuves de connaissance zéro.

## 2. Inspecter l'artefact Norito (optionnel)

Utilisez `ivm_tool` pour vérifier l'en-tête et les métadonnées intégrées :

```sh
ivm_tool inspect target/examples/hello.to
```

Vous devriez voir la version ABI, les drapeaux actifs et les points d'entrée exportés. C'est un contrôle rapide avant le déploiement.

## 3. Exécuter le contrat localement

Exécutez le bytecode avec `ivm_run` pour confirmer le comportement sans toucher un bouton :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` journalise une salutation et émet un syscall `SET_ACCOUNT_DETAIL`. L'exécution locale est utile pendant que vous itérez sur la logique du contrat avant de le publier en chaîne.

## 4. Déployeur via `iroha_cli`

Lorsque vous êtes satisfait du contrat, déployez-le sur un nu via la CLI. Fournissez un compte d'autorité, sa clé de signature et soit un fichier `.to` soit un payload Base64 :

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande soumet un bundle manifeste Norito + bytecode via Torii et affiche l'état de la transaction résultante. Une fois la transaction validée, le hash de code affiché dans la réponse peut servir à récupérer des manifestes ou à lister des instances :```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Exécuteur via Torii

Avec le bytecode enregistré, vous pouvez l'invoquer en soumettant une instruction qui référence le code stocke (p. ex., via `iroha_cli ledger transaction submit` ou votre client applicatif). Assurez-vous que les autorisations du compte autorisent les appels système souhaités (`set_account_detail`, `transfer_asset`, etc.).

## Conseils et dépannage

- Utilisez `make examples-run` pour compiler et exécuter les exemples en une seule commande. Surchargez les variables d'environnement `KOTO`/`IVM` si les binaires ne sont pas dans `PATH`.
- Si `koto_compile` refuse la version ABI, vérifiez que le compilateur et le nud ciblent bien ABI v1 (exécutez `koto_compile --abi` sans arguments pour lister le support).
- La CLI accepte les clés de signature en hexadécimal ou Base64. Pour les tests, vous pouvez utiliser les clés émises par `iroha_cli tools crypto keypair`.
- Lors du débogage des payloads Norito, la sous-commande `ivm_tool disassemble` aide à correler les instructions avec la source Kotodama.

Ce flux reflète les étapes utilisées en CI et dans les tests d'intégration. Pour une analyse plus approfondie de la grammaire Kotodama, des mappings de syscalls et des internals Norito, voir :

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`