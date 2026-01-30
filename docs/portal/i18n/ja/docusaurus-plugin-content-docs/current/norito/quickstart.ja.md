---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cc65ac9e2f70bc1a374969afded1aab9843834958fc0d9f5a94843aea5f7f880
source_last_modified: "2025-11-14T04:43:20.901058+00:00"
translation_last_reviewed: 2026-01-30
---

このウォークスルーは、初めて Norito と Kotodama を学ぶ開発者に期待するワークフローを反映しています。決定的な単一ピアネットワークを起動し、コントラクトをコンパイルし、ローカルでドライランしてから、参照 CLI で Torii に送信します。

例のコントラクトは呼び出し元のアカウントにキー/値ペアを書き込むため、`iroha_cli` ですぐに副作用を確認できます。

## 前提条件

- Compose V2 が有効な [Docker](https://docs.docker.com/engine/install/) ( `defaults/docker-compose.single.yml` で定義されたサンプルピアの起動に使用します)。
- 公開済みバイナリをダウンロードしない場合に備えて、Rust ツールチェーン (1.76+)。
- `koto_compile`、`ivm_run`、`iroha_cli` のバイナリ。以下のように workspace のチェックアウトからビルドするか、対応するリリースアーティファクトをダウンロードできます:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> 上記のバイナリは workspace の他のツールと併存してインストールしても安全です。
> `serde`/`serde_json` にはリンクせず、Norito のコードックは end-to-end で強制されます。

## 1. 単一ピアの開発ネットワークを起動する

リポジトリには `kagami swarm` が生成した Docker Compose バンドル (`defaults/docker-compose.single.yml`) が含まれています。デフォルトの genesis、クライアント設定、ヘルスプローブを接続し、Torii が `http://127.0.0.1:8080` で到達可能になるようにしています。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

コンテナは起動したままにしてください (前面でもデタッチでも構いません)。以降の CLI 呼び出しは `defaults/client.toml` を通じてこのピアを参照します。

## 2. コントラクトを作成する

作業用ディレクトリを作成し、最小の Kotodama 例を保存します:

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

> Kotodama のソースはバージョン管理に置くのが望ましいです。より充実した出発点が欲しい場合は、ポータルの [Norito 例ギャラリー](./examples/) にも例があります。

## 3. IVM でコンパイルしてドライランする

コントラクトを IVM/Norito バイトコード (`.to`) にコンパイルし、ネットワークに触れる前にローカルで実行してホストの syscalls が成功することを確認します:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナーは `info("Hello from Kotodama")` のログを出力し、モックされたホストに対して `SET_ACCOUNT_DETAIL` syscall を実行します。オプションの `ivm_tool` バイナリがある場合、`ivm_tool inspect target/quickstart/hello.to` で ABI ヘッダ、feature bits、エクスポートされた entrypoints を表示できます。

## 4. Torii 経由でバイトコードを送信する

ノードが動作している状態で、コンパイル済みバイトコードを CLI で Torii に送信します。デフォルトの開発用アイデンティティは `defaults/client.toml` の公開鍵から導出されるため、アカウント ID は
```
ih58...
```

Torii の URL、chain ID、署名鍵は設定ファイルで指定します:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI は Norito でトランザクションをエンコードし、開発用キーで署名して実行中のピアに送信します。Docker のログで `set_account_detail` syscall を確認するか、CLI 出力でコミット済みトランザクションのハッシュを確認してください。

## 5. 状態変更を確認する

同じ CLI プロファイルで、コントラクトが書き込んだアカウント詳細を取得します:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Norito によって支えられた JSON payload が表示されるはずです:

```json
{
  "hello": "world"
}
```

値が見当たらない場合は、Docker compose サービスが動作していることと、`iroha` が報告したトランザクションハッシュが `Committed` 状態に到達したことを確認してください。

## 次のステップ

- 自動生成された [例ギャラリー](./examples/) を探索し、
  より高度な Kotodama スニペットが Norito syscalls にどう対応するかを確認します。
- コンパイラ/ランナーのツール群、manifest デプロイ、IVM メタデータについてより深く理解するには、[Norito getting started](./getting-started) を参照してください。
- 自分のコントラクトを反復する際は、workspace で `npm run sync-norito-snippets` を実行して、
  ダウンロード可能なスニペットを再生成し、ポータルの docs と `crates/ivm/docs/examples/` のソースが同期されるようにしてください。
