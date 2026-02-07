---
id: preview-feedback-w1-plan
lang: hy
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| Նյութ | Մանրամասն |
| --- | --- |
| Ալիք | W1 — Գործընկերներ և Torii ինտեգրատորներ |
| Թիրախային պատուհան | Q2 2025 շաբաթ 3 |
| Արտեֆակտի պիտակ (պլանավորված) | `preview-2025-04-12` |
| Հետագծողի խնդիր | `DOCS-SORA-Preview-W1` |

## Նպատակներ

1. Ապահովեք իրավական + կառավարման հաստատումներ գործընկերների նախադիտման պայմանների համար:
2. Բեմադրեք «Փորձեք այն» վստահված անձի և հեռաչափության պատկերները, որոնք օգտագործվում են հրավերի փաթեթում:
3. Թարմացրեք ստուգիչ գումարի կողմից հաստատված նախադիտման արտեֆակտը և հետաքննության արդյունքները:
4. Ավարտեք գործընկերների ցուցակը + հարցումների ձևանմուշները՝ նախքան հրավերներ ուղարկելը:

## Առաջադրանքի բաշխում

| ID | Առաջադրանք | Սեփականատեր | Ժամկետային | Կարգավիճակը | Ծանոթագրություններ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Ստացեք օրինական հաստատում նախադիտման պայմանների հավելվածի համար | Docs/DevRel առաջատար → Իրավական | 2025-04-05 | ✅ Ավարտված | Օրինական տոմս `DOCS-SORA-Preview-W1-Legal` ստորագրված է 2025-04-05; PDF կցված է թրեքերին: |
| W1-P2 | Լուսանկարել Փորձեք այն պրոքսի բեմադրման պատուհանը (2025-04-10) և վավերացնել վստահված անձի առողջությունը | Docs/DevRel + Ops | 2025-04-06 | ✅ Ավարտված | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` կատարված 2025-04-06; CLI տառադարձում + `.env.tryit-proxy.bak` արխիվացված: |
| W1-P3 | Կառուցեք նախադիտման արտեֆակտ (`preview-2025-04-12`), գործարկեք `scripts/preview_verify.sh` + `npm run probe:portal`, արխիվային նկարագրիչ/ստուգիչ գումարներ | Պորտալ TL | 2025-04-08 | ✅ Ավարտված | Արտեֆակտ + ստուգման մատյաններ, որոնք պահվում են `artifacts/docs_preview/W1/preview-2025-04-12/`-ում; զոնդի ելքը կցված է որոնիչին: |
| W1-P4 | Վերանայեք գործընկերների ընդունման ձևերը (`DOCS-SORA-Preview-REQ-P01…P08`), հաստատեք կոնտակտները + NDAs | Կառավարման կապ | 2025-04-07 | ✅ Ավարտված | Բոլոր ութ հարցումները հաստատվել են (վերջին երկուսը մաքրվել են 2025-04-11 թվականներին); հաստատումները կապված են թրեքերում: |
| W1-P5 | Հրավերի սևագիր պատճենը (հիմնված `docs/examples/docs_preview_invite_template.md`-ի վրա), հավաքածու `<preview_tag>` և `<request_ticket>` յուրաքանչյուր գործընկերոջ համար | Docs/DevRel առաջատար | 2025-04-08 | ✅ Ավարտված | Հրավերի նախագիծն ուղարկվել է 2025-04-12 15:00UTC՝ արտեֆակտ հղումների հետ մեկտեղ: |

## Նախնական թռիչքի ստուգաթերթ

> Հուշում. գործարկեք `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`-ը՝ 1-5-րդ քայլերն ավտոմատ կերպով կատարելու համար (կառուցում, ստուգիչ գումարի ստուգում, պորտալի ստուգում, հղումների ստուգում և «Փորձեք այն» վստահված անձի թարմացում): Սցենարը գրանցում է JSON մատյան, որը կարող եք կցել հետագծման խնդրին:

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`-ով)՝ վերականգնելու `build/checksums.sha256` և `build/release.json`:
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` և արխիվացնել `build/link-report.json` նկարագրիչի կողքին:
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (կամ տրամադրել համապատասխան թիրախ `--tryit-target`-ի միջոցով); միացրեք թարմացված `.env.tryit-proxy`-ը և պահեք `.bak`-ը հետդարձի համար:
6. Թարմացրեք W1 թրեյքերի խնդիրը գրանցամատյանների ուղիների հետ (նկարագրիչի ստուգիչ գումար, զոնդերի ելք, փորձեք այն վստահված անձի փոփոխություն, Grafana նկարներ):

## Ապացույցների ստուգաթերթ

- [x] Ստորագրված օրինական հաստատում (PDF կամ տոմսի հղում)՝ կցված `DOCS-SORA-Preview-W1`-ին:
- [x] Grafana սքրինշոթներ `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` համար:
- [x] `preview-2025-04-12` նկարագրիչ + ստուգիչ գումարի մատյան, որը պահվում է `artifacts/docs_preview/W1/`-ում:
- [x] Հրավիրեք ցուցակի աղյուսակը `invite_sent_at` ժամադրոշմներով (տե՛ս հետագծող W1 մատյան):
- [x] Հետադարձ կապի արտեֆակտները արտացոլված են [`preview-feedback/w1/log.md`]-ում (./log.md) մեկ տողով՝ յուրաքանչյուր գործընկերոջ համար (թարմացվել է 2025-04-26՝ ցուցակի/հեռաչափության/թողարկման տվյալների հետ):

Թարմացրեք այս պլանը՝ առաջադրանքների առաջընթացի հետ մեկտեղ. որոնիչը հղում է անում դրան՝ ճանապարհային քարտեզը պահելու համար
աուդիտի ենթարկվող.

## Հետադարձ աշխատանքի հոսք

1. Յուրաքանչյուր գրախոսի համար կրկնօրինակեք ձևանմուշը
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   լրացրեք մետատվյալները և պահեք ավարտված պատճենը տակ
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Ամփոփեք հրավերները, հեռաչափության անցակետերը և բաց հարցերը ուղիղ մատյանում
   [`preview-feedback/w1/log.md`](./log.md), որպեսզի կառավարման վերանայողները կարողանան վերարտադրել ամբողջ ալիքը
   առանց շտեմարանից դուրս գալու:
3. Երբ գիտելիքի ստուգման կամ հետազոտության արտահանումները հասնեն, կցեք դրանք մատյանում նշված արտեֆակտի ճանապարհին
   և խաչաձև կապեք հետախույզի խնդիրը: