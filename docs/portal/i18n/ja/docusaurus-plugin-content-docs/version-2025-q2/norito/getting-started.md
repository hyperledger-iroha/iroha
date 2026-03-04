---
lang: ja
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T15:55:55+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito はじめに

このクイック ガイドでは、Kotodama コントラクトをコンパイルするための最小限のワークフローを示します。
生成された Norito バイトコードを検査し、ローカルで実行し、デプロイします。
Iroha ノードに送信します。

## 前提条件

1. Rust ツールチェーン (1.76 以降) をインストールし、このリポジトリをチェックアウトします。
2. サポートするバイナリをビルドまたはダウンロードします。
   - `koto_compile` – IVM/Norito バイトコードを発行する Kotodama コンパイラ
   - `ivm_run` および `ivm_tool` – ローカル実行および検査ユーティリティ
   - `iroha_cli` – Torii 経由のコントラクト展開に使用されます

   リポジトリ Makefile は、`PATH` 上にこれらのバイナリを想定しています。どちらでもできます
   事前に構築されたアーティファクトをダウンロードするか、ソースからビルドします。コンパイルすると、
   ツールチェーンをローカルに配置し、Makefile ヘルパーにバイナリを指定します。

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. 導入ステップに到達したときに、Iroha ノードが実行されていることを確認します。の
   以下の例では、Torii が、設定された URL で到達可能であると仮定しています。
   `iroha_cli` プロファイル (`~/.config/iroha/cli.toml`)。

## 1. Kotodama コントラクトをコンパイルする

リポジトリは最小限の「hello world」コントラクトを次の形式で同梱します。
`examples/hello/hello.ko`。これを Norito/IVM バイトコード (`.to`) にコンパイルします。

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

主要なフラグ:

- `--abi 1` は、コントラクトを ABI バージョン 1 (現在サポートされている唯一のバージョン) にロックします。
  執筆時）。
- `--max-cycles 0` は無制限の実行を要求します。正の数値を境界に設定します
  ゼロ知識証明のためのサイクルパディング。

## 2. Norito アーティファクトを検査します (オプション)

`ivm_tool` を使用して、ヘッダーと埋め込みメタデータを確認します。

```sh
ivm_tool inspect target/examples/hello.to
```

ABI バージョン、有効な機能フラグ、エクスポートされたエントリが表示されます。
ポイント。これは、展開前の簡単な健全性チェックです。

## 3. ローカルでコントラクトを実行する

`ivm_run` でバイトコードを実行し、何も触れずに動作を確認します。
ノード:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` の例では、グリーティングをログに記録し、`SET_ACCOUNT_DETAIL` syscall を発行します。
ローカルで実行すると、公開前にコントラクト ロジックを反復するときに便利です
それはオンチェーンです。

## 4. `iroha_cli` 経由でデプロイする

コントラクトに満足したら、CLI を使用してノードにデプロイします。
権限アカウント、その署名キー、および `.to` ファイルまたは
Base64 ペイロード:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

このコマンドは、Norito マニフェスト + バイトコード バンドルを Torii 経由で送信し、出力します。
結果として得られるトランザクションのステータス。トランザクションがコミットされると、コードは
応答に示されるハッシュを使用して、マニフェストを取得したり、インスタンスをリストしたりできます。

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii に対して実行します。

バイトコードが登録されている場合は、命令を送信することで呼び出すことができます
保存されたコードを参照するもの (例: `iroha_cli ledger transaction submit` 経由)
またはアプリケーションクライアント)。アカウントの権限で必要な権限が許可されていることを確認してください
システムコール (`set_account_detail`、`transfer_asset` など)。

## ヒントとトラブルシューティング

- `make examples-run` を使用して、提供された例を 1 つにコンパイルして実行します
  撃った。バイナリが存在しない場合は、`KOTO`/`IVM` 環境変数をオーバーライドします。
  `PATH`。
- `koto_compile` が ABI バージョンを拒否した場合は、コンパイラとノードが
  両方とも ABI v1 をターゲットとしています (リストへの引数を指定せずに `koto_compile --abi` を実行します)
  サポート）。
- CLI は 16 進数または Base64 署名キーを受け入れます。テストには、次を使用できます
  `iroha_cli tools crypto keypair` によって発行されたキー。
- Norito ペイロードをデバッグする場合、`ivm_tool disassemble` サブコマンドが役立ちます
  命令を Kotodama ソースと関連付けます。

このフローは、CI および統合テストで使用される手順を反映しています。さらに深くするために
Kotodama の文法、syscall マッピング、および Norito の内部構造について詳しくは、以下を参照してください。

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`