---
lang: hy
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI Նմուշային հարմարանքներ

Այս գրացուցակը փաթեթավորում է նմուշից առաջացած դետերմինիստական արտեֆակտներ
ծանրաբեռնվածություն `fixtures/sorafs_manifest/ci_sample/`-ի տակ: Փաթեթը ցույց է տալիս
վերջից մինչև վերջ SoraFS փաթեթավորման և ստորագրման խողովակաշար, որն իրականացնում է CI աշխատանքային հոսքերը:

## Արտեֆակտի գույքագրում

| Ֆայլ | Նկարագրություն |
|------|-------------|
| `payload.txt` | Աղբյուրի օգտակար բեռը, որն օգտագործվում է հարմարանքների սցենարների կողմից (պարզ տեքստային նմուշ): |
| `payload.car` | Մեքենայի արխիվ՝ թողարկված `sorafs_cli car pack`-ի կողմից: |
| `car_summary.json` | Ամփոփում, որը ստեղծվել է `car pack`-ի կողմից՝ ֆիքսելով մասնաբաժինները և մետատվյալները: |
| `chunk_plan.json` | Fetch-plan JSON, որը նկարագրում է մասնաբաժնի միջակայքերը և մատակարարի ակնկալիքները: |
| `manifest.to` | Norito մանիֆեստ՝ արտադրված `sorafs_cli manifest build`-ի կողմից: |
| `manifest.json` | Մարդկանց համար ընթեռնելի մանիֆեստի ձևավորում վրիպազերծման համար: |
| `proof.json` | PoR ամփոփագիր՝ թողարկված `sorafs_cli proof verify`-ի կողմից: |
| `manifest.bundle.json` | `sorafs_cli manifest sign`-ի կողմից ստեղծված առանց բանալի ստորագրության փաթեթ: |
| `manifest.sig` | Անջատված Ed25519 ստորագրությունը, որը համապատասխանում է մանիֆեստին: |
| `manifest.sign.summary.json` | Ստորագրման ընթացքում թողարկված CLI ամփոփագիր (հեշ, փաթեթի մետատվյալներ): |
| `manifest.verify.summary.json` | CLI ամփոփագիր `manifest verify-signature`-ից: |

Թողարկման նշումներում և փաստաթղթերում հիշատակված բոլոր ամփոփումները սկզբնաղբյուր են եղել
այս ֆայլերը: `ci/check_sorafs_cli_release.sh` աշխատանքային հոսքը վերականգնում է նույնը
արտեֆակտները և դրանք տարբերում է կատարված տարբերակներից։

## Սարքավորումների վերականգնում

Գործարկեք ստորև նշված հրամանները պահեստի արմատից՝ հարմարանքների հավաքածուն վերականգնելու համար:
Դրանք արտացոլում են `sorafs-cli-fixture` աշխատանքային հոսքի կողմից օգտագործվող քայլերը.

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Եթե որևէ քայլ առաջացնում է տարբեր հեշեր, ուսումնասիրեք նախքան հարմարանքները թարմացնելը:
CI աշխատանքային հոսքերը հիմնված են դետերմինիստական ​​արդյունքի վրա՝ ռեգրեսիաները հայտնաբերելու համար:

## Ապագա ծածկույթ

Քանի որ լրացուցիչ chunker պրոֆիլները և ապացույցների ձևաչափերը ավարտում են ճանապարհային քարտեզը,
դրանց կանոնական սարքերը կավելացվեն այս գրացուցակի տակ (օրինակ՝
`sorafs.sf2@1.0.0` (տես `fixtures/sorafs_manifest/ci_sample_sf2/`) կամ PDP
հոսքային ապացույցներ): Յուրաքանչյուր նոր պրոֆիլ կհետևի նույն կառուցվածքին` բեռնատար, մեքենա,
պլան, մանիֆեստ, ապացույցներ և ստորագրության արտեֆակտներ, այնպես որ ներքևի ավտոմատացումը կարող է
diff թողարկումներ առանց հատուկ սկրիպտավորման: