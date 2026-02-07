---
lang: uz
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

# FASTPQ metall yadro to'plami

Apple Silicon backend har bir narsani o'z ichiga olgan bitta `fastpq.metallib` ni yetkazib beradi.
Prover tomonidan qo'llaniladigan metall Shading Language (MSL) yadrosi. Bu eslatma tushuntiradi
mavjud kirish nuqtalari, ularning ish zarralari chegaralari va determinizm
GPU yo'lini skalyar qaytarilish bilan almashtirishni kafolatlaydi.

Kanonik amalga oshirish ostida yashaydi
`crates/fastpq_prover/metal/kernels/` va tomonidan tuzilgan
MacOS tizimida `fastpq-gpu` yoqilganda `crates/fastpq_prover/build.rs`.
Ish vaqti metama'lumotlari (`metal_kernel_descriptors`) quyidagi ma'lumotlarni aks ettiradi
mezon va diagnostika bir xil faktlarni yuzaga keltirishi mumkin dasturiy jihatdan.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Yadro inventarizatsiyasi| Kirish nuqtasi | Operatsiya | Mavzular guruhi qopqog'i | Plitka sahna qopqog'i | Eslatmalar |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | FFT-ni iz ustunlari ustidan yo'naltirish | 256 mavzu | 32 bosqich | Birinchi bosqichlar uchun umumiy xotira plitalaridan foydalanadi va rejalashtiruvchi IFFT rejimini talab qilganda teskari masshtabni qo'llaydi.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | Plitka chuqurligiga erishilgandan so'ng FFT/IFFT/LDE ni yakunlaydi | 256 mavzu | — | Qolgan kapalaklarni to'g'ridan-to'g'ri qurilma xotirasidan chiqaradi va xostga qaytishdan oldin yakuniy koset/teskari omillarni boshqaradi.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Ustunlar bo'ylab past darajali kengaytma | 256 mavzu | 32 bosqich | Koeffitsientlarni baholash buferiga ko'chiradi, sozlangan koset bilan qoplangan bosqichlarni bajaradi va yakuniy bosqichlarni `fastpq_fft_post_tiling` ga qoldiradi. kerak.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Xesh ustunlari va hisoblash chuqurligi‑1 ota-onani bir o‘tishda | 256 mavzu | — | `poseidon_hash_columns` bilan bir xil yutilish/o'zgartirishni amalga oshiradi, barg hazm qilishlarini to'g'ridan-to'g'ri chiqish buferiga saqlaydi va darhol `(left,right)` juftligini `fastpq:v1:trace:node` domeni ostida buklaydi, shunda `(⌈columns / 2⌉)` ota-onalar bargdan keyin erga tushadilar. G'alati ustunlar soni qurilmadagi oxirgi bargni takrorlaydi, birinchi Merkle qatlami uchun keyingi yadro va protsessor zaxirasini yo'q qiladi.【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src:2/407rs.】.
| `poseidon_permute` | Poseidon2 almashtirish (STATE_WIDTH = 3) | 256 mavzu | — | Mavzular guruhlari dumaloq konstantalarni/MDS satrlarini mavzular guruhi xotirasida keshlaydi, MDS satrlarini har bir mavzu registrlariga ko'chiradi va holatlarni 4-holatli bo'laklarda qayta ishlaydi, shuning uchun har bir davra konstantasi oldinga siljishdan oldin bir nechta holatlar bo'ylab qayta ishlatiladi. Raundlar to'liq ochilgan bo'lib qoladi va har bir bo'lak hali ham bir nechta shtatlarni bosib o'tadi va har bir jo'natma uchun ≥4096 mantiqiy ipni kafolatlaydi. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` ishga tushirish kengligi va har bir chiziqli partiyani qayta tiklamasdan mahkamlang. metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

Deskriptorlar ish vaqtida orqali mavjud
Ko'rsatmoqchi bo'lgan asboblar uchun `fastpq_prover::metal_kernel_descriptors()`
bir xil metama'lumotlar.

## Deterministik Goldilocks arifmetikasi- Barcha yadrolar Goldilocks maydonida belgilangan yordamchilar bilan ishlaydi
  `field.metal` (modulli qo'shimcha/mul/sub, teskari, `pow5`).【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE bosqichlari protsessor rejalashtiruvchisi ishlab chiqaradigan bir xil twiddle jadvallarini qayta ishlatadi.
  `compute_stage_twiddles` har bir bosqichda bir twiddli va xostni oldindan hisoblaydi
  Har bir jo'natishdan oldin massivni 1-bufer uyasi orqali yuklaydi, bu esa kafolatlanadi
  GPU yo'li bir xil ildizlardan foydalanadi.【crates/fastpq_prover/src/metal.rs:1527】
- LDE uchun Coset ko'paytirish yakuniy bosqichga birlashtiriladi, shuning uchun GPU hech qachon
  protsessor kuzatuvi tartibidan ajralib turadi; xost baholash buferini nol bilan to'ldiradi
  jo'natishdan oldin, to'ldirish xatti-harakatlarini deterministik holda saqlang.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib avlodi

`build.rs` individual `.metal` manbalarini `.air` obyektlariga kompilyatsiya qiladi va keyin
ularni `fastpq.metallib` ga bog'laydi va yuqorida sanab o'tilgan har bir kirish nuqtasini eksport qiladi.
`FASTPQ_METAL_LIB`ni ushbu yo'lga o'rnatish (qurilish skripti buni amalga oshiradi
avtomatik) ish vaqti kutubxonani qat'iy nazar yuklash imkonini beradi
`cargo` qurilish artefaktlarini joylashtirgan joydan.【crates/fastpq_prover/build.rs:45】

CI ishlanmalari bilan tenglik uchun kutubxonani qo'lda qayta tiklashingiz mumkin:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Mavzular guruhini o'lchash evristikasi

`metal_config::fft_tuning` qurilmaning ishlash kengligi va har biriga maksimal iplarni o'tkazadi
ish vaqti jo'natmalari apparat chegaralarini hurmat qilish uchun rejalashtiruvchiga ish zarrachalarini kiriting.
Standartlar log o'lchami kattalashgani sayin 32/64/128/256 qatorlarga mahkamlanadi va
plitka chuqurligi endi `log_len ≥ 12` da besh bosqichdan to'rt bosqichga o'tadi, keyin esa
umumiy xotira o'tish 12/14/16 bosqichlari uchun faol iz kesib bir marta
Ishni plitkadan keyingi yadroga topshirishdan oldin `log_len ≥ 18/20/22`. Operator
bekor qilish (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) orqali oqim
`FftArgs::threadgroup_lanes`/`local_stage_limit` va yadrolar tomonidan qo'llaniladi
yuqoridagi metallibni qayta tiklamasdan.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Yechilgan sozlash qiymatlarini olish va buni tekshirish uchun `fastpq_metal_bench` dan foydalaning
ko'p o'tishli yadrolar (JSONda `post_tile_dispatches`) oldin ishlatilgan
benchmark to'plamini jo'natish.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】