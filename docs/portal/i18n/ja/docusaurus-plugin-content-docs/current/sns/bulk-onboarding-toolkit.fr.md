---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
ページを参照して `docs/source/sns/bulk_onboarding_toolkit.md` ファイルを参照してください
SN-3b は保管場所を持たずに、外部からのミーム ガイダンスを操作します。
:::

# 山塊 SNS を搭載したツールキット (SN-3b)

**参考ロードマップ:** SN-3b「バルク オンボーディング ツール」  
**アーティファクト:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

登録官が登録を準備するための準備 `.sora` ou
`.nexus` 政府と和解の承認に関するミーム。
メインのペイロード JSON と CLI のリランサーの作成、SN-3b のスケールパス
ビルダーが CSV と Norito を決定して構造を準備します
CLI から `RegisterNameRequestV1` Torii を注ぎます。 L'helper valide Chaque ligne en
マニフェストの合意と新しい JSON の区切りを表示します
オプション、およびペイロードの自動処理を実行します。
登録者による構造の監査。

## 1. スキーマ CSV

Le parseur exige la ligne d'en-tete suivante (柔軟な選択):

|コロンヌ |必須 |説明 |
|----------|----------|---------------|
| `label` |おうい |リベル要求 (casse mixte acceptee; l'outil 正規化 selon Norm v1 et UTS-46)。 |
| `suffix_id` |おうい |接尾辞の識別子 (10 進数または `0x` 16 進数)。 |
| `owner` |おうい | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` |おうい |エンティア `1..=255`。 |
| `payment_asset_id` |おうい |和解行為 (`61CtjvNd9T3THAR65GsMVHr82Bjc` など)。 |
| `payment_gross` / `payment_net` |おうい | Entiers nonsignes 代表者 des 団結ネイティブ de l'actif。 |
| `settlement_tx` |おうい | Valeur JSON は、トランザクションの支払いとハッシュのチェーンリッターの決定を行います。 |
| `payment_payer` |おうい | AccountId は支払いの自動化に使用されます。 |
| `payment_signature` |おうい | JSON は、管理者と管理者の署名を含む、一連のコンテンツです。 |
| `controllers` |オプション |ポイント仮想とコンピューティング コントローラのアドレスを個別にリストします。デフォルトの `[owner]` は省略されています。 |
| `metadata` |オプション | JSON インライン ou `@path/to/file.json` リゾルバーのヒント、TXT 登録など。デフォルトの `{}`。 |
| `governance` |オプション | JSON インライン ou `@path` ポイントと `GovernanceHookV1` 。 `--require-governance` セッテコロンを課します。 |

Toute Colonne peut Referencer un fichier externe en prefixant la valeur de cellule
パー `@`。相対的な CSV を表示します。

## 2. ヘルパーの死刑執行人

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

オプションの設定:

- `--require-governance` フック・ド・ガバナンスなしのリジェット・レ・リーニュ（使用料の支払い）
  レ サンシェール プレミアム アウト レ アフェクション リザーブ)。
- `--default-controllers {owner,none}` セル コントローラーのビデオを決定します
  retombent sur le compte オーナー。
- `--controllers-column`、`--metadata-column`、および `--governance-column` 永続
  de renommer les Colonnes optionnelles lors d'exports amont。

成功した場合のマニフェスト合意のスクリプト:

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

Si `--ndjson` est fourni, chaque `RegisterNameRequestV1` est aussi ecrit comme un
自動化されたストリーマー ファイルに関するドキュメント JSON
Torii に対する指示を要求します:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. 送金の自動化

### 3.1 モード Torii REST

`--submit-torii-url` と `--submit-token` または `--submit-token-file` を指定します。
Pousser チャック メイン デュ マニフェスト ディレクション 対 Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- `POST /v1/sns/names` のリクエストとプレミアのサポートを提供します
  HTTP でエラーが発生しました。 NDJSON を登録するための応答を返します。
- `--poll-status` 再質問 `/v1/sns/names/{namespace}/{literal}` チャク前
  soumission (jusqu'a `--poll-attempts`、デフォルト 5) 確認者キューを注ぐ
  登録が表示されます。フルニセ `--suffix-map` (`suffix_id` の JSON)
  vers des valeurs "接尾語") pour que l'outilderive les litteraux
  `{label}.{suffix}` ポーリングを実行します。
- 調整可能: `--submit-timeout`、`--poll-attempts`、および `--poll-interval`。

### 3.2 モード iroha CLI

フェール パサー チャク メイン デュ マニフェスト パー ラ CLI、フォーニセ ル シュミン デュを注ぐ
ビネール:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- メインのコントローラー `Account` (`controller_type.kind = "Account"`)
  車の CLI は、コンピューティング上のコントローラー ベースの一意性を公開します。
- ブロックのメタデータとガバナンスに関するセキュリティ ダン デフィシエ テンポレアの説明
  `iroha sns register --metadata-json ... --governance-json ...` を要求して送信します。
- CLI の標準出力と標準エラーを記録し、出撃時のコードを記録します。
  ファイルはゼロ以外の一時的な実行をコードします。Les deux modes de soumission peuvent fonctionner ensemble (Torii et CLI) を注ぐ
レジストラによるデプロイメントとフォールバックのレピーター。

### 3.3 任務の遂行

Quand `--submission-log <path>` 4 つ目、メインのスクリプト NDJSON
捕虜:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Les reponses Torii reussies incluent des Champs 構造の特典
`NameRecordV1` または `RegisterNameResponseV1` (例 `record_status`、
`record_pricing_class`、`record_owner`、`record_expires_at_ms`、
`registry_event_version`、`suffix_id`、`label`) ダッシュボードなどについて
テキストリブレの検査官なしで、ログを管理するパーサーのレポートを作成します。
ジョイネス CE ログ補助チケット登録者は、証拠を提出するマニフェストを記録します
再現可能。

## 4. ポータルのリリースの自動化

職務CIおよびポータルの控訴人`docs/portal/scripts/sns_bulk_release.sh`、qui
ヘルパーとストックのアーティファクトをカプセル化します
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

スクリプト:

1. `registrations.manifest.json`、`registrations.ndjson` などを作成し、ファイルをコピーします
   CSV オリジナルのレパートリーをリリース。
2. Torii et/ou la CLI (quand configure)、暗号化経由のファイルマニフェスト
   `submissions.log` avec les recus 構造 ci-dessus。
3. Emet `summary.json` derivant la release (chemins、URL Torii、chemin CLI、
   タイムスタンプ) アップローダーのバンドル版の自動化をサポートします
   芸術品の在庫。
4. 製品 `metrics.prom` (`--metrics` 経由で上書き) コンテナント
   au 形式 Prometheus リクエストの合計数、サフィックスの分布、
   資産と使命の結果。 JSON 再開ポワントバージョン
   CEフィシエ。

ワークフローは、アーティファクトをリリースするレパートリーを単純化してアーカイブし、
監査を行う必要はなく、管理を強化する必要があります。

## 5. テレメトリとダッシュボード

`sns_bulk_release.sh` による指標のジャンルの公開シリーズ
スイバンテス:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` および votre サイドカー Prometheus (Promtail ou による例)
インポートバッチ）アライナレジストラ、スチュワード、および政府当局のペアを注ぐ
一斉に前進。ル タブロー Grafana
`dashboards/grafana/sns_bulk_release.json` ミームの平均値を視覚化する
接尾語のコンプ、量と比率のパノラマを注ぎます
reussite/echec des somissions。 `release` によるフィルター フィルター
Auditeurs puissent se concentrer sur une seule execution CSV。

## 6. 検証と実装方法

- **ラベルの正規化:** Python IDNA plus の主成分を正規化する
  小文字と文字フィルターの標準 v1。ラベルはエコーエントバイトを無効にします
  アヴァン・タウト・アペル・レゾー。
- **派手な数字:** サフィックス ID、期間年数、価格設定のヒントなど
  レスター・ダン・レ・ボーン `u16` と `u8`。 Les champs de paiement acceptent des
  10 進数または 16 進数 `i64::MAX`。
- **メタデータの解析とガバナンス:** ファイルの JSON インライン est 解析ディレクション。レ
  CSV の配置に関連する解決策の決定を参照します。
  オブジェクトではないメタデータが検証に失敗する可能性があります。
- **コントローラー:** セルセルは、関連する `--default-controllers` を参照します。フルニセズ
  明示的なリスト (`i105...;i105...` など) は、完全に削除されたものです。
  俳優は所有者ではありません。

Les echecs Sont signales avec des numeros de ligne contextuels (par example)
`error: row 12 term_years must be between 1 and 255`)。スクリプトソートアベックファイル
コード `1` 検証エラーと `2` lorsque le chemin CSV manque。

## 7. テストと来歴

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` クーブル ファイルの CSV 解析、
  NDJSON の排出、ガバナンスの執行、および管理 CLI または Torii。
- Python のヘルパー (聴覚依存性追加) とツールの一部
  ou `python3` est 責任があります。歴史的な歴史
  CLI はデポのプリンシパルであり、再生産を行っています。

プロダクションの実行、マニフェストジェネレーションおよびバンドル NDJSON au の実行を開始します。
レジストラのチケット、スチュワードのペイロードの正確な内容
そうみは Torii です。