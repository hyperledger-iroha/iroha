---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
タイトル: 決済に関するよくある質問
説明: 決済ルーティング、XOR 変換、および آڈٹ ثبوت کا احاطہ کرتے ہیں۔
---

決済 FAQ (`docs/source/nexus_settlement_faq.md`) 決済 FAQ (`docs/source/nexus_settlement_faq.md`) 決済 FAQ (`docs/source/nexus_settlement_faq.md`) 決済 FAQ (`docs/source/nexus_settlement_faq.md`)モノリポジトリ بغیر دیکھ سکیں۔ یہ وضاحت کرتا ہے کہ Settlement Router ادائیگیوں کو کیسے پروسیس کرتا ہے، کن میٹرکس کی نگرانی SDK Norito ペイロード数

## ナオト

1. **レーン میپنگ** — ہر データスペース ایک `settlement_handle` کا اعلان کرتا ہے (`xor_global`، `xor_lane_weighted`، `xor_hosted_custody` یا `xor_dual_fund`)۔ `docs/source/project_tracker/nexus_config_deltas/` میں تازہ ترین レーン カタログ دیکھیں۔
2. ** ルータ 決済 ガバナンス 流動性ソース XOR 計算レーン数 XOR バッファ数ヘアカット صرف تب لاگو ہوتے ہیں جب バッファ پالیسی سے باہر جائیں۔
3. **ٹیلی میٹری** — `nexus_settlement_latency_seconds`، 変換カウンター、ヘアカット ゲージ、انیٹر کریں۔ダッシュボード `dashboards/grafana/nexus_settlement.json` میں اور アラート `dashboards/alerts/nexus_audit_rules.yml` میں ہیں۔
4. **監査** — 監査、設定、ルーター ログ、テレメトリ エクスポート、調整レポート
5. **SDK の説明** — SDK の決済ヘルパー、レーン ID、Norito ペイロード エンコーダー、ルーターの説明और देखें

## और देखें

|レーンجمع کرنے والا ثبوت | یہ کیا ثابت کرتا ہے |
|----------|-----------|----------------|
| `xor_hosted_custody` |ルーターのログ + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC バッファー XOR ڈیبٹ کرتے ہیں اور ヘアカット پالیسی کے اندر رہتے ہیں۔ |
| عوامی `xor_global` |ルーター ログ + DEX/TWAP データ + 遅延/コンバージョン メトリクス |流動性の高い流動性の高いヘアカット TWAP ゼロ ヘアカットの高品質|
| ہائبرڈ `xor_dual_fund` |ルーター ログ、パブリック シールド、シールド付きバージョン + テレメトリー カウンター |シールド/パブリック ガバナンス比率 پابندی کی اور ہر حصے پر لاگو ヘアカット کو ریکارڈ کیا۔ |

## مزید تفصیل چاہیے؟

- よくある質問: `docs/source/nexus_settlement_faq.md`
- 決済ルータ仕様：`docs/source/settlement_router.md`
- CBDC ハンドブック: `docs/source/cbdc_lane_playbook.md`
- 運用ランブック: [Nexus 運用](./nexus-operations)