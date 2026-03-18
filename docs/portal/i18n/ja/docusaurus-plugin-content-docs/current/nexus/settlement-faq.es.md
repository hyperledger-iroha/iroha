---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
title: 決済に関するよくある質問
説明: 解決策の解決、XOR の変換、テレメトリと聴覚の証拠のためのオペラのレスキュー。
---

決済間の FAQ (`docs/source/nexus_settlement_faq.md`) を参照して、ポータルの改訂版を参照し、モノレポを参照してください。 Settlement Router の手順を説明し、Norito の SDK と統合されたペイロードを監視してメトリクスを監視します。

## デスタカドス

1. **レーンのマペオ** - `settlement_handle` (`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` または `xor_dual_fund`) によるデータスペース宣言。 `docs/source/project_tracker/nexus_config_deltas/` のレーンのカタログを参照してください。
2. **変換の決定性** - ルータは、解決策を決定するための XOR を変換します。 Las lanes privadas prefinancian バッファ XOR;ロス・ヘアカット・アプリカン・ソロ・クアンド・ロス・バッファス・セ・デスビアン・デ・ラ・ポリティカ。
3. **テレメトリア** - ヴィギラ `nexus_settlement_latency_seconds`、変換のコンタドールとヘアカットのメディドール。 `dashboards/grafana/nexus_settlement.json` でダッシュボードが表示され、`dashboards/alerts/nexus_audit_rules.yml` でアラートが表示されます。
4. **証拠** - アーカイブ構成、ルーターのログ、テレメトリーのエクスポート、および聴覚的な調停に関する情報。
5. **SD​​K の責任** - SDK の決済ヘルパーの説明、ペイロードの ID、Norito パラメーターの管理、ルーターの制御。

## フルホス デ エジェンプロ

|ティーポ・デ・レーン |捕虜の証拠 |ロ・ケ・デムエストラ |
|----------|-----------|----------------|
|プリバーダ `xor_hosted_custody` |ルーターのログ + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` |損失バッファーは CBDC デビタン XOR 決定論的損失ヘアカットを政治的に管理します。 |
|パブリカ `xor_global` |ルーターのログ + 参照 DEX/TWAP + 遅延/変換のメトリクス | TWAP のヘアカットを公開するために、液体の処理が必要です。 |
|ハイブリダ `xor_dual_fund` |ルーターの公開部門のログとシールド + テレメトリアのコンタドール |ラ・メズクラ・シールド/パブリック・レスペト・ロス・レシオ・デ・ゴベルナンザとレジストロ・エル・ヘアカット・アプリカド・ア・カダ・トラモ。 |

## 必要ですか?

- よくある質問の完全版: `docs/source/nexus_settlement_faq.md`
- 決済ルータの仕様: `docs/source/settlement_router.md`
- 政治のハンドブック CBDC: `docs/source/cbdc_lane_playbook.md`
- オペレーションのランブック: [Nexus](./nexus-operations) のオペレーション