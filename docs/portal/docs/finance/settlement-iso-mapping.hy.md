---
lang: hy
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54bf09c63cdc2720fe1968c90436344dc794ebc09313b41a050bdf548ae5809f
source_last_modified: "2026-01-22T14:35:36.859616+00:00"
translation_last_reviewed: 2026-02-07
id: settlement-iso-mapping
title: Settlement ↔ ISO 20022 Field Mapping
sidebar_label: Settlement ↔ ISO 20022
description: Canonical mapping between Iroha settlement flows and the ISO 20022 bridge.
---

:::note Canonical Source
:::

## Settlement ↔ ISO 20022 Field Mapping

This note captures the canonical mapping between Iroha settlement instructions
(`DvpIsi`, `PvpIsi`, repo collateral flows) and the ISO 20022 messages exercised
by the bridge. It reflects the message scaffolding implemented in
`crates/ivm/src/iso20022.rs` and serves as a reference when producing or
validating Norito payloads.

### Reference Data Policy (Identifiers and Validation)

This policy packages the identifier preferences, validation rules, and reference-data
obligations that the Norito ↔ ISO 20022 bridge must enforce before emitting messages.

**Anchor points inside the ISO message:**
- **Instrument identifiers** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (or the equivalent instrument field).
- **Parties / agents** → `DlvrgSttlmPties/Pty` and `RcvgSttlmPties/Pty` for `sese.*`,
  or the agent structures in `pacs.009`.
- **Accounts** → `…/Acct` elements for safekeeping/cash accounts; mirror the on-ledger
  `AccountId` in `SupplementaryData`.
- **Proprietary identifiers** → `…/OthrId` with `Tp/Prtry` and mirrored in
  `SupplementaryData`. Never replace regulated identifiers with proprietary ones.

#### Identifier preference by message family

##### `sese.023` / `.024` / `.025` (securities settlement)

- **Instrument (`FinInstrmId`)**
  - Preferred: **ISIN** under `…/ISIN`. It is the canonical identifier for CSDs / T2S.[^anna]
  - Fallbacks:
    - **CUSIP** or other NSIN under `…/OthrId/Id` with `Tp/Cd` set from the ISO external
      code list (e.g., `CUSP`); include the issuer in `Issr` when mandated.[^iso_mdr]
    - **Norito asset ID** as proprietary: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"`, and
      record the same value in `SupplementaryData`.
  - Optional descriptors: **CFI** (`ClssfctnTp`) and **FISN** where supported to ease
    reconciliation.[^iso_cfi][^iso_fisn]
- **Parties (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Preferred: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback: **LEI** where the version of the message exposes a dedicated LEI field; if
    absent, carry proprietary IDs with clear `Prtry` labels and include BIC in metadata.[^iso_cr]
- **Place of settlement / venue** → **MIC** for the venue and **BIC** for the CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` and `colr.007` (collateral management)

- Follow the same instrument rules as `sese.*` (ISIN preferred).
- Parties use **BIC** by default; **LEI** is acceptable where the schema exposes it.[^swift_bic]
- Cash amounts must use **ISO 4217** currency codes with correct minor units.[^iso_4217]

##### `pacs.009` / `camt.054` (PvP funding and statements)

- **Agents (`InstgAgt`, `InstdAgt`, debtor/creditor agents)** → **BIC** with optional
  LEI where allowed.[^swift_bic]
- **Accounts**
  - Interbank: identify by **BIC** and internal account references.
  - Customer-facing statements (`camt.054`): include **IBAN** when present and validate it
    (length, country rules, mod-97 checksum).[^swift_iban]
- **Currency** → **ISO 4217** 3-letter code, respect minor-unit rounding.[^iso_4217]
- **Torii ingestion** → Submit PvP funding legs via `POST /v1/iso20022/pacs009`; the bridge
  requires `Purp=SECU` and now enforces BIC crosswalks when reference data is configured.

#### Validation rules (apply before emission)

| Identifier | Validation rule | Notes |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` and Luhn (mod-10) check digit per ISO 6166 Annex C | Reject before bridge emission; prefer upstream enrichment.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` and modulus-10 with 2 weighting (characters map to digits) | Only when ISIN is unavailable; map via ANNA/CUSIP crosswalk once sourced.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` and mod-97 check digit (ISO 17442) | Validate against GLEIF daily delta files before acceptance.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Optional branch code (last three chars). Confirm active status in RA files.[^swift_bic] |
| **MIC** | Maintain from ISO 10383 RA file; ensure venues are active (no `!` termination flag) | Flag decommissioned MICs before emission.[^iso_mic] |
| **IBAN** | Country-specific length, uppercase alphanumeric, mod-97 = 1 | Use registry maintained by SWIFT; reject structurally invalid IBANs.[^swift_iban] |
| **Proprietary account/party IDs** | `Max35Text` (UTF-8, ≤35 characters) with trimmed whitespace | Applies to `GenericAccountIdentification1.Id` and `PartyIdentification135.Othr/Id` fields. Reject entries exceeding 35 characters so bridge payloads conform to ISO schemas. |
| **Proxy account identifiers** | Non-empty `Max2048Text` under `…/Prxy/Id` with optional type codes in `…/Prxy/Tp/{Cd,Prtry}` | Stored alongside the primary IBAN; validation still requires IBANs while accepting proxy handles (with optional type codes) to mirror PvP rails. |
| **CFI** | Six-character code, uppercase letters using ISO 10962 taxonomy | Optional enrichment; ensure characters match instrument class.[^iso_cfi] |
| **FISN** | Up to 35 characters, uppercase alphanumeric plus limited punctuation | Optional; truncate/normalise per ISO 18774 guidance.[^iso_fisn] |
| **Currency** | ISO 4217 3-letter code, scale determined by minor units | Amounts must round to permitted decimals; enforce on Norito side.[^iso_4217] |

#### Crosswalk and data maintenance obligations

- Maintain **ISIN ↔ Norito asset ID** and **CUSIP ↔ ISIN** crosswalks. Update nightly from
  ANNA/DSB feeds and version control the snapshots used by CI.[^anna_crosswalk]
- Refresh **BIC ↔ LEI** mappings from the GLEIF public relationship files so the bridge can
  emit both when required.[^bic_lei]
- Store **MIC definitions** alongside the bridge metadata so venue validation is
  deterministic even when RA files change mid-day.[^iso_mic]
- Record data provenance (timestamp + source) in bridge metadata for audit. Persist the
  snapshot identifier alongside emitted instructions.
- Configure `iso_bridge.reference_data.cache_dir` to persist a copy of each loaded dataset
  alongside provenance metadata (version, source, timestamp, checksum). This allows auditors
  and operators to diff historical feeds even after upstream snapshots rotate.
- ISO crosswalk snapshots are ingested by `iroha_core::iso_bridge::reference_data` using
  the `iso_bridge.reference_data` configuration block (paths + refresh interval). Gauges
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records`, and
  `iso_reference_refresh_interval_secs` expose runtime health for alerting. The Torii
  bridge rejects `pacs.008` submissions whose agent BICs are absent from the configured
  crosswalk, surfacing deterministic `InvalidIdentifier` errors when a counterparty is
  unknown.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN and ISO 4217 bindings are enforced at the same layer: pacs.008/pacs.009 flows now
  emit `InvalidIdentifier` errors when debtor/creditor IBANs lack configured aliases or when
  the settlement currency is missing from `currency_assets`, preventing malformed bridge
  instructions from reaching the ledger. IBAN validation also applies country-specific
  lengths and numeric check digits before the ISO 7064 mod‑97 pass so structurally invalid
  values are rejected early.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- The CLI settlement helpers inherit the same guard rails: pass
  `--iso-reference-crosswalk <path>` alongside `--delivery-instrument-id` to have the DvP
  preview validate instrument IDs before emitting the `sese.023` XML snapshot.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (and the CI wrapper `ci/check_iso_reference_data.sh`) lint
  crosswalk snapshots and fixtures. The command accepts `--isin`, `--bic-lei`, `--mic`, and
  `--fixtures` flags and falls back to the sample datasets in `fixtures/iso_bridge/` when run
  without arguments.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- The IVM helper now ingests real ISO 20022 XML envelopes (head.001 + `DataPDU` + `Document`)
  and validates the Business Application Header via the `head.001` schema so `BizMsgIdr`,
  `MsgDefIdr`, `CreDt`, and BIC/ClrSysMmbId agents are preserved deterministically; XMLDSig/XAdES
  blocks remain intentionally skipped. 

#### Regulatory and market-structure considerations

- **T+1 settlement**: US/Canada equity markets moved to T+1 in 2024; adjust Norito
  scheduling and SLA alerts accordingly.[^sec_t1][^csa_t1]
- **CSDR penalties**: Settlement discipline rules enforce cash penalties; ensure Norito
  metadata captures penalty references for reconciliation.[^csdr]
- **Same-day settlement pilots**: India’s regulator is phasing in T0/T+0 settlement; keep
  bridge calendars updated as pilots expand.[^india_t0]
- **Collateral buy-ins / holds**: Monitor ESMA updates on buy-in timelines and optional holds
  so conditional delivery (`HldInd`) aligns with the latest guidance.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Delivery-versus-Payment → `sese.023`

| DvP field                                              | ISO 20022 path                          | Notes |
|--------------------------------------------------------|----------------------------------------|-------|
| `settlement_id`                                        | `TxId`                                 | Stable lifecycle identifier |
| `delivery_leg.asset_definition_id` (security)          | `SctiesLeg/FinInstrmId`                | Canonical identifier (ISIN, CUSIP, …) |
| `delivery_leg.quantity`                                | `SctiesLeg/Qty`                        | Decimal string; honours asset precision |
| `payment_leg.asset_definition_id` (currency)           | `CashLeg/Ccy`                          | ISO currency code |
| `payment_leg.quantity`                                 | `CashLeg/Amt`                          | Decimal string; rounded per Numeric spec |
| `delivery_leg.from` (seller / delivering party)        | `DlvrgSttlmPties/Pty/Bic`              | BIC of delivering participant *(account canonical ID is currently exported in metadata)* |
| `delivery_leg.from` account identifier                 | `DlvrgSttlmPties/Acct`                 | Free-form; Norito metadata carries exact account ID |
| `delivery_leg.to` (buyer / receiving party)            | `RcvgSttlmPties/Pty/Bic`               | BIC of receiving participant |
| `delivery_leg.to` account identifier                   | `RcvgSttlmPties/Acct`                  | Free-form; matches receiving account ID |
| `plan.order`                                           | `Plan/ExecutionOrder`                  | Enum: `DELIVERY_THEN_PAYMENT` or `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity`                                       | `Plan/Atomicity`                       | Enum: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Message purpose**                                    | `SttlmTpAndAddtlParams/SctiesMvmntTp`  | `DELI` (deliver) or `RECE` (receive); mirrors which leg the submitting party executes. |
|                                                        | `SttlmTpAndAddtlParams/Pmt`            | `APMT` (against payment) or `FREE` (free-of-payment). |
| `delivery_leg.metadata`, `payment_leg.metadata`        | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Optional Norito JSON encoded as UTF‑8 |

> **Settlement qualifiers** – the bridge mirrors market practice by copying settlement condition codes (`SttlmTxCond`), partial settlement indicators (`PrtlSttlmInd`), and other optional qualifiers from Norito metadata into `sese.023/025` when present. Enforce the enumerations published in the ISO external code lists so the destination CSD recognises the values.

### Payment-versus-Payment Funding → `pacs.009`

The cash-for-cash legs that fund a PvP instruction are issued as FI-to-FI credit
transfers. The bridge annotates these payments so downstream systems recognise
they finance a securities settlement.

| PvP funding field                              | ISO 20022 path                                      | Notes |
|------------------------------------------------|-----------------------------------------------------|-------|
| `primary_leg.quantity` / {amount, currency}    | `IntrBkSttlmAmt` + `IntrBkSttlmCcy`                 | Amount/currency debited from the initiator. |
| Counterparty agent identifiers                 | `InstgAgt`, `InstdAgt`                              | BIC/LEI of sending and receiving agents. |
| Settlement purpose                             | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd`                  | Set to `SECU` for securities-related PvP funding. |
| Norito metadata (account ids, FX data)         | `CdtTrfTxInf/SplmtryData`                           | Carries full AccountId, FX timestamps, execution plan hints. |
| Instruction identifier / lifecycle linking     | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf`   | Matches the Norito `settlement_id` so the cash leg reconciles with the securities side. |

The JavaScript SDK’s ISO bridge aligns with this requirement by defaulting the
`pacs.009` category purpose to `SECU`; callers may override it with another
valid ISO code when emitting non-securities credit transfers, but invalid
values are rejected up front.

If an infrastructure requires an explicit securities confirmation, the bridge
continues to emit `sese.025`, but that confirmation reflects the securities leg
status (e.g., `ConfSts = ACCP`) rather than the PvP “purpose”.

### Payment-versus-Payment Confirmation → `sese.025`

| PvP field                                     | ISO 20022 path            | Notes |
|-----------------------------------------------|---------------------------|-------|
| `settlement_id`                               | `TxId`                    | Stable lifecycle identifier |
| `primary_leg.asset_definition_id`             | `SttlmCcy`                | Currency code for the primary leg |
| `primary_leg.quantity`                        | `SttlmAmt`                | Amount delivered by initiator |
| `counter_leg.asset_definition_id`             | `AddtlInf` (JSON payload) | Counter currency code embedded in supplemental info |
| `counter_leg.quantity`                        | `SttlmQty`                | Counter amount |
| `plan.order`                                  | `Plan/ExecutionOrder`     | Same enum set as DvP |
| `plan.atomicity`                              | `Plan/Atomicity`          | Same enum set as DvP |
| `plan.atomicity` status (`ConfSts`)           | `ConfSts`                 | `ACCP` when matched; bridge emits failure codes on rejection |
| Counterparty identifiers                      | `AddtlInf` JSON           | Current bridge serialises full AccountId/BIC tuples in metadata |

### Repo Collateral Substitution → `colr.007`

| Repo field / context                            | ISO 20022 path                     | Notes |
|-------------------------------------------------|-----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`)   | `OblgtnId`                        | Repo contract identifier |
| Collateral substitution Tx identifier           | `TxId`                            | Generated per substitution |
| Original collateral quantity                    | `Substitution/OriginalAmt`        | Matches pledged collateral before substitution |
| Original collateral currency                    | `Substitution/OriginalCcy`        | Currency code |
| Substitute collateral quantity                  | `Substitution/SubstituteAmt`      | Replacement amount |
| Substitute collateral currency                  | `Substitution/SubstituteCcy`      | Currency code |
| Effective date (governance margin schedule)     | `Substitution/EffectiveDt`        | ISO date (YYYY-MM-DD) |
| Haircut classification                          | `Substitution/Type`               | Currently `FULL` or `PARTIAL` based on governance policy |
| Governance reason / hair-cut note               | `Substitution/ReasonCd`           | Optional, carries governance rationale |

