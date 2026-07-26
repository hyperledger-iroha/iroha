---
lang: az
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

Apple Silicon arxa ucu hər şeyi ehtiva edən tək `fastpq.metallib` göndərir.
Prover tərəfindən istifadə edilən Metal Shading Language (MSL) nüvəsi. Bu qeyd izah edir
mövcud giriş nöqtələri, onların mövzu qrupları məhdudiyyətləri və determinizm
GPU yolunu skalyar geriləmə ilə əvəz edə bilən zəmanətlər.

Kanonik tətbiqi altında yaşayır
`crates/fastpq_prover/metal/kernels/` və tərtib edir
MacOS-da `fastpq-gpu` aktiv olduqda `crates/fastpq_prover/build.rs`.
İcra zamanı metadata (`metal_kernel_descriptors`) aşağıdakı məlumatları əks etdirir
meyarlar və diaqnostika eyni faktları üzə çıxara bilər proqramlı olaraq.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal /kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Kernel inventar| Giriş nöqtəsi | Əməliyyat | Mövzu qrupu qapağı | Kafel səhnə qapağı | Qeydlər |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | FFT-ni iz sütunları üzərində yönləndirin | 256 mövzu | 32 mərhələ | İlk mərhələlər üçün paylaşılan yaddaş plitələrindən istifadə edir və planlaşdırıcı IFFT rejimi tələb etdikdə tərs miqyaslama tətbiq edir.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | Kafel dərinliyinə çatdıqdan sonra FFT/IFFT/LDE-ni tamamlayır | 256 mövzu | — | Qalan kəpənəkləri birbaşa cihazın yaddaşından çıxarır və hosta qayıtmazdan əvvəl son koset/ters amilləri idarə edir.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Sütunlar arasında aşağı dərəcə uzadılması | 256 mövzu | 32 mərhələ | Əmsalları qiymətləndirmə buferinə köçürür, konfiqurasiya edilmiş koset ilə döşənmiş mərhələləri yerinə yetirir və son mərhələləri `fastpq_fft_post_tiling`-a buraxır. lazımdır.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Hash sütunları və hesablama dərinliyi‑1 valideyn bir keçiddə | 256 mövzu | — | `poseidon_hash_columns` ilə eyni udma/permütasiyanı həyata keçirir, yarpaq həzmlərini birbaşa çıxış buferində saxlayır və hər bir `(left,right)` cütünü dərhal `fastpq:v1:trace:node` domeni altında qatlayır, beləliklə `(⌈columns / 2⌉)` valideynlərin yarpaqdan sonra yerə enməsi. Tək sütunların sayları cihazdakı son vərəqi təkrarlayır, ilk Merkle təbəqəsi üçün təqib nüvəsini və CPU ehtiyatını ləğv edir.
| `poseidon_permute` | Poseidon2 permutasiyası (STATE_WIDTH = 3) | 256 mövzu | — | Mövzu qrupları dəyirmi sabitləri/MDS cərgələrini mövzu qrupunun yaddaşında saxlayır, MDS sətirlərini hər mövzu registrlərinə köçürür və vəziyyətləri 4 ştatlı hissələrdə emal edir ki, hər bir dəyirmi sabit gətirmə irəliləməzdən əvvəl bir çox dövlətlər arasında təkrar istifadə olunsun. Turlar tam açılmamış qalır və hər bir zolaq hələ də bir neçə ştatı gəzir və hər göndəriş üçün ≥4096 məntiqi ipliklərə zəmanət verir. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` başlanğıc enini və hər zolaqlı partiyanı yenidən qurmadan pin edin metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

Təsviredicilər iş vaxtında vasitəsilə mövcuddur
Göstərmək istəyən alətlər üçün `fastpq_prover::metal_kernel_descriptors()`
eyni metadata.

## Deterministik Goldilocks arifmetikası- Bütün ləpələr Goldilocks sahəsi üzərində müəyyən edilmiş köməkçilərlə işləyir
  `field.metal` (modul əlavə/mul/sub, tərs, `pow5`).【Crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE mərhələləri CPU planlayıcısının istehsal etdiyi eyni əyilmə cədvəllərini təkrar istifadə edir.
  `compute_stage_twiddles` hər mərhələdə və ev sahibi üçün bir twiddle əvvəlcədən hesablayır
  massivi hər göndərişdən əvvəl 1-ci bufer yuvası vasitəsilə yükləyir və bu, təmin edir
  GPU yolu birliyin eyni köklərindən istifadə edir.【crates/fastpq_prover/src/metal.rs:1527】
- LDE üçün Coset vurma son mərhələyə birləşdirilir, beləliklə GPU heç vaxt
  CPU izləmə sxemindən fərqlənir; ev sahibi qiymətləndirmə buferini sıfırla doldurur
  göndərilməzdən əvvəl doldurma davranışını deterministik saxlamaqla.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib nəsli

`build.rs` fərdi `.metal` mənbələrini `.air` obyektlərində tərtib edir və sonra
yuxarıda sadalanan hər bir giriş nöqtəsini ixrac edərək onları `fastpq.metallib` ilə əlaqələndirir.
`FASTPQ_METAL_LIB`-ni həmin yola təyin etmək (quraşdırma skripti bunu edir
avtomatik olaraq) işləmə müddətinin kitabxananı deterministik olaraq yükləməsinə imkan verir
`cargo` qurma artefaktlarını yerləşdirdiyi yer.【crates/fastpq_prover/build.rs:45】

CI əməliyyatları ilə paritet üçün kitabxananı əl ilə bərpa edə bilərsiniz:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Mövzu qrupunun ölçülməsi evristikası

`metal_config::fft_tuning` cihazın icra enini və hər bir maks.
iş vaxtı göndərişləri aparat məhdudiyyətlərinə riayət etməsi üçün planlayıcıya mövzu qrupunu birləşdirin.
Standartlar log ölçüsü artdıqca 32/64/128/256 zolaqlarına sıxılır və
kafel dərinliyi indi `log_len ≥ 12`-də beş mərhələdən dördə qədər gedir, sonra
paylaşılan yaddaş keçidi iz keçdikdən sonra 12/14/16 mərhələləri üçün aktivdir
`log_len ≥ 18/20/22` işi plitədən sonrakı nüvəyə təhvil verməzdən əvvəl. Operator
keçərlər (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) vasitəsilə axır
`FftArgs::threadgroup_lanes`/`local_stage_limit` və nüvələr tərəfindən tətbiq olunur
metallibi yenidən qurmadan yuxarıda.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Həll edilmiş tənzimləmə dəyərlərini əldə etmək və bunu yoxlamaq üçün `fastpq_metal_bench` istifadə edin
çox keçidli ləpələr (JSON-da `post_tile_dispatches`) əvvəl istifadə edilmişdir.
benchmark paketinin göndərilməsi.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】