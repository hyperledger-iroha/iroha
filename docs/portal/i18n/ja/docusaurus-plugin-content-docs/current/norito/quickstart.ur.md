---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: Norito کوئک اسٹارٹ
説明: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
スラグ: /norito/quickstart
---

یہ ウォークスルー اس ورک فلو کی عکاسی کرتا ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز پہلی بار Norito اور Kotodama سیکھتے وقت فالو کریں: ایک ڈیٹرمنسٹک سنگل-پیئر نیٹ ورک بوٹテスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テストTorii ذریعے بھیجیں۔

重要なキー/値のキー/値の説明 `iroha_cli` の副作用٩ی توثیق کر سکیں۔

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) Compose V2 فعال ہو (اسے `defaults/docker-compose.single.yml` متعین サンプルピア شروع کرنے کے لئےありがとうございます)。
- Rust ツールチェーン (1.76+) ヘルパー バイナリ ヘルパー バイナリ ヘルパー バイナリ ヘルパー バイナリ ヘルパー バイナリ ヘルパー バイナリ
- `koto_compile`、`ivm_run`、`iroha_cli` バイナリワークスペースのチェックアウト チェックアウト リリース アーティファクトの一致 リリース アーティファクトの一致名前:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> バイナリ ワークスペース ワークスペース バイナリ バイナリ バイナリ ワークスペース ワークスペース バイナリ バイナリ
> یہ کبھی `serde`/`serde_json` سے لنک نہیں کرتے؛ Norito コーデックのエンドツーエンド

## 1. 開発者 پیٹ ورک شروع کریں

バンドルを作成します (`defaults/docker-compose.single.yml`)。ジェネシス クライアント構成 ヘルス プローブ Torii `http://127.0.0.1:8080` クライアント構成

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (前景 میں یا 分離)。 بعد کے تمام CLI کالز `defaults/client.toml` کے ذریعے اسی ピア کو ہدف بناتی ہیں۔

＃＃２．

最小の Kotodama の最小値:

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

> Kotodama バージョン管理 بہتر ہے۔ホストされた مثالیں [Norito 例ギャラリー](./examples/) میں بھی دستیاب ہیں اگر آپ زیادہ بھرپور قطہ آغاز چاہتے ہیں۔

## 3. IVM テストの予行演習

IVM/Norito バイトコード (`.to`) 文字コード (`.to`) 文字コード (`.to`) 文字コードホスト システムコールのホスト システムコールの実行:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナー `info("Hello from Kotodama")` پرنٹ کرتا ہے اور 模擬ホスト کے خلاف `SET_ACCOUNT_DETAIL` syscall انجام دیتا ہے۔ `ivm_tool` バイナリ バージョン `ivm_tool inspect target/quickstart/hello.to` ABI ヘッダー機能ビット エクスポートされたエントリポイント バージョン

## 4. Torii ذریعے バイトコード بھیجیں

バイトコード、CLI、Torii、文字列、バイトコード、Torii開発 ID `defaults/client.toml` میں موجود 公開鍵 سے اخذ ہوتی ہے، اس لئے アカウント ID یہ ہے:
```
<i105-account-id>
```

Torii URL、チェーン ID、署名キー、設定、および構成:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI Norito セキュリティ エンコード 開発キー 署名 サポート ピア 送信❁❁❁❁ `set_account_detail` システムコール 説明 Docker ログ コミットされたトランザクション ハッシュ 説明 CLI 出力 説明

## 5. 状態の変化

アカウントの詳細を確認するには、CLI アカウントの詳細を確認してください。

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Norito でバックアップされた JSON ペイロードの説明:

```json
{
  "hello": "world"
}
```

اگر ویلیو موجود نہ ہو تو تصدیق کریں کہ Docker compose سروس ابھی بھی چل رہی ہے اور `iroha` کے رپورٹ کردہ ٹرانزیکشن ہیش نے `Committed` حالت حاصل کر لی ہے۔

## ああ、

- خودکار طور پر تیار کی گئی [サンプル ギャラリー](./examples/) دیکھیں تاکہ
  高度な Kotodama スニペット Norito システムコール سے میپ ہوتے ہوئے سمجھ سکیں۔
- مزید گہرائی کے لئے [Norito スタート ガイド](./getting-started) پڑھیں جس میں
  コンパイラ/ランナー ツール、マニフェストの展開、IVM メタデータ、および
- 処理の繰り返し処理 ワークスペース 処理 `npm run sync-norito-snippets` 処理
  ダウンロード可能なスニペット、スニペット、ドキュメント、アーティファクト、`crates/ivm/docs/examples/`、ソース、同期されたデータ