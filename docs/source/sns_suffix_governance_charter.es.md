---
lang: es
direction: ltr
source: docs/source/sns_suffix_governance_charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28f106c2d4e03c55194c9277729cd024fb8079625bcc9681726707927eed8379
source_last_modified: "2026-01-03T18:08:01.382063+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Sora Name Service Suffix Governance Charter
summary: Governance and accountability framework for onboarding and operating SNS suffixes.
---

# Sora Name Service Suffix Governance Charter

> **Status:** Ratified by the council (Q2 2025) and enforced; localization is tracked separately, but registrar and dispute flows now follow this charter.

## 1. Purpose & Scope

This charter codifies how new top-level suffixes (e.g., `.sora`, `.nexus`, `.dao`) are admitted into the Sora Name Service (SNS) and how existing suffixes remain accountable. It applies to every participant involved in suffix lifecycle management:

- **Proposers** who request a new suffix or a material policy change.
- **Suffix stewards** that operate the commercial and operational aspects of a suffix once approved.
- **Governance council & DAO voters** who must review and ratify every change.
- **Guardian board** that can pause or revoke suffixes when safety or legal issues arise.
- **Registry & DNS operators** that perform the deterministic on-ledger/state updates.

## 2. Role Matrix

| Role | Responsibilities | Escalation / Veto Powers |
|------|------------------|--------------------------|
| **Governance Council (9 seats)** | Review proposals, run due diligence, publish decisions on-ledger, steward KPI reviews, coordinate legal review. | 5/9 quorum to advance proposals; 6/9 super-majority to override guardian freezes. |
| **Guardian Board (3 seats)** | Monitor compliance, respond to takedown orders, trigger emergency freezes, run post-mortems. | Can freeze a suffix for up to 30 days; council super-majority required to unfreeze early. |
| **SNS DAO voters** | Provide broad legitimacy by co-signing council-approved proposals, delegate stake for proposer qualification, recall stewards. | ≥10% participating stake can issue a binding reconsideration vote. |
| **Suffix Stewards** | Operate the suffix (policy execution, customer support, reporting), meet KPIs, maintain dispute desks, host transparency dashboards. | None; violation of KPIs or legal obligations subjects steward to claw-back. |
| **Registry & DNS Operators** | Apply deterministic state transitions (namehash ownership, ZF/GAR publishing, DNS+gateway pinning), maintain audit trails. | Must reject instructions lacking completed governance artifacts. |

## 3. Proposal Eligibility & Intake

1. **Initiation threshold:** a proposer must either (a) bond **20 000 SNS** tokens for the duration of the review _or_ (b) collect delegations from addresses representing **≥0.25 % of circulating SNS**. Bonds are refunded when the vote concludes unless fraud is proven.
2. **Application pack:** proposals must include suffix name, steward candidate(s), dispute policy, pricing tiers, technical readiness evidence, and draft GAR/DNS plans.
3. **Intake SLA:** governance ops acknowledge submissions within **3 business days** and publish an intake record (ticket id + timestamp + proposer account) to the governance ledger topic.

## 4. Review Cadence & Timeline (28-Day Window)

| Phase | Day | Description |
|-------|-----|-------------|
| **Screening** | 0 – 5 | Council validates completeness, runs sanctions checks, assigns rapporteur. |
| **Public comment** | 6 – 13 | Proposal pack published to SNS portal; DAO delegates and stewards provide feedback. |
| **Due diligence** | 14 – 21 | Rapporteur collects KPI forecasts, GAR templates, financial models, legal memos. Guardians assess abuse surface. |
| **Decision window** | 22 – 26 | Council vote (Section 5) plus DAO signaling ballot; draft decision posted for 48 h cooling-off. |
| **Finalization** | 27 – 28 | Ratified decision, KPI covenant, and rollout checklist committed on-ledger; registry operators schedule activation. |

Proposals that miss documentation deadlines automatically reset to Day 0 with a status of “Returned for rework”.

## 5. Voting & Ratification Mechanics

1. **Council vote:** quorum is 5 seats; passage requires **simple majority (≥5/9)**.
2. **DAO signaling:** concurrent 7-day vote; ≥10% participating stake with ≥60% “approve” is required for activation. Failure to reach DAO quorum pauses the proposal until the next governance window.
3. **Dual approval:** both council majority and DAO signaling must succeed. When DAO signaling fails but the council approves, the proposal re-enters public comment with updated rationale.
4. **Record keeping:** decision artifacts (vote roll call, KPI baseline, steward contract hash) are stored under `docs/source/governance/sns/<proposal-id>/`.

## 6. Veto, Freeze & Override Rules

