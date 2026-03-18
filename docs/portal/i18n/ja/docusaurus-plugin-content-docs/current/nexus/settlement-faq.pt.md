---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
title: 決済に関するよくある質問
説明: 解決策のコブリンド ロテアメント、XOR 会話、テレメトリーと聴覚の証拠。
---

決済間の FAQ (`docs/source/nexus_settlement_faq.md`) は、ポータルの方向性や単一リポジトリの改訂を行うためのページです。 Settlement Router のプロセスを説明し、これらの SDK が統合 OS ペイロード Norito を開発しています。

## デスターク

1. **レーンのマップ** - `settlement_handle` (`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` または `xor_dual_fund`) のデータスペース宣言。最近のレーンのカタログ `docs/source/project_tracker/nexus_config_deltas/` を参照してください。
2. **決定的な会話** - ルータは、XOR または統治上の液体フォントと OS 決済を変換します。レーンは事前金融バッファ XOR を独占します。ヘアカットは、政治的な問題を解決するためにアプリケーションを実行します。
3. **テレメトリア** - `nexus_settlement_latency_seconds` を監視し、会話とヘアカットの管理を行います。ダッシュボードは `dashboards/grafana/nexus_settlement.json` とアラート、`dashboards/alerts/nexus_audit_rules.yml` を表示します。
4. **証拠** - ルーターの設定、ログを保存し、テレメトリや聴覚との関係をエクスポートします。
5. **SD​​K の責任** - SDK は、決済ヘルパーのエクスポート、ペイロードの ID、Norito パラメーターのパリダード com ルーターの ID を開発します。

## フルクソス・デ・サンプル

|ティーポ・デ・レーン |捕虜の証拠 | O que comprova |
|----------|-----------|----------------|
|プリバーダ `xor_hosted_custody` |ログドルーター + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` |バッファー CBDC デビタム XOR 決定性とヘアカットの政治的分析。 |
|パブリカ `xor_global` |ルーター + 参照 DEX/TWAP + 遅延/会話のメトリクスのログ | O カミーニョ デ リキッドデス コンパルティハド フィックス、プレコ ダ トランスファーエンシア、TWAP publicado com ゼロ ヘアカット。 |
|ハイブリダ `xor_dual_fund` |ルーターの公開対シールド + テレメトリアの管理をログに記録します。ミスチュラシールド/パブリックレペイトウOS比率デガバナンカ電子レジストルーOヘアカットアプリカダペルナ。 |

## プレサ・デ・マイス・デタルヘス?

- よくある質問の完全版: `docs/source/nexus_settlement_faq.md`
- 決済ルータの仕様: `docs/source/settlement_router.md`
- 政治のハンドブック CBDC: `docs/source/cbdc_lane_playbook.md`
- オペラのランブック: [オペラは Nexus](./nexus-operations)