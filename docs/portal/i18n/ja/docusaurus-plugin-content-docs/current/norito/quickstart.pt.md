---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 急速な実行 Norito
説明: 説明: コントラート Kotodama ツールのリリースと、UNICO ピアの再デプロイを有効にします。
スラグ: /norito/quickstart
---

エステは、定期的なエスペルハ、フラクソ ケ エスペラモス、デセンボルベドーレスのシガム アプレンダー Norito と Kotodama を最初に確認します: ユニコ ピアの初期設定、コントラートのコンパイル、ローカル メンテとデポジットの予行演習を開始します。 Torii com o CLI を参照して参照してください。

O contrato de exemplo grav um par Chave/valor na conta do Chamador para que voce possa verificar o efeito coicular immediatamente com `iroha_cli`。

## 前提条件

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 habilitado (`defaults/docker-compose.single.yml` を使用して模範を定義)。
- ツールチェーン Rust (1.76+) は、バイナリ OS のコンパイラを補助し、公開されています。
- ビナリオ `koto_compile`、`ivm_run`、`iroha_cli`。リリース担当者のコンピラの内容を確認し、ワークスペースで最も多くの情報を入手し、バイザーの OS アーティファクトを確認してください:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ワークスペースをインストールするためのオペレーティング システムをインストールします。
> Eles nunca fazem リンク com `serde`/`serde_json`; OS コーデック Norito SAO アプリケーションをポンタにインストールします。

## 1. Unico ピアの初期化

バンドル Docker を含むリポジトリ。`kagami swarm` (`defaults/docker-compose.single.yml`) でジェラドを作成します。ジェネシス パドラオに接続し、Torii から `http://127.0.0.1:8080` にアクセスできるクライアントの OS ヘルス プローブを構成します。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

デシェ・オ・コンテナ・ロダンド（エム・プリミイロ・プラノ・オウ・デタッチ）。 Todas は、`defaults/client.toml` を介して CLI 事後ピアに接続されます。

## 2. エスクレヴァ・オ・コントラート

Kotodama のトラバルホおよび治療用ミニモの例を作成します:

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

> OS フォント Kotodama を管理してください。例としては、[例として表示される Norito](./examples/) というポータルがありません。これは、重要な問題です。

## 3. ドライラン com IVM をコンパイルします。

バイトコード IVM/Norito (`.to`) をコンパイルし、実行する前にホスト機能をローカルで確認してシステムコールを実行します。

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナー インプリメ ログ `info("Hello from Kotodama")` システムコール `SET_ACCOUNT_DETAIL` ホスト モカドに対して実行します。バイナリオオプション `ivm_tool` ESTIVER ディスポニベル、`ivm_tool inspect target/quickstart/hello.to` ほとんどの ABI ヘッダー、OS 機能ビット、OS エントリポイントエクスポート。

## 4. Torii 経由の Envie バイトコード

Com o nodo ainda robando, envie o bytecode compilado para Torii usando o CLI。 `defaults/client.toml` の公開情報を公開し、情報を保存するための識別情報
```
ih58...
```

URL として Torii の設定パラメータを使用し、チェーン ID を組み合わせて保存します。

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

O CLI は、トランザクション com Norito をコード化し、ピア エム実行経由で開発環境を作成します。 OS ログを観察して、システムコール `set_account_detail` に関連する Docker を監視し、CLI に関連するハッシュやトランザクションのコミットを監視します。

## 5. ムダンカ・デ・エスタドを検証する

CLI バスカーの詳細情報を使用するか、アカウントの詳細を確認するか、詳細を確認します。

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Norito のペイロード JSON サポートの開発:

```json
{
  "hello": "world"
}
```

勇気を出して、サービス Docker を作成して、ハッシュ ダ トランザクション報告書 `iroha` を確認して、`Committed` を確認してください。

## プロキシモス パソス

- [galeria de exemplos](./examples/) gerada automamente para ver を探索してください
  コモ スニペット Kotodama は、システムコール Norito のマップを作成します。
- レイア [Norito](./getting-started) 説明を参照してください
  コンパイラ/ランナーのツールの作成、マニフェストのデプロイ、およびメタデータ IVM の実行が広範囲に行われます。
- 繰り返しを繰り返します。`npm run sync-norito-snippets` ワークスペース パラメータを使用しません。
  再生スニペットは、`crates/ivm/docs/examples/` のフォントとしてポータルとアーティファクトの同期を実行し、OS ドキュメントを管理します。