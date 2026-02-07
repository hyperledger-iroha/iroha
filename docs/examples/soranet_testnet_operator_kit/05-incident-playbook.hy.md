---
lang: hy
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Կրճատել արձագանքման գրքույկ

1. **Հայտնաբերել**
   - Զգուշացում `soranet_privacy_circuit_events_total{kind="downgrade"}` հրդեհների կամ
     brownout webhook triggers է կառավարման.
   - Հաստատեք `kubectl logs soranet-relay` կամ համակարգված ամսագրի միջոցով 5 րոպեի ընթացքում:

2. **Կայունացնել**
   - Սառեցման պահակի ռոտացիա (`relay guard-rotation disable --ttl 30m`):
   - Ազդեցության ենթարկված հաճախորդների համար միացնել միայն ուղղակի անտեսումը
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`):
   - Ներգրավել ընթացիկ համապատասխանության կազմաձևերի հեշը (`sha256sum compliance.toml`):

3. **Ախտորոշում**
   - Հավաքեք վերջին գրացուցակի նկարը և ռելեի չափման փաթեթը.
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Ուշադրություն դարձրեք PoW հերթի խորությանը, շնչափողի հաշվիչներին և GAR կատեգորիայի հասկերին:
   - Բացահայտեք՝ արդյոք PQ-ի դեֆիցիտը, համապատասխանության խախտումը կամ ռելեի ձախողումը առաջացրել են իրադարձությունը:

4. **Աճել**
   - Տեղեկացրեք կառավարման կամուրջին (`#soranet-incident`) ամփոփագրի և փաթեթի հեշի միջոցով:
   - Բացեք միջադեպի տոմսը, որը կապում է ահազանգին, ներառյալ ժամադրոշմները և մեղմացման քայլերը:

5. **Վերականգնել **
   - Երբ արմատական պատճառն ուղղվի, նորից միացրեք ռոտացիան
     (`relay guard-rotation enable`) և ետ վերադարձնել միայն ուղղակի անտեսումները:
   - Վերահսկել KPI-ները 30 րոպե; համոզվեք, որ նոր այտուցներ չհայտնվեն:

6. **Հետմահվան**
   - Ներկայացրեք միջադեպի մասին հաշվետվություն 48 ժամվա ընթացքում՝ օգտագործելով կառավարման ձևանմուշը:
   - Թարմացրեք runbooks-ը, եթե հայտնաբերվի ձախողման նոր ռեժիմ: