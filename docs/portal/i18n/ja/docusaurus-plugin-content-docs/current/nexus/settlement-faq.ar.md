---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
タイトル: الأسئلة الشائعة للتسوية
説明: XOR を使用して、XOR を実行します。
---

عكس هذه الصفحة الأسئلة الشائعة الداخلية للتسوية (`docs/source/nexus_settlement_faq.md`) بحيث يستطيع قراءモノレポをご覧ください。決済ルーター セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ SDK セキュリティNorito。

## いいえ

1. **レーン** — データスペース `settlement_handle` (`xor_global` `xor_lane_weighted` `xor_hosted_custody` `xor_dual_fund`)。レーンは `docs/source/project_tracker/nexus_config_deltas/` です。
2. **تحويل حتمي** — يحول الـ router جميع التسويات إلى XOR عبر مصادر السيولة المعتمدة من الحوكمة.レーン数と XOR 数ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット
3. ** عن بعد** — راقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس ヘアカット。 `dashboards/grafana/nexus_settlement.json` と `dashboards/alerts/nexus_audit_rules.yml` を確認してください。
4. ******* — ルーター、ルーター、セキュリティ、セキュリティああ。
5. **مسؤوليات SDK** — يجب على كل SDK توفير أدوات مساعدة للتسوية ومعرفات レーン ومشفري حمولات Noritoルーターを使用してください。

## أمثلة على التدفقات

|レーン |ログイン | ログインログインしてください。
|----------|-----------|----------------|
|認証済み `xor_hosted_custody` |ルーター + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC の XOR ヘアカットを選択してください。 |
| `xor_global` |ルーター + DEX/TWAP + セキュリティ セキュリティ |ヘアカットをするのに必要なヘアカット。 |
| هجينة `xor_dual_fund` |ルーター セキュリティ パブリック シールド + ルーター セキュリティ |シールド付き/パブリック ヘアカット ヘアカット ヘアカット。 |

## هل تحتاج مزيدا من التفاصيل؟

- よくある質問: `docs/source/nexus_settlement_faq.md`
- 決済ルーター: `docs/source/settlement_router.md`
- CBDC: `docs/source/cbdc_lane_playbook.md`
- دليل التشغيل: [عمليات Nexus](./nexus-operations)