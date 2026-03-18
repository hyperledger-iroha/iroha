---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
title: よくある質問 決済
説明: ルーティング決済、XOR 変換、遠隔測定、および監査の要求に対する応答。
---

ページは、和解に関する FAQ (`docs/source/nexus_settlement_faq.md`) を再表示し、単一レポートを参照せずに、ポータル サイトでの講義を提供します。決済ルータの詳細なコメント、ペイロード、ペイロード Norito の SDK のインテグレータの詳細な監視とコメント。

## ポイント数

1. **レーンのマップ** - チャック データスペースは `settlement_handle` (`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` または `xor_dual_fund`) を宣言します。 `docs/source/project_tracker/nexus_config_deltas/` のレーン カタログを参照してください。
2. **変換確定** - ルータは、政府による清算承認のソースを介して XOR で決済を変換します。レーンはバッファ XOR よりも優先されます。ヘアカットは、政治的要素から派生した、アップリケのヘアカットです。
3. **テレメトリー** - `nexus_settlement_latency_seconds` の監視、変換とヘアカットの検査。ダッシュボードは `dashboards/grafana/nexus_settlement.json` で設定され、アラートは `dashboards/alerts/nexus_audit_rules.yml` で表示されます。
4. **Preuves** - 設定ファイルのアーカイブ、ルーターのログ、テレメトリーのエクスポート、および調整と監査の報告。
5. **責任 SDK** - SDK は、決済ヘルパーの公開者、ペイロードの ID およびエンコーダ Norito を提供し、アベレージルーターを整列させます。

## フラックスの例

|レーンを入力 |コレクターであるプレーヴ | 写真セ・ケ・セラ・プルーヴェ |
|----------|-----------|----------------|
| Privee `xor_hosted_custody` |ルーターのログ + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | XOR 決定で CBDC 借方をバッファし、政治上のヘアカットを保留します。 |
|パブリック `xor_global` |ルーターのログ + リファレンス DEX/TWAP + 遅延/コンバージョンのメトリクス | TWAP 公開の平均ゼロ ヘアカットを修正するために、流動的な部分を修正します。 |
|ハイブリッド `xor_dual_fund` |ルーター モントラントの再パーティションのログ、パブリック vs シールド + テレメトリのコンピューティング |シールド付き/公共の比率を尊重し、ヘアカット アップリケとチャック ジャンブを登録します。 |

## 詳細についてはどうですか?

- よくある質問の完了: `docs/source/nexus_settlement_faq.md`
- 決済ルータの仕様: `docs/source/settlement_router.md`
- 政治戦略ハンドブック CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook オペレーション: [オペレーション Nexus](./nexus-operations)