### Funding and Statements

| Iroha context                    | ISO 20022 message | Mapping location |
|----------------------------------|-------------------|------------------|
| Repo cash leg ignition / unwind  | `pacs.009`        | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` populated from DvP/PvP legs |
| Post-settlement statements       | `camt.054`        | Payment leg movements recorded under `Ntfctn/Ntry[*]`; bridge injects ledger/account metadata in `SplmtryData` |

### Usage Notes

* All amounts are serialised using the Norito numeric helpers (`NumericSpec`)
  to ensure scale conformance across asset definitions.
* `TxId` values are `Max35Text` — enforce UTF‑8 length ≤ 35 characters before
  exporting to ISO 20022 messages.
* BICs must be 8 or 11 uppercase alphanumeric characters (ISO 9362); reject
  Norito metadata that fails this check before emitting payments or settlement
  confirmations.
* Account identifiers (AccountId / ChainId) are exported into supplementary
  metadata so receiving participants can reconcile against their local ledgers.
* `SupplementaryData` must be canonical JSON (UTF‑8, sorted keys, JSON-native
  escaping). SDK helpers enforce this so signatures, telemetry hashes, and ISO
  payload archives remain deterministic across rebuilds.
* Currency amounts follow ISO 4217 fraction digits (for example JPY has 0
  decimals, USD has 2); the bridge clamps Norito numeric precision accordingly.
* The CLI settlement helpers (`iroha app settlement ... --atomicity ...`) now emit
  Norito instructions whose execution plans map 1:1 to `Plan/ExecutionOrder` and
  `Plan/Atomicity` above.
* The ISO helper (`ivm::iso20022`) validates the fields listed above and rejects
  messages where DvP/PvP legs violate Numeric specs or counterparty reciprocity.

### SDK Builder Helpers

- The JavaScript SDK now exposes `buildPacs008Message` /
  `buildPacs009Message` (see `javascript/iroha_js/src/isoBridge.js`) so client
  automation can convert structured settlement metadata (BIC/LEI, IBANs,
  purpose codes, supplementary Norito fields) into deterministic pacs XML
  without reimplementing the mapping rules from this guide.
- Both helpers require an explicit `creationDateTime` (ISO‑8601 with timezone)
  so operators must thread a deterministic timestamp from their workflow instead
  of letting the SDK default to wall-clock time.
- `recipes/iso_bridge_builder.mjs` demonstrates how to wire those helpers into
  a CLI that merges environment variables or JSON config files, prints the
  generated XML, and optionally submits it to Torii (`ISO_SUBMIT=1`), reusing
  the same wait cadence as the ISO bridge recipe.


### References

- LuxCSD / Clearstream ISO 20022 settlement examples showing `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) and `Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- Clearstream DCP specifications covering settlement qualifiers (`SttlmTxCond`, `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- SWIFT PMPG guidance recommending `pacs.009` with `CtgyPurp/Cd = SECU` for securities-related PvP funding.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- ISO 20022 message definition reports for identifier length constraints (BIC, Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ANNA DSB guidance on ISIN format and checksum rules.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### Usage Tips

- Always paste the relevant Norito snippet or CLI command so LLM can inspect
  exact field names and Numeric scales.
- Request citations (`provide clause references`) to keep a paper trail for
  compliance and auditor review.
- Capture the answer summary in `docs/source/finance/settlement_iso_mapping.md`
  (or linked appendices) so future engineers do not need to repeat the query.

## Event Ordering Playbooks (ISO 20022 ↔ Norito Bridge)

### Scenario A — Collateral Substitution (Repo / Pledge)

**Participants:** collateral giver/taker (and/or agents), custodian(s), CSD/T2S  
**Timing:** per market cut-offs and T2S day/night cycles; orchestrate the two legs so they complete within the same settlement window.

#### Message choreography
1. `colr.010` Collateral Substitution Request → collateral giver/taker or agent.  
2. `colr.011` Collateral Substitution Response → accept/reject (optional rejection reason).  
3. `colr.012` Collateral Substitution Confirmation → confirms substitution agreement.  
4. `sese.023` instructions (two legs):  
   - Return original collateral (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Deliver substitute collateral (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Link the pair (see below).  
5. `sese.024` status advices (accepted, matched, pending, failing, rejected).  
6. `sese.025` confirmations once booked.  
7. Optional cash delta (fees/haircut) → `pacs.009` FI-to-FI Credit Transfer with `CtgyPurp/Cd = SECU`; status via `pacs.002`, returns via `pacs.004`.

#### Required acknowledgements / statuses
- Transport level: gateways may emit `admi.007` or rejects before business processing.  
- Settlement lifecycle: `sese.024` (processing statuses + reason codes), `sese.025` (final).  
- Cash side: `pacs.002` (`PDNG`, `ACSC`, `RJCT` etc.), `pacs.004` for returns.

#### Conditionality / unwind fields
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) to chain the two instructions.  
- `SttlmParams/HldInd` to hold until criteria met; release via `sese.030` (`sese.031` status).  
- `SttlmParams/PrtlSttlmInd` to control partial settlement (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` for market-specific conditions (`NOMC`, etc.).  
- Optional T2S Conditional Securities Delivery (CoSD) rules when supported.

#### References
- SWIFT collateral management MDR (`colr.010/011/012`).  
- CSD/T2S usage guides (e.g., DNB, ECB Insights) for linking and statuses.  
- SMPG settlement practice, Clearstream DCP manuals, ASX ISO workshops.

### Scenario B — FX Window Breach (PvP Funding Failure)

**Participants:** counterparties and cash agents, securities custodian, CSD/T2S  
**Timing:** FX PvP windows (CLS/bilateral) and CSD cut-offs; keep securities legs on hold pending cash confirmation.

#### Message choreography
1. `pacs.009` FI-to-FI Credit Transfer per currency with `CtgyPurp/Cd = SECU`; status via `pacs.002`; recall/cancel via `camt.056`/`camt.029`; if already settled, `pacs.004` return.  
2. `sese.023` DvP instruction(s) with `HldInd=true` so the securities leg waits for cash confirmation.  
3. Lifecycle `sese.024` notices (accepted/matched/pending).  
4. If both `pacs.009` legs reach `ACSC` before the window expires → release with `sese.030` → `sese.031` (mod status) → `sese.025` (confirmation).  
5. If the FX window is breached → cancel/recall cash (`camt.056/029` or `pacs.004`) and cancel securities (`sese.020` + `sese.027`, or `sese.026` reversal if already confirmed per market rule).

#### Required acknowledgements / statuses
- Cash: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` for returns.  
- Securities: `sese.024` (pending/failing reasons like `NORE`, `ADEA`), `sese.025`.  
- Transport: `admi.007` / gateway rejects before business processing.

#### Conditionality / unwind fields
- `SttlmParams/HldInd` + `sese.030` release/cancel on success/failure.  
- `Lnkgs` to tie securities instructions to the cash leg.  
- T2S CoSD rule if using conditional delivery.  
- `PrtlSttlmInd` to prevent unintended partials.  
- On `pacs.009`, `CtgyPurp/Cd = SECU` flags securities-related funding.

#### References
- PMPG / CBPR+ guidance for payments in securities processes.  
- SMPG settlement practices, T2S insights on linking/holds.  
- Clearstream DCP manuals, ECMS documentation for maintenance messages.

### pacs.004 return mapping notes

- return fixtures now normalise `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) and proprietary return reasons exposed as `TxInf[*]/RtrdRsn/Prtry`, so bridge consumers can replay fee attribution and operator codes without re-parsing the XML envelope.
- AppHdr signature blocks inside `DataPDU` envelopes remain ignored on ingest; audits should rely on channel provenance rather than embedded XMLDSIG fields.

### Operational checklist for the bridge
- Enforce the choreography above (collateral: `colr.010/011/012 → sese.023/024/025`; FX breach: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- Treat `sese.024`/`sese.025` statuses and `pacs.002` outcomes as gating signals; `ACSC` triggers release, `RJCT` forces unwind.  
- Encode conditional delivery via `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond`, and optional CoSD rules.  
- Use `SupplementaryData` to correlate external IDs (e.g., UETR for the `pacs.009`) when required.  
- Parameterise hold/unwind timing by market calendar/cut-offs; issue `sese.030`/`camt.056` before cancellation deadlines, fallback to returns when necessary.

### Sample ISO 20022 Payloads (Annotated)

#### Collateral substitution pair (`sese.023`) with instruction linkage

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Submit the linked instruction `SUBST-2025-04-001-B` (FoP receive of substitute collateral) with `SctiesMvmntTp=RECE`, `Pmt=FREE`, and the `WITH` linkage pointing back to `SUBST-2025-04-001-A`. Release both legs with a matching `sese.030` once the substitution is approved.

#### Securities leg on hold pending FX confirmation (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Release once both `pacs.009` legs reach `ACSC`:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` confirms the hold release, followed by `sese.025` once the securities leg is booked.

#### PvP funding leg (`pacs.009` with securities purpose)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` tracks the payment status (`ACSC` = confirmed, `RJCT` = reject). If the window is breached, recall via `camt.056`/`camt.029` or send `pacs.004` to return settled funds.
