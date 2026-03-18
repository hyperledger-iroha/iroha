---
lang: hy
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

Օգտագործեք այս համառոտագիրը՝ SNNet-9-ի համապատասխանության կոնֆիգուրացիան կրկնվող,
աուդիտի համար հարմար գործընթաց: Զուգակցեք այն իրավասության վերանայման հետ, որպեսզի յուրաքանչյուր օպերատոր
օգտագործում է նույն ամփոփումները և ապացույցների դասավորությունը:

## Քայլեր

1. **Հավաքեք կազմաձևը**
   - Ներմուծեք `governance/compliance/soranet_opt_outs.json`:
   - Միավորեք ձեր `operator_jurisdictions`-ը հրապարակված ատեստավորման ամփոփագրերի հետ
     [իրավասության վերանայման] մեջ (gar-jurisdictional-review):
2. **Վավերացնել**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Լրացուցիչ՝ `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Ձեռք բերեք ապացույցներ**
   - Պահպանեք `artifacts/soranet/compliance/<YYYYMMDD>/` տակ:
     - `config.json` (վերջնական համապատասխանության բլոկ)
     - `attestations.json` (URIs + digests)
     - վավերացման տեղեկամատյաններ
     - հղումներ ստորագրված PDF-ների/Norito ծրարներին
4. **Ակտիվացնել**
   - Նշեք թողարկումը (`gar-opt-out-<date>`), վերաբաշխեք նվագախմբի/SDK կազմաձևերը,
     և հաստատեք, որ `compliance_*` իրադարձությունները թողարկվեն տեղեկամատյաններում, որտեղ սպասվում էր:
5. **Փակել**
   - Ներկայացրեք ապացույցների փաթեթը Կառավարման խորհրդին:
   - Մուտքագրեք ակտիվացման պատուհանը + հաստատողներին GAR մատյանում:
   - Պլանավորեք հաջորդ վերանայման ամսաթվերը իրավասության վերանայման աղյուսակից:

## Արագ ստուգաթերթ

- [ ] `jurisdiction_opt_outs`-ը համապատասխանում է կանոնական կատալոգին:
- [ ] Ատեստավորման ամփոփագրերը ճշգրիտ պատճենված են:
- [ ] Վավերացման հրամանները գործարկվում և արխիվացվում են:
- [ ] Ապացույցների փաթեթը պահվում է `artifacts/soranet/compliance/<date>/`-ում:
- [ ] Տեղադրման պիտակը + GAR մատյանը թարմացվել է:
- [ ] Հաջորդ վերանայման հիշեցումները սահմանված են:

## Տես նաև

- [GAR իրավասության վերանայում] (gar-jurisdictional-review)
- [GAR Compliance Playbook (աղբյուր)] (../../../source/soranet/gar_compliance_playbook.md)