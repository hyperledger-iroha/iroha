---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: デマラージュ Norito
説明: コントラット Kotodama を構成し、デフォルトのモノペアをリリースおよび再設定します。
スラグ: /norito/quickstart
---

CE ウォークスルーはワークフローを再現し、開発 Norito および Kotodama をプレミアに提供します: モノペアの決定、コンパイラ、コントラクト、ドライラン ロケール、監視者経由の決定Torii の CLI リファレンス。

Le contrat d'example ecrit une pare cle/valeur dans le compte de l'appelant afin que vous puissiez verifier l'efet de bord avec `iroha_cli`.

## 前提条件

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 がアクティブです (`defaults/docker-compose.single.yml` のサンプル定義のペアを使用します)。
- ツールチェーン Rust (1.76+) は、公共の場で電気料金を徴収する補助金を構築します。
- バイネール `koto_compile`、`ivm_run`、および `iroha_cli`。リリース担当者が、テレチャージャーやテレチャージャーの作業スペースでのチェックアウトを検討します。:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> 危険なインストーラーを使用せずに、ワークスペースを安全に保つことができます。
> Ils ne lient jamais `serde`/`serde_json` ;コーデック Norito ソント アップリケ デ ブアウト。

## 1. Demarrer un reseau dev モノペア

デポにはバンドル Docker が含まれており、`kagami swarm` (`defaults/docker-compose.single.yml`) のジャンルを構成します。 Torii を使用して、`http://127.0.0.1:8080` と接続できるように、デフォルトでジェネシスを接続し、構成クライアントとヘルス プローブを接続します。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur tourner (au premier plan ou en detache)。 `defaults/client.toml` 経由で CLI のコマンドを実行します。

## 2. エクリール ル コントラ

Creez un repertoire de travail および enregistrez の例 Kotodama minimum :

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

> バージョン管理のソース Kotodama を優先します。 Des example heberges sur le portail Sont aussi disponibles dans la [galerie d'examples Norito](./examples/) 出発点と豊かさを感じてください。

## 3. コンパイラとドライラン avec IVM

バイトコード IVM/Norito (`.to`) をコンパイルし、ホストの再使用前にシステムコールを確認してロケールを実行します。

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナーの主要なログ `info("Hello from Kotodama")` とシステムコール `SET_ACCOUNT_DETAIL` のホスト シミュレーションの影響。バイナリ オプション `ivm_tool` の最も有効なオプション、`ivm_tool inspect target/quickstart/hello.to` の ABI 添付ファイル、機能およびエントリ ポイントのエクスポートのビット。

## 4. Torii 経由のスメットル ファイル バイトコード

Le noeud etant toujours en cours d'execution、envoyez le バイトコードは Torii avec le CLI をコンパイルします。 `defaults/client.toml` から公開されたデフォルトの開発の識別情報、準拠する ID の取得
```
ih58...
```

URL Torii、チェーン ID および署名の設定を使用します。

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI は、トランザクションの平均 Norito をエンコードし、開発の署名および実行のペアを実行します。ログ Docker システムコール `set_account_detail` を監視し、CLI で出撃したトランザクションのハッシュをコミットしました。

## 5. 変更決定の検証者

アカウントの詳細を確認するには、ミーム プロファイル CLI を使用します。

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Norito をサポートするペイロード JSON を確認します。

```json
{
  "hello": "world"
}
```

不在の場合は、サービス Docker を確認して、`iroha` のトランザクション信号を作成し、`Committed` を確認してください。

## エテープ・スイバンテス

- Explorez la [galerie d'examples](./examples/) 自動生成
  コメント スニペット Kotodama と、システムコール Norito のマップを作成します。
- Lisez le [ガイド Norito 入門](./getting-started) を説明してください
  さらに、コンパイル/ランナーのアプリケーション、マニフェストとメタドンの展開 IVM。
- Lorsque vous iterez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  ワークスペースにリジェネラーのスニペットが表示され、Telechargeables のドキュメントとポータルのドキュメントと保存されたアーティファクトが表示されます。
  avec les ソースを `crates/ivm/docs/examples/` と同期します。