---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
:::

## Նպատակը

Այս գրքույկը առաջնորդում է կառավարման օպերատորներին՝ ներկայացնելով SoraFS հզորության վեճերը, համակարգելով չեղյալ հայտարարումները և ապահովելով տվյալների տարհանումը վճռականորեն ավարտված:

## 1. Գնահատեք միջադեպը

- ** Գործարկման պայմանները. ** SLA-ի խախտման հայտնաբերում (ժամանակի/PoR ձախողում), կրկնօրինակման անբավարարություն կամ հաշվարկային անհամաձայնություն:
- **Հաստատեք հեռաչափությունը.** նկարահանեք `/v1/sorafs/capacity/state` և `/v1/sorafs/capacity/telemetry` նկարներ մատակարարի համար:
- **Տեղեկացնել շահագրգիռ կողմերին.** Պահպանման թիմին (մատակարարի գործառնություններ), Կառավարման խորհուրդ (որոշման մարմին), դիտարկելիություն (վահանակի թարմացումներ):

## 2. Պատրաստեք ապացույցների փաթեթ

1. Հավաքեք չմշակված արտեֆակտներ (հեռաչափություն JSON, CLI տեղեկամատյաններ, աուդիտորական նշումներ):
2. Նորմալացնել դետերմինիստական ​​արխիվի մեջ (օրինակ՝ թարբոլի); գրառում:
   - BLAKE3-256 digest (`evidence_digest`)
   - Մեդիա տեսակը (`application/zip`, `application/jsonl` և այլն)
   - Հոսթինգ URI (օբյեկտների պահեստ, SoraFS փին կամ Torii-հասանելի վերջնակետ)
3. Պահպանեք փաթեթը կառավարման ապացույցների հավաքագրման դույլում՝ գրելու մեկ անգամ մուտքով:

## 3. Ներկայացրե՛ք վեճը

1. Ստեղծեք սպեցիֆիկ JSON `sorafs_manifest_stub capacity dispute`-ի համար.

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Գործարկեք CLI-ը.

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Վերանայեք `dispute_summary.json` (հաստատեք տեսակը, ապացույցների ամփոփումը, ժամանակի դրոշմները):
4. Ներկայացրե՛ք JSON հարցումը Torii `/v1/sorafs/capacity/dispute`-ին կառավարման գործարքների հերթի միջոցով: Գրեք `dispute_id_hex` պատասխանի արժեքը; այն խարսխում է չեղյալ համարելու հետագա գործողությունները և աուդիտի հաշվետվությունները:

## 4. Տարհանում և չեղարկում

1. **Grace պատուհան.** տեղեկացնել մատակարարին մոտալուտ չեղարկման մասին; թույլատրել ամրացված տվյալների տարհանումը, երբ քաղաքականությունը թույլ է տալիս:
2. **Ստեղծեք `ProviderAdmissionRevocationV1`:**
   - Օգտագործեք `sorafs_manifest_stub provider-admission revoke` հաստատված պատճառով:
   - Ստուգեք ստորագրությունները և չեղյալ համարելը:
3. **Հրապարակել չեղյալ հայտարարումը.**
   - Ներկայացրե՛ք չեղյալ համարելու հայցը Torii հասցեին:
   - Համոզվեք, որ մատակարարի գովազդներն արգելափակված են (ակնկալեք, որ `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` բարձրանա):
4. **Թարմացրեք վահանակները.** դրոշակեք մատակարարին որպես չեղարկված, հղում կատարեք վեճի ID-ին և կապեք ապացույցների փաթեթը:

## 5. Հետմահու և հետագա հետազոտություն

- Գրանցեք ժամանակացույցը, հիմնական պատճառն ու վերացման գործողությունները կառավարման միջադեպերի հետագծում:
- Որոշեք փոխհատուցումը (ցիցերի կրճատում, վճարների վերադարձ, հաճախորդների փոխհատուցում):
- Փաստաթղթային ուսուցում; անհրաժեշտության դեպքում թարմացնել SLA շեմերը կամ մոնիտորինգի ազդանշանները:

## 6. Տեղեկատվական նյութեր

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (վեճերի բաժին)
- `docs/source/sorafs/provider_admission_policy.md` (չեղարկման աշխատանքային հոսք)
- Դիտորդական վահանակ՝ `SoraFS / Capacity Providers`

## ստուգաթերթ

- [ ] Ապացույցների փաթեթը գրավված և հաշված է:
- [ ] Վեճերի ծանրաբեռնվածությունը հաստատված է տեղում:
- [ ] Torii վեճի գործարքն ընդունված է:
- [ ] Չեղյալ հայտարարումն իրականացվել է (եթե հաստատվել է):
- [ ] Վահանակները/վազքագրքերը թարմացվել են:
- [ ] Հետմահու ներկայացվել է կառավարման խորհրդին: