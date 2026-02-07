---
id: training-collateral
lang: hy
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Հայելիներ `docs/source/sns/training_collateral.md`. Օգտագործեք այս էջը ճեպազրույցի ժամանակ
> գրանցող, DNS, խնամակալ և ֆինանսական թիմեր յուրաքանչյուր վերջածանցի գործարկումից առաջ:

## 1. Ուսումնական պլանի պատկեր

| Հետևել | Նպատակները | Նախնական ընթերցումներ |
|-------|------------|-----------|
| Գրանցողի գործեր | Ներկայացրեք մանիֆեստներ, վերահսկեք KPI վահանակները, ավելացրեք սխալները: | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & gateway | Կիրառեք լուծիչի կմախքներ, փորձեք սառեցում/վերադարձ: | `sorafs/gateway-dns-runbook`, ուղղակի ռեժիմի քաղաքականության նմուշներ: |
| Խնամակալներ և խորհուրդ | Իրականացնել վեճերը, թարմացնել կառավարման հավելումները, գրանցման հավելվածները: | `sns/governance-playbook`, ստյուարդի գնահատականներ: |
| Ֆինանսներ և վերլուծություն | Ձեռք բերեք ARPU/մեծ չափումներ, հրապարակեք հավելվածների փաթեթներ: | `finance/settlement-iso-mapping`, KPI վահանակ JSON: |

### Մոդուլի հոսք

1. **M1 — KPI կողմնորոշում (30 րոպե):** Քայլելու վերջածանցի զտիչներ, արտահանումներ և փախուստ
   սառեցնել հաշվիչներ. Առաքման հնարավորություն՝ PDF/CSV ակնարկներ SHA-256 դիջիստով:
2. **M2 — մանիֆեստի կյանքի ցիկլը (45 րոպե):** Ստեղծել և հաստատել գրանցման մանիֆեստները,
   ստեղծեք լուծիչի կմախքներ `scripts/sns_zonefile_skeleton.py`-ի միջոցով: Առաքվող:
   git diff-ը ցույց է տալիս կմախք + ԳԱՐ ապացույց:
3. **M3 — Վեճերի վարժություններ (40 րոպե):** Խնամակալների սառեցում + բողոքարկում, գրավում
   guardian CLI-ի տեղեկամատյանները `artifacts/sns/training/<suffix>/<cycle>/logs/`-ի տակ:
4. **M4 — Հավելվածի նկարահանում (25 րոպե):** Արտահանեք JSON վահանակը և գործարկեք.

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Առաքում. թարմացված հավելված Markdown + կարգավորիչ + պորտալի հուշագրությունների բլոկներ:

## 2. Տեղայնացման աշխատանքային հոսք

- Լեզուներ.
- Յուրաքանչյուր թարգմանություն ապրում է աղբյուրի ֆայլի կողքին
  (`docs/source/sns/training_collateral.<lang>.md`): Թարմացրեք `status` +
  `translation_last_reviewed` թարմացումից հետո:
- Ակտիվները մեկ լեզվի տակ են
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (սլայդներ/, աշխատանքային գրքույկներ/,
  ձայնագրություններ/, տեղեկամատյաններ/):
- Անգլերենը խմբագրելուց հետո գործարկեք `python3 scripts/sync_docs_i18n.py --lang <code>`-ը
  աղբյուր, որպեսզի թարգմանիչները տեսնեն նոր հեշը:

### Առաքման ստուգաթերթ

1. Թարմացրեք թարգմանության կոճակը (`status: complete`) տեղայնացվելուց հետո:
2. Արտահանեք սլայդները PDF և վերբեռնեք յուրաքանչյուր լեզվով `slides/` գրացուցակում:
3. Արձանագրել ≤10 րոպե KPI քայլք; հղումը լեզվի անավարտից։
4. Ֆայլի կառավարման տոմս՝ հատկորոշված `sns-training`, որը պարունակում է սլայդ/աշխատանքային գրքույկ
   մարսողություններ, ձայնագրման հղումներ և հավելվածների ապացույցներ:

## 3. Վերապատրաստման ակտիվներ

- Սլայդի ուրվագիծը՝ `docs/examples/sns_training_template.md`:
- Աշխատանքային գրքույկի ձևանմուշ՝ `docs/examples/sns_training_workbook.md` (մեկը յուրաքանչյուր մասնակցի համար):
- Հրավիրել + հիշեցումներ՝ `docs/examples/sns_training_invite_email.md`:
- Գնահատման ձև՝ `docs/examples/sns_training_eval_template.md` (պատասխաններ
  արխիվացված `artifacts/sns/training/<suffix>/<cycle>/feedback/` տակ):

## 4. Ժամանակացույց և չափումներ

| Ցիկլ | Պատուհան | Չափիչ | Ծանոթագրություններ |
|-------|--------|---------|-------|
| 2026-03 | Տեղադրել KPI վերանայում | Հաճախումների %, հավելվածի ամփոփում գրանցված | `.sora` + `.nexus` խմբեր |
| 2026-06 | Pre `.dao` GA | Ֆինանսական պատրաստվածություն ≥90% | Ներառել քաղաքականության թարմացում |
| 2026-09 | Ընդարձակում | Վեճային վարժանք <20 րոպե, հավելված SLA ≤2 օր | Համապատասխանեցվել SN-7 խթաններին |

Ստացեք անանուն արձագանք `docs/source/sns/reports/sns_training_feedback.md`-ում
այնպես որ հետագա խմբերը կարող են բարելավել տեղայնացումը և լաբորատորիաները: