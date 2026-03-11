---
id: developer-cli
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Կանոնական աղբյուր
:::

Համախմբված `sorafs_cli` մակերեսը (տրամադրված է `sorafs_car` տուփով
`cli` ֆունկցիան միացված է) ցուցադրում է SoraFS պատրաստման համար անհրաժեշտ յուրաքանչյուր քայլ
արտեֆակտներ. Օգտագործեք այս խոհարարական գիրքը՝ ուղղակիորեն անցնելու ընդհանուր աշխատանքային հոսքերին. զուգակցել այն
մանիֆեստի խողովակաշարը և նվագախմբի գործառնական համատեքստի համար նախատեսված գրքույկները:

## Փաթեթի օգտակար բեռներ

Օգտագործեք `car pack`՝ ստեղծելու դետերմինիստական CAR արխիվներ և մասերի պլաններ: Այն
հրամանը ավտոմատ կերպով ընտրում է SF-1 բլոկը, եթե բռնակ նախատեսված չէ:

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Լռելյայն բռնակ՝ `sorafs.sf1@1.0.0`:
- Գրացուցակի մուտքերը կատարվում են բառարանագրական կարգով, որպեսզի ստուգիչ գումարները մնան կայուն
  հարթակներում:
- JSON-ի ամփոփագիրը ներառում է օգտակար բեռների ամփոփումներ, յուրաքանչյուր կտորի մետատվյալներ և արմատը
  CID ճանաչված ռեեստրի և նվագախմբի կողմից:

## Կառուցեք դրսևորումներ

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` ընտրանքները քարտեզագրվում են անմիջապես `PinPolicy` դաշտերում
  `sorafs_manifest::ManifestBuilder`.
- Տրամադրեք `--chunk-plan`, երբ ցանկանում եք, որ CLI-ն վերահաշվարկի SHA3 հատվածը
  մարսել ներկայացնելուց առաջ; հակառակ դեպքում այն կրկին օգտագործում է ներկառուցված մարսողությունը
  ամփոփում.
- JSON ելքը արտացոլում է Norito ծանրաբեռնվածությունը՝ ընթացքում պարզ տարբերությունների համար
  ակնարկներ.

## Նշանը դրսևորվում է առանց երկարակյաց ստեղների

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Ընդունում է ներկառուցված նշաններ, շրջակա միջավայրի փոփոխականներ կամ ֆայլի վրա հիմնված աղբյուրներ:
- Ավելացնում է ծագման մետատվյալներ (`token_source`, `token_hash_hex`, կտոր ամփոփում)
  առանց չմշակված JWT-ի պահպանման, եթե `--include-token=true`:
- Լավ է աշխատում CI-ում. միավորել GitHub Actions OIDC-ի հետ՝ կարգավորելով
  `--identity-token-provider=github-actions`.

## Ներկայացրե՛ք մանիֆեստները Torii-ին

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Կատարում է Norito վերծանում կեղծանունների ապացույցների համար և ստուգում, որ դրանք համապատասխանում են
  մանիֆեստի ամփոփում Torii-ում փակցնելուց առաջ:
- Վերահաշվարկում է SHA3-ի մասնաբաժինը պլանից՝ անհամապատասխան հարձակումները կանխելու համար:
- Պատասխանների ամփոփագրերը ներառում են HTTP կարգավիճակը, վերնագրերը և ռեեստրի օգտակար բեռները
  ավելի ուշ աուդիտ:

## Ստուգեք ավտոմեքենայի բովանդակությունը և ապացույցները

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Վերակառուցում է PoR ծառը և համեմատում է օգտակար բեռնվածքի ամփոփումները մանիֆեստի ամփոփման հետ:
- Գրավում է թվերն ու նույնացուցիչները, որոնք պահանջվում են կրկնօրինակման ապացույցներ ներկայացնելիս
  կառավարմանը։

## Հոսքի դիմացկուն հեռաչափություն

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Արտադրում է NDJSON տարրեր յուրաքանչյուր հոսքային ապացույցի համար (անջատել վերարտադրումը
  `--emit-events=false`):
- միավորում է հաջողության/ձախողումների թվերը, հետաձգման հիստոգրամները և ընտրված ձախողումները
  JSON ամփոփագիրը, որպեսզի վահանակները կարողանան գծագրել արդյունքները առանց մատյանները քերելու:
- Դուրս է գալիս ոչ զրոյական, երբ դարպասը հայտնում է ձախողումների կամ տեղական PoR ստուգման մասին
  (`--por-root-hex`-ի միջոցով) մերժում է ապացույցները: Կարգավորեք շեմերը
  `--max-failures` և `--max-verification-failures` փորձնական վազքի համար:
- Աջակցում է PoR-ին այսօր; PDP-ն և PoTR-ը կրկին օգտագործում են նույն ծրարը մեկ անգամ SF-13/SF-14
  հողատարածք։
- `--governance-evidence-dir`-ը գրում է ներկայացված ամփոփագիրը, մետատվյալները (ժամանականիշը,
  CLI տարբերակ, դարպասի URL, մանիֆեստի ամփոփում) և մանիֆեստի պատճենը
  մատակարարված գրացուցակը, որպեսզի կառավարման փաթեթները կարողանան արխիվացնել ապացույցների հոսքը
  ապացույցներ՝ առանց վազքի կրկնության:

## Լրացուցիչ հղումներ

- `docs/source/sorafs_cli.md` - դրոշի սպառիչ փաստաթղթեր:
- `docs/source/sorafs_proof_streaming.md` — Ապացուցիչ հեռաչափության սխեման և Grafana
  վահանակի ձևանմուշ:
- `docs/source/sorafs/manifest_pipeline.md` — խորը սուզում բեկորների վրա, դրսևորվում է
  կազմը և մեքենա վարելը: