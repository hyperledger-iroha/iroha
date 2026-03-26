---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Nom du robot avec Norito

Ce projet de contrat de travail miniaturisé comprend le contrat Kotodama, pour lequel le code de batterie général est fourni Norito, installation locale et déploiement sur l'utilisation Iroha.

## Требования

1. Installez la chaîne d'outils Rust (1.76 ou nouvelle) et clonez ce dépôt.
2. Sélectionnez ou téléchargez vos fichiers binaires :
   - `koto_compile` - Compilateur Kotodama, pour générer le code de batterie IVM/Norito
   - `ivm_run` et `ivm_tool` - Services locaux d'inspection et d'inspection
   - `iroha_cli` - utilisé pour le déploiement de contrats à partir de Torii

   Le dépôt Makefile contient ces fichiers binaires dans `PATH`. Vous pouvez acheter des objets d'art ou les comparer à des objets. Si vous compilez une chaîne d'outils localement, utilisez l'aide de Makefile pour le binaire :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Assurez-vous d'utiliser le Iroha au moment du déploiement. Nous proposons par exemple que Torii soit téléchargé via l'URL du profil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler le contrat Kotodama

Dans le référentiel, il y a un contrat mini "hello world" dans `examples/hello/hello.ko`. Compilez-le dans le code de batterie Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Drapeaux clés :- `--abi 1` fixe le contrat pour la version 1 d'ABI (la version disponible est la version actuelle).
- `--max-cycles 0` запрашивает неограниченное выполнение ; Utilisez un outil simple pour organiser les cycles de remplissage pour les chercheurs à connaissance nulle.

## 2. Vérifier l'artéfact Norito (officiellement)

Utilisez `ivm_tool` pour vérifier les paramètres et les métadonnées disponibles :

```sh
ivm_tool inspect target/examples/hello.to
```

Vous avez la version ABI, les drapeaux et les points d'entrée d'exportation. C'est un contrôle de santé mentale effectué avant le déploiement.

## 3. Souscrire un contrat localement

Utilisez le code de batterie `ivm_run` pour pouvoir effectuer des opérations sans vous soucier de votre utilisation :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` vous permet d'accéder au journal et d'utiliser l'appel système `SET_ACCOUNT_DETAIL`. Les contrats locaux conclus avec des logiciels d'ingénierie sont destinés à la publication en chaîne.

## 4. Déployez ici `iroha_cli`

Lorsque vous contractez, vous pouvez l'utiliser en utilisant CLI. Recherchez l'auteur du compte, entre autres, et le fichier `.to`, qui utilise la charge utile Base64 :

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande ouvre le manifeste du bundle Norito + le code de batterie correspondant à Torii et indique la transition d'état. Une fois que le comité a décidé d'ouvrir votre code, vous pouvez l'utiliser pour les manifestes ou les installations de surveillance :

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Запуск через ToriiAprès l'enregistrement du code de navigation, vous pouvez vous y rendre, en suivant les instructions, qui correspondent au code correspondant (par exemple, ici `iroha_cli ledger transaction submit` ou votre offre client). En outre, votre compte déclenche tous les appels système (`set_account_detail`, `transfer_asset` et etc.).

## Problèmes de résolution et d'exploitation

- Utilisez `make examples-run` pour sauvegarder et utiliser les exemples de configuration. Sélectionnez l'option `KOTO`/`IVM` si les fichiers binaires ne sont pas disponibles dans `PATH`.
- Si `koto_compile` ouvre la version ABI, vérifiez ce que le compilateur et l'utilisation de ABI v1 (installez `koto_compile --abi` sans arguments, чтобы увидеть подддержку).
- La CLI permet d'accéder aux clés en hexadécimal ou en Base64. Pour les tests, vous pouvez utiliser les clés `iroha_cli tools crypto keypair`.
- Lorsque vous utilisez les charges utiles Norito en utilisant la commande `ivm_tool disassemble`, vous pouvez utiliser les instructions relatives aux applications Kotodama.

Cela pourrait vous aider à utiliser les tests CI et d'intégration. Pour plus de détails sur la grammaire Kotodama, les appels système sont mappés et la configuration externe Norito est la suivante :

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`