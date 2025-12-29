<!-- Japanese translation of docs/portal/docs/sns/onboarding-kit.md -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
translation_last_reviewed: 2025-02-18
translator: manual
---

# SNS メトリクスとオンボーディングキット

ロードマップ項目 **SN-8** には次の 2 つの約束が含まれます。

1. `.sora`、`.nexus`、`.dao` の登録件数・更新件数・ARPU・係争・凍結ウィンドウを公開するダッシュボードを提供すること。
2. どのサフィックスでも稼働前に DNS・料金・API を一貫して配線できるよう、レジストラ／スチュワード向けのオンボーディングキットを提供すること。

このページはソース版
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
をミラーしており、外部レビュアーも同じ手順を辿れます。

## 1. メトリクスバンドル

### Grafana ダッシュボードとポータル埋め込み

- `dashboards/grafana/sns_suffix_analytics.json`
  を Grafana（もしくは別のアナリティクス基盤）へ標準 API でインポートします。

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- 同じ JSON がこのポータルページの iframe（**SNS KPI Dashboard** セクション）も駆動します。
  ダッシュボードを更新したら `docs/portal` 配下で
  `npm run build && npm run serve-verified-preview`
  を実行し、Grafana 本体と埋め込みの両方が同期していることを確認してください。

### パネルと証跡

| パネル | 参照メトリクス | ガバナンス証跡 |
|--------|----------------|----------------|
| 登録と更新 | `sns_registrar_status_total`（success + renewal resolver ラベル） | サフィックス別スループットと SLA 監視 |
| ARPU / ネットユニット | `sns_bulk_release_payment_net_units`、`sns_bulk_release_payment_gross_units` | レジストラマニフェストと収益を財務が突合 |
| 係争と凍結 | `guardian_freeze_active`、`sns_dispute_outcome_total`、`sns_governance_activation_total` | 開いている凍結、仲裁ペース、ガーディアン負荷 |
| SLA / エラー率 | `torii_request_duration_seconds`、`sns_registrar_status_total{status="error"}` | 顧客影響前に API 回帰を検出 |
| バルクマニフェストトラッカー | `sns_bulk_release_manifest_total`、`manifest_id` ラベル付き支払いメトリクス | CSV ドロップと決済チケットを関連付け |

月次 KPI レビュー時には Grafana（もしくは埋め込み iframe）から PDF / CSV をエクスポートし、
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`
にある該当付属書へ添付します。スチュワードはエクスポートしたバンドルの SHA-256 を
`docs/source/sns/reports/`（例: `steward_scorecard_2026q1.md`）にも記録し、監査時に証跡を再現できるようにします。

### 付属書オートメーション

レビュアーに一貫したダイジェストを渡せるよう、ダッシュボードエクスポートから直接付属書を生成します。

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- ヘルパーはエクスポートのハッシュ、UID、タグ、パネル数を収集し、
  `docs/source/sns/reports/.<suffix>/<cycle>.md`
  に Markdown 付属書を書き出します（コミット済みの `.sora/2026-03` サンプルを参照）。
- `--dashboard-artifact` はエクスポートを
  `artifacts/sns/regulatory/<suffix>/<cycle>/`
  へコピーし、付属書が正規証跡パスを参照できるようにします。
  外部アーカイブを参照する必要がある場合のみ `--dashboard-label`
  を使って任意パスを記載してください。
- `--regulatory-entry` で指定した規制メモには `KPI Dashboard Annex`
  ブロックが挿入（または置換）され、付属書パス、ダッシュボードアーティファクト、ダイジェスト、タイムスタンプが記録されます。
- `--portal-entry` を指定すると `docs/portal/docs/sns/regulatory/*.md`
  のポータル写しにも同じブロックが入るため、レビュー時に手動で差分を追う必要がなくなります。
- `--regulatory-entry` / `--portal-entry` を省略した場合は、生成したファイルを
  それぞれのメモに手動添付し、Grafana で取得した PDF/CSV も必ずアップロードしてください。

これにより、SN-8 が求める付属書証跡を手作業なしで揃えられます。

## 2. オンボーディングキットの構成要素

### サフィックス配線

- レジストリスキーマとセレクタールール:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md) /
  [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)
- DNS スケルトンヘルパー:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py) と
  [ゲートウェイ／DNS ランブック](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md) に記載されたリハーサル手順。
- 各レジストラローンチでは
  `docs/source/sns/reports/` に短いノートを作成し、セレクター例、GAR 証明、DNS ハッシュをまとめます。

### 料金チートシート

| ラベル長 | 基礎料金 (USD 相当) |
|----------|--------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

サフィックス係数: `.sora` = 1.0×、`.nexus` = 0.8×、`.dao` = 1.3×  
期間倍率: 2 年 = −5%、5 年 = −12%。猶予期間 30 日、償還期間 60 日（手数料 20%、最低 $5、上限 $200）。交渉済みの例外はレジストラチケットに記録してください。

### プレミアムオークション vs 更新

1. **プレミアムプール** — シールドビッドのコミット／リビール（SN-3）。
   `sns_premium_commit_total` で入札を追跡し、マニフェストを
   `docs/source/sns/reports/` に公開します。
2. **ダッチリオープン** — 猶予 + 償還が切れた後に 7 日間のダッチセールを開始します。
   初期 10× から日次 15% で減衰させ、`manifest_id` を付けてダッシュボードに進捗を表示させます。
3. **更新** — `sns_registrar_status_total{resolver="renewal"}`
   を監視し、オートリニュー手順（通知、SLA、代替決済レール）をレジストラチケットへ記載します。

### 開発者 API と自動化

- API 契約:
  [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)
- バルクヘルパーと CSV スキーマ:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)
- 実行例:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

`--submission-log` に出力される manifest ID を KPI ダッシュボードのフィルターへ入力し、
財務がリリース別に収益パネルを突合できるようにします。

### 証跡バンドル

1. 連絡先、サフィックス範囲、決済レールを含むレジストラチケット。
2. DNS / リゾルバ証跡（ゾーンファイルスケルトン + GAR 証明）。
3. 料金ワークシートとガバナンスが承認した例外。
4. API / CLI スモークテストアーティファクト（`curl` サンプル、CLI ログ）。
5. KPI ダッシュボードのスクリーンショット + CSV エクスポート（対象月の付属書へ添付）。

## 3. ローンチチェックリスト

| 手順 | 担当 | アーティファクト |
|------|------|-------------------|
| ダッシュボードをインポート | プロダクトアナリティクス | Grafana API レスポンス + ダッシュボード UID |
| ポータル埋め込み検証 | Docs/DevRel | `npm run build` ログ + プレビュー画像 |
| DNS リハーサル完了 | Networking/Ops | `sns_zonefile_skeleton.py` 出力 + ランブックログ |
| レジストラ自動化のドライラン | レジストラエンジニア | `sns_bulk_onboard.py` の submissions log |
| ガバナンス証跡の提出 | ガバナンス評議会 | 付属書リンク + エクスポートしたダッシュボードの SHA-256 |

このチェックリストを完了し、得られたアーティファクトを提案書に紐付けることで、SN-8 のゲートを監査可能な状態で通過できます。