- **Guardian freeze:** guardians may issue an emergency freeze (max 30 days) when a suffix induces security, legal, or compliance risk. Freeze notices must include incident id, scope, and remediation expectations.
- **Council override:** requires ≥6 council votes plus DAO signaling with ≥15 % participating stake. Overrides shorter than 7 days must document risk acceptance.
- **Steward recall:** DAO voters holding ≥15 % participating stake may trigger a steward recall vote. If recall succeeds, council appoints an interim steward while re-running the steward selection process.
- **Appeals:** proposers can appeal rejections by posting new evidence plus a 5 000 SNS refundable bond. Appeals follow the same 28-day cadence.

## 7. KPI & Reporting Covenant

| KPI | Definition | Target | Reporting Cadence | Owner |
|-----|------------|--------|-------------------|-------|
| **Resolution availability** | % of DNS + gateway queries served within SLA | ≥99.5 % monthly | Monthly dashboard & signed report | Steward |
| **Dispute turnaround** | Median time to resolve abuse or trademark disputes | ≤7 days | Quarterly | Steward + Governance |
| **Renewal retention** | % expiring names renewed on time | ≥85 % | Quarterly | Steward |
| **Compliance incidents** | # of guardian freezes or policy violations | 0 critical / ≤2 minor per quarter | Continuous | Guardians |
| **Financial remittance** | Timely revenue split payouts | 100% on schedule | Monthly | Treasury ops |

Non-compliance triggers corrective action plans; repeated misses allow guardians to freeze the suffix or claw back steward allocations.

## 8. Revenue Split & Settlement

- **Default split:** 70 % of net suffix revenue routes to the protocol treasury, 30 % to the steward. Optional referral rebates (≤10%) must be budget-neutral to the steward share and documented in the proposal.
- **Settlement mechanics:** revenue distributions are emitted as deterministic on-ledger instructions referencing `RevenueShareV1 { suffix, epoch_id, steward_share, treasury_share }`. Payments settle weekly with Norito receipts stored alongside financial statements.
- **Escrow & clawback:** treasury retains an escrow equal to one month of steward share. Guardians may lock or claw back escrow when KPI breaches occur or when audits discover misreporting.

## 9. Compliance, Audit & Transparency

- Every suffix must publish quarterly transparency bundles (runbooks, KPI dashboards, financial statements) under `docs/source/sns/reports/<suffix>/<YYYY-MM>.md`.
- GAR, ZF (Zonefile), and DNS artifacts must be signed and pinned via the deterministic registry workflow (`docs/source/soradns/soradns_registry_rfc.md`).
- All governance actions referencing a suffix must include the suffix id, proposal id, and decision hash to keep the audit chain replayable.

## 10. Change Management

- Charter revisions follow the same proposer thresholds unless explicitly categorized as “editorial”. Council maintains a backlog of requested clarifications in `docs/source/sns_suffix_governance_charter.md`.
- Localized copies (he/ja) are regenerated once the English source stabilizes; translators must not alter semantics.

## 11. Regulatory Findings Integration

National regulatory findings (e.g., telecom naming mandates, financial sanctions guidance, or country-specific consumer protection rulings) flow into the SNS charter through a deterministic four-step loop:

1. **Intake & Triage (Days 0-5).** Guardians monitor regulator bulletins and legal advisories. New findings are logged in the governance tracker with jurisdiction, citation, and required response time. Emergency findings automatically trigger a guardian freeze assessment per Section 6.
2. **Impact Mapping (Days 6-13).** The assigned council rapporteur works with legal counsel to map the finding onto suffix obligations: pricing controls, disclosure wording, registrar vetting, escrow rules, or DNS/GAR hosting constraints. Results are captured in a memo appended to the active proposal or steward covenant.
3. **Charter & KPI Updates (Days 14-21).** When the finding requires durable policy changes, the council publishes an addendum that amends the relevant charter section (roles, KPIs, dispute policy). Stewards must acknowledge the addendum, and KPI dashboards gain a compliance indicator tied to the jurisdiction (e.g., `compliance.eu-dsa`).
4. **Activation & Evidence (Days 22-28).** Registry/DNS operators enforce the change (updated GAR templates, dispute queues, or name restrictions). Evidence bundles (legal memo hash, steward acknowledgement, configuration diff) are stored under `docs/source/sns/regulatory/<jurisdiction>/<YYYY-MM>.md` and referenced in guardian runbooks.

Routine findings that do not alter policy outright (e.g., renewed telecom reporting cadence) still follow intake/mapping but conclude with a “no-op” decision recorded in the tracker so auditors can trace the disposition. All emergency freezes or suffix revocations tied to regulatory findings must cite the relevant memo id to keep the audit trail verifiable.

---

**Contact:** governance@sora.net for clarifications or to schedule pre-submission consultations.
