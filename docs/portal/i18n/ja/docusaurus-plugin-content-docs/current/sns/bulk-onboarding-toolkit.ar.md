---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى المشغلون الخارجيون
SN-3b を使用してください。
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**重要な情報:** SN-3b「バルク オンボーディング ツール」  
** 回答:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

`.sora` او `.nexus` مع نفس
すごいです。ペイロード JSON يدويا او اعادة تشغيل
CLI は SN-3b ビルダー、CSV は Norito 、 هياكل
`RegisterNameRequestV1` Torii CLI。 يتحقق المساعد من كل صف مسبقا،
マニフェスト マニフェスト JSON マニフェスト マニフェスト JSON マニフェスト マニフェスト JSON マニフェスト マニフェスト JSON マニフェスト マニフェスト
ペイロードは、最大のペイロードです。

## 1.CSV

يتطلب المحلل صف العناوين التالي (الترتيب مرن):

|ああ | और देखेंああ |
|----------|----------|----------|
| `label` |とん | التسمية المطلوبة (يقبل حالة مختلطة; الاداة تطبع حسب Norm v1 و UTS-46)。 |
| `suffix_id` |とん | معرف لاحقة رقمي (عشري او `0x` hex)。 |
| `owner` |とん | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` |とん | عدد صحيح `1..=255`。 |
| `payment_asset_id` |とん | صل التسوية (مثل `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `payment_gross` / `payment_net` |とん | عداد صحيحة غير موقعة تمثل وحدات الاصل. |
| `settlement_tx` |とん | JSON ハッシュ。 |
| `payment_payer` |とん | AccountId は です。 |
| `payment_signature` |とん | JSON は、スチュワードとスチュワードの両方をサポートします。 |
| `controllers` |認証済み |コントローラー。 `[owner]` です。 |
| `metadata` |認証済み | JSON インライン `@path/to/file.json` セキュリティ リゾルバー TXT です。 `{}`。 |
| `governance` |認証済み | JSON インライン `@path` または `GovernanceHookV1`。 `--require-governance` يفرض هذا العمود. |

يمكن لاي عمود الاشارة الى ملف خارجي عبر بادئة قيمة الخلية بـ `@`。
CSV を使用します。

## 2. いいえ

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

意味:

- `--require-governance` يرفض الصفوف بدون フック حوكمة (مفيد لمزادات プレミアム او
  ）。
- `--default-controllers {owner,none}` コントローラー
  オーナー。
- `--controllers-column`、`--metadata-column`、`--governance-column`
  輸出も可能です。

マニフェストの内容:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

`--ndjson` 評価 `RegisterNameRequestV1` 評価 JSON واحد
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. いいえ

### 3.1 وضع Torii REST

`--submit-torii-url` `--submit-token` `--submit-token-file` ログイン
マニフェスト Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- `POST /v1/sns/names` HTTP を使用します。
  NDJSON をご覧ください。
- `--poll-status` يعيد الاستعلام عن `/v1/sns/names/{namespace}/{literal}` بعد كل
  (`--poll-attempts`, 5) を参照してください。うーん
  `--suffix-map` (JSON يحول `suffix_id` الى قيم "suffix") كي تتمكن الاداة من
  `{label}.{suffix}` ポーリングです。
- バージョン: `--submit-timeout`、`--poll-attempts`、`--poll-interval`。

### 3.2 iroha CLI

マニフェストと CLI のリスト:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- コントローラ `Account` (`controller_type.kind = "Account"`)
  CLI を使用して、コントローラを管理します。
- BLOB とメタデータとガバナンスを管理する
  `iroha sns register --metadata-json ... --governance-json ...`。
- 標準出力と標準エラー出力の標準出力最高のパフォーマンスを見せてください。

يمكن تشغيل وضعي الارسال معا (Torii و CLI) للتحقق المتقاطع من نشر المسجل او
フォールバック。

### 3.3 説明

`--submission-log <path>` يضيف السكربت سجلات NDJSON تلتقط:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Torii 認証 `NameRecordV1` 認証
`RegisterNameResponseV1` (`record_status`、`record_pricing_class`、
`record_owner`、`record_expires_at_ms`、`registry_event_version`、`suffix_id`、
`label`) حتى تتمكن لوحات المتابعة وتقارير الحوكمة من تحليل السجل دون تفتيش
ああ。マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト
ああ。

## 4. いいえ、いいえ、いいえ。

تستدعي مهام CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` الذي يلف
`artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

説明:1. يبني `registrations.manifest.json` و `registrations.ndjson` وينسخ CSV الاصلي
   そうです。
2. マニフェスト Torii و/او CLI (国際規格) `submissions.log` مع
   ありがとうございます。
3. يصدر `summary.json` الذي يصف الاصدار (المسارات، عنوان Torii، مسار CLI،)
   タイムスタンプ) 時間、時間、時間。
4. ينتج `metrics.prom` (`--metrics` をオーバーライド) متضمنا عدادات متوافقة مع
   Prometheus 問題を解決してください。
   JSON を使用してください。

ワークフローの説明 ワークフロー ワークフロー ワークフロー ワークフロー ワークフロー ワークフロー ワークフロー ワークフロー ワークフロー
ありがとうございます。

## 5. いいえ

セキュリティ `sns_bulk_release.sh` 番号:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

قم بتغذية `metrics.prom` الى サイドカー Prometheus لديك (مثلا عبر Promtail او
مستورد دفعات) للحفاظ على توافق المسجلين وstewards وشركاء الحوكمة حول تقدم
ああ。 Grafana `dashboards/grafana/sns_bulk_release.json` の評価
ログインしてください。 ログインしてください。
`release` حتى يتمكن المدققون من التعمق في تشغيل CSV
うーん。

## 6. いいえ。

- **توحيد ラベル:** يتم تطبيع الادخالات باستخدام Python IDNA مع 小文字 وفلاتر
  ノルム v1.最高のパフォーマンスを見せてください。
- ** حواجز رقمية:** يجب ان تقع サフィックス ID 、 期間年数 、 価格のヒント 、
  `u16` と `u8`。 16 進数は `i64::MAX` です。
- **メタデータとガバナンス:** JSON インライン セキュリティありがとう
  CSV を使用します。メタデータは、次のとおりです。
- **コントローラー:** `--default-controllers`。 قدم قوائم
  コントローラ (مثل `<i105-account-id>;<i105-account-id>`) は、コントローラを制御します。

يتم الابلاغ عن الاخطاء مع ارقام صفوف سياقية (مثلا)
`error: row 12 term_years must be between 1 and 255`)。セキュリティ `1`
`2` と CSV を表示します。

## 7. ああ、

- يغطي `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` 評価 CSV
  NDJSON を使用して、CLI を使用して Torii を実行します。
- المساعد مكتوب ببايثون فقط (بدون تبعيات اضافية) ويعمل حيث يتوفر `python3`。
  CLI を使用して、セキュリティを強化します。

マニフェストを作成する NDJSON を作成する スチュワードを作成する
ペイロードは Torii です。