---
lang: hy
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ Metal Kernel Suite

Apple Silicon backend-ը առաքում է մեկ `fastpq.metallib`, որը պարունակում է ամեն
Metal Shading Language (MSL) միջուկը, որն օգտագործվում է պրովերի կողմից: Այս նշումը բացատրում է
հասանելի մուտքի կետերը, դրանց խմբերի սահմանները և դետերմինիզմը
երաշխիքներ, որոնք դարձնում են GPU-ի ուղին փոխարինելի սկալյար հետադարձի հետ:

Կանոնական իրականացումը ապրում է տակ
`crates/fastpq_prover/metal/kernels/` և կազմված է
`crates/fastpq_prover/build.rs`, երբ `fastpq-gpu`-ը միացված է macOS-ում:
Գործարկման ժամանակի մետատվյալները (`metal_kernel_descriptors`) արտացոլում են ստորև բերված տեղեկատվությունը
Հենանիշերն ու ախտորոշիչները կարող են ի հայտ բերել նույն փաստերը ծրագրային կերպով։【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## միջուկի գույքագրում| Մուտքի կետ | Շահագործում | Թելային խմբի գլխարկ | Սալիկի բեմի գլխարկ | Ծանոթագրություններ |
| ----------- | --------- | ---------------- | --------------- | ----- |
| `fastpq_fft_columns` | Փոխանցել FFT-ը հետքի սյունակների վրայով | 256 թել | 32 փուլ | Առաջին փուլերի համար օգտագործում է ընդհանուր հիշողության սալիկներ և կիրառում է հակադարձ մասշտաբավորում, երբ պլանավորողը պահանջում է IFFT ռեժիմ:【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:26
| `fastpq_fft_post_tiling` | Ավարտում է FFT/IFFT/LDE սալիկի խորությունը հասնելուց հետո | 256 թել | — | Մնացած թիթեռները անմիջապես դուրս է հանում սարքի հիշողությունից և մշակում է վերջնական կոսետը/հակադարձ գործոնները՝ նախքան հոսթ վերադառնալը:【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Ցածր աստիճանի ընդլայնում սյունակների միջով | 256 թել | 32 փուլ | Պատճենում է գործակիցները գնահատման բուֆերում, կատարում է սալիկապատ փուլերը կազմաձևված կոսետով և վերջնական փուլերը թողնում է `fastpq_fft_post_tiling`-ին, երբ անհրաժեշտ է:【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Հաշել սյունակները և հաշվարկել խորությունը‑1 ծնող մեկ անցումով | 256 թել | — | Գործարկում է նույն կլանումը/փոխարկումը, ինչ `poseidon_hash_columns`-ը, պահում է տերևների մարսողությունը անմիջապես ելքային բուֆերի մեջ և անմիջապես ծալում է `(left,right)` յուրաքանչյուր զույգ `fastpq:v1:trace:node` տիրույթի տակ, որպեսզի `(⌈columns / 2⌉)` ծնողները վայրէջք կատարեն: Սյունակների տարօրինակ թվերը կրկնօրինակում են սարքի վրա գտնվող վերջին տերևը՝ վերացնելով հաջորդող միջուկը և CPU-ի հետադարձ կապը Merkle-ի առաջին շերտի համար:【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src240al
| `poseidon_permute` | Poseidon2 փոխակերպում (STATE_WIDTH = 3) | 256 թել | — | Թելային խմբերը պահում են կլոր հաստատունները/MDS տողերը թելային խմբի հիշողության մեջ, պատճենում են MDS տողերը յուրաքանչյուր շղթայի գրանցամատյաններում և գործընթացի վիճակները 4 վիճակի կտորներով, այնպես որ յուրաքանչյուր կլոր հաստատուն բեռնումը նորից օգտագործվի մի քանի վիճակներում՝ առաջ անցնելուց առաջ: Շրջանները մնում են ամբողջությամբ բացված, և յուրաքանչյուր գիծ դեռևս անցնում է մի քանի վիճակներով՝ երաշխավորելով ≥4096 տրամաբանական շղթաներ մեկ ուղարկման համար: `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` ամրացրեք մեկնարկի լայնությունը և յուրաքանչյուր գծի խմբաքանակ՝ առանց վերակառուցելու metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

