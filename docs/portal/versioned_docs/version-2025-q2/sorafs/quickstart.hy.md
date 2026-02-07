---
lang: hy
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.997191+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Արագ մեկնարկ

Այս գործնական ուղեցույցը անցնում է որոշիչ SF-1 chunker պրոֆիլի միջով,
մանիֆեստի ստորագրում և բազմաթիվ մատակարարների առբերման հոսք, որը հիմնված է SoraFS-ի վրա
պահեստային խողովակաշար: Զուգակցել այն [մանիֆեստ խողովակաշարի խորը սուզման] հետ (manifest-pipeline.md)
դիզայնի նշումների և CLI դրոշի տեղեկատու նյութի համար:

## Նախադրյալներ

- Rust գործիքների շղթա (`rustup update`), տեղական կլոնավորված աշխատանքային տարածք:
- Լրացուցիչ՝ [OpenSSL-ի կողմից ստեղծված Ed25519 ստեղնային զույգ] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  մանիֆեստների ստորագրման համար։
- Լրացուցիչ. Node.js ≥ 18, եթե նախատեսում եք նախադիտել Docusaurus պորտալը:

Փորձարկելիս սահմանեք `export RUST_LOG=info` օգտակար CLI հաղորդագրությունները:

## 1. Թարմացրեք դետերմինիստական հարմարանքները

Վերականգնեք կանոնական SF-1 բեկորային վեկտորները: Հրամանատարությունը թողարկում է նաև ստորագրված
մանիֆեստի ծրարներ, երբ մատակարարվում է `--signing-key`; օգտագործել `--allow-unsigned`
միայն տեղական զարգացման ընթացքում:

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Արդյունքներ:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (եթե ստորագրված է)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Կտրեք օգտակար բեռը և ստուգեք պլանը

Օգտագործեք `sorafs_chunker` կամայական ֆայլը կամ արխիվը բաժանելու համար.

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Հիմնական դաշտերը.

- `profile` / `break_mask` – հաստատում է `sorafs.sf1@1.0.0` պարամետրերը:
- `chunks[]` – պատվիրված հաշվանցումներ, երկարություններ և BLAKE3-ի մասնաբաժիններ:

Ավելի մեծ հարմարանքների համար գործարկեք ռեգրեսիա՝ հիմնված պրոտեստի վրա՝ ապահովելու հոսքը և
խմբաքանակի մասնատումը համաժամեցված է.

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Կառուցեք և ստորագրեք մանիֆեստ

Հատված պլանը, անունները և կառավարման ստորագրությունները փաթեթավորեք մանիֆեստի մեջ՝ օգտագործելով
`sorafs-manifest-stub`. Ստորև բերված հրամանը ցուցադրում է մեկ ֆայլի ծանրաբեռնվածություն. անցնել
գրացուցակի ուղի` ծառ փաթեթավորելու համար (CLI-ն քայլում է այն բառարանագրորեն):

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Վերանայեք `/tmp/docs.report.json`-ը հետևյալի համար.

- `chunking.chunk_digest_sha3_256` – SHA3 շեղումների/երկարությունների ամփոփում, համապատասխանում է
  chunker հարմարանքներ.
- `manifest.manifest_blake3` – BLAKE3 digest ստորագրված մանիֆեստի ծրարում:
- `chunk_fetch_specs[]` – պատվիրել են բեռնելու հրահանգներ նվագախմբի համար:

Երբ պատրաստ եք իրական ստորագրություններ տրամադրել, ավելացրեք `--signing-key` և `--signer`
փաստարկներ. Հրամանը ստուգում է յուրաքանչյուր Ed25519 ստորագրությունը՝ նախքան այն գրելը
ծրար.

## 4. Մոդելավորել բազմապրովայդերների որոնումը

Օգտագործեք ծրագրավորողի առբերման CLI-ն՝ մեկ կամ մի քանիսի հետ միավորի պլանը վերարտադրելու համար
մատակարարներ. Սա իդեալական է CI ծխի թեստերի և նվագախմբի նախատիպերի համար:

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Պնդումներ.

- `payload_digest_hex`-ը պետք է համապատասխանի մանիֆեստի հաշվետվությանը:
- `provider_reports[]` մակերեսների հաջողություն/ձախողում հաշվարկվում է յուրաքանչյուր մատակարարի համար:
- Ոչ զրոյական `chunk_retry_total`-ը կարևորում է հետևի ճնշման ճշգրտումները:
- Անցեք `--max-peers=<n>`՝ սահմանափակելու համար նախատեսված պրովայդերների թիվը
  և պահել CI սիմուլյացիաները՝ կենտրոնացած առաջնային թեկնածուների վրա:
- `--retry-budget=<n>`-ը անտեսում է յուրաքանչյուր կտորի համար կրկնվող փորձի կանխադրված թիվը (3), որպեսզի դուք
  կարող է ավելի արագ երևալ նվագախմբի ռեգրեսիաները ձախողումներ ներարկելիս:

Չհաջողվելու համար ավելացրեք `--expect-payload-digest=<hex>` և `--expect-payload-len=<bytes>`
արագ, երբ վերակառուցված օգտակար բեռը շեղվում է մանիֆեստից:

## 5. Հաջորդ քայլերը

- **Կառավարման ինտեգրում** – շարադրել մանիֆեստը և
  `manifest_signatures.json` մուտքագրեք խորհրդի աշխատանքային հոսքը, որպեսզի Pin Registry-ը կարողանա
  գովազդել մատչելիությունը:
- **Ռեգիստրի բանակցություններ** – խորհրդակցեք [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  նախքան նոր պրոֆիլներ գրանցելը: Ավտոմատացումը պետք է նախընտրի կանոնական բռնակներ
  (`namespace.name@semver`) թվային ID-ների նկատմամբ:
- **CI ավտոմատացում** – ավելացրեք վերը նշված հրամանները՝ խողովակաշարերը թողարկելու համար, որպեսզի փաստաթղթերը,
  հարմարանքները և արտեֆակտները ստորագրված կողքին հրապարակում են դետերմինիստական մանիֆեստներ
  մետատվյալներ.