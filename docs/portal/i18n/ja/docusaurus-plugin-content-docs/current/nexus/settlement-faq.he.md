---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c2c870a31f96c4fe7c64412cbda2280094dbe33f8f096a55a8ea550bfde56281
source_last_modified: "2025-11-14T04:43:20.566610+00:00"
translation_last_reviewed: 2026-01-30
---

このページは社内のSettlement FAQ（`docs/source/nexus_settlement_faq.md`）を反映し、ポータルの読者がmono-repoを探さず同じガイダンスを確認できるようにしています。Settlement Routerが支払いを処理する方法、監視すべきメトリクス、SDKがNoritoペイロードを統合する方法を説明します。

## ハイライト

1. **レーンのマッピング** — 各dataspaceは`settlement_handle`（`xor_global`、`xor_lane_weighted`、`xor_hosted_custody`、`xor_dual_fund`）を宣言します。最新のレーンカタログは`docs/source/project_tracker/nexus_config_deltas/`を参照してください。
2. **決定的な変換** — ルーターはガバナンス承認済みの流動性ソースを通じて、すべてのsettlementをXORに変換します。プライベートレーンはXORバッファを事前に補填し、バッファがポリシー範囲外にずれた場合のみhaircutが適用されます。
3. **テレメトリ** — `nexus_settlement_latency_seconds`、変換カウンター、haircutゲージを監視します。ダッシュボードは`dashboards/grafana/nexus_settlement.json`、アラートは`dashboards/alerts/nexus_audit_rules.yml`にあります。
4. **証跡** — 監査に備えて、設定、ルーターログ、テレメトリエクスポート、照合レポートを保管します。
5. **SDKの責務** — すべてのSDKは、settlementヘルパー、lane ID、Noritoペイロードエンコーダを公開し、ルーターと同等の機能を保つ必要があります。

## 例のフロー

| レーン種別 | 取得する証跡 | 何を示すか |
|-----------|--------------------|----------------|
| プライベート `xor_hosted_custody` | ルーターログ + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDCバッファが決定的なXORをデビットし、haircutがポリシー内に収まること。 |
| パブリック `xor_global` | ルーターログ + DEX/TWAP参照 + レイテンシ/変換メトリクス | 共有流動性パスが公開TWAPで転送価格を決定し、haircutはゼロであること。 |
| ハイブリッド `xor_dual_fund` | public vs shieldedの分割を示すルーターログ + テレメトリカウンター | shielded/publicの混在がガバナンス比率を守り、各レッグに適用されたhaircutを記録したこと。 |

## 詳しく知りたい場合

- 詳細FAQ: `docs/source/nexus_settlement_faq.md`
- Settlement router仕様: `docs/source/settlement_router.md`
- CBDCポリシープレイブック: `docs/source/cbdc_lane_playbook.md`
- 運用ランブック: [Nexus運用](./nexus-operations)