Նկարագրիչները հասանելի են գործարկման ժամանակ՝ միջոցով
`fastpq_prover::metal_kernel_descriptors()` գործիքակազմի համար, որը ցանկանում է ցուցադրել
նույն մետատվյալները:

## Դետերմինիստական ​​Ոսկիների թվաբանություն- Բոլոր միջուկներն աշխատում են Goldilocks դաշտի վրա՝ նշված օգնականներով
  `field.metal` (մոդուլային ավելացնել/mul/sub, հակադարձ, `pow5`):【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE փուլերը վերօգտագործում են նույն ծալքավոր աղյուսակները, որոնք արտադրում է պրոցեսորի պլանավորողը:
  `compute_stage_twiddles`-ը նախապես հաշվարկում է մեկ twiddle յուրաքանչյուր փուլի և հյուրընկալողի համար
  վերբեռնում է զանգվածը բուֆերային բնիկով 1 յուրաքանչյուր առաքումից առաջ՝ երաշխավորելով
  GPU ուղին օգտագործում է միասնության նույնական արմատներ:【crates/fastpq_prover/src/metal.rs:1527】
- Կոզետների բազմապատկումը LDE-ի համար միաձուլվում է վերջնական փուլում, այնպես որ GPU-ն երբեք
  շեղվում է պրոցեսորի հետքի դասավորությունից. հյուրընկալողը զրո-լրացնում է գնահատման բուֆերը
  առաքումից առաջ՝ պահպանելով լցոնման վարքագիծը որոշիչ։【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib սերունդ

`build.rs`-ը հավաքում է `.metal` առանձին աղբյուրները `.air` օբյեկտների մեջ, այնուհետև
դրանք կապում է `fastpq.metallib`-ի հետ՝ արտահանելով վերը թվարկված յուրաքանչյուր մուտքի կետ:
Սահմանելով `FASTPQ_METAL_LIB`-ը այդ ճանապարհին (կառուցման սցենարը դա անում է
ինքնաբերաբար) թույլ է տալիս գործարկման ժամանակը որոշիչ կերպով բեռնել գրադարանը, անկախ նրանից
որտեղ `cargo` տեղադրեց շինարարական արտեֆակտները:【crates/fastpq_prover/build.rs:45】

CI գործարկումների հետ հավասարության համար կարող եք ձեռքով վերականգնել գրադարանը՝

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Թելային խմբի չափերի էվրիստիկա

`metal_config::fft_tuning` թելեր է սարքի կատարման լայնությունը և առավելագույն թելերը մեկ
threadgroup մեջ պլանավորող, այնպես որ գործարկման ժամանակի առաքումները հարգում են ապարատային սահմանափակումները:
Լռելյայն սեղմիչը սեղմվում է 32/64/128/256 գծերի վրա, քանի որ լոգի չափը մեծանում է, և
սալիկի խորությունը այժմ անցնում է հինգ փուլից մինչև չորս `log_len ≥ 12`-ում, այնուհետև պահպանում է
Համօգտագործվող հիշողության անցաթուղթը ակտիվ է 12/14/16 փուլերի համար, երբ հետքը հատվի
`log_len ≥ 18/20/22` նախքան աշխատանքը սալիկապատման միջուկին հանձնելը: Օպերատոր
գերազանցում է (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) հոսում
`FftArgs::threadgroup_lanes`/`local_stage_limit` և կիրառվում են միջուկների կողմից
վերևում առանց մետաղալբի վերակառուցման:【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Օգտագործեք `fastpq_metal_bench`՝ կարգավորելու լուծված արժեքները գրավելու և դա հաստատելու համար
բազմապատիկ միջուկները գործարկվել են (`post_tile_dispatches` JSON-ում) նախկինում
ուղենշային փաթեթի առաքում:【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】