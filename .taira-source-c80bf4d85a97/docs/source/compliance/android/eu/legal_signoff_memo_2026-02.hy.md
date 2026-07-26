---
lang: hy
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 ԵՄ իրավական գրանցման հուշագիր — 2026.1 GA (Android SDK)

## Ամփոփում

- **Թողարկում / Գնացք՝ ** 2026.1 GA (Android SDK)
- **Վերանայման ամսաթիվ՝ ** 2026-04-15
- **Խորհրդական / Գրախոս.** Սոֆյա Մարտինս — Համապատասխանություն և իրավական
- **Ծավալը.** ETSI EN 319 401 անվտանգության թիրախ, GDPR DPIA ամփոփում, SBOM ատեստավորում, AND6 սարքի լաբորատոր անկանխատեսելի ապացույցներ
- **Հարակից տոմսեր.

## Արտեֆակտ ստուգաթերթ

| Արտեֆակտ | SHA-256 | Գտնվելու վայրը / Հղում | Ծանոթագրություններ |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Համապատասխանում է 2026.1 GA թողարկման նույնացուցիչներին և սպառնալիքների մոդելի դելտաներին (Torii NRPC հավելումներ): |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Հղումներ AND7 հեռաչափության քաղաքականություն (`docs/source/sdk/android/telemetry_redaction.md`): |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore փաթեթ (`android-sdk-release#4821`): | CycloneDX + ծագման վերանայում; համապատասխանում է Buildkite աշխատանքին `android-sdk-release#4821`: |
| Ապացույցների մատյան | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (տող `android-device-lab-failover-20260220`) | Հաստատում է գրանցամատյանում նկարահանված փաթեթի հեշերը + հզորության ակնթարթային նկարը + հուշագրի մուտքագրումը: |
| Սարք-լաբորատոր անսպասելիության փաթեթ | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json`-ից վերցված հեշ; տոմս AND6-DR-202602 գրանցված հանձնում իրավական/Համապատասխանություն: |

## Գտածոներ և բացառություններ

- Արգելափակման խնդիրներ չեն հայտնաբերվել: Արտեֆակտները համապատասխանում են ETSI/GDPR պահանջներին. AND7 հեռաչափության հավասարությունը նշված է DPIA ամփոփագրում և լրացուցիչ մեղմացումներ չեն պահանջվում:
- Առաջարկություն. վերահսկեք պլանավորված DR-2026-05-Q2 վարժանքը (տոմս AND6-DR-202605) և ստացված փաթեթը կցեք ապացույցների մատյանին մինչև հաջորդ կառավարման անցակետը:

## Հաստատում

- **Որոշում.** Հաստատված է
- **Ստորագրություն / Ժամացույց.** _Sofia Martins (թվային ստորագրված կառավարման պորտալի միջոցով, 2026-04-15 14:32 UTC)_
- **Հետևող սեփականատերեր.** Device Lab Ops (առաքել DR-2026-05-Q2 ապացույցների փաթեթը մինչև 2026-05-31)