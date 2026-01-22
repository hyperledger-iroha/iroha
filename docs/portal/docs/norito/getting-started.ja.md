---
lang: ja
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5cae8fa9d9a69d506d0fc49903e801382041d29f2e9a052321224bd3cb7a72d1
source_last_modified: "2025-11-02T04:40:39.595528+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito 入門

このクイックガイドでは、Kotodama コントラクトをコンパイルし、生成された Norito バイトコードを確認し、ローカルで実行して Iroha ノードにデプロイするまでの最小の流れを示します。

## 前提条件

1. Rust ツールチェーン (1.76 以上) をインストールし、このリポジトリをチェックアウトします。
2. サポート用バイナリをビルドまたはダウンロードします:
   - `koto_compile` - IVM/Norito バイトコードを生成する Kotodama コンパイラ
   - `ivm_run` と `ivm_tool` - ローカル実行と検査のユーティリティ
   - `iroha_cli` - Torii 経由でのコントラクトデプロイに使用

   リポジトリの Makefile はこれらのバイナリが `PATH` にあることを前提としています。事前ビルド済み成果物をダウンロードするか、ソースからビルドできます。ローカルでツールチェーンをビルドする場合は、Makefile のヘルパーにバイナリのパスを指定します:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. デプロイ手順に進む際は Iroha ノードが稼働していることを確認してください。以下の例では、Torii が `iroha_cli` プロファイル (`~/.config/iroha/cli.toml`) に設定された URL で到達可能であることを前提としています。

## 1. Kotodama コントラクトをコンパイルする

リポジトリには最小の "hello world" コントラクトが `examples/hello/hello.ko` に含まれています。Norito/IVM バイトコード (`.to`) にコンパイルします:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

主なフラグ:

- `--abi 1` はコントラクトを ABI バージョン 1 に固定します (執筆時点で唯一サポートされるバージョン)。
- `--max-cycles 0` は無制限実行を要求します。ゼロ知識証明のサイクルパディングを制限するには正の数を設定します。

## 2. Norito アーティファクトを確認する (任意)

`ivm_tool` を使ってヘッダと埋め込まれたメタデータを検証します:

```sh
ivm_tool inspect target/examples/hello.to
```

ABI バージョン、有効なフラグ、エクスポートされたエントリポイントが表示されます。デプロイ前の簡単な確認です。

## 3. コントラクトをローカルで実行する

`ivm_run` でバイトコードを実行し、ノードに触れずに挙動を確認します:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` の例は挨拶をログに出力し、`SET_ACCOUNT_DETAIL` syscall を発行します。ローカル実行は、オンチェーン公開前にコントラクトロジックを反復する際に有用です。

## 4. `iroha_cli` でデプロイする

コントラクトに満足したら、CLI を使ってノードにデプロイします。権限アカウント、署名鍵、そして `.to` ファイルまたは Base64 payload を指定します:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

このコマンドは Norito マニフェスト + バイトコードの bundle を Torii 経由で送信し、トランザクションのステータスを表示します。コミット後、レスポンスに表示されるコードハッシュを使ってマニフェストの取得やインスタンス一覧ができます:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii 経由で実行する

バイトコードが登録されると、保存済みコードを参照する命令を送信して呼び出せます (例: `iroha_cli ledger transaction submit` やアプリケーションクライアント経由)。アカウントの権限が必要な syscall (`set_account_detail`, `transfer_asset` など) を許可していることを確認してください。

## ヒントとトラブルシューティング

- `make examples-run` を使うと、提供された例を一括でコンパイルして実行できます。バイナリが `PATH` にない場合は `KOTO`/`IVM` 環境変数で上書きしてください。
- `koto_compile` が ABI バージョンを拒否する場合、コンパイラとノードが ABI v1 を対象としていることを確認してください (`koto_compile --abi` を引数なしで実行すると対応一覧を表示します)。
- CLI は hex または Base64 の署名鍵を受け付けます。テストでは `iroha_cli tools crypto keypair` が出力する鍵を使えます。
- Norito payload をデバッグする際は、`ivm_tool disassemble` サブコマンドが Kotodama ソースと命令を対応付けるのに役立ちます。

この流れは CI と統合テストで使われる手順を反映しています。Kotodama の文法、syscall の対応、Norito の内部に関する詳細は以下を参照してください:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
