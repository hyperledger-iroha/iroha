---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: Norito の急速な進行
説明: クレア、契約 Kotodama のリリースとソロ ピアの事前決定の確認を確認します。
スラグ: /norito/quickstart
---

エステは、Norito と Kotodama の最初の段階で、リフレハ エル フルホ ケ エスペラモス ケ シガン ロス デザロラドーレスを確認します。ソロ ピア、コンパイル、コントラート、ヘイサー、ドライラン ローカル、ルエゴ エンビアロの決定を確認します。 Torii の CLI 参照。

El contrato de ejemplo は、`iroha_cli` の効果を検証するためのクエンタ デル ラマドールのクラーベ/勇気を記述します。

## 以前の要件

- [Docker](https://docs.docker.com/engine/install/) Compose V2 の機能 (`defaults/docker-compose.single.yml` を参照)。
- Toolchain de Rust (1.76+) は、公開されたデータをダウンロードすることなく、補助的な構築を可能にします。
- ビナリオ `koto_compile`、`ivm_run`、`iroha_cli`。リリース対応の成果物をダウンロードして、ワークスペースでチェックアウトを実行する方法:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ワークスペースを保存する前に、事前にセットアップを行う必要があります。
> ヌンカ・エンラザン・コン `serde`/`serde_json`; los コーデック Norito はエンドツーエンドのアプリケーションです。

## 1. ソロピアで赤い開発を始める

このリポジトリには、Docker のバンドル、`kagami swarm` (`defaults/docker-compose.single.yml`) のジェネレータが含まれています。欠陥の発生源に接続し、Torii から `http://127.0.0.1:8080` にアクセス可能な健康プローブをクライアントに設定します。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deja el contenedor corriendo (英語の入門書、またはデサコプラド)。後方からの CLI の検査は、ピア中央部 `defaults/client.toml` で行われます。

## 2. コントラートの編集

Kotodama の管理者および管理者用のミニモを作成します:

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

> Kotodama のバージョン管理を管理します。ポータル サイトでのアロハドスの損失は、[ガレリア デ エジェンプロス Norito](./examples/) で、すべてのパートタイムが完了します。

## 3. コンパイルと危険性のドライランコン IVM

バイトコード IVM/Norito (`.to`) をコンパイルして、ホスト機能の確認のためのローカル システムコールの確認を行ってください:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナー インプリメ ログ `info("Hello from Kotodama")` とシステムコール `SET_ACCOUNT_DETAIL` の対照ホスト シミュレーションを実行します。オプションの `ivm_tool` 保存可能、`ivm_tool inspect target/quickstart/hello.to` の ABI 管理、機能のビットとエントリポイントのエクスポートの損失。

## 4. Torii 経由の Envia el バイトコード

ノードを修正し、Torii を使用して CLI を使用してバイトコードをコンパイルします。 `defaults/client.toml` で公開されているクラーベの情報の欠陥を特定し、ID を確認します
```
<i105-account-id>
```

Torii の URL、クラベのチェーン ID での管理のアーカイブの使用:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Norito でのトランザクション コードの CLI は、クライアントからの排出をサポートするクラベ デ サロロイの会社です。システムコール `set_account_detail` の Docker のログを監視し、CLI のトランザクション ハッシュの監視を監視します。

## 5. ベリフィカ エル カンビオ デ スタド

CLI に関する米国のミスモ パーフィルとアカウント詳細の確認:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Deberias バージョンのペイロード JSON 応答 Norito:

```json
{
  "hello": "world"
}
```

勇気を出して、Docker のサービスを確認して、`iroha` の操作と `Committed` の操作と排出とトランザクションのハッシュのレポートを作成してください。

## シギエンテス パソス

- Explora la [galeria de ejemplos](./examples/) autogenerada para ver
  フラグメント Kotodama は、システムコール Norito を実行します。
- Lee la [guia de inicio de Norito](./getting-started) パラ ウナ エクスプリカシオン
  コンパイラ/ランナーのツール、IVM のメタデータのマニフェストの詳細。
- Cuando iteres en tus propios contratos、米国 `npm run sync-norito-snippets` en el
  ワークスペース パラ リジェネラー スニペット デ モード ドキュメント デル ポータル および ロス アーティファクト
  SE mantengan sincronizados con las fuentes en `crates/ivm/docs/examples/`。