---
lang: kk
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

# FASTPQ металл ядросы жиынтығы

Apple Silicon сервері барлығын қамтитын жалғыз `fastpq.metallib` жеткізеді.
Провер қолданатын металл көлеңкелеу тілі (MSL) ядросы. Бұл жазба түсіндіреді
қолжетімді кіру нүктелері, олардың ағындар тобының шектеулері және детерминизм
GPU жолын скалярлық қалпына келтірумен алмастыруға болатын кепілдіктер.

Канондық іске асыру астында өмір сүреді
`crates/fastpq_prover/metal/kernels/` және құрастырған
`crates/fastpq_prover/build.rs` MacOS жүйесінде `fastpq-gpu` қосылған сайын.
Орындалу уақытының метадеректері (`metal_kernel_descriptors`) төмендегі ақпаратты көрсетеді
эталондар мен диагностика бірдей фактілерді көрсете алады бағдарламалық түрде.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Ядролық түгендеу| Кіру нүктесі | Операция | Тақырып тобының қақпағы | Плитка сатысының қақпағы | Ескертпелер |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | FFT тізбегі бағандары бойынша бағыттау | 256 ағын | 32 кезең | Бірінші кезеңдер үшін ортақ жад тақталарын пайдаланады және жоспарлаушы IFFT режимін сұраған кезде кері масштабтауды қолданады.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | FFT/IFFT/LDE плитка тереңдігіне жеткеннен кейін аяқтайды | 256 ағын | — | Қалған көбелектерді тікелей құрылғы жадынан шығарады және хостқа оралмас бұрын соңғы косет/кері факторларды өңдейді.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Бағандар бойынша төмен дәрежелі кеңейту | 256 ағын | 32 кезең | Коэффициенттерді бағалау буферіне көшіреді, конфигурацияланған косетпен қапталған кезеңдерді орындайды және соңғы кезеңдерді `fastpq_fft_post_tiling` күйіне қалдырады. қажет.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Хэш бағандары және есептеу тереңдігі‑1 ата-ананы бір өтуде | 256 ағын | — | `poseidon_hash_columns` сияқты жұтылу/орналастыруды іске қосады, жапырақ дайджесттерін тікелей шығыс буферіне сақтайды және `(left,right)` жұбын `fastpq:v1:trace:node` доменінің астына бірден бүктейді, осылайша `(⌈columns / 2⌉)` ата-аналар жапырақтан кейін жерге түседі. Тақ бағандар саны құрылғыдағы соңғы жапырақтың көшірмесін жасайды, бірінші Merkle қабаты үшін кейінгі ядро мен орталық процессордың резервін жояды.【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src:2/407rs.】.
| `poseidon_permute` | Посейдон2 ауыстыру (STATE_WIDTH = 3) | 256 ағын | — | Ағындар топтары дөңгелек константаларды/MDS жолдарын ағындар тобының жадында кэштейді, MDS жолдарын әр ағындық регистрлерге көшіреді және күйлерді 4-күй бөліктерінде өңдейді, осылайша әрбір дөңгелек тұрақты алу алға жылжу алдында бірнеше күйлерде қайта пайдаланылады. Раундтар толығымен оралмайды және әрбір жолақ әлі де бірнеше күйлерді басып өтеді, бұл әр жөнелтуде ≥4096 логикалық ағынға кепілдік береді. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` іске қосу енін және әр жолақты пакетті қайта құрмай-ақ бекітіңіз. metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

Дескрипторлар орындалу уақытында арқылы қол жетімді
Көрсеткісі келетін құралға арналған `fastpq_prover::metal_kernel_descriptors()`
бірдей метадеректер.

## Детерминистік Goldilocks арифметикасы- Барлық ядролар Goldilocks өрісінде анықталған көмекшілермен жұмыс істейді
  `field.metal` (модульдік қосу/mul/sub, кері мәндер, `pow5`).【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE кезеңдері процессорды жоспарлаушы шығаратын бірдей бұралмалы кестелерді қайта пайдаланады.
  `compute_stage_twiddles` әр кезеңде және хост үшін бір иілуді алдын ала есептейді
  әрбір жөнелту алдында алапты буфер ұяшығы 1 арқылы жүктеп салады, бұл кепілдік береді
  GPU жолы бірліктің бірдей түбірлерін пайдаланады.【crates/fastpq_prover/src/metal.rs:1527】
- LDE үшін косеталық көбейту соңғы кезеңге біріктірілген, сондықтан GPU ешқашан болмайды
  процессорды бақылау схемасынан алшақтайды; хост бағалау буферін нөлмен толтырады
  жіберу алдында толтыру әрекетін детерминистік түрде сақтай отырып.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Металлиб ұрпағы

`build.rs` жеке `.metal` көздерін `.air` нысандарына құрастырады, содан кейін
жоғарыда аталған әрбір кіру нүктесін экспорттай отырып, оларды `fastpq.metallib` ішіне байланыстырады.
`FASTPQ_METAL_LIB` сол жолға орнату (құрастыру сценарийі мұны жасайды
автоматты түрде) орындау уақытына қарамастан кітапхананы анықтауға мүмкіндік береді
`cargo` құрастыру артефактілерін орналастырды.【crates/fastpq_prover/build.rs:45】

CI іске қосуларымен теңдік үшін кітапхананы қолмен қалпына келтіруге болады:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Тақырып тобының өлшемін анықтау эвристикасы

`metal_config::fft_tuning` құрылғының орындалу енін және ең көп ағындарды ағындарын
жіптер тобын жоспарлаушыға енгізіңіз, осылайша орындалу уақыты жөнелтілімдері аппараттық құрал шектеулерін сақтайды.
Әдепкілер журнал өлшемі ұлғайған сайын 32/64/128/256 жолақтарға қысылады және
плитка тереңдігі енді `log_len ≥ 12` бес сатыдан төртке дейін жүреді, содан кейін
ортақ жад өтуі із кесіп өткеннен кейін 12/14/16 кезеңдері үшін белсенді
`log_len ≥ 18/20/22` жұмысты плиткадан кейінгі ядроға тапсыру алдында. Оператор
қайта анықтау (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) арқылы өтетін
`FftArgs::threadgroup_lanes`/`local_stage_limit` және ядролар арқылы қолданылады
жоғарыда metallib қалпына келтірілмейді.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Шешілген реттеу мәндерін түсіру және оны тексеру үшін `fastpq_metal_bench` пайдаланыңыз
көп өту ядролары бұрын орындалған (JSON ішінде `post_tile_dispatches`)
эталондық жинақты жеткізу.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】