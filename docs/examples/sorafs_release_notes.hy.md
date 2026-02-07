---
lang: hy
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI & SDK — Թողարկման նշումներ (v0.1.0)

## Կարևորություններ
- `sorafs_cli`-ն այժմ փաթաթում է ամբողջ փաթեթավորման խողովակաշարը (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`), ուստի CI վազորդները կանչում են
  միայնակ երկուական՝ պատվիրված օգնականների փոխարեն: Նոր առանց բանալի ստորագրման հոսքը կանխադրված է
  `SIGSTORE_ID_TOKEN`, հասկանում է GitHub Actions OIDC պրովայդերներին և թողարկում է դետերմինիստական
  ամփոփագիր JSON՝ ստորագրության փաթեթի կողքին:
- Բազմաղբյուրի բեռնման *ցուցատախտակը* առաքվում է որպես `sorafs_car`. այն նորմալանում է
  մատակարարի հեռաչափությունը, կիրառում է հնարավորությունների տույժեր, պահպանում է JSON/Norito հաշվետվությունները և
  սնուցում է նվագախմբի սիմուլյատորին (`sorafs_fetch`) ընդհանուր ռեեստրի բռնակի միջոցով:
  `fixtures/sorafs_manifest/ci_sample/` տակ գտնվող հարմարանքները ցույց են տալիս դետերմինիստականը
  մուտքեր և ելքեր, որոնցից ակնկալվում է, որ CI/CD-ն կտարբերվի:
- Թողարկման ավտոմատացումը ծածկագրված է `ci/check_sorafs_cli_release.sh`-ում և
  `scripts/release_sorafs_cli.sh`. Այժմ յուրաքանչյուր թողարկում արխիվացնում է մանիֆեստի փաթեթը,
  ստորագրությունը, `manifest.sign/verify` ամփոփագրերը և ցուցատախտակի պատկերը, այնպես որ կառավարումը
  վերանայողները կարող են հետևել արտեֆակտներին՝ առանց խողովակաշարի վերագործարկման:

## Թարմացման քայլեր
1. Թարմացրեք հավասարեցված արկղերը ձեր աշխատանքային տարածքում.
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Կրկին գործարկեք թողարկման դարպասը տեղական (կամ CI-ով)՝ fmt/clippy/փորձարկման ծածկույթը հաստատելու համար.
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Վերականգնեք ստորագրված արտեֆակտները և ամփոփագրերը ընտրված կազմաձևով.
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Պատճենեք թարմացված փաթեթները/ապացույցները `fixtures/sorafs_manifest/ci_sample/`-ում, եթե
   թողարկել կանոնական հարմարանքների թարմացումները:

## Ստուգում
- Ազատման դարպասի կատարում՝ `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` դարպասի հաջողության անմիջապես հետո):
- `ci/check_sorafs_cli_release.sh` ելք՝ արխիվացված
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (կցված է թողարկման փաթեթին):
- Մանիֆեստի փաթեթի ամփոփում. `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`):
- Ապացույցի ամփոփագիր՝ `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`):
- Մանիֆեստի ամփոփում (ներքևում գտնվող ատեստավորման խաչաձև ստուգումների համար).
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json`-ից):

## Նշումներ օպերատորների համար
- Torii դարպասն այժմ ապահովում է `X-Sora-Chunk-Range` կարողությունների վերնագիր: Թարմացնել
  թույլտվությունների ցուցակներ, որպեսզի հաճախորդները, որոնք ներկայացնում են նոր հոսքային նշանների շրջանակները, ընդունվեն. ավելի հին նշաններ
  առանց միջակայքի պահանջի կհեռացվի:
- `scripts/sorafs_gateway_self_cert.sh`-ն ինտեգրում է մանիֆեստի ստուգումը: Վազելիս
  ինքնահաստատման զրահը, մատակարարեք նոր ստեղծված մանիֆեստի փաթեթը, որպեսզի փաթաթան կարողանա
  արագ ձախողվում է ստորագրության դրեյֆի վրա:
- Հեռուստաչափության վահանակները պետք է ընդունեն նոր արդյունքների արտահանումը (`scoreboard.json`) դեպի
  հաշտեցնել մատակարարի իրավասությունը, քաշի նշանակումները և մերժման պատճառները:
- Արխիվացրեք չորս կանոնական ամփոփագրերը յուրաքանչյուր թողարկման հետ.
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Կառավարման տոմսերը վերաբերում են հենց այս ֆայլերին ընթացքում
  հաստատում։

## Երախտագիտություն
- Պահպանման թիմ՝ վերջից մինչև վերջ CLI համախմբում, մասերի պլանի մատուցում և արդյունքների տախտակ
  հեռաչափական սանտեխնիկա.
- Tooling WG — թողարկման խողովակաշար (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) և դետերմինիստական հարմարանքների փաթեթ:
- Gateway Operations – հնարավորությունների մուտք, հոսքային նշանների քաղաքականության վերանայում և թարմացում
  ինքնահաստատման գրքույկներ.