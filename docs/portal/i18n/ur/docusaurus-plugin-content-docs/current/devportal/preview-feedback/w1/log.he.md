---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b07b74fa24e3d38c2f7f41418bc99bbf0ed5e8763130b0b6954f787439df1f52
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-log
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

یہ لاگ **W1 پارٹنر preview** کے لئے invite roster، ٹیلیمیٹری checkpoints، اور reviewer feedback محفوظ کرتا ہے
جو [`preview-feedback/w1/plan.md`](./plan.md) کی acceptance tasks اور wave tracker اندراج
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) کے ساتھ وابستہ ہیں۔ جب دعوت ارسال ہو،
ٹیلیمیٹری snapshot ریکارڈ ہو، یا feedback item triage ہو تو اسے اپ ڈیٹ کریں تاکہ governance reviewers
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت replay کر سکیں۔

## Cohort roster

| Partner ID | Request ticket | NDA موصول | Invite sent (UTC) | Ack/first login (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مکمل 2025-04-26 | sorafs-op-01; orchestrator docs parity evidence پر فوکس۔ |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مکمل 2025-04-26 | sorafs-op-02; Norito/telemetry cross-links کی توثیق۔ |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مکمل 2025-04-26 | sorafs-op-03; multi-source failover drills چلائے۔ |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مکمل 2025-04-26 | torii-int-01; Torii `/v1/pipeline` + Try it cookbook review۔ |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مکمل 2025-04-26 | torii-int-02; Try it screenshot update میں ساتھ دیا (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مکمل 2025-04-26 | sdk-partner-01; JS/Swift cookbook feedback + ISO bridge sanity checks۔ |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مکمل 2025-04-26 | sdk-partner-02; compliance 2025-04-11 کو clear، Connect/telemetry notes پر فوکس۔ |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مکمل 2025-04-26 | gateway-ops-01; gateway ops guide audit + anonymised Try it proxy flow۔ |

**Invite sent** اور **Ack** کے timestamps فوری طور پر درج کریں جب outbound email جاری ہو۔
وقت کو W1 پلان میں دی گئی UTC schedule کے مطابق رکھیں۔

## Telemetry checkpoints

| Timestamp (UTC) | Dashboards / probes | Owner | Result | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ All green | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` transcript | Ops | ✅ Staged | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards اوپر + `probe:portal` | Docs/DevRel + Ops | ✅ Pre-invite snapshot, no regressions | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards اوپر + Try it proxy latency diff | Docs/DevRel lead | ✅ Midpoint check passed (0 alerts; Try it latency p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards اوپر + exit probe | Docs/DevRel + Governance liaison | ✅ Exit snapshot, zero outstanding alerts | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

روزانہ office-hour samples (2025-04-13 -> 2025-04-25) NDJSON + PNG exports کے طور پر
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` میں موجود ہیں، فائل نام
`docs-preview-integrity-<date>.json` اور متعلقہ screenshots کے ساتھ۔

## Feedback اور issue log

اس جدول کو reviewer findings کے خلاصے کے لئے استعمال کریں۔ ہر entry کو GitHub/discuss
ticket اور اس structured form سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کے ذریعے بھرا گیا۔

| Reference | Severity | Owner | Status | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | ✅ Resolved 2025-04-18 | Try it nav wording + sidebar anchor واضح کیا (`docs/source/sorafs/tryit.md` نئے label کے ساتھ اپ ڈیٹ). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | ✅ Resolved 2025-04-19 | Try it screenshot + caption reviewer کی درخواست پر اپ ڈیٹ؛ artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | 🟢 Closed | باقی تبصرے صرف Q&A تھے؛ ہر پارٹنر کے feedback form میں محفوظ ہیں `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Knowledge check اور survey tracking

1. ہر reviewer کے quiz scores ریکارڈ کریں (target >=90%); exported CSV کو invite artefacts کے ساتھ attach کریں۔
2. feedback form سے حاصل qualitative survey answers جمع کریں اور انہیں
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` کے تحت mirror کریں۔
3. threshold سے نیچے والوں کے لئے remediation calls schedule کریں اور انہیں اس فائل میں log کریں۔

تمام آٹھ reviewers نے knowledge check میں >=94% اسکور کیا (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). کوئی remediation calls درکار نہیں ہوئیں؛
ہر پارٹنر کے survey exports یہاں موجود ہیں:
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Artefact inventory

- Preview descriptor/checksum bundle: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Probe + link-check summary: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Try it proxy change log: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetry exports: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Daily office-hour telemetry bundle: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Feedback + survey exports: reviewer-specific folders رکھیں
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` کے تحت
- Knowledge check CSV اور summary: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Inventory کو tracker issue کے ساتھ sync رکھیں۔ جب artefacts کو governance ticket میں کاپی کریں تو hashes attach کریں
تاکہ auditors فائلیں بغیر shell access کے verify کر سکیں۔
