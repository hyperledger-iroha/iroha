---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: بدء سريع ل Norito
説明: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الافتراضية ذات العقدةああ。
スラグ: /norito/quickstart
---

يعكس هذا الدليل العملي سير العمل الذي نتوقع ان يتبعه المطورون عند تعلم Norito و Kotodama 問題: テストのテスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テスト テストTorii CLI を使用してください。

يكتب عقد المثال زوج مفتاح/قيمة في حساب المستدعي حتى تتمكن من التحقق من الاثر الجانبي `iroha_cli`。

## いいえ

- [Docker](https://docs.docker.com/engine/install/) Compose V2 (ピア العينة المحدد في `defaults/docker-compose.single.yml`)。
- Rust (1.76+) をインストールしてください。
- `koto_compile` و`ivm_run` و`iroha_cli`。チェックアウト ワークスペース ワークスペース アーティファクト ワークスペース:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ワークスペース。
> `serde`/`serde_json` を参照してください。 Norito コーデックは、Norito コーデックに対応しています。

## 1. شغيل شبكة تطوير بعقدة واحدة

バンドル Docker `kagami swarm` (`defaults/docker-compose.single.yml`) を作成します。ジェネシス、健康プローブ、Torii、`http://127.0.0.1:8080`。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

اترك الحاوية تعمل (في المقدمة او مفصولة)。 CLI を使用して、ピア `defaults/client.toml` を実行します。

## 2. كتابة العقد

Kotodama 番号:

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

> يفضل حفظ مصادر Kotodama تحت التحكم بالنسخ. توجد ايضا امثلة مستضافة على البوابة ضمن [معرض امثلة Norito](./examples/) اذا كنت تريد और देखें

## 3. 予行演習 IVM

バイトコード IVM/Norito (`.to`) のシステムコールを実行します。意味:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナー `info("Hello from Kotodama")` syscall `SET_ACCOUNT_DETAIL` を実行します。 `ivm_tool` セキュリティ `ivm_tool inspect target/quickstart/hello.to` セキュリティ ABI ヘッダー、機能ビット、エントリ ポイント。

## 4. バイトコード Torii

バイトコード Torii CLI を使用します。 `defaults/client.toml` の意味:
```
soraカタカナ...
```

アクセス先 URL アクセス Torii チェーン ID アクセス:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI を使用して、Norito を使用して、ピアにアクセスします。 Docker syscall `set_account_detail` と CLI のハッシュ。

## 5. 本当のことを言う

CLI アカウントの詳細情報:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

ペイロード JSON と Norito:

```json
{
  "hello": "world"
}
```

اذا كانت القيمة مفقودة، تاكد من خدمة Docker compose ما زالت تعمل وان hash المعاملة الذي ابلغ `iroha` と `Committed` です。

## ああ、

- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  Kotodama は Norito のシステムコールを呼び出します。
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  ランナー/ランナーはマニフェスト IVM を表示します。
- عند التكرار على عقودك، استخدم `npm run sync-norito-snippets` في
  ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース ワークスペース
  `crates/ivm/docs/examples/` を確認してください。