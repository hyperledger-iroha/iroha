<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5cae8fa9d9a69d506d0fc49903e801382041d29f2e9a052321224bd3cb7a72d1
source_last_modified: "2025-11-02T04:40:39.595528+00:00"
translation_last_reviewed: 2025-12-30
---

# Prise en main de Norito

Ce guide rapide présente le workflow minimal pour compiler un contrat Kotodama, inspecter le bytecode Norito généré, l'exécuter localement et le déployer sur un nœud Iroha.

## Prérequis

1. Installez la toolchain Rust (1.76 ou plus) et récupérez ce dépôt.
2. Construisez ou téléchargez les binaires de support :
   - `koto_compile` - compilateur Kotodama qui émet du bytecode IVM/Norito
   - `ivm_run` et `ivm_tool` - utilitaires d'exécution locale et d'inspection
   - `iroha_cli` - utilisé pour le déploiement de contrats via Torii

   Le Makefile du dépôt attend ces binaires dans `PATH`. Vous pouvez télécharger des artefacts précompilés ou les construire depuis les sources. Si vous compilez la toolchain localement, pointez les helpers du Makefile vers les binaires :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Assurez-vous qu'un nœud Iroha est en cours d'exécution lorsque vous atteignez l'étape de déploiement. Les exemples ci-dessous supposent que Torii est accessible à l'URL configurée dans votre profil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler un contrat Kotodama

Le dépôt fournit un contrat minimal "hello world" dans `examples/hello/hello.ko`. Compilez-le en bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Options clés :

- `--abi 1` verrouille le contrat sur la version ABI 1 (la seule supportée au moment de la rédaction).
- `--max-cycles 0` demande une exécution sans limite ; fixez un nombre positif pour borner le padding de cycles pour les preuves de connaissance zéro.

## 2. Inspecter l'artefact Norito (optionnel)

Utilisez `ivm_tool` pour vérifier l'en-tête et les métadonnées intégrées :

```sh
ivm_tool inspect target/examples/hello.to
```

Vous devriez voir la version ABI, les flags activés et les points d'entrée exportés. C'est un contrôle rapide avant le déploiement.

## 3. Exécuter le contrat localement

Exécutez le bytecode avec `ivm_run` pour confirmer le comportement sans toucher un nœud :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` journalise une salutation et émet un syscall `SET_ACCOUNT_DETAIL`. L'exécution locale est utile pendant que vous itérez sur la logique du contrat avant de le publier on-chain.

## 4. Déployer via `iroha_cli`

Lorsque vous êtes satisfait du contrat, déployez-le sur un nœud via le CLI. Fournissez un compte d'autorité, sa clé de signature et soit un fichier `.to` soit un payload Base64 :

```sh
iroha_cli contracts deploy \
  --authority alice@wonderland \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande soumet un bundle manifeste Norito + bytecode via Torii et affiche l'état de la transaction résultante. Une fois la transaction validée, le hash de code affiché dans la réponse peut servir à récupérer des manifests ou lister des instances :

```sh
iroha_cli contracts manifest --code-hash 0x<hash>
iroha_cli contracts instances --namespace apps --table
```

## 5. Exécuter via Torii

Avec le bytecode enregistré, vous pouvez l'invoquer en soumettant une instruction qui référence le code stocké (p. ex., via `iroha_cli transaction submit` ou votre client applicatif). Assurez-vous que les permissions du compte autorisent les syscalls souhaités (`set_account_detail`, `transfer_asset`, etc.).

## Conseils et dépannage

- Utilisez `make examples-run` pour compiler et exécuter les exemples en une seule commande. Surchargez les variables d'environnement `KOTO`/`IVM` si les binaires ne sont pas dans `PATH`.
- Si `koto_compile` refuse la version ABI, vérifiez que le compilateur et le nœud ciblent bien ABI v1 (exécutez `koto_compile --abi` sans arguments pour lister le support).
- Le CLI accepte des clés de signature en hex ou Base64. Pour les tests, vous pouvez utiliser les clés émises par `iroha_cli crypto keypair`.
- Lors du debug de payloads Norito, le sous-commande `ivm_tool disassemble` aide à corréler les instructions avec le source Kotodama.

Ce flux reflète les étapes utilisées en CI et dans les tests d'intégration. Pour une analyse plus approfondie de la grammaire Kotodama, des mappings de syscalls et des internals Norito, voir :

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
