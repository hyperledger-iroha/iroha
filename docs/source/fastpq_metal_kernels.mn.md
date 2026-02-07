---
lang: mn
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

# FASTPQ металл цөмийн багц

Apple Silicon backend нь бүх зүйлийг агуулсан ганц `fastpq.metallib`-ийг нийлүүлдэг.
Металл сүүдэрлэх хэл (MSL) цөм нь prover хэрэгжүүлсэн. Энэ тэмдэглэлийг тайлбарлав
боломжтой нэвтрэх цэгүүд, тэдгээрийн хэлхээний бүлгийн хязгаар, детерминизм
GPU замыг скаляр буцаалттай сольж болох баталгаа.

Каноник хэрэгжилт нь доор амьдардаг
`crates/fastpq_prover/metal/kernels/` ба эмхэтгэсэн
MacOS дээр `fastpq-gpu` идэвхжсэн үед `crates/fastpq_prover/build.rs`.
Ажиллах үеийн мета өгөгдөл (`metal_kernel_descriptors`) нь доорх мэдээллийг тусгадаг
жишиг болон оношлогоо нь ижил баримтуудыг харуулж чадна программын хувьд.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Цөмийн тооллого| Нэвтрэх цэг | Үйл ажиллагаа | Threadgroup cap | Хавтанцарын тайзны таг | Тэмдэглэл |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | Мөр багана дээр FFT-ийг урагшлуулах | 256 утас | 32 үе шат | Эхний үе шатанд хуваалцсан санах ойн хавтанг ашигладаг бөгөөд төлөвлөгч IFFT горимыг хүсэх үед урвуу масштабыг ашигладаг.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262
| `fastpq_fft_post_tiling` | Хавтангийн гүнд хүрсэний дараа FFT/IFFT/LDE-г гүйцээнэ | 256 утас | — | Үлдсэн эрвээхэйг төхөөрөмжийн санах ойноос шууд ажиллуулж, хост руу буцахаасаа өмнө эцсийн косет/урвуу хүчин зүйлсийг зохицуулдаг.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Багана хоорондын бага зэргийн өргөтгөл | 256 утас | 32 үе шат | Коэффициентийг үнэлгээний буферт хуулж, тохируулсан косетоор хавтанцар үе шатуудыг гүйцэтгэж, эцсийн шатыг `fastpq_fft_post_tiling` болгон үлдээдэг. хэрэгтэй.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Хэш багана болон тооцоолох гүн-1 эцэг эхийг нэг дамжуулалтаар | 256 утас | — | `poseidon_hash_columns`-тэй ижил шимэгдэлт/оролцолтыг ажиллуулж, навчны шингээлтийг шууд гаралтын буферт хадгалж, `(left,right)` хос бүрийг `fastpq:v1:trace:node` домэйны дор нэн даруй нугалж, `(⌈columns / 2⌉)` эцэг эх нь навчны дараа газардах болно. Сонирхолтой баганын тоонууд нь төхөөрөмж дээрх эцсийн хуудасны давхаргыг хуулбарлаж, эхний Merkle давхаргын дараагийн цөм болон CPU-ийн нөөцийг арилгана.【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src:2/407rs】rs.
| `poseidon_permute` | Poseidon2 солих (STATE_WIDTH = 3) | 256 утас | — | Threadgroups нь урсгалын бүлгийн санах ойд дугуй тогтмолууд/MDS мөрүүдийг кэш болгож, MDS мөрүүдийг урсгал тус бүрийн бүртгэлд хуулж, төлөвүүдийг 4 төлөвт хэсгүүдэд боловсруулдаг тул дугуй тогтмол таталт бүрийг урагшлуулахын өмнө олон төлөвт дахин ашигладаг. Тойрог бүрэн тайлагдаагүй хэвээр байх бөгөөд эгнээ бүр олон мужийг алхсан хэвээр байгаа нь нэг илгээлтэд ≥4096 логик хэлхээг баталгаажуулдаг. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` хөөргөх өргөн болон эгнээ тус бүрийн багцыг дахин бүтээхгүйгээр тогтооно. metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

Тодорхойлогчийг ажиллуулах үед дамжуулан авах боломжтой
`fastpq_prover::metal_kernel_descriptors()` нь харуулахыг хүссэн багаж хэрэгсэлд зориулагдсан
ижил мета өгөгдөл.

## Детерминист Goldilocks арифметик- Бүх цөм нь Goldilocks талбар дээр тодорхойлогдсон туслахуудтай ажилладаг
  `field.metal` (modular add/mul/sub, урвуу, `pow5`).【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE үе шатууд нь CPU төлөвлөгчийн гаргадаг ижил twiddle хүснэгтүүдийг дахин ашигладаг.
  `compute_stage_twiddles` нь үе шат бүрт нэг эргэлдэж, хостыг урьдчилан тооцоолдог.
  илгээх бүрийн өмнө 1-р буферийн үүрээр массивыг байршуулж, баталгаажуулдаг
  GPU зам нь нэгдмэл байдлын ижил үндэсийг ашигладаг.【crates/fastpq_prover/src/metal.rs:1527】
- LDE-д зориулсан Coset үржүүлгийг эцсийн шатанд нэгтгэсэн тул GPU хэзээ ч ажиллахгүй
  CPU-ийн ул мөрийн зохион байгуулалтаас ялгаатай; хост нь үнэлгээний буферийг тэгээр дүүргэдэг
  илгээхээс өмнө дүүргэлтийн зан төлөвийг тодорхойлогч байлгах.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Металлибын үе

`build.rs` `.metal` эх сурвалжуудыг `.air` объект болгон эмхэтгээд дараа нь
тэдгээрийг `fastpq.metallib` руу холбож, дээр дурдсан бүх нэвтрэх цэг бүрийг экспортолдог.
`FASTPQ_METAL_LIB`-г тэр замд тохируулснаар (бүтээх скрипт үүнийг хийдэг
автоматаар) нь ажиллах цагийг үл харгалзан номын санг тодорхой хэмжээгээр ачаалах боломжийг олгодог
`cargo` бүтээх олдворуудыг байрлуулсан газар.【crates/fastpq_prover/build.rs:45】

CI гүйлтүүдтэй ижил байхын тулд та номын санг гараар дахин үүсгэж болно:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Threadgroup хэмжээг тодорхойлох эвристик

`metal_config::fft_tuning` нь төхөөрөмжийн гүйцэтгэлийн өргөн ба нэг бүрийн хамгийн их урсгалыг дамжуулдаг.
Threadgroup төлөвлөгч рүү оруулснаар ажиллах хугацааны илгээмжүүд техник хангамжийн хязгаарлалтыг хүндэтгэдэг.
Бүртгэлийн хэмжээ ихсэх тусам өгөгдмөл нь 32/64/128/256 эгнээнд бэхлэгддэг.
хавтангийн гүн нь одоо `log_len ≥ 12` дээр таваас дөрөв хүртэл алхаж, дараа нь
12/14/16 үе шат дамжсан дундын санах ойн дамжуулалт идэвхтэй байна
Хавтанцарын дараах цөмд ажлыг хүлээлгэн өгөхөөс өмнө `log_len ≥ 18/20/22`. Оператор
хүчингүй болгох (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) дамжин өнгөрөх
`FftArgs::threadgroup_lanes`/`local_stage_limit` ба цөмд ашиглагддаг
металлибыг дахин бүтээхгүйгээр дээрх.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

`fastpq_metal_bench` ашиглан шийдэгдсэн тааруулах утгыг авч, баталгаажуулна уу
олон нэвтрүүлэх цөмүүдийг өмнө нь ашигласан (JSON-д `post_tile_dispatches`)
жишиг багцыг хүргэж байна.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】