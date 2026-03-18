---
lang: ur
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_incentive_parliament_packet/rollback_plan.md کا اردو ترجمہ -->

# Relay Incentive Rollback Plan

اگر governance halt مانگے یا telemetry guardrails fire ہوں تو automatic relay payouts کو disable کرنے کے لئے یہ playbook استعمال کریں.

1. **Automation freeze کریں.** ہر orchestrator host پر incentives daemon روکیں
   (`systemctl stop soranet-incentives.service` یا equivalent container deployment) اور یقینی بنائیں کہ process مزید نہیں چل رہا.
2. **Pending instructions drain کریں.**
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   چلائیں تاکہ کوئی outstanding payout instructions نہ ہوں۔ حاصل شدہ Norito payloads کو audit کے لئے archive کریں.
3. **Governance approval revoke کریں.** `reward_config.json` میں
   `"budget_approval_id": null` سیٹ کریں، اور config کو
   `iroha app sorafs incentives service init` کے ذریعے redeploy کریں (یا long-lived daemon کے لئے `update-config`). payout engine اب
   `MissingBudgetApprovalId` کے ساتھ fail closed ہو جاتا ہے، لہذا daemon نئے approval hash کے restore ہونے تک payouts mint کرنے سے انکار کرتا ہے۔ incident log میں git commit اور modified config کا SHA-256 ریکارڈ کریں.
4. **Sora Parliament کو notify کریں.** drained payout ledger، shadow-run رپورٹ، اور مختصر incident summary منسلک کریں۔ Parliament minutes میں revoked configuration کا hash اور daemon halt ہونے کا وقت درج ہونا چاہئے.
5. **Rollback validation.** daemon کو disable رکھیں جب تک:
   - telemetry alerts (`soranet_incentives_rules.yml`) >=24 h کے لئے green ہوں،
   - treasury reconciliation report zero missing transfers دکھائے، اور
   - Parliament نئے budget hash کو approve کرے.

جب governance budget approval hash دوبارہ جاری کرے، `reward_config.json`
کو نئے digest سے اپڈیٹ کریں، تازہ telemetry پر `shadow-run` دوبارہ چلائیں،
اور incentives daemon ری اسٹارٹ کریں.

</div>
