---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` パラ ケ オペラドール エクステルノス
vejam はメスマ オリエンタカオ SN-3b の sem クローンまたはリポジトリです。
:::

# オンボーディング エム マッサ SNS ツールキット (SN-3b)

**ロードマップの参照:** SN-3b「バルク オンボーディング ツール」  
**アルテファトス:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

レジストラは頻繁に登録を準備しており、`.sora` ou
`.nexus` は、決済の統治とレールの管理を行っています。モンタル
ペイロード JSON マニュアルを使用して CLI を再実行し、エスカラ、SN-3B を実行します
Norito による CSV の確定ビルダーの準備
CLI の `RegisterNameRequestV1` Torii。おお、ヘルパー バリダ カダ リンハ
com antecedencia、エミテタントウムマニフェストアグレガドクアントJSONデリミタドポル
ケブラ デ リンハ オプショナル、電子レンジ環境 OS ペイロードの自動処理
聴覚の登録者。

## 1. エスケマ CSV

O parser exige a seguinte linha de cabecalho (a ordem e flexivel):

|コルナ |オブリガトリオ |説明 |
|----------|---------------|----------|
| `label` |シム | solicitada にラベルを付けます (大文字と小文字が混在した aceita、Norm v1 e UTS-46 に準拠した標準規格)。 |
| `suffix_id` |シム |接尾辞の識別子 (10 進数または `0x` 16 進数)。 |
| `owner` |シム | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` |シム |インテイロ `1..=255`。 |
| `payment_asset_id` |シム |決済処理 (`xor#sora` の例)。 |
| `payment_gross` / `payment_net` |シム | Interos sem sinal representando unidades nativas do ativo. |
| `settlement_tx` |シム | Valor JSON または文字列リテラルの説明、トランザクションのパガメント、ハッシュ。 |
| `payment_payer` |シム | AccountId は自動更新されます。 |
| `payment_signature` |シム | JSON と文字列リテラルの内容は、スチュワードとテスラリアのプロバ デ アシナチュアを実行します。 |
| `controllers` |オプション |コントローラーを制御するために、コンピューターとコンピューターを分離してください。パドラオ `[owner]` クアンド オミティド。 |
| `metadata` |オプション | JSON インライン ou `@path/to/file.json` は、リゾルバー、レジストリ TXT などのヒントを示します。Padrao `{}`。 |
| `governance` |オプション | JSON インライン ou `@path` は `GovernanceHookV1` に対応します。 `--require-governance` エキシージ エスタ コルナ。 |

Qualquer の情報は、`@` の外部プレフィックスと valor da celula を参照します。
CSV を参照して解決策を見つけてください。

## 2. 執行者またはヘルパー

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

主要な運用:

- `--require-governance` レジェイタ・リンハス・セム・ウム・フック・デ・ガバナンカ (ユーティリティパラ
  leiloes プレミアムまたは atribuicoes reservadas)。
- `--default-controllers {owner,none}` コントローラのセルラスを決定します
  ボルタムパラコンタオーナー。
- `--controllers-column`、`--metadata-column`、`--governance-column` 許可
  Renomear Colunas opcionais ao trabalhar com は上流にエクスポートします。

スクリプトを実行して、マニフェストを集約してください:

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

Se `--ndjson` fornecido、cada `RegisterNameRequestV1` tambem e escrito como
um documento JSON de linha unica para que automacoes possam transfer request
指示 ao Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. 自動化を実行する

### 3.1 Modo Torii REST

特定の `--submit-torii-url` は `--submit-token` または `--submit-token-file` です
パラ環境は、マニフェストを実行します Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- O ヘルパーは、`POST /v2/sns/registrations` 要求と中止のプライムロを発行します
  HTTPエラーです。 as respostas sao anexadas ao log como registros NDJSON。
- `--poll-status` reconsulta `/v2/sns/registrations/{selector}` apos cada envio
  (ate `--poll-attempts`、デフォルトは 5) 登録情報を確認します。
  Forneca `--suffix-map` (`suffix_id` パラメータ「サフィックス」の JSON) パラメータ
  フェラメンタは、`{label}.{suffix}` パラオポーリングを派生します。
- 調整: `--submit-timeout`、`--poll-attempts`、e `--poll-interval`。

### 3.2 もどいろは CLI

マニフェストペラ CLI を実行し、ビナリオを実行するために必要な情報:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- コントローラは、`Account` (`controller_type.kind = "Account"`) のエントリを開発します。
  CLI を実際に取得して、コントローラーのベースを公開します。
- リクエストごとにメタデータとガバナンスを一時的に保存する BLOB
  `iroha sns register --metadata-json ... --governance-json ...` のエンカミンハドス。
- CLI の標準出力と標準エラー出力、最新のコード、登録情報。コディゴス・ナオ
  ゼロは実行を中止します。

アンボス モードのサブミッション ポデム ロダル ジュントス (Torii e CLI) パラ チェック
デプロイメントでは、レジストラまたはフォールバックが実行されます。

### 3.3 服従の義務Quando `--submission-log <path>` e fornecido、o script anexa entradas NDJSON que
キャプチャーラム:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Respostas Torii bem-sucedidas incluem Campos estruturados extraidos デ
`NameRecordV1` または `RegisterNameResponseV1` (例 `record_status`、
`record_pricing_class`、`record_owner`、`record_expires_at_ms`、
`registry_event_version`、`suffix_id`、`label`) ダッシュボードと関連性の比較
デ・ガバナンカ・ポッサム・パーサーとログセキュリティ検査テキストライブラリ。アネックスエステログ
チケット登録者による複製のマニフェストとして。

## 4. ポータルの自動リリース

Jobs de CI e do portal Chamam `docs/portal/scripts/sns_bulk_release.sh`、que
カプセル、ヘルパー、アルマゼナの芸術品のすすり泣き
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

O スクリプト:

1. コンストロイ `registrations.manifest.json`、`registrations.ndjson`、電子コピーまたは CSV
   オリジナルパラオディレトリオのリリース。
2. マニフェスト Torii を CLI (Quando configurado) でサブメタ化し、グラバンド
   `submissions.log` com os estruturados acima をご覧ください。
3. `summary.json` 説明をリリース (caminhos、URL do Torii、caminho da) を発行します。
   CLI、タイムスタンプ) ポータルの環境設定を自動で実行し、バンドルパラメータを設定します。
   保管品の保存。
4. Produz `metrics.prom` (`--metrics` 経由でオーバーライド) contendo contadores compativeis
   com Prometheus リクエストの合計、サフィックスの配布、資産の合計
   提出された結果。 O JSON デ レスモ アポンタ パラ エステ アルキーボ。

OS のワークフローは、アーキバムとリリースの共同作業を簡素化します。
公聴会のような統治施設を訪問してください。

## 5. Telemetria e ダッシュボード

`sns_bulk_release.sh` の統計情報をセギンテス シリーズとして公開します:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` は Prometheus のサイドカーを使用しません (Promtail 経由の例)
um importadorバッチ）パラマンターレジストラ、スチュワード、パレスデガバナンカ
アリーニャドス ソブレ オ プログレソ エム マッサ。オークアドロ Grafana
`dashboards/grafana/sns_bulk_release.json` 視覚化 OS メスモス ダドス COM 痛み
後続感染症、スセッソ/ファルハデの比率のパガメントのボリューム
提出します。 O Quadro filtra por `release` para que Auditores possam focar em uma
CSV を実行します。

## 6. バリダカオとモドス・デ・ファルハ

- **ラベルの登録:** Python IDNA の標準的なアクセス許可
  小文字の e filtros de caracteres Norm v1。ラベル 無効なファルハム ラピド
  アンテ・デ・クアルケール・チャマダ・デ・レデ。
- **ガードレールの数値:** サフィックス ID、期間年数、価格設定のヒントの開発
  デントロ ドスは `u16` と `u8` を制限します。カンポス デ パガメント アセイタム インテイロス デシマイス
  16 進数は `i64::MAX` を食べました。
- **メタデータの解析とガバナンス:** JSON インラインでの解析。
  CSV のローカル情報を参照して解決策を参照します。メタデータ
  正しいオブジェクトを作成し、検証します。
- **コントローラー:** セルラス エム ブランコ レスペイタム `--default-controllers`。フォルネカ
  listas明示的 (例`i105...;i105...`) 所有者パラアトレスのデレガー。

Falhas sao reportadas com numeros de linha contextuais (por exemplo)
`error: row 12 term_years must be between 1 and 255`)。 O script sai com codigo
`1` 検証エラーが発生しました `2` CSV を確認しました。

## 7. 精巣と手続き

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cobre 解析 CSV、
  NDJSON のエミッション、ガバナンスの執行と命令の提出、CLI ou Torii。
- O ヘルパーと Python puro (sem dependencias adicionais) e Roda em qualquer lugar
  オンデ `python3` エスティバー ディスポニベル。歴史的なコミットメントと最後の瞬間
  CLI には再現性のあるリポジトリ プリンシパルはありません。

生産物、マニフェスト ジェラド、バンドル NDJSON 青チケットの付録を実行します。
レジストラパラケスチュワードポッサム再現OSペイロードexatos que foram
サブメティドス ao Torii。