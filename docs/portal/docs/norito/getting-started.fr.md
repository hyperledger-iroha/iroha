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

Ce guide rapide presente le workflow minimal pour compiler un contrat Kotodama, inspecter le bytecode Norito genere, l'executer localement et le deployer sur un nud Iroha.

## Prerequis

1. Installez la toolchain Rust (1.76 ou plus) et recuperez ce depot.
2. Construisez ou telechargez les binaires de support :
   - `koto_compile` - compilateur Kotodama qui emet du bytecode IVM/Norito
   - `ivm_run` et `ivm_tool` - utilitaires d'execution locale et d'inspection
   - `iroha_cli` - utilise pour le deploiement de contrats via Torii

   Le Makefile du depot attend ces binaires dans `PATH`. Vous pouvez telecharger des artefacts precompiles ou les construire depuis les sources. Si vous compilez la toolchain localement, pointez les helpers du Makefile vers les binaires :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Assurez-vous qu'un nud Iroha est en cours d'execution lorsque vous atteignez l'etape de deploiement. Les exemples ci-dessous supposent que Torii est accessible a l'URL configuree dans votre profil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compiler un contrat Kotodama

Le depot fournit un contrat minimal "hello world" dans `examples/hello/hello.ko`. Compilez-le en bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Options cles :

- `--abi 1` verrouille le contrat sur la version ABI 1 (la seule supportee au moment de la redaction).
- `--max-cycles 0` demande une execution sans limite ; fixez un nombre positif pour borner le padding de cycles pour les preuves de connaissance zero.

## 2. Inspecter l'artefact Norito (optionnel)

Utilisez `ivm_tool` pour verifier l'en-tete et les metadonnees integrees :

```sh
ivm_tool inspect target/examples/hello.to
```

Vous devriez voir la version ABI, les flags actives et les points d'entree exportes. C'est un controle rapide avant le deploiement.

## 3. Executer le contrat localement

Executez le bytecode avec `ivm_run` pour confirmer le comportement sans toucher un nud :

```sh
ivm_run target/examples/hello.to --args '{}'
```

L'exemple `hello` journalise une salutation et emet un syscall `SET_ACCOUNT_DETAIL`. L'execution locale est utile pendant que vous iterez sur la logique du contrat avant de le publier on-chain.

## 4. Deployer via `iroha_cli`

Lorsque vous etes satisfait du contrat, deployez-le sur un nud via le CLI. Fournissez un compte d'autorite, sa cle de signature et soit un fichier `.to` soit un payload Base64 :

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

La commande soumet un bundle manifeste Norito + bytecode via Torii et affiche l'etat de la transaction resultante. Une fois la transaction validee, le hash de code affiche dans la reponse peut servir a recuperer des manifests ou lister des instances :

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Executer via Torii

Avec le bytecode enregistre, vous pouvez l'invoquer en soumettant une instruction qui reference le code stocke (p. ex., via `iroha_cli ledger transaction submit` ou votre client applicatif). Assurez-vous que les permissions du compte autorisent les syscalls souhaites (`set_account_detail`, `transfer_asset`, etc.).

## Conseils et depannage

- Utilisez `make examples-run` pour compiler et executer les exemples en une seule commande. Surchargez les variables d'environnement `KOTO`/`IVM` si les binaires ne sont pas dans `PATH`.
- Si `koto_compile` refuse la version ABI, verifiez que le compilateur et le nud ciblent bien ABI v1 (executez `koto_compile --abi` sans arguments pour lister le support).
- Le CLI accepte des cles de signature en hex ou Base64. Pour les tests, vous pouvez utiliser les cles emises par `iroha_cli tools crypto keypair`.
- Lors du debug de payloads Norito, le sous-commande `ivm_tool disassemble` aide a correler les instructions avec le source Kotodama.

Ce flux reflete les etapes utilisees en CI et dans les tests d'integration. Pour une analyse plus approfondie de la grammaire Kotodama, des mappings de syscalls et des internals Norito, voir :

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
