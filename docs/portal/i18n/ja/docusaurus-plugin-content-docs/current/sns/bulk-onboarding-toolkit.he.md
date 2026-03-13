---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c004a9ecd432e0ba9c996a7b638351a1c847719a2bfdd7f44b8f0b27bad4c894
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: bulk-onboarding-toolkit
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sns/bulk_onboarding_toolkit.md` を反映しており、
外部オペレーターがリポジトリをクローンせずに同じ SN-3b ガイダンスを
参照できるようにします。
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**ロードマップ参照:** SN-3b "Bulk onboarding tooling"  
**成果物:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

大規模な registrar は、同じガバナンス承認と settlement rails を使って
`.sora` や `.nexus` の登録を数百件まとめて事前準備することがよくあります。
JSON payload を手作業で作成したり CLI を再実行したりするのは拡張性が
ないため、SN-3b は CSV から Norito への決定的 builder を提供し、Torii
または CLI 向けの `RegisterNameRequestV1` 構造を準備します。ヘルパーは
各行を事前に検証し、集約 manifest とオプションの改行区切り JSON を
出力し、監査向けの構造化レシートを記録しながら自動送信できます。

## 1. CSV スキーマ

パーサーは以下のヘッダ行を要求します (順序は任意):

| カラム | 必須 | 説明 |
|--------|------|------|
| `label` | Yes | 申請ラベル (大小文字混在可; ツールは Norm v1 と UTS-46 で正規化)。 |
| `suffix_id` | Yes | 数値のサフィックス ID (10進または `0x` hex)。 |
| `owner` | Yes | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Yes | 整数 `1..=255`. |
| `payment_asset_id` | Yes | settlement アセット (例: `xor#sora`). |
| `payment_gross` / `payment_net` | Yes | アセット単位を表す符号なし整数。 |
| `settlement_tx` | Yes | 支払いトランザクションまたは hash を表す JSON 値または文字列。 |
| `payment_payer` | Yes | 支払いを承認した AccountId。 |
| `payment_signature` | Yes | steward または treasury の署名証明を含む JSON/文字列。 |
| `controllers` | Optional | controller アカウントアドレスのセミコロン/カンマ区切りリスト。省略時は `[owner]`。 |
| `metadata` | Optional | resolver ヒント、TXT レコード等を含む inline JSON か `@path/to/file.json`。既定は `{}`。 |
| `governance` | Optional | `GovernanceHookV1` を指す inline JSON か `@path`。`--require-governance` で必須化。 |

任意の列はセル値に `@` を付けることで外部ファイルを参照できます。
パスは CSV ファイルからの相対になります。

## 2. ヘルパーの実行

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

主なオプション:

- `--require-governance` はガバナンスフックがない行を拒否します
  (premium オークションや予約割当てで有用)。
- `--default-controllers {owner,none}` は空の controllers セルを owner に
  フォールバックするかを決定します。
- `--controllers-column`, `--metadata-column`, `--governance-column` は
  上流エクスポートの任意列名を変更できます。

成功時にスクリプトは集約 manifest を出力します:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
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

`--ndjson` を指定すると、各 `RegisterNameRequestV1` を 1 行 JSON としても
出力し、Torii へ直接ストリームできます:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. 自動送信

### 3.1 Torii REST モード

`--submit-torii-url` に加えて `--submit-token` または `--submit-token-file`
を指定すると、manifest の各エントリを Torii に直接送信できます:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- ヘルパーは各リクエストに対して `POST /v2/sns/registrations` を送信し、
  最初の HTTP エラーで停止します。レスポンスは NDJSON としてログに追記。
- `--poll-status` は送信後に `/v2/sns/registrations/{selector}` を再照会
  (最大 `--poll-attempts`, 既定 5) して可視性を確認します。`suffix_id` から
  `"suffix"` への JSON を `--suffix-map` で渡すと `{label}.{suffix}` を
  生成してポーリングできます。
- 調整項目: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 iroha CLI モード

manifest の各エントリを CLI 経由で送るにはバイナリパスを指定します:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- controllers は `Account` エントリ (`controller_type.kind = "Account"`) 必須。
  CLI は現在アカウントベースのみ対応しています。
- metadata と governance の blob は各リクエストごとに一時ファイルへ書き出し、
  `iroha sns register --metadata-json ... --governance-json ...` に渡します。
- CLI の stdout/stderr と終了コードを記録し、非ゼロコードで中断します。

Torii と CLI の両方を併用して registrar の展開をクロスチェックしたり、
フォールバックのリハーサルに使うこともできます。

### 3.3 送信レシート

`--submission-log <path>` を指定すると、スクリプトは NDJSON エントリを
追記します:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

成功した Torii 応答には `NameRecordV1` または `RegisterNameResponseV1` から
抽出された構造化フィールド (例: `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) が含まれ、ダッシュボードやガバナンス報告がフリーテキストを
解析せずにログを利用できます。manifest と一緒にこのログを registrar
チケットへ添付し、再現可能な証跡としてください。

## 4. ポータルのリリース自動化

CI とポータルのジョブは `docs/portal/scripts/sns_bulk_release.sh` を呼び、
ヘルパーをラップして `artifacts/sns/releases/<timestamp>/` に成果物を保存します:

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

このスクリプトは:

1. `registrations.manifest.json` と `registrations.ndjson` を生成し、元の CSV を
   リリースディレクトリへコピーします。
2. Torii および/または CLI で manifest を送信し、`submissions.log` に
   構造化レシートを記録します。
3. `summary.json` を出力し、リリース情報 (パス、Torii URL、CLI パス、
   timestamp) を記載してポータル自動化がアーティファクト格納へアップ
   ロードできるようにします。
4. `metrics.prom` (override は `--metrics`) を生成し、総リクエスト数、
   サフィックス分布、資産合計、送信結果の Prometheus 互換カウンタを
   含めます。summary JSON からこのファイルにリンクされます。

ワークフローはリリースディレクトリを単一アーティファクトとして
アーカイブするだけで、監査に必要なものが全て含まれます。

## 5. テレメトリとダッシュボード

`sns_bulk_release.sh` が生成するメトリクスファイルは以下を公開します:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` を Prometheus sidecar (例: Promtail やバッチインポータ) に
投入し、registrar、steward、ガバナンス各者の進捗を揃えます。Grafana
ボード `dashboards/grafana/sns_bulk_release.json` は同じデータを、サフィックス
ごとの件数、支払いボリューム、送信成功/失敗比率のパネルで可視化します。
ボードは `release` でフィルタでき、監査者は単一 CSV 実行にドリルダウン
できます。

## 6. 検証と失敗モード

- **ラベル正規化:** 入力は Python IDNA と lowercase、Norm v1 文字フィルタで
  正規化されます。無効なラベルはネットワーク前に即失敗。
- **数値ガードレール:** suffix ids、term years、pricing hints は `u16` と `u8`
  の範囲内である必要があります。支払い欄は 10進/hex の整数で `i64::MAX`
  まで受け付けます。
- **metadata/governance のパース:** inline JSON は直接解析し、ファイル参照は
  CSV 位置から相対解決。オブジェクトでない metadata は検証エラーになります。
- **Controllers:** 空セルは `--default-controllers` に従います。非 owner へ委任
  する場合は `i105...;i105...` など明示的なリストを指定します。

失敗時は行番号付きで報告されます (例: `error: row 12 term_years must be between 1 and 255`).
スクリプトは検証エラーで `1`、CSV パス欠落で `2` を返します。

## 7. テストと来歴

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` は CSV 解析、NDJSON
  出力、ガバナンス必須化、CLI/Torii 送信経路をカバーします。
- ヘルパーは純粋な Python (依存追加なし) で、`python3` があればどこでも
  実行できます。コミット履歴は CLI と並んでメインリポジトリに保持され、
  再現性が確保されます。

本番運用では、生成された manifest と NDJSON バンドルを registrar チケットに
添付し、steward が Torii に送信された payloads を再現できるようにしてください。
