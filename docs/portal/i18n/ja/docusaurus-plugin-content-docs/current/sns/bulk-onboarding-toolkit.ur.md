---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز ریپو کلون کئے بغیر وہی SN-3b رہنمائی دیکھ سکیں۔
:::

# SNS バルク オンボーディング ツールキット (SN-3b)

** روڈمیپ حوالہ:** SN-3b 「バルク オンボーディング ツール」  
**アーティファクト:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

レジストラ `.sora` یا `.nexus` 登録数 ガバナンス
承認 決済レール کے ساتھ سیکڑوں کی تعداد میں پہلے سے تیار کرتے
やあJSON ペイロード、CLI、スケール、スケール、JSON ペイロード、JSON ペイロード、CLI、スケール、スケール
SN-3b 決定論的 CSV-to-Norito ビルダー Torii CLI
`RegisterNameRequestV1` 構造体ヘルパー ہر 行 کو پہلے
集約されたマニフェストを検証する オプションの改行区切りの JSON を検証する
監査 構造化された領収書 ペイロード ペイロード
送信してください ہے۔

## 1. CSV ファイル

パーサーのヘッダー行 (順序は柔軟です):

|コラム |必須 |説明 |
|----------|----------|---------------|
| `label` |はい |要求されたラベル (大文字と小文字の混合が受け入れられます。ツール Norm v1 اور UTS-46 کے مطابق 正規化 کرتا ہے)。 |
| `suffix_id` |はい |数値サフィックス識別子 (10 進数、`0x` 16 進数)。 |
| `owner` |はい | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` |はい |整数 `1..=255`。 |
| `payment_asset_id` |はい |決済資産 (`61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `payment_gross` / `payment_net` |はい |符号なし整数は資産固有の単位を表します。 |
| `settlement_tx` |はい | JSON 値 リテラル文字列 支払いトランザクション ハッシュ値|
| `payment_payer` |はい | AccountId 支払いを承認する|
| `payment_signature` |はい | JSON یا リテラル文字列 جس میں スチュワード/財務省の署名証明 ہو۔ |
| `controllers` |オプション |コントローラー アカウント アドレス (セミコロン/カンマ区切りのリスト) خالی ہونے پر `[owner]`。 |
| `metadata` |オプション |インライン JSON `@path/to/file.json` リゾルバー ヒント TXT レコードデフォルト `{}`۔ |
| `governance` |オプション |インライン JSON `@path` جو `GovernanceHookV1` طرف ہو۔ `--require-governance` 番号列 ٩و لازمی کرتا ہے۔ |

列 سیل ویلیو میں `@` لگا کر 外部ファイル کو 参照 کر سکتا ہے۔
パス CSV ファイル 相対解決 ہ​​یں۔

## 2. ヘルパー

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

主なオプション:

- `--require-governance` ガバナンス フック بغیر 行が拒否 کرتا ہے (プレミアム)
  オークション (予約済み割り当て)。
- `--default-controllers {owner,none}` コントローラ セルの所有者
  アカウント پر واپس جائیں یا نہیں。
- `--controllers-column`、`--metadata-column`、`--governance-column` アップストリーム
  オプションの列をエクスポートします。

スクリプト集約マニフェスト:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
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

説明 `--ndjson` 説明 `RegisterNameRequestV1` 単一行 JSON
ドキュメントの自動化リクエスト Torii
ストリームストリーム:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. 自動送信

### 3.1 Torii REST モード

`--submit-torii-url` ساتھ `--submit-token` یا `--submit-token-file` دیں تاکہ
マニフェスト ہر エントリ براہ راست Torii کو プッシュ ہو:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- ヘルパーリクエスト `POST /v1/sns/names` HTTP リクエスト
  エラー 中断 ہے۔応答ログのパスを NDJSON レコードに追加します
  ہوتے ہیں۔
- `--poll-status` 6 回の提出 ٩ے بعد `/v1/sns/names/{namespace}/{literal}` ٩و
  دوبارہ クエリ کرتا ہے (زیادہ سے زیادہ `--poll-attempts`、デフォルト 5) تاکہ レコード
  見える ہونے کی تصدیق ہو۔ `--suffix-map` (JSON と `suffix_id` の「サフィックス」値)
  マップ (マップ)) ツール `{label}.{suffix}` リテラルの派生 کر سکے۔
- 調整可能パラメータ: `--submit-timeout`、`--poll-attempts`、`--poll-interval`。

### 3.2 iroha CLIモード

マニフェスト エントリと CLI のバイナリ パスの説明:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- コントローラー `Account` エントリ (`controller_type.kind = "Account"`)
  CLI によるアカウント ベースのコントローラーの公開
- メタデータ ガバナンス BLOB リクエスト 一時ファイル 一時ファイル
  ہیں اور `iroha sns register --metadata-json ... --governance-json ...` کو پاس
  ٩ئے جاتے ہیں۔
- CLI stdout/stderr 終了コード ログ ہوتے ہیں؛ゼロ以外のコードは実行され、中止されます。

サブミッション モード アプリケーション アプリケーション レジストラ (Torii アプリケーション CLI) レジストラ
導入のクロスチェック、フォールバック、リハーサル、リハーサル

### 3.3 提出物の受領書

`--submission-log <path>` スクリプト NDJSON エントリに追加されるスクリプト:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```成功した Torii 応答 میں `NameRecordV1` یا `RegisterNameResponseV1` سے نکالے
構造化フィールド شامل ہوتے ہیں (مثال `record_status`、`record_pricing_class`、
`record_owner`、`record_expires_at_ms`、`registry_event_version`、`suffix_id`、
`label`) ダッシュボード ガバナンス レポート ログ 自由形式のテキスト
解析するログを作成し、マニフェストを作成し、レジストラ チケットを添付し、添付ファイルを作成します。
再現可能な証拠

## 4. ドキュメント ポータルのリリースの自動化

CI ポータル ジョブ `docs/portal/scripts/sns_bulk_release.sh` に電話する ہیں، جو
ヘルパー、ラップ、アーティファクト、`artifacts/sns/releases/<timestamp>/`、
ストアの価格:

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

スクリプト:

1. `registrations.manifest.json`、`registrations.ndjson` オリジナル
   CSV リリース ディレクトリ میں کاپی کرتا ہے۔
2. Torii اور/یا CLI کے ذریعے マニフェスト送信 کرتا ہے (構成設定) ، اور
   `submissions.log` میں اوپر والے 構造化された領収書 لکھتا ہے۔
3. `summary.json` は、リリースと説明を発行します (パス、Torii URL、
   CLI パス、タイムスタンプ) ポータル自動化バンドルとアーティファクト ストレージ
   アップロードする
4. `metrics.prom` بناتا ہے (`--metrics` کے ذریعے オーバーライド) ، جس میں Prometheus-
   フォーマットカウンター: リクエストの合計、サフィックスの分布、資産の合計、
   「提出結果」概要 JSON ファイル リンク ہے۔

ワークフロー リリース ディレクトリ アーティファクト アーカイブ アーカイブ リリース ディレクトリ
管理 ガバナンス 監査 管理

## 5. テレメトリーダッシュボード

`sns_bulk_release.sh` メトリクス ファイル シリーズが公開されています:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے Prometheus サイドカー میں feed کریں (مثال کے طور پر Promtail
バッチ インポータ (バッチ インポータ) レジストラ、スチュワード ガバナンス ピア バルク
進捗状況が整列しましたGrafana ボード
`dashboards/grafana/sns_bulk_release.json` وہی データ パネル میں دکھاتا ہے: サフィックスごと
支払額、提出成功/失敗率をカウントします。ボード `release`
フィルター処理 監査員処理 CSV 実行 ドリル処理

## 6. 検証と失敗モード

- **ラベルの正規化:** Python IDNA を入力、小文字、標準 v1
  文字フィルター 正規化 ہوتے ہیں۔無効なラベルのネットワーク呼び出し
  早く失敗してください ہوتے ہیں۔
- **数値ガードレール:** サフィックス ID、期間年 価格設定のヒント `u16` `u8`
  境界 کے اندر ہونے چاہئیں۔支払いフィールド 10 進数、16 進数、整数 `i64::MAX`
  受け入れます ہیں۔
- **メタデータ ガバナンス解析:** インライン JSON 解析 解析ファイル
  CSV の場所を参照します。 相対解決。非オブジェクトメタデータ
  検証エラー دیتا ہے۔
- **コントローラー:** セル `--default-controllers` 名誉 کرتے ہیں۔非所有者
  アクター、デリゲート、明示的コントローラー リスト、 (مثال `i105...;i105...`)۔

失敗コンテキスト行番号レポート ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`)。スクリプト検証エラー
`1` CSV パスがありません `2` 終了します ہے۔

## 7. 出所のテスト

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV 解析 - NDJSON
  排出量、ガバナンスの実施、CLI/Torii 提出パス、カバー、データ
- ヘルパー純粋な Python ہے (کوئی اضافی 依存関係 نہیں) اور جہاں `python3` دستیاب
  ہو وہاں چلتا ہے۔コミット履歴 CLI のメインリポジトリのトラック ہوتی ہے
  再現性の高さ

本番稼働の実行、生成されたマニフェスト、NDJSON バンドル、レジストラ チケット、
スチュワード Torii を添付し、正確なペイロードをリプレイします。
ありがとうございます