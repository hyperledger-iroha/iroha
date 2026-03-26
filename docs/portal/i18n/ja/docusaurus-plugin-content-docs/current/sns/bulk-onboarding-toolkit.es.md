---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
Refleja `docs/source/sns/bulk_onboarding_toolkit.md` パラ ケ ロス オペラドーレス エクステルノス ビーン
ラ ミスマ ギア SN-3b のクローン エル リポジトリ。
:::

# masivo SNS オンボーディングのツールキット (SN-3b)

**ロードマップの参照:** SN-3b「バルク オンボーディング ツール」  
**アーティファクト:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

レジストラは、事前準備を完了するためのメニューを拡大します。 `.sora` または `.nexus`
問題は解決策であり、解決策です。 Armar ペイロード JSON
マノ・オ・ボルバー、エジェクター、CLIなし、エスカラなし、SN-3bエントレガ・アン・ビルダー
CSV の確定情報 Norito 構造の準備 `RegisterNameRequestV1` パラ
Torii CLI から。エル・ヘルパー・バリダ・カダ・フィラ・デ・アンテマノ、エミテ・タント・アン・マニフィスト
アグレガド コモ JSON デリミタド ポル サルトス デ リネア オプショナル、プエデ エンビア ロス
ペイロードは、聴覚的な構造を自動的に記録します。

## 1. エスケマ CSV

El parser requiere la siguiente fila de encabezado (柔軟な順序で):

|コラムナ |レケリド |説明 |
|----------|-----------|---------------|
| `label` |シ | Etiqueta solicitada (安全性を保証する/マイナス; ラ・ヘルラミエンタ・ノーマルリザ・セグン・ノルムv1 y UTS-46)。 |
| `suffix_id` |シ |識別番号 (10 進数または `0x` 16 進数)。 |
| `owner` |シ | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` |シ |エンテロ `1..=255`。 |
| `payment_asset_id` |シ |決済活動 (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `payment_gross` / `payment_net` |シ | Enteros sin signo que は、unidades nativas del activo を表します。 |
| `settlement_tx` |シ | Valor JSON またはカデナ リテラルは、トランザクション デパゴまたはハッシュを記述します。 |
| `payment_payer` |シ | AccountId que autorizo​​ el pago。 |
| `payment_signature` |シ | JSON またはカデナ リテラル コン ラ プルエバ デ ファーム デ スチュワード または テソレリア。 |
| `controllers` |オプション |制御装置の指示を確認してください。欠陥 `[owner]` が表示されます。 |
| `metadata` |オプション | JSON インライン o `@path/to/file.json` は、リゾルバー、レジストリ TXT などのヒントを証明します。`{}` の欠陥です。 |
| `governance` |オプション | JSON インライン o `@path` は `GovernanceHookV1` に対応します。 `--require-governance` エキシージ エスタ コラムナ。 |

`@` の外部プレフィジャンド エル ヴァロールを参照してください。
相対的なアーカイブ CSV が再表示されます。

## 2. エジェクターエルヘルパー

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

オピオネス・クラーベ：

- `--require-governance` レチャザ フィラス シン アン フック デ ゴベルナンザ (ユーティリティ パラ)
  サブバス プレミアムまたはアシニャシオネス リザーダ）。
- `--default-controllers {owner,none}` コントローラーを使用するかどうかを決定します
  ブエルフェン・ア・ラクエンタのオーナー。
- `--controllers-column`、`--metadata-column`、y `--governance-column` 許可
  renombrar columnas opcionales cuando se trabaja con が上流に輸出されています。

終了スクリプトでは、集合体をマニフェストするように記述します。

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

`--ndjson` は、`RegisterNameRequestV1` を参照してください。
ドキュメント JSON デ ウナ ソラ ライン パラケ ラス オートマティザシオネス プエダン トランスミッター
Torii へのお願い:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envios 自動化

### 3.1 Modo Torii REST

`--submit-torii-url` マス `--submit-token` または `--submit-token-file` パラ
Torii を指示するための命令を入力します:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- ヘルパーは、中絶前に `POST /v1/sns/names` の要求を出します
  プライマー エラー HTTP。登録されたログを参照してください。
  NDJSON。
- `--poll-status` コンサルタントを参照 `/v1/sns/names/{namespace}/{literal}` はデプスをサポートします
  cada envio (hasta `--poll-attempts`、デフォルト 5) 登録情報の確認
  見えます。 Proporcione `--suffix-map` (`suffix_id` 値「サフィックス」の JSON)
  パラ ケ ラ ヘルラミエンタは、リテラル `{label}.{suffix}` al hacer ポーリングを導出します。
- 調整: `--submit-timeout`、`--poll-attempts`、y `--poll-interval`。

### 3.2 もどいろは CLI

CLI の情報をマニフェストし、ビナリオの情報を確認します。

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- ロス コントローラー デベン サー エントラダス `Account` (`controller_type.kind = "Account"`)
  CLI の実際のソロのエクスポネ コントローラーを使用できるようになります。
- 一時的なアーカイブに記載されているメタデータとガバナンスのブロブが失われる
  `iroha sns register --metadata-json ... --governance-json ...` をお願いいたします。
- CLI マス・ロス・コディゴス・デ・サリダ・レジストランの標準出力と標準エラー。ロス・コディゴス
  排出の中止はありません。Ambos modos de envio pueden ejecutarse juntos (Torii y CLI) para verificar
レジストラまたはエンサヤのフォールバックを削除します。

### 3.3 環境のレシボス

Cuando se proporciona `--submission-log <path>`、スクリプト アネクサ エントラダス NDJSON クエリ
キャプチャラン:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Torii からの出口の回復には、外部構造の構築が含まれます
`NameRecordV1` または `RegisterNameResponseV1` (例 `record_status`、
`record_pricing_class`、`record_owner`、`record_expires_at_ms`、
`registry_event_version`、`suffix_id`、`label`) ダッシュボードとレポートのパラグラフ
ゴベルナンザ・プエダン・パーサー・エル・ログ・シン・インスペクション・テキスト・リブレ。アジャンテエステログa
チケットの登録者は、再現可能な証拠をマニフェストします。

## 4. ポータルのリリースの自動化

Los trabajos de CI y del portal llaman a `docs/portal/scripts/sns_bulk_release.sh`、
QUE envuelve el helper y Guarda artefactos bajo `artifacts/sns/releases/<timestamp>/`:

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

El スクリプト:

1. Construye `registrations.manifest.json`、`registrations.ndjson`、コピーピアエル
   CSV オリジナルのディレクトリをリリースします。
2. Envia は、Torii y/o la CLI (設定を変更)、エスクリビエンドをマニフェストします。
   `submissions.log` コンロスレシボス構造体。
3. `summary.json` リリースの説明を発行します (rutas、URL Torii、ruta CLI、
   タイムスタンプ）パラ ケ ラ オートマティザシオン デル ポータル プエダ カルガー エル バンドル
   アルマセナミエント デ アーティファクト。
4. `metrics.prom` (`--metrics` を介してオーバーライド) を生成します。
   en formato Prometheus para total de solicitudes, distribution de sufijos,
   資産と環境の結果の合計。エステの再開のための JSON
   アーカイブ。

ソロの作品をリリースするためのアーキバンと監督のワークフローをシンプルにし、
que ahora contiene todo lo que la gobernanza necesita para audiotoria。

## 5. テレメトリアとダッシュボード

`sns_bulk_release.sh` の記録に関する一般的な記録
シリーズ:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` と Prometheus のサイドカー (Promtail 経由でのアクセス)
輸入業者バッチ）パラ・マンテナー・レジストラ、スチュワード、パレス・デ・ゴベルナンザ
アリナドス・ソブレ・エル・プログレソ・マシボ。エル タベロ Grafana
`dashboards/grafana/sns_bulk_release.json` ミスモスデータを視覚化
完璧なコンテオス、出口/外の環境の比率をボリューム。エル
Tablero filtra por `release` para que los Auditores puedan entrar en una sola
CSVのコリーダ。

## 6. 失敗の検証

- **ラベルの正規化:** Python IDNA マスでの正規化の結果
  小文字の y filtros de caracteres Norm v1。インバウンド・ファラン・ラピド・アンテスにラベルを付ける
  デ・クアルキエ・ラマダ・デ・レッド。
- **Guardrails numericos:** サフィックス ID、期間年数、Y 価格設定のヒント デベン ケア
  デントロ デ リミテス `u16` と `u8`。ロス・カンポス・デ・パゴ・アセプタン・エンテロス・デシマル・オー
  ヘックスハスタ`i64::MAX`。
- **メタデータの解析とガバナンス:** JSON インライン分析を直接実行します。ラス
  CSV に関する相対的なアーカイブを参照します。メタデータ
  海のオブジェクトは検証のエラーを生成しません。
- **コントローラー:** セルダ エン ブランコ レスペタン `--default-controllers`。プロポルシオーネ
  コントローラーの明示的なリスト (`i105...;i105...` から) の委任
  俳優に所有者はいない。

状況に応じた情報の損失 (レポートの内容)
`error: row 12 term_years must be between 1 and 255`)。 El script sale con codigo
`1` 検証エラー `2` CSV のエラー。

## 7. 手順のテスト

- CSV を解析する `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` キューブ、
  エミッション NDJSON、ガバナンスの施行、CLI の環境保護、Torii。
- El helper es Python puro (sin dependencias adicionales) y corre en cualquier
  lugar donde `python3` este disponible。ラストレア ジュントの歴史
  CLI のリポジトリの主要な再現性。

生産の補助、生成用バンドルのマニフェスト、NDJSON AL
チケット デル レジストラ パラ ケ ロス スチュワード プエダン レプロデュシル ロス ペイロード エクストラクト
Torii を使用してください。