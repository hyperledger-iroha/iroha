---
lang: hy
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Հաշվարկային գոտի (SSC-1)

Հաշվարկային գոտին ընդունում է HTTP ոճի դետերմինիստական զանգեր, դրանք քարտեզագրում է Kotodama-ի վրա
մուտքի կետերը և գրանցում է հաշվառման/անդորրագրերը վճարումների և կառավարման վերանայման համար:
Այս RFC-ն սառեցնում է մանիֆեստի սխեման, զանգերի/անդորրագրի ծրարները, ավազատուփի պաշտպանիչ վանդակները,
և կազմաձևման կանխադրվածները առաջին թողարկման համար:

## Մանիֆեստ

- Սխեման՝ `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`):
- `abi_version`-ը ամրացված է `1`-ին; այլ տարբերակով դրսևորումները մերժվում են
  վավերացման ընթացքում:
- Յուրաքանչյուր երթուղի հայտարարում է.
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama մուտքի կետի անվանում)
  - կոդեկների թույլտվությունների ցուցակ (`codecs`)
  - TTL/gas/ հարցում/պատասխանի գլխարկներ (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - դետերմինիզմ/կատարման դաս (`determinism`, `execution_class`)
  - SoraFS մուտքի/մոդելի նկարագրիչներ (`input_limits`, կամընտիր `model`)
  - գնագոյացման ընտանիք (`price_family`) + ռեսուրսի պրոֆիլ (`resource_profile`)
  - Նույնականացման քաղաքականություն (`auth`)
- Sandbox-ի պաշտպանիչ բազրիքներն ապրում են մանիֆեստի `sandbox` բլոկում և համօգտագործվում են բոլորի կողմից
  երթուղիներ (ռեժիմ/պատահականություն/պահեստավորում և ոչ դետերմինիստական սիսկալի մերժում):

Օրինակ՝ `fixtures/compute/manifest_compute_payments.json`:

## Զանգեր, հարցումներ և ստացականներ

- Սխեման՝ `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome`
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()`-ն արտադրում է կանոնական հարցումների հեշը (վերնագրերը պահվում են
  դետերմինիստական `BTreeMap`-ում և օգտակար բեռը տեղափոխվում է որպես `payload_hash`):
- `ComputeCall`-ը գրավում է անվանատարածքը/երթուղին, կոդեկը, TTL/գազը/պատասխանի գլխարկը,
  ռեսուրսի պրոֆիլ + գների ընտանիք, վավերագիր (`Public` կամ UAID-ի հետ կապված
  `ComputeAuthn`), դետերմինիզմ (`Strict` vs `BestEffort`), կատարման դաս
  ակնարկներ (CPU/GPU/TEE), հայտարարված SoraFS մուտքագրման բայթ/կտոր, կամընտիր հովանավոր
  բյուջեն և կանոնական խնդրանքի ծրարը։ Հարցման հեշը օգտագործվում է
  կրկնակի պաշտպանություն և երթուղղում:
- Երթուղիները կարող են զետեղել կամընտիր SoraFS մոդելի հղումներ և մուտքագրման սահմանափակումներ
  (ներկառուցված / կտոր գլխարկներ); մանիֆեստի ավազատուփի կանոնների դարպասի GPU/TEE հուշումներ:
- `ComputePriceWeights::charge_units`-ը չափիչ տվյալները փոխակերպում է հաշվարկային հաշվարկի
  միավորներ առաստաղի բաժանման միջոցով ցիկլերի և ելքի բայթերի վրա:
- `ComputeOutcome` հաշվետվություններ `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, կամ `InternalError` և ընտրովի ներառում է պատասխան հեշեր/
  չափսեր/կոդեկ աուդիտի համար:

Օրինակներ.
- Զանգել՝ `fixtures/compute/call_compute_payments.json`
- Անդորրագիր՝ `fixtures/compute/receipt_compute_payments.json`

## Sandbox և ռեսուրսների պրոֆիլներ- `ComputeSandboxRules`-ը լռելյայն կողպում է կատարման ռեժիմը `IvmOnly`-ում,
  սերմերը դետերմինիստական պատահականություն է հարցումների հեշից, թույլ է տալիս միայն կարդալու SoraFS
  մուտք և մերժում է ոչ դետերմինիստական սիսկալները: GPU/TEE ակնարկները փակված են
  `allow_gpu_hints`/`allow_tee_hints`՝ կատարումը դետերմինիստական պահելու համար:
- `ComputeResourceBudget`-ը սահմանում է յուրաքանչյուր պրոֆիլի գլխարկներ ցիկլերի, գծային հիշողության, կույտի վրա
  չափը, IO-ի բյուջեն և արտահոսքը, ինչպես նաև GPU-ի ակնարկների և WASI-lite օգնականների փոխարկիչներ:
- Լռելյայն առաքում է երկու պրոֆիլ (`cpu-small`, `cpu-balanced`) տակ
  `defaults::compute::resource_profiles` դետերմինիստական հետադարձ հետքերով:

## Գնային և հաշվարկային միավորներ

- Գների ընտանիքները (`ComputePriceWeights`) քարտեզագրում են ցիկլերը և ելքային բայթերը հաշվարկի մեջ
  միավորներ; լռելյայն վճարել `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` հետ
  `unit_label = "cu"`. Ընտանիքները մուտքագրված են `price_family`-ով մանիֆեստներում և
  ուժի մեջ է մտնում ընդունելության ժամանակ:
- Չափման գրառումները պարունակում են `charged_units` գումարած չմշակված ցիկլ/մուտք/ելք/տեւողություն
  հաշտեցման համար նախատեսված գումարներ: Գանձումները ուժեղացվում են ըստ կատարման դասի և
  դետերմինիզմի բազմապատկիչներ (`ComputePriceAmplifiers`) և սահմանաչափով
  `compute.economics.max_cu_per_call`; ելքը սեղմված է
  `compute.economics.max_amplification_ratio`՝ կապված պատասխանի ուժեղացման համար:
- Հովանավորների բյուջեները (`ComputeCall::sponsor_budget_cu`) հարկադրված են
  մեկ զանգի / օրական գլխարկներ; հաշվարկված միավորները չպետք է գերազանցեն հայտարարված հովանավոր բյուջեն:
- Կառավարման գների թարմացումները օգտագործում են ռիսկի դասի սահմանները
  `compute.economics.price_bounds` և ելակետային ընտանիքները գրանցված են
  `compute.economics.price_family_baseline`; օգտագործել
  `ComputeEconomics::apply_price_update`՝ թարմացումից առաջ դելտաները հաստատելու համար
  ակտիվ ընտանիքի քարտեզ. Torii կոնֆիգուրացիայի թարմացումների օգտագործումը
  `ConfigUpdate::ComputePricing`, և kiso-ն այն կիրառում է նույն սահմաններով
  Պահպանեք կառավարման խմբագրումները որոշիչ:

## Կազմաձևում

Նոր հաշվարկային կոնֆիգուրացիան գործում է `crates/iroha_config/src/parameters`-ում.

- Օգտատիրոջ դիտում. `Compute` (`user.rs`) env-ի վերափոխումներով.
  - `COMPUTE_ENABLED` (կանխադրված `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Գին/տնտեսագիտություն. `compute.economics` գրավում է
  `max_cu_per_call`/`max_amplification_ratio`, վճարի բաժանում, հովանավորների սահմանաչափեր
  (մեկ զանգի և օրական ՄՄ), գների ընտանեկան ելակետեր + ռիսկի դասեր/սահմաններ
  կառավարման թարմացումներ և կատարողական դասի բազմապատկիչներ (GPU/TEE/best-fort):
- Փաստացի/կանխադրված՝ `actual.rs` / `defaults.rs::compute` ցուցադրվում է վերլուծված
  `Compute` կարգավորումներ (անունների տարածքներ, պրոֆիլներ, գների ընտանիքներ, ավազատուփ):
- Անվավեր կոնֆիգուրացիաներ (դատարկ անունների տարածքներ, լռելյայն պրոֆիլ/ընտանեկան բացակայում է, TTL գլխարկ
  ինվերսիաներ) վերլուծության ընթացքում երևում են որպես `InvalidComputeConfig`:

## Թեստեր և հարմարանքներ

- Դետերմինիստական օգնականներ (`request_hash`, գնագոյացում) և հարմարանքային շրջագայություններ ապրում են
  `crates/iroha_data_model/src/compute/mod.rs` (տես `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`):
- JSON հարմարանքները ապրում են `fixtures/compute/`-ում և իրականացվում են տվյալների մոդելի կողմից
  թեստեր ռեգրեսիայի ծածկույթի համար:

## SLO զրահ և բյուջեներ- `compute.slo.*` կոնֆիգուրացիան բացահայտում է դարպասի SLO կոճակները (թռիչքի ընթացքում հերթ
  խորությունը, RPS գլխարկը և հետաձգման թիրախները) in
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Կանխադրվածներ՝ 32
  թռիչքի ժամանակ, 512 հերթում յուրաքանչյուր երթուղու համար, 200 RPS, p50 25ms, p95 75ms, p99 120ms:
- Գործարկեք թեթև նստարանի ամրագոտիը՝ SLO ամփոփագրերը և հարցումը/ելքը գրավելու համար
  snapshot՝ «cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [կրկնումներ] [համաժամանակակից] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 կրկնություններ, համաժամանակյա 16, ելքեր տակ
  `artifacts/compute_gateway/bench_summary.{json,md}`): Նստարանն օգտագործում է
  որոշիչ օգտակար բեռներ (`fixtures/compute/payload_compute_payments.json`) և
  յուրաքանչյուր հարցման վերնագրեր՝ վարժությունների ընթացքում կրկնվող բախումներից խուսափելու համար
  `echo`/`uppercase`/`sha3` մուտքի կետեր:

## SDK/CLI հավասարության հարմարանքներ

- Canonical հարմարանքները գործում են `fixtures/compute/`-ի ներքո՝ մանիֆեստ, զանգ, օգտակար բեռնվածություն և
  դարպասի ոճի պատասխանի/ստացման դասավորությունը: Օգտակար բեռի հեշերը պետք է համապատասխանեն զանգին
  `request.payload_hash`; օգնականը ապրում է
  `fixtures/compute/payload_compute_payments.json`.
- CLI-ն առաքում է `iroha compute simulate` և `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` ապրում է
  `javascript/iroha_js/src/compute.js` ռեգրեսիոն թեստերով տակ
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift. `ComputeSimulator`-ը բեռնում է նույն հարմարանքները, վավերացնում է բեռնաթափման հեշերը,
  և մոդելավորում է մուտքի կետերը թեստերով
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift օգնականները բոլորը կիսում են նույն Norito հարմարանքները, որպեսզի SDK-ները կարողանան
  վավերացնել հարցումների կառուցումը և հեշ մշակումը անցանց՝ առանց a հարվածելու
  վազող դարպաս: