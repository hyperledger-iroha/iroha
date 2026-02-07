---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Chunking → Manifest Pipeline

Արագ մեկնարկի այս ուղեկիցը հետևում է ծայրից ծայր խողովակաշարին, որը դառնում է հում
բայթերով Norito մանիֆեստներում, որոնք հարմար են SoraFS Pin ռեեստրի համար: Բովանդակությունն է
հարմարեցված է [`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
խորհրդակցեք այդ փաստաթղթի կանոնական ճշգրտման և փոփոխության մատյանի համար:

## 1. Հատված դետերմինիստորեն

SoraFS-ն օգտագործում է SF-1 (`sorafs.sf1@1.0.0`) պրոֆիլը՝ FastCDC-ով ոգեշնչված շարժակազմ
Հեշ՝ 64 ԿիԲ նվազագույն կտորի չափով, 256 ԿԲ թիրախ, 512 ԿԲ առավելագույնով և
`0x0000ffff` ընդմիջման դիմակ: Պրոֆիլը գրանցված է
`sorafs_manifest::chunker_registry`.

### Ժանգի օգնականներ

- `sorafs_car::CarBuildPlan::single_file` – Արտանետում է հատվածի շեղումները, երկարությունները և
  BLAKE3-ը մարսում է CAR մետատվյալները պատրաստելիս:
- `sorafs_car::ChunkStore` – Հոսում է օգտակար բեռներ, պահպանում է մետատվյալների մի մասը և
  ստացվում է 64KiB / 4KiB Proof-of-Retrievability (PoR) նմուշառման ծառը:
- `sorafs_chunker::chunk_bytes_with_digests` – Գրադարանի օգնական երկու CLI-ների հետևում:

### CLI գործիքավորում

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON-ը պարունակում է պատվիրված օֆսեթներ, երկարություններ և մասերի ամփոփումներ: Համառեք
պլան, երբ կառուցում է մանիֆեստներ կամ նվագախմբի հետ բերելու բնութագրերը:

### PoR վկաներ

`ChunkStore` բացահայտում է `--por-proof=<chunk>:<segment>:<leaf>` և
`--por-sample=<count>`, որպեսզի աուդիտորները կարողանան պահանջել որոշիչ վկաների հավաքածուներ: Զույգ
այդ դրոշակները `--por-proof-out` կամ `--por-sample-out`-ով JSON-ը ձայնագրելու համար:

## 2. Փաթեթավորեք մանիֆեստը

`ManifestBuilder`-ը համատեղում է մետատվյալների մի մասը կառավարման հավելվածների հետ.

- Root CID (dag-cbor) և CAR պարտավորություններ:
- Այլանունների ապացույցներ և մատակարարի հնարավորությունների պահանջներ:
- Խորհրդի ստորագրություններ և կամընտիր մետատվյալներ (օրինակ՝ շինարարական ID-ներ):

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Կարևոր արդյունքներ.

- `payload.manifest` – Norito կոդավորված մանիֆեստի բայթ:
- `payload.report.json` – Մարդու/ավտոմատիկայի ընթեռնելի ամփոփագիր, ներառյալ
  `chunk_fetch_specs`, `payload_digest_hex`, CAR վերլուծություններ և այլանունների մետատվյալներ:
- `payload.manifest_signatures.json` – Ծրար, որը պարունակում է մանիֆեստ BLAKE3
  digest, chunk-plan SHA3 digest և տեսակավորված Ed25519 ստորագրությունները:

Օգտագործեք `--manifest-signatures-in`՝ արտաքին կողմից մատակարարված ծրարները ստուգելու համար
ստորագրողներին, նախքան դրանք հետ գրելը, և `--chunker-profile-id` կամ
`--chunker-profile=<handle>`՝ ռեեստրի ընտրությունը կողպելու համար:

## 3. Հրապարակեք և ամրացրեք

1. **Կառավարման ներկայացում** – Տրամադրել մանիֆեստի ամփոփագիրը և ստորագրությունը
   ծրարը խորհրդին, որպեսզի քորոցը ընդունվի: Արտաքին աուդիտորները պետք է
   Պահպանեք chunk-plan SHA3 digest-ը մանիֆեստի մարսողության կողքին:
2. **Pin payloads** – Վերբեռնեք CAR արխիվը (և կամընտիր CAR ինդեքսը) հղումով
   մանիֆեստում Pin Registry-ում: Համոզվեք, որ մանիֆեստը և CAR-ը կիսում են
   նույն արմատային CID.
3. **Ձայնագրեք հեռաչափություն** – Պահպանեք JSON զեկույցը, PoR վկաները և ցանկացած առբերում
   չափումներ թողարկման արտեֆակտներում: Այս գրառումները սնուցում են օպերատորների վահանակները և
   օգնում է վերարտադրել խնդիրները՝ առանց մեծ բեռներ ներբեռնելու:

## 4. Բազմամատակարարների առբերման մոդելավորում

«բեռների գործարկում -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=provider/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>`-ը մեծացնում է յուրաքանչյուր մատակարարի զուգահեռությունը (`#4` վերևում):
- `@<weight>` մեղեդիների պլանավորման կողմնակալություն; կանխադրված է 1:
- `--max-peers=<n>`-ը սահմանափակում է մատակարարների թիվը, որոնք նախատեսված են գործարկման, երբ
  բացահայտումը ցանկալիից ավելի շատ թեկնածուներ է տալիս:
- `--expect-payload-digest` և `--expect-payload-len` պաշտպանում են լռությունից
  կոռուպցիա.
- `--provider-advert=name=advert.to`-ը նախկինում ստուգում է մատակարարի հնարավորությունները
  օգտագործելով դրանք սիմուլյացիայի մեջ:
- `--retry-budget=<n>`-ը անտեսում է յուրաքանչյուր կտորի կրկնակի փորձի քանակը (կանխադրված՝ 3), ուստի CI
  կարող է ավելի արագ առաջացնել ռեգրեսիաներ ձախողման սցենարների փորձարկման ժամանակ:

`fetch_report.json` մակերեսների ագրեգացված չափումներ (`chunk_retry_total`,
`provider_failure_rate` և այլն) հարմար է CI պնդումների և դիտելիության համար։

## 5. Ռեեստրի թարմացումներ և կառավարում

Նոր chunker պրոֆիլներ առաջարկելիս.

1. Հեղինակ է նկարագրիչը `sorafs_manifest::chunker_registry_data`-ում:
2. Թարմացրեք `docs/source/sorafs/chunker_registry.md` և հարակից կանոնադրությունները:
3. Վերականգնել հարմարանքները (`export_vectors`) և գրավել ստորագրված մանիֆեստները:
4. Ներկայացնել կանոնադրության համապատասխանության հաշվետվությունը կառավարման ստորագրություններով:

Ավտոմատացումը պետք է նախընտրի կանոնական բռնակներ (`namespace.name@semver`) և ընկնել