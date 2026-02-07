---
lang: hy
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

Այս էջը արտացոլում է ներքին հաշվարկային ՀՏՀ (`docs/source/nexus_settlement_faq.md`)
այնպես որ պորտալի ընթերցողները կարող են վերանայել նույն ուղեցույցը՝ առանց փորփրելու
մոնո-ռեպո. Այն բացատրում է, թե ինչպես է Settlement Router-ը մշակում վճարումները, ինչ չափումներ
վերահսկելու համար և ինչպես պետք է SDK-ները ինտեգրեն Norito օգտակար բեռները:

## Կարևորություններ

1. **Գոտիների քարտեզագրում** — յուրաքանչյուր տվյալների տարածություն հայտարարում է `settlement_handle`
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, կամ
   `xor_dual_fund`): Խորհրդակցեք վերջին գծերի կատալոգի տակ
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Deterministic conversion** — երթուղիչը բոլոր բնակավայրերը փոխակերպում է XOR-ի միջոցով
   կառավարման կողմից հաստատված իրացվելիության աղբյուրները: Մասնավոր ուղիների նախնական ֆինանսավորում XOR բուֆերներ;
   սանրվածքները կիրառվում են միայն այն դեպքում, երբ բուֆերները դուրս են մղվում քաղաքականությունից:
3. **Հեռաչափություն** — ժամացույց `nexus_settlement_latency_seconds`, փոխակերպման հաշվիչներ,
   և սանրվածքի չափիչներ։ Վահանակներն ապրում են `dashboards/grafana/nexus_settlement.json`-ում
   և ահազանգեր `dashboards/alerts/nexus_audit_rules.yml`-ում:
4. **Ապացույցներ** — արխիվային կազմաձևեր, երթուղիչի տեղեկամատյաններ, հեռաչափության արտահանումներ և
   աուդիտի համար հաշտեցման հաշվետվություններ:
5. **SDK-ի պարտականությունները** — յուրաքանչյուր SDK-ն պետք է բացահայտի բնակավայրերի օգնականներին, երթուղու ID-ներին,
   և Norito բեռնատար կոդավորիչներ՝ երթուղիչի հետ հավասարությունը պահպանելու համար:

## Օրինակ հոսքեր

| Գոտի տեսակը | Ապացույցներ գրավելու | Ինչ է դա ապացուցում |
|-----------|-------------------------------------|
| Մասնավոր `xor_hosted_custody` | Երթուղիչի մատյան + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC բուֆերները դեբետում են դետերմինիստական ​​XOR-ը, իսկ սանրվածքները մնում են քաղաքականության մեջ: |
| Հանրային `xor_global` | Ուղղորդիչի մատյան + DEX/TWAP հղում + հետաձգման/փոխակերպման չափումներ | Համատեղ իրացվելիության ուղին փոխադրումը գնահատեց հրապարակված TWAP-ով զրոյական սանրվածքով: |
| Հիբրիդ `xor_dual_fund` | Երթուղիչի մատյան, որը ցույց է տալիս հանրային և պաշտպանված պառակտում + հեռաչափական հաշվիչներ | Պաշտպանված/հանրային խառնուրդը հարգեց կառավարման գործակիցները և գրանցեց յուրաքանչյուր ոտքի վրա կիրառվող սանրվածքը: |

## Պետք է ավելի մանրամասն?

- Ամբողջական ՀՏՀ՝ `docs/source/nexus_settlement_faq.md`
- Հաշվարկային երթուղիչի սպեցիֆիկացիա՝ `docs/source/settlement_router.md`
- CBDC քաղաքականության խաղագիրք՝ `docs/source/cbdc_lane_playbook.md`
- Գործառնությունների մատյան՝ [Nexus գործողություններ] (./nexus-operations)