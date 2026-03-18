---
id: chunker-conformance
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Կանոնական աղբյուր
:::

Այս ուղեցույցը կոդավորում է այն պահանջները, որոնք պետք է հետևի յուրաքանչյուր իրականացում, որպեսզի մնա
համահունչ SoraFS դետերմինիստական chunker պրոֆիլին (SF1): Այն նաև
փաստաթղթավորում է վերականգնման աշխատանքների ընթացքը, ստորագրման քաղաքականությունը և ստուգման քայլերը
հարմարանքների սպառողները SDK-ներում մնում են համաժամեցված:

## Կանոնական պրոֆիլ

- Պրոֆիլի բռնակ՝ `sorafs.sf1@1.0.0`
- Մուտքային սերմ (վեցանկյուն)՝ `0000000000dec0ded`
- Թիրախային չափը՝ 262144 բայթ (256 ԿԲ)
- Նվազագույն չափը՝ 65536 բայթ (64 ԿԲ)
- Առավելագույն չափը՝ 524288 բայթ (512 ԿԲ)
- Գլորվող բազմանդամ՝ `0x3DA3358B4DC173`
- Փոխանցման սեղանի սերմ` `sorafs-v1-gear`
- Ընդմիջման դիմակ՝ `0x0000FFFF`

Հղումների իրականացում՝ `sorafs_chunker::chunk_bytes_with_digests_profile`:
Ցանկացած SIMD արագացում պետք է առաջացնի նույնական սահմաններ և մարսումներ:

## հարմարանքների փաթեթ

`cargo run --locked -p sorafs_chunker --bin export_vectors`-ը վերածնում է
հարմարեցնում և թողարկում է հետևյալ ֆայլերը `fixtures/sorafs_chunker/`-ի ներքո.

- `sf1_profile_v1.{json,rs,ts,go}` - կանոնական հատվածի սահմաններ Rust-ի համար,
  TypeScript և Go սպառողներ: Յուրաքանչյուր ֆայլ գովազդում է կանոնական բռնակը որպես
  առաջին (և միակ) մուտքը `profile_aliases`-ում: Պատվերը կատարվում է
  `ensure_charter_compliance` և ՊԵՏՔ ՉԻ փոփոխվի:
- `manifest_blake3.json` — BLAKE3-ով հաստատված մանիֆեստ, որը ծածկում է բոլոր հարմարանքների ֆայլը:
- `manifest_signatures.json` — Խորհրդի ստորագրություններ (Ed25519) մանիֆեստի վրա
  մարսել.
- `sf1_profile_v1_backpressure.json` և հում մարմիններ `fuzz/`-ի ներսում —
  դետերմինիստական հոսքային սցենարներ, որոնք օգտագործվում են chunker back-pressure tests-ի կողմից:

### Ստորագրման քաղաքականություն

Սարքավորումների վերականգնումը **պետք է** ներառի խորհրդի վավեր ստորագրություն: Գեներատորը
մերժում է անստորագիր ելքը, քանի դեռ `--allow-unsigned`-ը հստակ չի փոխանցվել (նախատեսված
միայն տեղական փորձերի համար): Ստորագրության ծրարները միայն կցվում են և
կրկնօրինակված է յուրաքանչյուր ստորագրողի համար:

Ավագանու ստորագրություն ավելացնելու համար.

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Ստուգում

CI օգնականը `ci/check_sorafs_fixtures.sh` կրկնում է գեներատորը
`--locked`. Եթե ​​հարմարանքները շեղվում են կամ ստորագրությունները բացակայում են, ապա աշխատանքը ձախողվում է: Օգտագործեք
այս սցենարը գիշերային աշխատանքային հոսքերում և նախքան հարմարանքների փոփոխությունները ներկայացնելը:

Ձեռքով ստուգման քայլեր.

1. Գործարկել `cargo test -p sorafs_chunker`:
2. Տեղայնորեն կանչեք `ci/check_sorafs_fixtures.sh`:
3. Հաստատեք, որ `git status -- fixtures/sorafs_chunker`-ը մաքուր է:

## Թարմացրեք Playbook-ը

Նոր chunker պրոֆիլ առաջարկելիս կամ SF1-ը թարմացնելիս.

Տես նաև՝ [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
մետատվյալների պահանջները, առաջարկի ձևանմուշները և վավերացման ստուգաթերթերը:

1. Ստեղծեք `ChunkProfileUpgradeProposalV1` (տես RFC SF‑1) նոր պարամետրերով:
2. Վերականգնեք հարմարանքները `export_vectors`-ի միջոցով և գրանցեք նոր մանիֆեստի ամփոփումը:
3. Ստորագրեք մանիֆեստը խորհրդի պահանջվող քվորումով: Բոլոր ստորագրությունները պետք է լինեն
   կցված է `manifest_signatures.json`-ին:
4. Թարմացրեք SDK-ի ազդեցության տակ գտնվող հարմարանքները (Rust/Go/TS) և ապահովեք համաչափ գործարկման ժամանակ:
5. Վերականգնեք fuzz corpora-ը, եթե պարամետրերը փոխվեն:
6. Թարմացրեք այս ուղեցույցը նոր պրոֆիլի բռնակով, սերմերով և մարսողությամբ:
7. Ներկայացրե՛ք փոփոխությունը թարմացված թեստերի և ճանապարհային քարտեզի թարմացումների հետ մեկտեղ:

Փոփոխություններ, որոնք ազդում են հատվածի սահմանների կամ մարսողության վրա՝ առանց այս գործընթացին հետևելու
անվավեր են և չպետք է միաձուլվեն: