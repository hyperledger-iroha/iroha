---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# پریویو دعوت ٹریکر

یہ ٹریکر docs پورٹل کی ہر پریویو ویو ریکارڈ کرتا ہے تاکہ DOCS-SORA کے مالکان اور governance reviewers دیکھ سکیں کہ کون سی cohort فعال ہے، کس نے دعوتیں منظور کیں، اور کون سے artifacts ابھی توجہ چاہتے ہیں۔ جب بھی دعوتیں بھیجی جائیں، واپس لی جائیں یا مؤخر ہوں تو اسے اپ ڈیٹ کریں تاکہ audit trail ریپوزٹری کے اندر رہے۔

## ویو اسٹیٹس

| ویو | cohort | ٹریکر ایشو | approver(s) | اسٹیٹس | ہدف ونڈو | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Docs + SDK maintainers جو checksum فلو validate کرتے ہیں | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Docs/DevRel lead + Portal TL | مکمل | Q2 2025 ہفتے 1-2 | دعوتیں 2025-03-25 کو بھیجی گئیں، ٹیلی میٹری سبز رہی، exit summary 2025-04-08 کو شائع ہوا۔ |
| **W1 - Partners** | SoraFS آپریٹرز، Torii integrators تحت NDA | `DOCS-SORA-Preview-W1` | Docs/DevRel lead + governance liaison | مکمل | Q2 2025 ہفتہ 3 | دعوتیں 2025-04-12 -> 2025-04-26، تمام آٹھ پارٹنرز کی تصدیق؛ evidence [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) میں اور exit digest [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) میں۔ |
| **W2 - Community** | منتخب community waitlist (<=25 ایک وقت میں) | `DOCS-SORA-Preview-W2` | Docs/DevRel lead + community manager | مکمل | Q3 2025 ہفتہ 1 (عارضی) | دعوتیں 2025-06-15 -> 2025-06-29، پوری مدت میں ٹیلی میٹری سبز؛ evidence + findings [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) میں۔ |
| **W3 - Beta cohorts** | finance/observability beta + SDK partner + ecosystem advocate | `DOCS-SORA-Preview-W3` | Docs/DevRel lead + governance liaison | مکمل | Q1 2026 ہفتہ 8 | دعوتیں 2026-02-18 -> 2026-02-28؛ digest + portal data `preview-20260218` ویو سے تیار (دیکھیں [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> نوٹ: ہر ٹریکر ایشو کو متعلقہ preview request tickets سے لنک کریں اور انہیں `docs-portal-preview` پروجیکٹ میں archive کریں تاکہ approvals قابلِ دریافت رہیں۔

## فعال کام (W0)

- preflight artifacts ریفریش (GitHub Actions `docs-portal-preview` رن 2025-03-24، descriptor `scripts/preview_verify.sh` کے ذریعے `preview-2025-03-24` ٹیگ سے verify)۔
- ٹیلی میٹری baselines محفوظ (`docs.preview.integrity`, `TryItProxyErrors` dashboard snapshot W0 issue میں محفوظ)۔
- outreach متن [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے ساتھ lock، preview tag `preview-2025-03-24`۔
- پہلے پانچ maintainers کے لئے intake requests لاگ (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- پہلی پانچ دعوتیں 2025-03-25 10:00-10:20 UTC کو بھیجیں، سات دن مسلسل سبز ٹیلی میٹری کے بعد؛ acknowledgements `DOCS-SORA-Preview-W0` میں محفوظ۔
- ٹیلی میٹری مانیٹرنگ + host office hours (2025-03-31 تک روزانہ check-ins; checkpoint log نیچے)۔
- midpoint feedback / issues اکٹھے کر کے `docs-preview/w0` ٹیگ (دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- ویو summary شائع + invite exit confirmations (exit bundle تاریخ 2025-04-08; دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- W3 beta wave ٹریک; مستقبل کی ویوز governance review کے بعد شیڈول۔

## W1 partners ویو خلاصہ

- قانونی اور governance approvals۔ Partner addendum 2025-04-05 کو سائن؛ approvals `DOCS-SORA-Preview-W1` میں اپ لوڈ۔
- Telemetry + Try it staging۔ Change ticket `OPS-TRYIT-147` 2025-04-06 کو اجرا؛ `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` کے Grafana snapshots آرکائیو۔
- Artefact + checksum prep۔ `preview-2025-04-12` bundle verify؛ descriptor/checksum/probe logs `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ۔
- Invite roster + dispatch۔ آٹھ partner requests (`DOCS-SORA-Preview-REQ-P01...P08`) منظور؛ دعوتیں 2025-04-12 15:00-15:21 UTC کو بھیجیں، ہر reviewer کا ack ریکارڈ۔
- Feedback instrumentation۔ روزانہ office hours + telemetry checkpoints ریکارڈ؛ digest کے لئے [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) دیکھیں۔
- Final roster/exit log۔ [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) اب invite/ack timestamps، telemetry evidence، quiz exports، اور artefact pointers 2025-04-26 تک ریکارڈ کرتا ہے تاکہ governance wave کو reproduce کر سکے۔

## Invite log - W0 core maintainers

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Active | checksum verification کی تصدیق؛ nav/sidebar review پر توجہ۔ |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Active | SDK recipes + Norito quickstarts ٹیسٹ کر رہا ہے۔ |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Active | Try it console + ISO flows ویلیڈیٹ۔ |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Active | SoraFS runbooks + orchestration docs آڈٹ۔ |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Active | telemetry/incident appendices کا جائزہ؛ Alertmanager coverage کے ذمہ دار۔ |

تمام دعوتیں ایک ہی `docs-portal-preview` artefact (run 2025-03-24، tag `preview-2025-03-24`) اور `DOCS-SORA-Preview-W0` میں محفوظ verification transcript کو ریفرنس کرتی ہیں۔ کسی بھی اضافے/وقفے کو اگلی ویو پر جانے سے پہلے اوپر والی ٹیبل اور tracker issue دونوں میں لاگ کریں۔

## Checkpoint log - W0

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-03-26 | Telemetry baseline review + office hours | `docs.preview.integrity` + `TryItProxyErrors` سبز رہے؛ office hours نے checksum verification مکمل ہونے کی تصدیق کی۔ |
| 2025-03-27 | Midpoint feedback digest posted | Summary [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) میں محفوظ؛ دو چھوٹے nav issues `docs-preview/w0` کے تحت، کوئی incident نہیں۔ |
| 2025-03-31 | Final week telemetry spot check | آخری office hours؛ reviewers نے باقی کام تصدیق کیا، کوئی alert نہیں۔ |
| 2025-04-08 | Exit summary + invite closures | reviews مکمل، عارضی رسائی منسوخ، findings [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) میں آرکائیو؛ W1 سے پہلے tracker اپ ڈیٹ۔ |

## Invite log - W1 partners

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completed | orchestrator ops feedback 2025-04-20 کو دیا؛ exit ack 15:05 UTC۔ |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completed | rollout comments `docs-preview/w1` میں لاگ؛ exit ack 15:10 UTC۔ |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completed | dispute/blacklist edits فائل؛ exit ack 15:12 UTC۔ |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completed | Try it auth walkthrough قبول؛ exit ack 15:14 UTC۔ |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completed | RPC/OAuth comments لاگ؛ exit ack 15:16 UTC۔ |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completed | preview integrity feedback merge؛ exit ack 15:18 UTC۔ |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completed | telemetry/redaction review مکمل؛ exit ack 15:22 UTC۔ |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completed | gateway DNS runbook comments فائل؛ exit ack 15:24 UTC۔ |

## Checkpoint log - W1

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-04-12 | Invite dispatch + artefact verification | `preview-2025-04-12` descriptor/archive کے ساتھ آٹھ partners کو ای میل؛ acknowledgements tracker میں محفوظ۔ |
| 2025-04-13 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` سبز؛ office hours نے checksum verification مکمل ہونے کی تصدیق کی۔ |
| 2025-04-18 | Mid-wave office hours | `docs.preview.integrity` سبز رہا؛ دو doc نٹس `docs-preview/w1` کے تحت لاگ (nav wording + Try it screenshot)۔ |
| 2025-04-22 | Final telemetry spot check | proxy + dashboards صحت مند؛ کوئی نئی issues نہیں، exit سے پہلے tracker میں نوٹ۔ |
| 2025-04-26 | Exit summary + invite closures | تمام partners نے review completion کی تصدیق کی، invites revoke، evidence [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) میں آرکائیو۔ |

## W3 beta cohort recap

- 2026-02-18 کو invites بھیجی گئیں، checksum verification + acknowledgements اسی دن لاگ۔
- feedback `docs-preview/20260218` میں جمع، governance issue `DOCS-SORA-Preview-20260218`؛ digest + summary `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` سے تیار۔
- 2026-02-28 کو final telemetry check کے بعد access revoke؛ tracker + portal tables اپ ڈیٹ کر کے W3 مکمل دکھایا۔

## Invite log - W2 community

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completed | Ack 16:06 UTC; SDK quickstarts پر فوکس؛ exit 2025-06-29 کو کنفرم۔ |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completed | governance/SNS review مکمل؛ exit 2025-06-29 کو کنفرم۔ |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completed | Norito walkthrough feedback لاگ؛ exit ack 2025-06-29۔ |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completed | SoraFS runbook review مکمل؛ exit ack 2025-06-29۔ |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completed | Accessibility/UX notes شیئر؛ exit ack 2025-06-29۔ |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completed | Localization feedback لاگ؛ exit ack 2025-06-29۔ |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completed | Mobile SDK doc checks مکمل؛ exit ack 2025-06-29۔ |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completed | Observability appendix review مکمل؛ exit ack 2025-06-29۔ |

## Checkpoint log - W2

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-06-15 | Invite dispatch + artefact verification | `preview-2025-06-15` descriptor/archive 8 community reviewers کے ساتھ شیئر؛ acknowledgements tracker میں محفوظ۔ |
| 2025-06-16 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` dashboards سبز؛ Try it proxy logs میں community tokens فعال۔ |
| 2025-06-18 | Office hours & issue triage | دو suggestions (`docs-preview/w2 #1` tooltip wording، `#2` localization sidebar) - دونوں Docs کو routed۔ |
| 2025-06-21 | Telemetry check + doc fixes | Docs نے `docs-preview/w2 #1/#2` حل کیا؛ dashboards سبز، کوئی incident نہیں۔ |
| 2025-06-24 | Final week office hours | reviewers نے باقی feedback submissions کنفرم کیے؛ کوئی alert نہیں۔ |
| 2025-06-29 | Exit summary + invite closures | acks ریکارڈ، preview access revoke، telemetry snapshots + artefacts آرکائیو (دیکھیں [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Office hours & issue triage | دو documentation suggestions `docs-preview/w1` کے تحت لاگ؛ کوئی incidents یا alerts نہیں۔ |

## Reporting hooks

- ہر بدھ، اوپر والی table اور active invite issue کو مختصر status note سے اپ ڈیٹ کریں (invites sent, active reviewers, incidents).
- جب کوئی ویو بند ہو، feedback summary path شامل کریں (مثال: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) اور اسے `status.md` سے لنک کریں۔
- اگر [preview invite flow](./preview-invite-flow.md) کے pause criteria ٹرگر ہوں تو invites دوبارہ شروع کرنے سے پہلے remediation steps یہاں شامل کریں۔
