---
lang: hy
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Payload v1-ի թողարկման հաստատում (SDK Խորհուրդ, 2026-04-28):
//!
//! Գրում է SDK խորհրդի որոշման հուշագիրը, որը պահանջվում է `roadmap.md:M1`-ով, որպեսզի
//! գաղտնագրված օգտակար բեռը v1-ի թողարկումն ունի աուդիտի ենթակա գրառում (առաքվող M1.4):

# Payload v1 Տարածման որոշում (2026-04-28)

- **Նախագահ՝** SDK խորհրդի ղեկավար (Մ. Տակեմիա)
- **Քվեարկող անդամներ.** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Դիտորդներ.** Ծրագիր Mgmt, Telemetry Ops

## Մուտքագրումները վերանայվել են

1. **Swift bindings & submitters** — `ShieldRequest`/`UnshieldRequest`, async ներկայացնողներ և Tx builder օգնականներ, որոնք վայրէջք կատարեցին հավասարության թեստերով և փաստաթղթեր:
2. **CLI էրգոնոմիկա** — `iroha app zk envelope` օգնականն ընդգրկում է կոդավորման/ստուգման աշխատանքային հոսքերը, գումարած խափանումների ախտորոշումը, որը համահունչ է ճանապարհային քարտեզի էրգոնոմիկայի պահանջներին։【crates/iroha_cli/src/zk.rs:1256】
3. **Դետերմինիստական հարմարանքներ և հավասարաչափ փաթեթներ** — ընդհանուր սարք + Rust/Swift վավերացում՝ Norito բայթ/սխալի մակերեսները պահելու համար aligned.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Որոշում

- **Հաստատեք SDK-ի և CLI-ի համար օգտակար բեռնվածք v1-ի թողարկումը**՝ հնարավորություն տալով Swift դրամապանակներին ստեղծել գաղտնի ծրարներ՝ առանց պատվերով սանտեխնիկայի:
- **Պայմանները:** 
  - Պահպանեք հավասարաչափ սարքերը CI դրեյֆի ազդանշանների ներքո (կապված `scripts/check_norito_bindings_sync.py`-ի հետ):
  - Փաստաթղթավորեք գործառնական գրքույկը `docs/source/confidential_assets.md`-ում (արդեն թարմացվել է Swift SDK PR-ի միջոցով):
  - Ձայնագրեք տրամաչափումը + հեռաչափության ապացույցները՝ նախքան արտադրության որևէ դրոշակ շուռ տալը (հետևվում է M2-ի տակ):

## Գործողությունների կետեր

| Սեփականատեր | Նյութ | Ժամկետային |
|-------|------|-----|
| Արագ կապար | Հայտարարեք GA-ի առկայությունը + README հատվածներ | 2026-05-01 |
| CLI Maintainer | Ավելացնել `iroha app zk envelope --from-fixture` օգնական (ըստ ցանկության) | Հետադարձ (ոչ արգելափակող) |
| DevRel WG | Թարմացրեք դրամապանակի արագ մեկնարկները payload v1 հրահանգներով | 2026-05-05 |

> **Նշում.** Այս հուշագիրը փոխարինում է `roadmap.md:2426`-ի ժամանակավոր «սպասող խորհրդի հաստատմանը» կոչը և բավարարում է որոնիչի M1.4 կետը: Թարմացրեք `status.md`-ը, երբ հետագա գործողությունների կետերը փակվեն: