---
lang: hy
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Արտակարգ իրավիճակների Canon և TTL քաղաքականություն (MINFO-6a)

Ճանապարհային քարտեզի հղում՝ **MINFO-6a — Արտակարգ իրավիճակների կանոն և TTL քաղաքականություն**:

Այս փաստաթուղթը սահմանում է հերքման ցուցակի մակարդակի կանոնները, TTL-ի կիրարկումը և կառավարման պարտավորությունները, որոնք այժմ առաքվում են Torii-ում և CLI-ում: Օպերատորները պետք է հետևեն այս կանոններին՝ նախքան նոր գրառումներ հրապարակելը կամ արտակարգ իրավիճակների կանոնները կանչելը:

## մակարդակի սահմանումներ

| Շերտ | Կանխադրված TTL | Վերանայման պատուհան | Պահանջներ |
|------|-------------|---------------|-------------|
| Ստանդարտ | 180 օր (`torii.sorafs_gateway.denylist.standard_ttl`) | հ/հ | Պետք է տրամադրվի `issued_at`: `expires_at` կարող է բաց թողնել; Torii-ը կանխադրված է որպես `issued_at + standard_ttl` և մերժում է ավելի երկար պատուհանները: |
| Արտակարգ | 30 օր (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 օր (`torii.sorafs_gateway.denylist.emergency_review_window`) | Պահանջվում է ոչ դատարկ `emergency_canon` պիտակ՝ հղում կատարելով նախապես հաստատված կանոնին (օրինակ՝ `csam-hotline`): `issued_at` + `expires_at`-ը պետք է վայրէջք կատարի 30-օրյա պատուհանի ներսում, իսկ վերանայման ապացույցները պետք է նշեն ավտոմատ ստեղծվող վերջնաժամկետը (`issued_at + review_window`): |
| Մշտական ​​| Ժամկետը չկա | հ/հ | Վերապահված է գերմեծամասնության կառավարման որոշումների համար: Գրառումները պետք է նշեն ոչ դատարկ `governance_reference` (քվեարկության id, մանիֆեստի հեշ և այլն): `expires_at`-ը մերժված է: |

Կանխադրվածները մնում են կարգավորելի `torii.sorafs_gateway.denylist.*`-ի միջոցով, իսկ `iroha_cli`-ը արտացոլում է անվավեր գրառումները բռնելու սահմանները մինչև Torii ֆայլը վերաբեռնելը:

## Աշխատանքային ընթացք

1. **Պատրաստեք մետատվյալներ.** ներառեք `policy_tier`, `issued_at`, `expires_at` (եթե կիրառելի է) և `emergency_canon`/`emergency_canon`/`governance_reference` յուրաքանչյուր JSON000000 մուտքագրում (I01X010000000036X100000000000036X)
2. **Վավերացրեք տեղայնորեն.** գործարկեք `iroha app sorafs gateway lint-denylist --path <denylist.json>`, որպեսզի CLI-ն կիրառի մակարդակի հատուկ TTL-ները և պահանջվող դաշտերը, նախքան ֆայլը կցվի կամ ամրացվի:
3. **Հրապարակեք ապացույցներ.** կցեք կանոնի id-ը կամ կառավարման տեղեկանքը, որը նշված է GAR գործերի փաթեթի մուտքագրում (օրակարգի փաթեթ, հանրաքվեի արձանագրություններ և այլն), որպեսզի աուդիտորները կարողանան հետևել որոշմանը:
4. **Վերանայեք արտակարգ իրավիճակների գրառումները.** արտակարգ իրավիճակների կանոնները ավտոմատ կերպով լրանում են 30 օրվա ընթացքում: Օպերատորները պետք է ավարտեն հետֆակտո վերանայումը 7-օրյա պատուհանի ներսում և գրանցեն արդյունքը նախարարության հետագծում/SoraFS ապացույցների պահեստում:
5. **Վերբեռնեք Torii:** վավերացումից հետո տեղադրեք denylist ուղին `torii.sorafs_gateway.denylist.path`-ի միջոցով և վերագործարկեք/վերաբեռնեք Torii; գործարկման ժամանակը կիրառում է նույն սահմանները նախքան գրառումները ընդունելը:

## Գործիքներ և հղումներ

- Runtime-ի քաղաքականության կիրարկումը գործում է `sorafs::gateway::denylist`-ում (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`), և բեռնիչը այժմ կիրառում է մակարդակի մետատվյալներ `torii.sorafs_gateway.denylist.*` մուտքերը վերլուծելիս:
- CLI վավերացումը արտացոլում է գործարկման ժամանակի իմաստաբանությունը `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) ներսում: Լինտերը ձախողվում է, երբ TTL-ները գերազանցում են կազմաձևված պատուհանը կամ երբ բացակայում են կանոնների/կառավարման պարտադիր հղումները:
- Կազմաձևման կոճակները սահմանված են `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) ներքո, որպեսզի օպերատորները կարողանան շտկել TTL-ները/վերանայել վերջնաժամկետները, եթե ղեկավարությունը հաստատի տարբեր սահմաններ:
- Հանրային նմուշի մերժման ցուցակը (`docs/source/sorafs_gateway_denylist_sample.json`) այժմ ցույց է տալիս բոլոր երեք մակարդակները և պետք է օգտագործվի որպես կանոնական ձևանմուշ նոր մուտքերի համար:Այս պաշտպանիչ բազրիքները բավարարում են ճանապարհային քարտեզի **MINFO-6a** կետը՝ կոդավորելով արտակարգ իրավիճակների կանոնների ցանկը, կանխելով անսահմանափակ TTL-ները և ստիպելով բացահայտ կառավարման ապացույցներ մշտական ​​բլոկների համար:

## Ռեեստրի ավտոմատացում և ապացույցների արտահանում

Արտակարգ կանոնների հաստատումները պետք է պարունակեն գրանցման դետերմինիստական պատկեր և ա
diff փաթեթը նախքան Torii-ը բեռնում է հերքման ցուցակը: Գործիքավորումը տակ
`xtask/src/sorafs.rs` գումարած CI ամրագոտի `ci/check_sorafs_gateway_denylist.sh`
ծածկել ամբողջ աշխատանքային հոսքը:

### Կանոնական փաթեթների սերունդ

1. Բեմադրեք չմշակված գրառումները (սովորաբար կառավարման կողմից վերանայված ֆայլը) աշխատանքային համակարգում
   գրացուցակ.
2. Կանոնականացրեք և կնքեք JSON-ը հետևյալի միջոցով.
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Հրամանը թողարկում է ստորագրված `.json` փաթեթ, Norito `.to`
   ծրարը և Merkle-root տեքստային ֆայլը, որը սպասվում է կառավարման վերանայողների կողմից:
   Պահպանեք գրացուցակը `artifacts/ministry/denylist_registry/` (կամ ձեր
   ընտրված ապացույցների դույլ), այնպես որ `scripts/ministry/transparency_release.py` կարող է
   վերցրեք այն ավելի ուշ `--artifact denylist_bundle=<path>`-ով:
3. Ստեղծված `checksums.sha256`-ը պահեք փաթեթի կողքին՝ նախքան այն մղելը
   դեպի SoraFS/GAR. CI's `ci/check_sorafs_gateway_denylist.sh` վարժությունները նույնն են
   `pack` օգնական՝ ընդդեմ նմուշի մերժման՝ գործիքավորման աշխատանքը երաշխավորելու համար
   յուրաքանչյուր թողարկում:

### Տարբերություն + աուդիտի փաթեթ

1. Համեմատեք նոր փաթեթը նախորդ արտադրության նկարի հետ՝ օգտագործելով
   xtask diff helper:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON զեկույցը թվարկում է բոլոր ավելացումները/հեռացումը և արտացոլում է ապացույցները
   կառուցվածքը սպառված է `MinistryDenylistChangeV1`-ի կողմից (հղում
   `docs/source/sorafs_gateway_self_cert.md` և համապատասխանության պլան):
2. Կցեք `denylist_diff.json` Կանոնի յուրաքանչյուր հարցումին (դա ապացուցում է, թե քանի
   մուտքերը շոշափվել են, թե որ մակարդակն է փոխվել, և ինչպիսի ապացույցներ են հաշման քարտեզները
   կանոնական փաթեթ):
3. Երբ տարբերությունները ստեղծվում են ավտոմատ կերպով (CI կամ թողարկման խողովակաշարեր), արտահանեք
   `denylist_diff.json` ճանապարհը `--artifact denylist_diff=<path>`-ի միջոցով, որպեսզի
   թափանցիկության մանիֆեստն այն արձանագրում է ախտահանված չափումների հետ մեկտեղ: Նույն CI
   օգնականն ընդունում է `--evidence-out <path>`, որն իրականացնում է CLI ամփոփման քայլը և
   պատճենում է ստացված JSON-ը պահանջվող վայրում՝ հետագայում հրապարակելու համար:

### Հրապարակում և թափանցիկություն1. Փաթեթը + տարբեր արտեֆակտները գցեք եռամսյակային թափանցիկության գրացուցակում
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`): Թափանցիկությունը
   ազատման օգնականն այնուհետև կարող է ներառել դրանք.
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Եռամսյակային հաշվետվության մեջ նշեք առաջացած փաթեթը/տարբերությունը
   (`docs/source/ministry/reports/<YYYY-Q>.md`) և միացրեք նույն ուղիները
   GAR քվեարկության փաթեթ, որպեսզի աուդիտորները կարողանան վերարտադրել ապացույցների հետքը առանց մուտքի
   ներքին CI. `ci/check_sorafs_gateway_denylist.sh --ապացույցներ դուրս \
   artifacts/ministry/denylist_registry//denylist_evidence.json` այժմ
   կատարում է փաթեթի/տարբերությունների/ապացույցների չոր գործարկումը (զանգահարելով՝ «iroha_cli» հավելվածը «sorafs gateway»
   ապացույց՝ գլխարկի տակ), այնպես որ ավտոմատացումը կարող է շարունակել ամփոփումը կողքին
   կանոնական փաթեթներ.
3. Հրապարակումից հետո ամրացրեք կառավարման օգտակար բեռը միջոցով
   `cargo xtask ministry-transparency anchor` (կոչվում է ավտոմատ կերպով
   `transparency_release.py`, երբ տրամադրվում է `--governance-dir`), ուստի
   denylist registry digest-ը հայտնվում է նույն DAG ծառում, ինչպես թափանցիկությունը
   ազատում.

Այս գործընթացից հետո փակվում է «ռեգիստրի ավտոմատացումը և ապացույցների արտահանումը»
բացը նշված է `roadmap.md:450`-ում և ապահովում է, որ յուրաքանչյուր արտակարգ իրավիճակի կանոն
որոշումը գալիս է վերարտադրվող արտեֆակտներով, JSON տարբերություններով և թափանցիկության մատյանով
գրառումները.

### TTL & Canon Evidence Helper

Փաթեթ/տարբերակ զույգը արտադրվելուց հետո գործարկեք CLI ապացույցների օգնականը՝ գրավելու համար
TTL ամփոփագրերը և արտակարգ իրավիճակների վերանայման ժամկետները, որոնք պահանջում են կառավարումը.

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Հրամանը հեշավորում է JSON աղբյուրը, վավերացնում է յուրաքանչյուր մուտք և թողարկում կոմպակտ
ամփոփագիր, որը պարունակում է.

- Ընդհանուր գրառումները ըստ `kind`-ի և յուրաքանչյուր քաղաքականության մակարդակի՝ ամենավաղ/վերջին
  դիտարկված ժամանակային դրոշմանիշներ:
- `emergency_reviews[]` ցուցակ, որը թվարկում է յուրաքանչյուր արտակարգ կանոն իր հետ
  նկարագրիչ, գործող ժամկետի ավարտ, առավելագույն թույլատրելի TTL և հաշվարկված
  `review_due_by` վերջնաժամկետ:

Կցեք `denylist_evidence.json` փաթեթավորված փաթեթի/տարբերության կողքին, որպեսզի աուդիտորները կարողանան
հաստատել TTL-ի համապատասխանությունը՝ առանց CLI-ի վերագործարկման: CI աշխատատեղեր, որոնք արդեն առաջացնում են
փաթեթները կարող են կանչել օգնականին և հրապարակել ապացույցների արտեֆակտը (օրինակ՝ ըստ
զանգահարելով `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`), ապահովելով
յուրաքանչյուր կանոնական հարցումը հասնում է հետևողական ամփոփագրի:

### Մերկլի ռեեստրի ապացույցներ

MINFO-6-ում ներդրված Merkle ռեգիստրը պահանջում է օպերատորներից հրապարակել այն
արմատային և մեկ մուտքի ապացույցներ TTL ամփոփագրի հետ մեկտեղ: Վազելուց անմիջապես հետո
ապացույցների օգնականը՝ գրավել Մերկլի արտեֆակտները.

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```Snapshot JSON-ը գրանցում է BLAKE3 Merkle արմատը, տերևների քանակը և յուրաքանչյուրը
նկարագրիչ/հեշ զույգ, որպեսզի GAR-ի ձայները կարողանան վկայակոչել այն ծառը, որը հաշվել է:
`--norito-out` մատակարարելը JSON-ի կողքին պահում է `.to` արտեֆակտ՝ թույլ տալով
դարպասները կլանում են ռեեստրի գրառումները անմիջապես Norito-ի միջոցով՝ առանց քերելու
stdout. `merkle proof`-ը թողարկում է ուղղության բիթերը և եղբոր կամ քույրերի հեշերը ցանկացածի համար
զրոյական վրա հիմնված մուտքի ինդեքս, ինչը հեշտացնում է յուրաքանչյուրի համար ներառման ապացույց կցելը
Արտակարգ իրավիճակների կանոնը, որը նշված է GAR հուշագրում. կամընտիր Norito պատճենը պահպանում է ապացույցը
պատրաստ է բաշխման մատյանում: Հաջորդը պահեք և՛ JSON, և՛ Norito արտեֆակտները
TTL-ի ամփոփման և տարբերության փաթեթին, որպեսզի թափանցիկության թողարկումները և կառավարումը
խարիսխները վկայակոչում են նույն արմատը: