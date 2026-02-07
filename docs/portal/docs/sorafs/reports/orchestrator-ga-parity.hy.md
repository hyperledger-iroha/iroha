---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a206b033b430fc9895f64d402cd53bfea35c3f269b2c18bb12a1f929114423aa
source_last_modified: "2025-12-29T18:16:35.200252+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Orchestrator GA Parity Report

Դետերմինիստական բազմաֆունկցիոնալ հավասարությունը այժմ հետևվում է SDK-ի համար, որպեսզի թողարկման ինժեներները կարողանան հաստատել դա
ծանրաբեռնված բայթերը, կտորների անդորրագրերը, մատակարարի հաշվետվությունները և ցուցատախտակի արդյունքները մնում են համահունչ
իրականացումները։ Յուրաքանչյուր ամրագոտի սպառում է կանոնական բազմաֆունկցիոնալ փաթեթը տակ
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, որը փաթեթավորում է SF1 պլանը, մատակարար
մետատվյալներ, հեռաչափության լուսանկար և նվագախմբի ընտրանքներ:

## Ժանգի հիմքը

- **Հրաման՝** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Շրջանակ.** Գործարկում է `MultiPeerFixture` պլանը երկու անգամ ընթացքի մեջ գտնվող նվագախմբի միջոցով՝ ստուգելով
  հավաքված օգտակար բեռների բայթեր, կտոր անդորրագրեր, մատակարարների հաշվետվություններ և ցուցատախտակի արդյունքներ: Գործիքավորում
  նաև հետևում է առավելագույն համաժամանակությանը և արդյունավետ աշխատանքային հավաքածուի չափին (`max_parallel × max_chunk_length`):
- **Կատարման պահակ.** Յուրաքանչյուր գործարկում պետք է ավարտվի 2 վրկ-ի ընթացքում CI սարքավորման վրա:
- **Աշխատանքային կոմպլեկտ առաստաղ.** SF1 պրոֆիլով ամրագոտիներն ամրացնում են `max_parallel = 3`-ը, ինչը հանգեցնում է
  ≤196608 բայթ պատուհան:

Նմուշի մատյան ելք.

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK զրահ

- **Հրաման՝** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Շրջանակ.** Կրկնում է նույն սարքը `iroha_js_host::sorafsMultiFetchLocal`-ի միջոցով՝ համեմատելով օգտակար բեռները,
  անդորրագրեր, մատակարարների հաշվետվություններ և ցուցատախտակի նկարներ՝ հաջորդական վազքերի ընթացքում:
- **Կատարման պահակ. ** Յուրաքանչյուր կատարում պետք է ավարտվի 2 վայրկյանի ընթացքում; զրահը տպում է չափվածը
  տևողությունը և վերապահված բայթ առաստաղը (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`):

Օրինակ ամփոփ գիծ.

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK զրահ

- **Հրաման՝** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Շրջանակ.** Գործարկում է `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`-ում սահմանված հավասարաչափ փաթեթը,
  SF1 սարքը երկու անգամ կրկնելով Norito կամրջով (`sorafsLocalFetch`): Զարդարակը ստուգում է
  բեռնվածության բայթերը, կտորների անդորրագրերը, մատակարարի հաշվետվությունները և ցուցատախտակի գրառումները՝ օգտագործելով նույն դետերմինիստը
  մատակարարի մետատվյալներ և հեռաչափության պատկերներ, ինչպես Rust/JS փաթեթները:
- **Կամուրջի բեռնախցիկ.** ամրագոտին բացում է `dist/NoritoBridge.xcframework.zip` ըստ պահանջի և բեռների
  macOS-ի հատվածը `dlopen`-ի միջոցով: Երբ xcframework-ը բացակայում է կամ չունի SoraFS կապերը, այն
  վերադառնում է `cargo build -p connect_norito_bridge --release`-ին և կապվում է դեմ
  `target/release/libconnect_norito_bridge.dylib`, ուստի CI-ում ձեռքով կարգավորում չի պահանջվում:
- **Կատարման պահակ.** Յուրաքանչյուր կատարում պետք է ավարտվի 2 վրկ-ի ընթացքում CI սարքաշարի վրա; զրահը տպում է
  չափված տեւողությունը և վերապահված բայթ առաստաղը (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`):

Օրինակ ամփոփ գիծ.

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Հրաման՝** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Ծավալը.
  տվյալների դասակարգեր, այնպես որ կանոնական սարքը հոսում է նույն API-ի միջով, որը կանչում են անիվների սպառողները: Թեստը
  վերակառուցում է մատակարարի մետատվյալները `providers.json`-ից, ներարկում հեռաչափության պատկերը և ստուգում
  ծանրաբեռնված բայթեր, անդորրագրեր, մատակարարների հաշվետվություններ և ցուցատախտակի բովանդակություն, ինչպես Rust/JS/Swift-ը
  սուիթներ.
- **Նախնական պահանջ.** Գործարկեք `maturin develop --release` (կամ տեղադրեք անիվը), որպեսզի `_crypto`-ը բացահայտի
  `sorafs_multi_fetch_local` կապում նախքան pytest-ը կանչելը; զրահը ավտոմատ կերպով բաց է թողնում, երբ կապում է
  անհասանելի է։
- **Կատարման պահակ.** Նույն ≤2s բյուջեն, ինչ Rust փաթեթը; pytest-ը գրանցում է հավաքված բայթերի քանակը
  և մատակարարի մասնակցության ամփոփագիրը թողարկման արտեֆակտի համար:

Release gating-ը պետք է վերցնի ամփոփ ելքը յուրաքանչյուր զրահից (Rust, Python, JS, Swift), որպեսզի
Արխիվացված հաշվետվությունը կարող է միատեսակ տարբերել օգտակար բեռների ստացականներն ու չափումները՝ նախքան շինարարությունը խթանելը: Վազիր
`ci/sdk_sorafs_orchestrator.sh`՝ յուրաքանչյուր հավասարության փաթեթ (Rust, Python bindings, JS, Swift) գործարկելու համար
մեկ անցում; CI արտեֆակտները պետք է կցեն այդ օգնականի գրանցամատյանի քաղվածքը գումարած գեներացվածը
`matrix.md` (SDK/կարգավիճակ/տևողության աղյուսակ) թողարկման տոմսին, որպեսզի վերանայողները կարողանան ստուգել հավասարությունը
մատրիցա՝ առանց լոկալ փաթեթը կրկնելու: