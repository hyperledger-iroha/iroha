---
lang: uz
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ Prover ishining parchalanishi

Ushbu hujjat ishlab chiqarishga tayyor FASTPQ-ISI proverini yetkazib berish va uni ma'lumotlar maydonini rejalashtirish quvuriga ulash uchun bosqichma-bosqich rejani qamrab oladi. Quyidagi har bir ta'rif, agar TODO sifatida belgilanmagan bo'lsa, normativ hisoblanadi. Hisoblangan mustahkamlik Qohira uslubidagi DEEP-FRI chegaralaridan foydalanadi; Agar o'lchangan chegara 128 bitdan pastga tushsa, CIda avtomatik rad etish-namuna olish sinovlari muvaffaqiyatsizlikka uchraydi.

## 0-bosqich - Xesh to'ldiruvchisi (qo'ndi)
- BLAKE2b majburiyati bilan deterministik Norito kodlash.
- `BackendUnavailable` qaytaruvchi to'ldiruvchi backend.
- `fastpq_isi` tomonidan taqdim etilgan kanonik parametrlar jadvali.

## 1-bosqich - Trace Builder prototipi

> **Holat (2025-11-09):** `fastpq_prover` endi kanonik qadoqlashni ochib beradi
> yordamchilar (`pack_bytes`, `PackedBytes`) va deterministik Poseidon2
> Goldilocks ustidan buyurtma berish majburiyati. Konstantalar qadalgan
> `ark-poseidon2` `3f2b7fe` majburiyatini bajarib, vaqtinchalik BLAKE2 ni almashtirishni yakunlaydi
> toʻldiruvchi yopilgan. Oltin armatura (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) endi regressiya to'plamini bog'laydi.

### Maqsadlar
- KV-yangilangan AIR uchun FASTPQ iz quruvchisini amalga oshiring. Har bir qator kodlashi kerak:
  - `key_limbs[i]`: kanonik kalit yo'lining 256 ta qismi (7 bayt, kichik-endian).
  - `value_old_limbs[i]`, `value_new_limbs[i]`: oldingi/post qiymatlari uchun bir xil qadoqlash.
  - Selektor ustunlari: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, I100NI67X, `s_perm`.
  - Yordamchi ustunlar: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Asset ustunlari: `asset_id_limbs[i]` 7 baytlik limblardan foydalangan holda.
  - `ℓ` darajasidagi SMT ustunlari: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, shuningdek aʼzo boʻlmaganlar uchun `neighbour_leaf`.
  - Metadata ustunlari: `dsid`, `slot`.
- **Deterministik tartiblash.** Barqaror tartiblash yordamida qatorlarni leksikografik jihatdan `(key_bytes, op_rank, original_index)` bo‘yicha tartiblang. `op_rank` xaritalash: `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, Prometheus. `original_index` - tartiblashdan oldin 0 ga asoslangan indeks. Olingan Poseidon2 buyurtma xeshini davom ettiring (domen yorlig'i `fastpq:v1:ordering`). Xesh preimageni `[domain_len, domain_limbs…, payload_len, payload_limbs…]` sifatida kodlang, bunda uzunliklar u64 maydon elementlari bo'lib, keyingi nol baytlar ajralib turadi.
- Guvohni qidirish: saqlangan `s_perm` ustuni (`s_role_grant` va `s_role_revoke` mantiqiy OR) 1 bo'lsa, `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` hosil qiling. Rol/ruxsat identifikatorlari qattiq kenglikdagi 32 bayt LE; davr 8 bayt LE.
- AIRdan oldin ham, ichida ham invariantlarni tatbiq eting: selektorlar o'zaro eksklyuziv, har bir aktivni saqlash, dsid/slot konstantalari.
- `N_trace = 2^k` (qatorlar sonining `pow2_ceiling`); `N_eval = N_trace * 2^b` bu erda `b` portlash ko'rsatkichi.
- Armatura va mulk sinovlarini taqdim eting:
  - Qaytish-qayta qadoqlash (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Buyurtma barqarorligi xesh (`tests/fixtures/ordering_hash.json`).
  - Partiya armaturalari (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### AIR ustun sxemasi
| Ustunlar guruhi | Ismlar | Tavsif |
| ----------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Faoliyat | `s_active` | Haqiqiy qatorlar uchun 1, to'ldirish uchun 0.                                                                                       |
| Asosiy | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Qadoqlangan Goldilocks elementlari (kichik endian, 7 baytlik limblar).                                                             |
| Aktiv | `asset_id_limbs[i]` | Oʻrnatilgan kanonik aktiv identifikatori (kichik endian, 7 baytlik limblar).                                                      |
| Selektorlar | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Cheklov: S selektorlar (jumladan, `s_perm`) = `s_active`; `s_perm` rollarni berish/bekor qilish qatorlarini aks ettiradi.              |
| Yordamchi | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | Cheklovlar, saqlash va audit yo'llari uchun ishlatiladigan davlat.                                                           |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Har bir darajadagi Poseidon2 kirish/chiqish va a'zo bo'lmaslik uchun qo'shni guvoh.                                         |
| Qidiruv | `perm_hash` | Ruxsat qidirish uchun Poseidon2 xesh (faqat `s_perm = 1` bilan cheklangan).                                            |
| Metadata | `dsid`, `slot` | Satrlar bo'ylab doimiy.                                                                                                 |### Matematika va cheklovlar
- **Field packing:** baytlar 7 baytlik qismlarga bo'lingan (kichik-endian). Har bir a'zo `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; a'zolarni rad etish ≥ Goldilocks moduli.
- **Balans/saqlash:** ruxsat `δ = value_new - value_old`. `asset_id` bo'yicha qatorlarni guruhlang. Har bir aktiv guruhining birinchi qatorida `r_asset_start = 1` ni belgilang (aks holda 0) va cheklang
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  Har bir aktiv guruhining oxirgi qatorida tasdiqlang
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  Transferlar cheklovlarni avtomatik ravishda qondiradi, chunki ularning d qiymatlari guruh bo'ylab nolga teng. Misol: agar `value_old = 100` va `value_new = 120` yalpiz qatorida bo'lsa, d = 20, shuning uchun yalpiz summasi +20 hissa qo'shadi va kuyish sodir bo'lmaganda yakuniy tekshirish nolga teng bo'ladi.
- **To'ldirish:** `s_active` bilan tanishtiring. Barcha qator cheklovlarini `s_active` ga ko'paytiring va qo'shni prefiksni kiriting: `s_active[i] ≥ s_active[i+1]`. To'ldiruvchi qatorlar (`s_active=0`) doimiy qiymatlarni saqlashi kerak, lekin boshqa tarzda cheklanmagan.
- **Xeshni buyurtma qilish:** Poseidon2 xesh (domeni `fastpq:v1:ordering`) satr kodlashlari ustida; audit mumkinligi uchun Public IO da saqlanadi.

## 2-bosqich — STARK Prover Core

### Maqsadlar
- Baholash vektorlarini kuzatish va qidirish bo'yicha Poseidon2 Merkle majburiyatlarini yarating. Parametrlar: tezlik=2, sig‘im=1, to‘liq aylanishlar=8, qisman aylanishlar=57, `ark-poseidon2` ga mahkamlangan konstantalar `3f2b7fe` (v0.3.0).
- Past darajadagi kengaytma: `D = { g^i | i = 0 .. N_eval-1 }` domenidagi har bir ustunni baholang, bu erda `N_eval = 2^{k+b}` Goldilocksning 2-adik sig'imini ajratadi. `g = ω^{(p-1)/N_eval}` `ω` bilan Goldilocksning o'zgarmas ibtidoiy ildizi va `p` moduli bo'lsin; asosiy kichik guruhdan foydalaning (koset yo'q). Transkriptga `g` ni yozing (`fastpq:v1:lde` yorlig'i).
- Kompozitsiya polinomlari: har bir `C_j` cheklovi uchun quyida keltirilgan daraja chegaralari bilan `F_j(X) = C_j(X) / Z_N(X)` shaklida.
- Argumentni qidirish (ruxsatlar): transkriptdan `γ` namunasi. Iz mahsulot `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Jadval mahsuloti `T = ∏_j (table_perm_j - γ)`. Chegara cheklovi: `Z_final / T = 1`.
- `r ∈ {8, 16}` arityli DEEP-FRI: har bir qatlam uchun `fastpq:v1:fri_layer_ℓ` yorlig'i bilan ildizni o'zlashtiring, `β_ℓ` namunasini (`fastpq:v1:beta_ℓ` yorlig'i) va `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` orqali katlayın.
- Tasdiqlash obyekti (Norito kodlangan):
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- proverni aks ettiruvchi prover; oltin transkriptlar bilan 1k/5k/20k qatorli izlarda regressiya to'plamini ishga tushiring.

### Buxgalteriya darajasi
| Cheklov | Bo'linishdan oldingi daraja | Selektordan keyingi daraja | Margin vs `deg(Z_N)` |
|------------|------------------------|------------------------|----------------------|
| Transfer/yalpiz/kuyishni saqlash | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Rol berish/bekor qilish qidiruvi | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Metadata to'plami | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT xeshi (har bir daraja uchun) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Katta mahsulotni qidiring | mahsulot munosabati | Yo'q | Chegara cheklovi |
| Chegara ildizlari / ta'minot jami | 0 | 0 | aniq |

To'ldirish qatorlari `s_active` orqali qayta ishlanadi; qo'g'irchoq qatorlar cheklovlarni buzmasdan izni `N_trace` ga uzaytiradi.## Kodlash va transkripsiya (Global)
- **Bayt qadoqlash:** tayanch-256 (7-bayt limblar, kichik-endian). `fastpq_prover/tests/packing.rs` da testlar.
- **Field kodlash:** kanonik Goldilocks (kichik endian 64-bit limb, rad ≥ p); Poseidon2 chiqishlari/SMT ildizlari 32 baytlik kichik endian massivlar sifatida seriallashtirilgan.
- **Transkript (Fiat–Shamir):**
  1. BLAKE2b `protocol_version`, `params_version`, `parameter_set`, `public_io` va Poseidon2 yorlig'ini (`fastpq:v1:init`) absorbe qiladi.
  2. `trace_root`, `lookup_root` (`fastpq:v1:roots`) ni absorbe qiling.
  3. `γ` (`fastpq:v1:gamma`) qidirib toping.
  4. `α_j` (`fastpq:v1:alpha_j`) kompozitsiya muammolarini chiqaring.
  5. Har bir FRI qatlam ildizi uchun `fastpq:v1:fri_layer_ℓ` bilan so'riladi, `β_ℓ` (`fastpq:v1:beta_ℓ`) hosil bo'ladi.
  6. So'rovlar indekslarini chiqaring (`fastpq:v1:query_index`).

  Teglar kichik ASCII; tekshiruvchilar namuna olishdan oldin mos kelmaslikni rad etadi. Oltin transkript moslamasi: `tests/fixtures/transcript_v1.json`.
- **Versiya:** `protocol_version = 1`, `params_version` `fastpq_isi` parametrlar toʻplamiga mos keladi.

## Argumentni qidirish (ruxsatlar)
- `(role_id_bytes, permission_id_bytes, epoch_le)` boʻyicha leksikografik saralangan va Poseidon2 Merkle daraxti orqali tuzilgan (`PublicIO` da `perm_root`) tuzilgan jadval.
- Kuzatuv guvohi `perm_hash` va `s_perm` selektoridan foydalanadi (yoki rolni berish/bekor qilish). Kortej belgilangan kengliklarga ega (32, 32, 8 bayt) `role_id_bytes || permission_id_bytes || epoch_u64_le` sifatida kodlangan.
- Mahsulot munosabati:
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Chegarani tasdiqlash: `Z_final / T = 1`. Beton akkumulyator haqida ma'lumot olish uchun `examples/lookup_grand_product.md` ga qarang.

## Merkle daraxtining siyrak cheklovlari
- `SMT_HEIGHT` (darajalar soni) ni aniqlang. `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` ustunlari barcha `ℓ ∈ [0, SMT_HEIGHT)` uchun paydo bo'ladi.
- `ark-poseidon2` ga mahkamlangan Poseidon2 parametrlari `3f2b7fe` (v0.3.0); domen yorlig'i `fastpq:v1:poseidon_node`. Barcha tugunlar kichik endian maydon kodlashdan foydalanadi.
- Har bir daraja uchun qoidalarni yangilang:
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- `(node_in_0 = 0, node_out_0 = value_new)` qo'shimchalar to'plami; `(node_in_0 = value_old, node_out_0 = 0)` to'plamini o'chiradi.
- A'zolik bo'lmagan dalillar so'ralgan interval bo'shligini ko'rsatish uchun `neighbour_leaf` bilan ta'minlaydi. Ishlangan misol va JSON tartibi uchun `examples/smt_update.md` ga qarang.
- Chegara cheklovi: yakuniy xesh oldingi qatorlar uchun `old_root` va keyingi qatorlar uchun `new_root` ga teng.

## Ovoz parametrlari va SLOlar
| N_trace | portlash | FRI arity | qatlamlar | so'rovlar | est bits | Isbot o'lchami (≤) | RAM (≤) | P95 kechikish (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1,5 GB | 0,40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2,5 GB | 0,75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3,5 GB | 1,20 s (A100) |

Chiqamalar A ilovasiga muvofiq. CI jabduqlari noto‘g‘ri shakllangan dalillarni keltirib chiqaradi va agar taxminiy bitlar <128 bo‘lsa, ishlamay qoladi.## Ommaviy IO sxemasi
| Maydon | Baytlar | Kodlash | Eslatmalar |
|----------------|-------|---------------------------------------|------------------------------------|
| `dsid` | 16 | little-endian UUID | `fastpq:v1:dsid` yorlig'i bilan xeshlangan kirish chizig'i uchun ma'lumotlar maydoni identifikatori (birlamchi qator uchun global). |
| `slot` | 8 | little-endian u64 | Davrdan beri nanosoniyalar.            |
| `old_root` | 32 | oz-endian Poseidon2 maydon baytlari | To'plamdan oldin SMT ildizi.              |
| `new_root` | 32 | oz-endian Poseidon2 maydon baytlari | To'plamdan keyin SMT ildizi.               |
| `perm_root` | 32 | oz-endian Poseidon2 maydon baytlari | Slot uchun ruxsat jadvali ildizi. |
| `tx_set_hash` | 32 | BLAKE2b | Saralangan ko'rsatma identifikatorlari.     |
| `parameter` | var | UTF-8 (masalan, `fastpq-lane-balanced`) | Parametrlar to'plamining nomi.                 |
| `protocol_version`, `params_version` | 2 tadan | little-endian u16 | Versiya qiymatlari.                      |
| `ordering_hash` | 32 | Poseidon2 (kichik endian) | Saralangan qatorlarning barqaror xeshi.         |

O'chirish nol qiymatli a'zolar bilan kodlangan; yo'q kalitlar nol barg + qo'shni guvohdan foydalanadi.

`FastpqTransitionBatch.public_inputs` - `dsid`, `slot` va asosiy majburiyatlar uchun kanonik tashuvchi;
ommaviy metadata kirish xesh/transkript hisobini yuritish uchun ajratilgan.

## Xeshlarni kodlash
- Buyurtma xesh: Poseidon2 (teg `fastpq:v1:ordering`).
- Partiya artefakt xeshi: `PublicIO || proof.commitments` dan BLAKE2b (teg `fastpq:v1:artifact`).

## Bajarilgan bosqich ta'riflari (DoD)
- **DD 1-bosqich**
  - Qadoqlash bo'ylab sayohat sinovlari va moslamalar birlashtirildi.
  - AIR spetsifikatsiyasi (`docs/source/fastpq_air.md`) `s_active`, aktiv/SMT ustunlari, selektor taʼriflari (jumladan, `s_perm`) va ramziy cheklovlarni oʻz ichiga oladi.
  - Buyurtma xesh PublicIO-da qayd etilgan va armatura orqali tasdiqlangan.
  - A'zolik va a'zo bo'lmagan vektorlar bilan amalga oshirilgan SMT/qidiruv guvohlarini yaratish.
  - Konservatsiya sinovlari transfer, yalpiz, kuyish va aralash partiyalarni qamrab oladi.
- **DD 2-bosqich**
  - Transkript spetsifikatsiyasi amalga oshirildi; oltin transkript (`tests/fixtures/transcript_v1.json`) va domen teglari tasdiqlangan.
  - Poseidon2 parametri `3f2b7fe` arxitekturalar bo'ylab endianness testlari bilan prover va verifierda biriktirilgan.
  - Soundness CI himoyasi faol; isbot hajmi/RAM/kechikish SLO qayd etilgan.
- **DD 3-bosqich**
  - identifikatsiya kalitlari bilan hujjatlashtirilgan Scheduler API (`SubmitProofRequest`, `ProofResult`).
  - Qayta urinish/orqaga o'chirish bilan manzilli kontent saqlangan isbotlangan artefaktlar.
  - Telemetriya navbat chuqurligi, navbat kutish vaqti, proverni bajarish kechikishi, qayta urinishlar soni, backend nosozliklari va GPU/CPU foydalanish uchun, har bir koʻrsatkich uchun asboblar paneli va ogohlantirish chegaralari bilan eksport qilinadi.## 5-bosqich - GPU tezlashtirish va optimallashtirish
- Maqsadli yadrolar: LDE (NTT), Poseidon2 xeshing, Merkle daraxti konstruktsiyasi, FRI katlamasi.
- Determinizm: tez matematikani o'chirib qo'ying, CPU, CUDA, Metal bo'ylab bir xil bit chiqishini ta'minlang. CI barcha qurilmalarda isbot ildizlarini solishtirishi kerak.
- Protsessor va GPUni mos yozuvlar apparatida (masalan, Nvidia A100, AMD MI210) taqqoslaydigan benchmark to'plami.
- Metall orqa tomon (Apple Silicon):
  - Build skripti yadro to'plamini (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) `fastpq.metallib` ichiga `xcrun metal`/`xcrun metallib` orqali kompilyatsiya qiladi; macOS ishlab chiquvchi vositalariga Metall asboblar zanjiri (`xcode-select --install`, keyin kerak bo'lsa `xcodebuild -downloadComponent MetalToolchain`) kiritilganligiga ishonch hosil qiling.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - CI isitish yoki deterministik qadoqlash uchun qo'lda qayta qurish (`build.rs` oynalari):
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Muvaffaqiyatli qurilishlar `FASTPQ_METAL_LIB=<path>` chiqaradi, shuning uchun ish vaqti metallibni aniq yuklashi mumkin.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - LDE yadrosi endi baholash buferi xostda noldan ishga tushirilgan deb hisoblaydi. Mavjud `vec![0; ..]` ajratish yo'lini yoki ularni qayta ishlatishda aniq nol buferlarni saqlang.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal14:14
  - Kosetni ko'paytirish qo'shimcha o'tishni oldini olish uchun oxirgi FFT bosqichiga birlashtiriladi; LDE bosqichidagi har qanday o'zgarishlar o'sha o'zgarmasni saqlab qolishi kerak.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - Umumiy xotira FFT/LDE yadrosi endi plitka chuqurligida to'xtaydi va qolgan kapalaklarni va har qanday teskari masshtabni maxsus `fastpq_fft_post_tiling` o'tish joyiga topshiradi. Rust xosti ikkala yadro orqali bir xil ustunlar to‘plamini o‘tkazadi va faqat `log_len` plitka chegarasidan oshib ketganda post-plitka jo‘natmasini ishga tushiradi, shuning uchun navbat chuqurligi telemetriyasi, yadro statistikasi va qaytarilish harakati aniq bo‘lib qoladi, GPU esa keng bosqichli ishni to‘liq boshqaradi. qurilmada.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - Ishga tushirish shakllari bilan tajriba o'tkazish uchun `FASTPQ_METAL_THREADGROUP=<width>` ni o'rnating; jo'natish yo'li qiymatni qurilma chegarasiga bosadi va bekor qilishni qayd qiladi, shuning uchun profillash ishlari qayta kompilyatsiya qilmasdan ish zarralari o'lchamlarini o'chirishi mumkin.【crates/fastpq_prover/src/metal.rs:321】- FFT plitkasini to'g'ridan-to'g'ri sozlang: xost endi ip guruhlari qatorlarini oladi (qisqa izlar uchun 16 ta, `log_len ≥ 6` bir marta 32, `log_len ≥ 10` bir marta 64, `log_len ≥ 14` bir marta 128 va Prometheus va kichik bosqichda 256) izlar, `log_len ≥ 12` bo‘lganda 4 va domen `log_len ≥ 18/20/22` ga yetgandan so‘ng umumiy xotira o‘tishi so‘ralgan domendan post-plitka yadrosiga nazoratni topshirishdan oldin 12/14/16 bosqichda ishlaydi va qurilmaning ishlash kengligi/maksimal iplari. Muayyan ishga tushirish shakllarini mahkamlash uchun `FASTPQ_METAL_FFT_LANES` (8 va 256 orasida ikkita quvvat) va `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) bilan bekor qiling; ikkala qiymat ham `FftArgs` orqali oqadi, qo'llab-quvvatlanadigan oynaga mahkamlanadi va profil yaratish uchun jurnalga yoziladi. supurish.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT va LDE ustunlari toʻplami endi hal qilingan tarmoq kengligidan kelib chiqadi: xost har bir buyruq buferiga taxminan 4096 mantiqiy ipni maqsad qilib qoʻyadi, bir vaqtning oʻzida 64 tagacha ustunni aylana-bufer plitkalari bilan birlashtiradi va faqat 64 → 32 → 16 → 8 → eva ustunlari → 2 domenini kesib o'tadi. 2¹⁶/2¹⁸/2²⁰/2²² chegaralar. Bu 20k qatorli suratga olishni har bir jo'natma uchun ≥64 ustunda ushlab turadi va uzoq kosetlarning aniq yakunlanishini ta'minlaydi. Moslashuvchan rejalashtiruvchi hali ham jo'natmalar ≈2ms nishonga yaqinlashguncha ustun kengligini ikki baravar oshiradi va endi namunali jo'natma shu maqsaddan ≥30% ga tushsa, to'plamni avtomatik ravishda yarmiga qisqartiradi, shuning uchun har bir ustun narxini oshiruvchi qator/plitka o'tishlari qo'lda bekor qilinmasdan qaytariladi. Poseidon almashtirishlari bir xil moslashuvchi rejalashtiruvchiga ega va `fastpq_metal_bench`-dagi `metal_heuristics.batch_columns.poseidon` bloki endi hal qilingan holat sonini, cheklovini, oxirgi davomiyligini va bekor qilish bayrog‘ini yozib oladi, shuning uchun navbatda chuqurlikdagi telemetriya to‘g‘ridan-to‘g‘ri Poseidon sozlashiga bog‘lanishi mumkin. Aniq FFT to'plami hajmini belgilash uchun `FASTPQ_METAL_FFT_COLUMNS` (1–64) bilan bekor qiling va belgilangan ustunlar sonini hisobga olish uchun LDE dispetcheri kerak bo'lganda `FASTPQ_METAL_LDE_COLUMNS` (1–64) dan foydalaning; Metall dastgoh har bir suratga olishda hal qilingan `kernel_profiles.*.columns` yozuvlarini qoplaydi, shuning uchun sozlash tajribalari saqlanib qoladi takrorlanishi mumkin.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:12- Ko'p navbatli jo'natish endi diskret Mac kompyuterlarida avtomatik: xost `is_low_power`, `is_headless` va qurilmaning joylashuvini tekshiradi va ikkita metall buyruqlar navbatini yig'ish yoki yo'qligini hal qiladi, faqat ish yuki kamida 16 ta ustunni ko'targanda ishlaydi (halokatli kolonkalar bilan o'lchanadi va har ikkisi ham uzoq masofani ushlab turadi), GPU yo'llari determinizmdan voz kechmasdan band. Bufer-bufer semafori endi "har bir navbatda ikkita parvoz" qavatini qo'llaydi va navbat telemetriyasi umumiy o'lchov oynasini (`window_ms`) va global semafor uchun normallashtirilgan bandlik nisbatlarini (`busy_ratio`) qayd qiladi va har bir navbat yozuvi shu tarzda qolishi mumkin ≥0 bir xil vaqt oralig'ida band. `FASTPQ_METAL_QUEUE_FANOUT` (1–4 qator) va `FASTPQ_METAL_COLUMN_THRESHOLD` (fan-outdan oldin minimal jami ustunlar) bilan standart sozlamalarni bekor qiling; Metall paritet testlari bekor qilishni majbur qiladi, shuning uchun ko'p GPUli Maclar qoplanadi va hal qilingan siyosat navbat chuqurligi telemetriyasi va yangi `metal_dispatch_queue.queues[*]` bilan birga qayd etiladi. blok.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【Crate s/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- Metallni aniqlash endi `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices`-ni to'g'ridan-to'g'ri tekshiradi (boshsiz qobiqlarda CoreGraphics-ni isitadi) `system_profiler` ga qaytishdan oldin va `FASTPQ_DEBUG_METAL_ENUM` sanab o'tilgan qurilmalarni chop etadi, shuning uchun boshsiz ishga tushirilganda CI nima uchun ekanligini tushuntirishi mumkin. `FASTPQ_GPU=gpu` hali ham protsessor yo'liga tushirildi. Agar bekor qilish `gpu` ga o'rnatilgan bo'lsa-da, lekin tezlatkich aniqlanmasa, `fastpq_metal_bench` protsessorda jimgina davom etish o'rniga disk raskadrovka tugmasi ko'rsatkichi bilan darhol xato qiladi. Bu WP2‑E-da chaqirilgan "CPUning jim qayta tiklanishi" sinfini toraytiradi va operatorlarga o'ralgan holda ro'yxatga olish jurnallarini yozib olish tugmachasini beradi. benchmarks.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU vaqtlari endi protsessorning zaxiralarini "GPU" ma'lumotlari sifatida ko'rib chiqishni rad etadi. `hash_columns_gpu` tezlatkich haqiqatda ishlaganmi yoki yo'qmi, deb xabar beradi, `measure_poseidon_gpu` quvur liniyasi orqaga qaytganda namunalarni tushiradi (va ogohlantirishni qayd qiladi) va GPU xeshlash mavjud bo'lmasa, Poseidon microbench bolasi xato bilan chiqadi. Natijada, `gpu_recorded=false` har doim Metall ijrosi orqaga qaytsa, navbat xulosasi muvaffaqiyatsiz jo'natish oynasini yozib oladi va asboblar panelidagi xulosalar regressiyani darhol belgilaydi. Oʻram (`scripts/fastpq/wrap_benchmark.py`) endi `metal_dispatch_queue.poseidon.dispatch_count == 0` da ishlamay qoladi, shuning uchun Stage7 toʻplamlarini haqiqiy GPU Poseidon joʻnatmasisiz imzolab boʻlmaydi. dalil.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_bench:912.- Poseidon xeshing endi o'sha sahnalashtirish shartnomasini aks ettiradi. `PoseidonColumnBatch` yassilangan foydali yuk buferlari va ofset/uzunlik identifikatorlarini ishlab chiqaradi, xost har bir partiya uchun identifikatorlarni qayta asoslaydi va `COLUMN_STAGING_PIPE_DEPTH` qo'sh buferini ishga tushiradi, shuning uchun foydali yuk + deskriptor yuklamalari GPU ishi bilan bir-biriga mos keladi va ikkala metall/CUDA yadrosi ham barcha displeylar tezligini to'g'ridan-to'g'ri blokirovka qiladi ustun dayjestlarini chiqarishdan oldin qurilmada. `hash_columns_from_coefficients` endi ushbu partiyalarni GPU ishchi ipi orqali uzatadi va diskret GPUlarda sukut bo'yicha 64+ ustunni parvozda saqlaydi (`FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH` orqali sozlanishi mumkin). Metall dastgoh `metal_dispatch_queue.poseidon_pipeline` ostida hal qilingan quvur liniyasi sozlamalarini + partiyalar sonini qayd qiladi va `kernel_profiles.poseidon.bytes` tavsiflovchi trafigini o'z ichiga oladi, shuning uchun Stage7 yozuvlari yangi ABI ni isbotlaydi. end-to-end.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_prover/cuda/fastpq_cuda.
- Stage7-P2 birlashtirilgan Poseidon xeshing endi ikkala GPU orqa tomoniga tushadi. Striming ishchisi qoʻshni `PoseidonColumnBatch::column_window()` boʻlaklarini `hash_columns_gpu_fused` ichiga yuboradi, bu esa ularni `poseidon_hash_columns_fused` ga oʻtkazadi, shuning uchun har bir joʻnatma `leaf_digests || parent_digests` kanonik Prometheus bilan yozadi. `ColumnDigests` ikkala bo‘lakni ham saqlaydi va `merkle_root_with_first_level` ota-qatlamni darhol iste’mol qiladi, shuning uchun protsessor hech qachon chuqurlik-1 tugunlarini qayta hisoblamaydi va Stage7 telemetriyasi GPU birlashtirilgan yadroda nol “qayta” ota-ona hisobotini olishini tasdiqlaydi. muvaffaqiyatli.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】 【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` endi Metall qurilma nomi, registr identifikatori, `low_power`/`headless` bayroqlari, joylashuvi (oʻrnatilgan, uyasi, tashqi), diskret indikatori, Apple 0X va Prometheus yorligʻi, `device_profile` blokini chiqaradi. (masalan, “M3 Max”). Stage7 asboblar paneli bu maydonni M4/M3 va diskret GPUlar orqali xost nomlarini tahlil qilmasdan suratga olish uchun ishlatadi va JSON navbat/evristik dalillar yonida jo‘natiladi, shuning uchun har bir reliz artefakti qaysi flot sinfi yugurishni amalga oshirganini isbotlaydi.【crates/fastpq_prover/src/bin/fastpq_prover/src/bin/fastpq_metal:23.2.- FFT xost/qurilma bir-biriga mos kelishi endi ikki buferli bosqichli oynadan foydalanmoqda: *n* to‘plami `fastpq_fft_post_tiling` ichida tugaydi, xost *n+1* to‘plamini ikkinchi bosqichli buferga tekislaydi va faqat bufer qayta ishlanishi kerak bo‘lganda pauza qiladi. Backend qancha partiyalar tekislanganligini va GPU tugashini kutishga nisbatan tekislash uchun sarflangan vaqtni qayd qiladi va `fastpq_metal_bench` yig‘ilgan `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` blokini yuzaga chiqaradi, shuning uchun reliz artefaktlari jim xost stendlari o‘rniga bir-biriga mos kelishini isbotlashi mumkin. JSON hisoboti endi `column_staging.phases.{fft,lde,poseidon}` ostida fazalar bo‘yicha jamilarni ajratadi, bu esa Stage7-ga FFT/LDE/Poseidon staging xostga bog‘langanligini yoki GPU tugashini kutayotganligini isbotlash imkonini beradi. Poseidon almashtirishlari bir xil birlashtirilgan sahnalashtirish buferlarini qayta ishlatadi, shuning uchun `--operation poseidon_hash_columns` suratga olishlar endi Poseidonga xos `column_staging` deltalarini navbat chuqurligi dalillari bilan bir qatorda buyurtma asboblarisiz chiqaradi. Yangi `column_staging.samples.{fft,lde,poseidon}` massivlari har bir partiyadagi `batch/flatten_ms/wait_ms/wait_ratio` kortejlarini yozib oladi, bu esa `COLUMN_STAGING_PIPE_DEPTH` bir-biriga mos kelishini isbotlashni (yoki xost GPUni qachon kutishini aniqlashni) ahamiyatsiz qiladi. tugatish).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/s tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:126】- Poseidon2 tezlashuvi endi yuqori band bo'lgan metall yadro sifatida ishlaydi: har bir ish zarrachasi dumaloq konstantalarni va MDS qatorlarini ish zarrachalari xotirasiga ko'chiradi, to'liq/qisman aylanmalarni o'chiradi va har bir bo'lakda bir nechta holatni bosib o'tadi, shuning uchun har bir jo'natma kamida 4096 mantiqiy ipni ishga tushiradi. `fastpq.metallib` ni qayta tiklamasdan profil yaratish tajribalarini takrorlash uchun `FASTPQ_METAL_POSEIDON_LANES` (ikkita quvvat 32 va 256 orasida, qurilma chegarasiga mahkamlangan) va `FASTPQ_METAL_POSEIDON_BATCH` (har bir chiziqda 1–32 holat) orqali ishga tushirish shaklini bekor qiling; Rust xosti jo'natishdan oldin hal qilingan sozlashni `PoseidonArgs` orqali o'tkazadi. Xost endi har bir yuklashda bir marta `MTLDevice::{is_low_power,is_headless,location}` suratini oladi va avtomatik ravishda diskret GPU-larni VRAM-darajali ishga tushirishga yo‘naltiradi (≥48GiB qismlarida `256×24`, 32GiB da `256×20`, I0320-da kam quvvat) SoC'lar `256×8` ga yopishadi (128/64 chiziqli uskuna uchun zaxiralar har bir yo'lda 8/6 holatdan foydalanishda davom etadi), shuning uchun operatorlar env varslariga tegmasdan > 16 shtatli quvur chuqurligini olishadi. `fastpq_metal_bench` o'zini `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ostida qayta ishga tushiradi va skalyar chiziqni ko'p shtatli yadro bilan solishtiradigan maxsus `poseidon_microbench` blokini oladi, shuning uchun reliz artefaktlari aniq tezlikni keltirishi mumkin. Xuddi shu `poseidon_pipeline` telemetriyasini suratga oladi (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`), shuning uchun Stage7 dalillari har bir GPUda bir-biriga o'xshash oynani isbotlaydi. trace.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】】 tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:19
  - LDE plitka staging endi FFT evristikasini aks ettiradi: og'ir izlar umumiy xotira o'tishda faqat `log₂(len) ≥ 18` dan 12 bosqichni bajaradi, log₂20 da 10 bosqichga tushadi va log₂22 da sakkiz bosqichga mahkamlanadi, shuning uchun keng kapalaklar postga o'tadi. Deterministik chuqurlik kerak bo'lganda `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) bilan bekor qiling; xost faqat evristik erta to'xtaganda, plitka qo'yishdan keyingi jo'natishni ishga tushiradi, shuning uchun navbat chuqurligi va yadro telemetriyasi deterministik bo'lib qoladi.【crates/fastpq_prover/src/metal.rs:827】
  - Yadro mikro-optimallashtirish: umumiy xotira FFT/LDE plitalari endi har bir kapalak uchun `pow_mod*` ni qayta baholash o'rniga, har bir chiziqli aylanma va koset qadamlarini qayta ishlatadi. Har bir chiziq `w_seed`, `w_stride` va (kerak bo'lganda) blokda bir marta koset qadamini oldindan hisoblab chiqadi, so'ngra ofsetlar bo'ylab oqimlarni o'tkazadi, `apply_stage_tile`/Prometheus o'rtacha qatorini pasaytiradi va L18NI00000342 ga tushiradi. Oxirgi evristika bilan ~1,55 s (950 ms maqsaddan hali ham yuqori, lekin faqat yig'ish uchun sozlash bo'yicha yana ~ 50 ms yaxshilanish).【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_benchson_run:11【- Endi yadro to'plamida har bir kirish nuqtasini, `fastpq.metallib` da qo'llaniladigan iplar guruhi/kafel chegaralarini va metallibni qo'lda kompilyatsiya qilish uchun ko'paytirish bosqichlarini hujjatlashtiradigan maxsus ma'lumotnoma (`docs/source/fastpq_metal_kernels.md`) mavjud.【docs/source/fastpq_metal_kernels.md:1s.
  - Benchmark hisoboti endi `post_tile_dispatches` ob'ektini chiqaradi, u maxsus plitka qo'yish yadrosida qancha FFT/IFFT/LDE partiyalari ishlaganligini qayd etadi (har bir turdagi jo'natmalar soni va bosqich/log₂ chegaralari). `scripts/fastpq/wrap_benchmark.py` blokni `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary` ga ko‘chiradi va manifest darvozasi dalillarni o‘tkazib yuboradigan GPU yozuvlarini rad etadi, shuning uchun har 20k qatorli artefakt ko‘p o‘tishli yadro ishlaganligini isbotlaydi. qurilmada.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - `FASTPQ_METAL_TRACE=1`-ni Instruments/Metal izi korrelyatsiyasi uchun har bir jo'natiladigan disk raskadrovka jurnallarini (quvur yorlig'i, ip guruhi kengligi, ishga tushirish guruhlari, o'tgan vaqt) chiqarish uchun sozlang.【crates/fastpq_prover/src/metal.rs:346】
- Endi jo'natish navbati moslashtirildi: `FASTPQ_METAL_MAX_IN_FLIGHT` bir vaqtning o'zida metall buyruqlar buferlarini yopib qo'yadi (avtomatik sukut bo'yicha `system_profiler` orqali aniqlangan GPU yadrolari sonidan kelib chiqadi, macOS qurilmasidan foydalanishni rad etish uchun xost-parallellik taqchilligi haqida xabar berish uchun hech bo'lmaganda navbatdagi fan chiqishi qavatiga mahkamlanadi). Eksport qilingan JSON `metal_dispatch_queue` ob'ektini `limit`, `dispatch_count`, `max_in_flight`, `busy_ms`, `busy_ms`, `busy_ms` va `busy_ms`, `metal_dispatch_queue` bilan olib yuradi, shuning uchun dastgoh navbat chuqurligidan namuna olish imkonini beradi. faqat Poseidon uchun suratga olish (`--operation poseidon_hash_columns`) ishga tushganda ichki o'rnatilgan `metal_dispatch_queue.poseidon` bloki va hal qilingan buyruq-bufer chegarasini tavsiflovchi `metal_heuristics` blokini hamda FFT/LDE to'plamini chiqaradi (jumladan, ustunlar qiymatlarini tekshirishga majbur qiladimi yoki yo'qmi) telemetriya bilan bir qatorda. Poseidon yadrolari yadro namunalaridan distillangan maxsus `poseidon_profiles` blokini ham oziqlantiradi, shuning uchun baytlar/iplar, bandlik va jo'natish geometriyasi artefaktlar bo'ylab kuzatiladi. Agar birlamchi ishga tushirish navbat chuqurligini yoki LDE nol toʻldirish statistikasini toʻplay olmasa (masalan, GPU joʻnatmasi protsessorga jimgina qaytsa), jabduqlar etishmayotgan telemetriyani toʻplash uchun avtomatik ravishda bitta prob joʻnatmasini ishga tushiradi va endi GPU har doim xabar berishdan bosh tortganida xost nol toʻldirish vaqtlarini sintez qiladi, shuning uchun IU18016 blok.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq _prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Metall asboblar zanjirisiz o'zaro kompilyatsiya qilishda `FASTPQ_SKIP_GPU_BUILD=1` ni o'rnating; ogohlantirish o'tkazib yuborishni qayd qiladi va rejalashtiruvchi protsessor yo'lida davom etadi.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- Ish vaqtini aniqlash metall qo'llab-quvvatlashni tasdiqlash uchun `system_profiler` dan foydalanadi; agar ramka, qurilma yoki metallibda yo'q bo'lsa, qurish skripti `FASTPQ_METAL_LIB` ni tozalaydi va rejalashtiruvchi deterministik protsessorda qoladi path.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover】4】s:
  - Operator nazorat ro'yxati (Metal xostlar):
    1. Asboblar zanjiri mavjudligini va kompilyatsiya qilingan `.metallib` da `FASTPQ_METAL_LIB` nuqtalari mavjudligini tasdiqlang (`echo $FASTPQ_METAL_LIB` `cargo build --features fastpq-gpu` dan keyin bo'sh bo'lmasligi kerak).【crates/fastpq_prover:18、
    2. GPU qatorlari yoqilgan holda paritet testlarini bajaring: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. Bu metall yadrolarini mashq qiladi va agar aniqlanmasa, avtomatik ravishda orqaga tushadi.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Boshqaruv panellari uchun benchmark namunasini oling: tuzilgan Metall kutubxonani toping
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), orqali eksport qiling
       `FASTPQ_METAL_LIB` va ishga tushiring
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       Kanonik `fastpq-lane-balanced` to'plami endi har bir suratga olishni 32 768 qatorga to'ldiradi, shuning uchun
       JSON so'ralgan 20k qatorlarni va GPUni boshqaradigan to'ldirilgan domenni aks ettiradi
       yadrolari. JSON/jurnalni dalillar do'koningizga yuklang; tungi macOS ish oqimi oynalari
      bu ishga tushiriladi va ma'lumot uchun artefaktlarni arxivlaydi. Hisobot qayd etilgan
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` har bir operatsiya `speedup` bilan birga,
     LDE bo'limiga `zero_fill.{bytes,ms,queue_delta}` qo'shiladi, shuning uchun artefaktlarni chiqarish determinizmni isbotlaydi,
     xost nol to‘ldirish xarajatlari va ortib borayotgan GPU navbatidan foydalanish (chegara, jo‘natish soni,
     eng yuqori parvoz vaqti, band/bir-biriga o'xshash vaqt) va yangi `kernel_profiles` bloki har bir yadroni ushlaydi.
     bandlik koeffitsientlari, taxminiy tarmoqli kengligi va davomiylik diapazonlari, shuning uchun asboblar paneli GPUni belgilashi mumkin
       xom namunalarni qayta ishlamasdan regressiyalar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Metall LDE yo'li 950 ms dan past bo'lishini kuting (Apple M seriyali apparatida `<1 s` maqsadi);
4. Boshqaruv panellari uzatish gadjetini chiza olishi uchun haqiqiy ExecWitness-dan qatordan foydalanish telemetriyasini oling.
   asrab olish. Torii dan guvohni keltiring
  (`iroha_cli audit witness --binary --out exec.witness`) va uni bilan dekodlang
  `iroha_cli audit witness --decode exec.witness` (ixtiyoriy ravishda qo'shing
  `--fastpq-parameter fastpq-lane-balanced` kutilgan parametrlar to'plamini tasdiqlash uchun; FASTPQ to'plamlari
  sukut bo'yicha chiqarish; `--no-fastpq-batches` ni faqat chiqishni kesish kerak bo'lganda o'tkazing).
   Har bir partiya yozuvi endi `row_usage` ob'ektini chiqaradi (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, har bir tanlovchi hisobi va `transfer_ratio`). JSON snippetini arxivlang
   xom transkriptlarni qayta ishlash.【crates/iroha_cli/src/audit.rs:209】 Yangi suratga olish bilan solishtiring
   `scripts/fastpq/check_row_usage.py` bilan oldingi asosiy chiziq, shuning uchun uzatish nisbatlari yoki
   jami qatorlar regressi:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```Tutun testlari uchun namuna JSON bloblari `scripts/fastpq/examples/` da mavjud. Mahalliy ravishda siz `make check-fastpq-row-usage` ni ishga tushirishingiz mumkin
   (`ci/check_fastpq_row_usage.sh` ni o'rab oladi) va CI majburiyatlarni solishtirish uchun `.github/workflows/fastpq-row-usage.yml` orqali bir xil skriptni ishga tushiradi
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` oniy suratlar, shuning uchun dalillar to'plami har doim tez muvaffaqiyatsiz bo'ladi
   uzatish qatorlari orqaga suriladi. Agar siz mashinada o'qiladigan farqni istasangiz, `--summary-out <path>` dan o'ting (CI ishi `fastpq_row_usage_summary.json` yuklaydi).
   ExecWitness qulay bo'lmasa, `fastpq_row_bench` bilan regressiya namunasini sintez qiling
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), u aynan bir xil `row_usage` chiqaradi
   konfiguratsiya qilinadigan selektor hisoblari uchun ob'ekt (masalan, 65536 qatorli stress testi):

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Stage7-3 tarqatish to'plamlari ham `scripts/fastpq/validate_row_usage_snapshot.py` dan o'tishi kerak.
   har bir `row_usage` yozuvi selektor hisoblarini o'z ichiga oladi va bu
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` yordamchini chaqiradi
   avtomatik ravishda bu oʻzgarmas toʻplamlar GPU qatorlari majburiy boʻlgunga qadar ishlamay qoladi.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       dastgoh manifest eshigi buni `--max-operation-ms lde=950` orqali amalga oshiradi, shuning uchun yangilang
       dalillaringiz bu chegaradan oshib ketganda qo'lga oling.
      Asboblar dalillari kerak bo'lganda, `--trace-dir <dir>` o'tkazing, shuning uchun jabduqlar
      o'zini `xcrun xctrace record` (standart "Metal tizim izi" shabloni) orqali qayta ishga tushiradi va
      JSON bilan birga vaqt tamg'asi bo'lgan `.trace` faylini saqlaydi; siz hali ham joylashuvni bekor qilishingiz mumkin /
      shablonni qo'lda `--trace-output <path>` va ixtiyoriy `--trace-template` /
      `--trace-seconds`. Olingan JSON `metal_trace_{template,seconds,output}` ni shunday reklama qiladi
      artefakt to'plamlari har doim olingan izni aniqlaydi.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Har bir tasvirni o'rang
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (agar imzolash identifikatorini qo'shish kerak bo'lsa, `--gpg-key <fingerprint>` qo'shing), shuning uchun to'plam muvaffaqiyatsiz tugadi
       GPU LDE o'rtacha 950 ms maqsadni buzsa, Poseidon 1 soniyadan oshsa yoki
       Poseidon telemetriya bloklari yo'q, `row_usage_snapshot` o'rnatadi
      JSON yonida, `benchmarks.poseidon_microbench` ostida Poseidon mikrobench xulosasini ko'rsatadi,
      va hali ham runbooks va Grafana asboblar paneli uchun metama'lumotlarni olib yuradi
    (`dashboards/grafana/fastpq_acceleration.json`). JSON endi `speedup.ratio` / chiqaradi
     Har bir operatsiya uchun `speedup.delta_ms`, shuning uchun dalillar GPU va boshqalarni isbotlashi mumkin
     CPU xom namunalarni qayta ishlamasdan yuklaydi va o'ram ikkalasini ham nusxalaydi
     nol toʻldirish statistikasi (plyus `queue_delta`) `zero_fill_hotspots` (bayt, kechikish, olingan)
     GB/s), `metadata.metal_trace` ostida Instruments metama'lumotlarini yozadi, ixtiyoriy
     `--row-usage <decoded witness>` ta'minlanganda `metadata.row_usage_snapshot` bloklanadi va uni tekislaydi
     yadro hisoblagichlari `benchmarks.kernel_summary` ga o'rnatiladi, shuning uchun to'siqlarni to'ldiradi, metall navbat
     foydalanish, yadro bandligi va tarmoqli kengligi regressiyalari bir qarashda ko'rinadi.
     xom ashyoni talaffuz qilish hisobot.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark .py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Qatordan foydalanish surati endi oʻralgan artefakt bilan sayohat qilganligi sababli, chiptalar oddiygina chiqariladi
     Ikkinchi JSON snippetini biriktirish o'rniga to'plamga murojaat qiling va CI o'rnatilgandan farq qilishi mumkin
    7-bosqich taqdimotlarini tasdiqlashda bevosita hisobga olinadi. Microbench ma'lumotlarini mustaqil ravishda arxivlash uchun,
    `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json` ishga tushiring
    va olingan faylni `benchmarks/poseidon/` ostida saqlang. Birlashtirilgan manifestni yangi saqlang
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    shuning uchun asboblar paneli/CI har bir faylni qo'lda yurmasdan to'liq tarixni farq qilishi mumkin.4. `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus oxirgi nuqtasi) yoki `telemetry::fastpq.execution_mode` jurnallarini qidirish orqali telemetriyani tasdiqlang; kutilmagan `resolved="cpu"` yozuvlari GPU niyatiga qaramay, xost orqaga qaytganini ko'rsatadi.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. `FASTPQ_GPU=cpu` (yoki konfiguratsiya tugmasi) dan foydalanib, texnik xizmat ko'rsatish vaqtida protsessorni bajarishga majburlang va zaxira jurnallari hali ham paydo bo'lishini tasdiqlang; bu SRE runbook'larini deterministik yo'lga moslab turadi.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetriya va qayta tiklash:
  - Bajarish rejimi jurnallari (`telemetry::fastpq.execution_mode`) va hisoblagichlar (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) so'ralgan va hal qilingan rejimni ochib beradi, shuning uchun jimgina qaytarilishlar ichida ko'rinadi. asboblar paneli.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ Acceleration Overview` Grafana platasi (`dashboards/grafana/fastpq_acceleration.json`) Metallni qabul qilish tezligini ko'rsatadi va benchmark artefaktlari bilan bog'lanadi, birlashtirilgan ogohlantirish qoidalari (`dashboards/alerts/fastpq_acceleration_rules.yml`) esa eshikning pasayishini davom ettiradi.
  - `FASTPQ_GPU={auto,cpu,gpu}` bekor qilish qo'llab-quvvatlanadi; noma'lum qiymatlar ogohlantirishlarni oshiradi, lekin hali ham audit uchun telemetriyaga tarqaladi.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - GPU paritet testlari (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) CUDA va Metal uchun o'tishi kerak; Metallib yo'q bo'lganda yoki aniqlash muvaffaqiyatsiz bo'lsa, CI chiroyli tarzda o'tkazib yuboradi.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Metall tayyorlik dalillari (har bir chiqishda quyidagi artefaktlarni arxivlang, shunda yo'l xaritasi auditi determinizm, telemetriya qamrovi va qayta ishlashni isbotlashi mumkin):| Qadam | Maqsad | Buyruq / Dalil |
    | ---- | ---- | ------------------ |
    | Qurilish metallib | `xcrun metal`/`xcrun metallib` mavjudligiga ishonch hosil qiling va bu majburiyat uchun deterministik `.metallib` chiqaring | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; eksport `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | Env var |ni tekshiring Qurilish skripti | tomonidan yozilgan env varni tekshirish orqali Metallning yoqilganligini tasdiqlang `echo $FASTPQ_METAL_LIB` (mutlaq yo'lni qaytarish kerak; bo'sh, orqa qism o'chirilganligini bildiradi).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU pariteti to'plami | Yuborishdan oldin yadrolar bajarilishini (yoki deterministik pasaytirish jurnallarini chiqarishini) isbotlang | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` va natijada `backend="metal"` yoki zaxira ogohlantirishni ko'rsatadigan jurnal parchasini saqlang.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:
    | Benchmark namunasi | `speedup.*` va FFT sozlanishini qayd qiluvchi JSON/log juftligini suratga oling, shunda asboblar paneli tezlatuvchi dalillarni qabul qilishi mumkin | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; JSON arxivini, vaqt tamg'asi bo'lgan `.trace` va stdout-ni nashr yozuvlari bilan birga arxivlang, shunda Grafana taxtasi Metall yugurishni tanlaydi (hisobotda so'ralgan 20 ming qator va to'ldirilgan 32,768 qatorli domen qayd etiladi, shuning uchun sharhlovchilar I0845000 ni tasdiqlaydilar. maqsad).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Hisobotni oʻrash va imzolash | Agar GPU LDE oʻrtacha 950ms dan oshsa, Poseidon 1s dan oshsa yoki Poseidon telemetriya bloklari yetishmasa va imzolangan artefakt toʻplamini yaratsa, chiqarilmaydi | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; oʻralgan JSON va yaratilgan `.json.asc` imzosini joʻnatib yuboring, shunda auditorlar ish yukini qayta ishga tushirmasdan ikkinchi soniya koʻrsatkichlarini tekshirishlari mumkin.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:7 |
    | Imzolangan dastgoh manifest | Metal/CUDA to'plamlari bo'ylab `<1 s` LDE dalillarini qo'llash va nashrni tasdiqlash uchun imzolangan dayjestlarni yozib olish | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; Chiqarish chiptasiga manifest + imzosini qo'shing, shunda quyi oqim avtomatizatsiyasi ikkinchi darajali isbot ko'rsatkichlarini tasdiqlashi mumkin.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA to'plami | SM80 CUDA tasvirini metall dalillar bilan qulflangan bosqichda saqlang, shuning uchun manifestlar ikkala GPU sinfini qamrab oladi. | Xeon+RTX xostidagi `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; Oʻralgan yoʻlni `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` ga qoʻshing, `.json`/`.asc` juftligini Metallar toʻplami yonida saqlang va auditorlar maʼlumotnomaga muhtoj boʻlganda, `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` ni keltiring. layout.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Telemetriya tekshiruvi | Prometheus/OTEL sirtlari `device_class="<matrix>", backend="metal"` aksini tasdiqlang (yoki pasaytirishni qayd qiling) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` va ishga tushirilganda chiqarilgan `telemetry::fastpq.execution_mode` jurnalidan nusxa oling.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| Majburiy qayta ishlash | SRE o'yin kitoblari uchun deterministik CPU yo'lini hujjatlang | `FASTPQ_GPU=cpu` yoki `zk.fastpq.execution_mode = "cpu"` bilan qisqa ish yukini ishga tushiring va operatorlar orqaga qaytarish jarayonini takrorlashi uchun pasaytirish jurnalini yozib oling.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_configmeter/7】s/s/s/s.
    | Tuzatish (ixtiyoriy) | Profil yaratishda, yadro chizigʻi/plitka bekor qilish keyinroq koʻrib chiqilishi uchun joʻnatish izlarini yozib oling | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` bilan bitta paritet testini qayta bajaring va ishlab chiqarilgan kuzatuv jurnalini chiqarish artefaktlariga biriktiring.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Chiqarish chiptasi bilan dalillarni arxivlang va xuddi shu nazorat roʻyxatini `docs/source/fastpq_migration_guide.md` da aks ettiring, shuning uchun sahnalashtirish/mahsulot chiqarishlar bir xil oʻyin kitobiga amal qiladi.【docs/source/fastpq_migration_guide.md:1】

### Nazorat ro'yxatini ijro etish

Har bir FASTPQ reliz chiptasiga quyidagi eshiklarni qo'shing. Relizlar barcha elementlar tugamaguncha bloklanadi
imzolangan artefaktlar sifatida to'ldirilgan va biriktirilgan.

1. **Sub-ikkinchi isbot ko'rsatkichlari** - Metallning kanonik mezonlari
   (`fastpq_metal_bench_*.json`) 20000 qatorli ish yukini (32768 toʻldirilgan qatorlar) tugashini isbotlashi kerak
   <1s. Aniqroq aytganda, `benchmarks.operations` yozuvi, bu erda `operation = "lde"` va mos keladigan
   `report.operations` namunasida `gpu_mean_ms ≤ 950` ko'rsatilishi kerak. Shiftdan oshib ketadigan yugurishlar talab qilinadi
   tekshirish va nazorat ro'yxati imzolanishidan oldin qayta qo'lga olish.
2. **Imzolangan benchmark manifest** — Yangi Metal + CUDA to'plamlarini yozib olgandan so'ng, ishga tushiring
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` chiqaradi
   `artifacts/fastpq_bench_manifest.json` va ajratilgan imzo
   (`artifacts/fastpq_bench_manifest.sig`). Ikkala faylni va ochiq kalit barmoq izini biriktiring
   Chipta chiqaring, shunda sharhlovchilar dayjest va imzoni mustaqil ravishda tekshirishlari mumkin.【xtask/src/fastpq.rs:1】
3. **Dalil qo‘shimchalari** — JSON, stdout jurnalining (yoki Instruments trace) xom standartini saqlang
   qo'lga olingan) va chiqish chiptasi bilan manifest/imzo juftligi. Tekshirish ro'yxati faqat
   Chipta o'sha artefaktlarga havola qilinganda va qo'ng'iroq bo'yicha tekshiruvchi tasdiqlaganida yashil deb hisoblanadi
   `fastpq_bench_manifest.json` da yozilgan dayjest yuklangan fayllarga mos keladi.【artifacts/fastpq_benchmarks/README.md:1】

## 6-bosqich - Qattiqlashtirish va hujjatlashtirish
- o'rin tutuvchining orqa qismi o'chirildi; ishlab chiqarish quvur liniyasi sukut bo'yicha hech qanday xususiyat o'chirilmasdan jo'natiladi.
- Qayta tiklanadigan tuzilmalar (pin asboblar zanjiri, konteyner tasvirlari).
- Trace, SMT, qidiruv tuzilmalari uchun fuzzers.
- Prover darajasidagi tutun testlari IVM toʻliq chiqarilishidan oldin Stage6 qurilmalarini barqaror saqlash uchun boshqaruv byulletenlari va pul oʻtkazmalarini qamrab oladi.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Ogohlantirish chegaralari, tuzatish tartib-qoidalari, imkoniyatlarni rejalashtirish bo'yicha ko'rsatmalarga ega Runbooks.
- CIda o'zaro arxitektura isbotini takrorlash (x86_64, ARM64).

### Skameyka manifest va chiqarish eshigi

Chiqarish dalillari endi Metall va ikkalasini ham qamrab olgan deterministik manifestni o'z ichiga oladi
CUDA benchmark to'plamlari. Yugurish:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```Buyruq o'ralgan to'plamlarni tasdiqlaydi, kechikish/tezlashtirish chegaralarini qo'llaydi,
BLAKE3 + SHA-256 digestlarini chiqaradi va (ixtiyoriy ravishda) manifestga imzo qo'yadi.
Ed25519 kaliti, shuning uchun chiqarish asboblari kelib chiqishini tekshirishi mumkin. Qarang
Amalga oshirish uchun `xtask/src/fastpq.rs`/`xtask/src/main.rs` va
Operatsion ko'rsatmalar uchun `artifacts/fastpq_benchmarks/README.md`.

> **Eslatma:** `benchmarks.poseidon_microbench` o'tkazib yuborilgan metall to'plamlar endi sabab bo'ladi
> manifest avlod muvaffaqiyatsizlikka uchradi. `scripts/fastpq/wrap_benchmark.py` ni qayta ishga tushiring
> (agar sizga mustaqil kerak bo'lsa, `scripts/fastpq/export_poseidon_microbench.py`
> Xulosa) qachon Poseidon dalillari etishmayotgan bo'lsa, manifestlarni qo'yib yuboring
> har doim skaler va standart taqqoslashni yozib oling.【xtask/src/fastpq.rs:409】

`--matrix` bayrog'i (birlamchi `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
mavjud bo'lganda) tomonidan olingan qurilmalar o'rtasidagi medianalarni yuklaydi
`scripts/fastpq/capture_matrix.sh`. Manifest 20000 qatorli qavatni kodlaydi va
Har bir qurilma sinfi uchun ish boshiga kechikish/tezlashtirish chegaralari, shuning uchun buyurtma
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` bekor qilinmagan
Agar ma'lum bir regressiyani tuzatmasangiz, uzoqroq talab qilinadi.

O'ralgan benchmark yo'llarini qo'shish orqali matritsani yangilang
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` ro'yxatlari va ishlayotgan
`scripts/fastpq/capture_matrix.sh`. Skript har bir qurilma medianasini suratga oladi,
konsolidatsiyalangan `matrix_manifest.json` ni chiqaradi va nisbiy yo'lni chop etadi
`cargo xtask fastpq-bench-manifest` iste'mol qiladi. AppleM4, Xeon+RTX va
Neoverse+MI300 suratga olish roʻyxatlari (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) va ularning o'ralgan
benchmark to'plamlari
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) endi tekshirildi
ichida, shuning uchun har bir nashr manifestdan oldin bir xil qurilmalar o'rtasidagi medianalarni amalga oshiradi
hisoblanadi imzolangan.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Tanqid xulosasi va ochiq harakatlar

## 7-bosqich - flotni qabul qilish va ishga tushirish dalillari

Stage7 proverni “hujjatlangan va taqqoslangan” (6-bosqich) ga olib boradi
"Ishlab chiqarish flotlari uchun standart bo'yicha tayyor". Asosiy e'tibor telemetriyani qabul qilishga qaratilgan,
qurilmalar o'rtasidagi tortishish pariteti va operator dalillar to'plamlari shunday GPU tezlashtirish
deterministik tarzda topshirilishi mumkin.- **7-1-bosqich — flot telemetriyasini qabul qilish va SLOs.** Ishlab chiqarish asboblar paneli
  (`dashboards/grafana/fastpq_acceleration.json`) ishlash uchun simli bo'lishi kerak
  Prometheus/OTel navbat chuqurligidagi stendlar uchun Alertmanager qamroviga ega,
  nol to'ldirish regressiyalari va jim CPU zaxiralari. Ogohlantirish paketi ostida qoladi
  `dashboards/alerts/fastpq_acceleration_rules.yml` va bir xil dalillarni oziqlantiradi
  6-bosqichda toʻplam talab qilinadi.【dashboards/grafana/fastpq_acceleration.json:1】【boshqaruv panellari/alerts/fastpq_acceleration_rules.yml:1】
  Endi asboblar panelida `device_class`, `chip_family`,
  va `gpu_kind`, operatorlarga metallni aniq matritsa bo'yicha qabul qilishga imkon beradi
  yorliq (masalan, `apple-m4-max`), Apple chiplari oilasi yoki diskret va boshqalar.
  so'rovlarni tahrir qilmasdan integratsiyalangan GPU sinflari.
  `irohad --features fastpq-gpu` bilan qurilgan macOS tugunlari endi chiqaradi
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (band/qoplamalar nisbati) va
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (chegara, parvozda_maksimal_ko'rsatkich, jo'natish_hisoblash, oyna_sekundlari) shuning uchun asboblar paneli va
  Alertmanager qoidalari to'g'ridan-to'g'ri Metall semafor ish sikli/bo'sh joydan o'qishi mumkin
  Prometheus benchmark to'plamini kutmasdan. Xostlar endi eksport qiladi
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` va
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` har doim
  LDE yordamchisi GPU baholash buferlarini nolga aylantiradi va Alertmanagerga ega bo'ldi
  `FastpqQueueHeadroomLow` (bo'sh joy  0,40 ms) qoidalari, shuning uchun bo'sh joyni navbatga qo'ying va
  ni kutish o'rniga darhol nol to'ldirish regressiya sahifa operatorlari
  keyingi o'ralgan benchmark. Yangi `FastpqCpuFallbackBurst` sahifa darajasidagi ogohlantirish treklari
  GPU ish yukining 5% dan ko'prog'i uchun CPU backendiga tushishni so'raydi,
  operatorlarni dalillarni qo'lga kiritishga majburlash va vaqtinchalik GPU nosozliklarini keltirib chiqaradi
  qayta urinishdan oldin rollout.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【boshqaruv paneli/al erts/fastpq_acceleration_rules.yml:1】【boshqaruv paneli/ogohlantirishlar/testlar/fastpq_acceleration_rules.test.yml:1】
  SLO to'plami endi ≥50% metall ish sikli maqsadini ham amalga oshiradi
  `FastpqQueueDutyCycleDrop` qoidasi, bu o'rtacha
  `fastpq_metal_queue_ratio{metric="busy"}` dumalab 15 daqiqali oyna va
  GPU ishi hali ham rejalashtirilganda, lekin navbat ushlab turolmasa, ogohlantiradi
  zarur bandlik. Bu jonli telemetriya shartnomasini to'g'rilab turadi
  GPU qatorlaridan oldin sinov dalillari majburiydir.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【boshqaruv panellari/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **7-2-bosqich — qurilmalar o‘rtasida suratga olish matritsasi.** Yangi
  `scripts/fastpq/capture_matrix.sh` quradi
  Har bir qurilmadan `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
  `artifacts/fastpq_benchmarks/matrix/devices/` ostida ro'yxatlarni yozib olish. AppleM4,
  Xeon + RTX va Neoverse + MI300 medianlari endi ular bilan birga repo-da yashaydi
  o'ralgan paketlar, shuning uchun `cargo xtask fastpq-bench-manifest` manifestni yuklaydi
  avtomatik ravishda, 20000 qatorli qavatni qo'llaydi va har bir qurilma uchun qo'llaniladi
  Relizlar to'plami chiqarilishidan oldin buyurtma bo'yicha CLI bayroqlarisiz kechikish/tezlashtirish chegaralaritasdiqlangan.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1sk/st】rs.
Yig'ilgan beqarorlik sabablari endi matritsa bilan birga keladi: o'tish
chiqarish uchun `--reason-summary-out` - `scripts/fastpq/geometry_matrix.py`
Xost yorlig'i va manba tomonidan kalitlangan nosozlik/ogohlantirish sabablarining JSON gistogrammasi
Xulosa, shuning uchun Stage7-2 sharhlovchilari protsessorning kamchiliklarini yoki etishmayotgan telemetriyani ko'rishlari mumkin
to'liq Markdown jadvalini skanerlamasdan bir qarash. Hozir ham xuddi shunday yordamchi
`--host-label chip_family:Chip` (bir nechta tugmalar uchun takrorlash) ni qabul qiladi, shuning uchun
Markdown/JSON chiqishlari ko'mish o'rniga tanlangan xost yorlig'i ustunlarini o'z ichiga oladi
o'sha metama'lumotlar xom xulosada, bu OS tuzilmalarini filtrlashni ahamiyatsiz qiladi yoki
Stage7-2 dalillar to'plamini kompilyatsiya qilishda metall haydovchi versiyalari.【scripts/fastpq/geometry_matrix.py:1】
Geometriya, shuningdek, ISO8601 `started_at` / `completed_at` maydonlariga muhr qo'yadi.
xulosa, CSV va Markdown chiqishlari, shuning uchun suratga olish paketlari oynani isbotlashi mumkin
Stage7-2 matritsalari bir nechta laboratoriya ishlarini birlashtirganda har bir xost.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` endi geometriya matritsasini bilan birga tikadi
`row_usage/*.json` oniy suratlar bitta Stage7 toʻplamiga (`stage7_bundle.json`)
+ `stage7_geometry.md`), orqali uzatish nisbatlarini tekshirish
`validate_row_usage_snapshot.py` va doimiy xost/env/reason/manba xulosalari
shuning uchun tarqatiladigan chiptalar jonglyorlik o'rniga bitta deterministik artefaktni biriktirishi mumkin
bosh host jadvallari.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **7-3-bosqich — Operatorni qabul qilish dalillari va orqaga qaytarish mashqlari.** Yangi
  `docs/source/fastpq_rollout_playbook.md` artefakt to'plamini tavsiflaydi
  (`fastpq_bench_manifest.json`, oʻralgan metall/CUDA ushlaydi, Grafana eksporti,
  Alertmanager snapshoti, orqaga qaytarish jurnallari) har bir chiqish chiptasiga hamroh bo'lishi kerak
  shuningdek bosqichli (uchuvchi → rampa → sukut bo'yicha) vaqt jadvali va majburiy qayta tiklash mashqlari.
  `ci/check_fastpq_rollout.sh` ushbu to'plamlarni tasdiqlaydi, shuning uchun CI Stage7ni amalga oshiradi
  chiqarishdan oldin darvoza oldinga siljiydi. Bo'shatish quvur liniyasi endi xuddi shunday tortishi mumkin
  orqali `artifacts/releases/<version>/fastpq_rollouts/…` ichiga to'plamlar
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, ta'minlash
  imzolangan manifestlar va tarqatilgan dalillar birga qoladi. Malumot to'plami amal qiladi
  saqlash uchun `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` ostida
  GitHub ish jarayoni (`.github/workflows/fastpq-rollout.yml`) haqiqiy bo'lsa ham yashil rangda
  taqdimotlar ko'rib chiqiladi.

### 7-bosqich FFT navbati`crates/fastpq_prover/src/metal.rs` endi `QueuePolicy` ni yaratadi
xost a xabar berganida avtomatik ravishda bir nechta Metall buyruq navbatlarini hosil qiladi
diskret GPU. O'rnatilgan GPU'lar bitta navbatli yo'lni ushlab turadi
(`MIN_QUEUE_FANOUT = 1`), diskret qurilmalar sukut bo'yicha ikkita navbatda va faqat
Agar ish yuki kamida 16 ta ustunni qamrab olgan bo'lsa, o'chiring. Ikkala evristik ham sozlanishi mumkin
yangi `FASTPQ_METAL_QUEUE_FANOUT` va `FASTPQ_METAL_COLUMN_THRESHOLD` orqali
atrof-muhit o'zgaruvchilari va rejalashtiruvchi FFT/LDE to'plamlari bo'ylab round-robins
bir xil navbatga qo'shilgan plitka qo'yishdan keyingi jo'natishdan oldin faol navbatlar
buyurtma kafolatlarini saqlab qolish uchun.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Tugun operatorlari endi bu env parametrlarini qo'lda eksport qilishlari shart emas: the
`iroha_config` profili `fastpq.metal_queue_fanout` va
`fastpq.metal_queue_column_threshold` va `irohad` ularni quyidagi orqali qo'llaydi
Metall backend ishga tushirilgunga qadar `fastpq_prover::set_metal_queue_policy`
flot profillari buyurtma bo'yicha ishga tushirish paketlarisiz takrorlanishi mumkin.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Teskari FFT to'plamlari endi faqat ish yuki bo'lganda bitta navbatga yopishadi
ventilyatsiya chegarasiga yetadi (masalan, 16 ustunli chiziqli muvozanatli suratga olish), bu
katta ustunli FFT/LDE/Poseidonni tark etganda WP2-D uchun ≥1,0× paritetni tiklaydi
ko'p navbatli yo'lda jo'natadi.【crates/fastpq_prover/src/metal.rs:2018】

Yordamchi testlar navbat siyosati qisqichlarini va tahlil qiluvchi tekshiruvni amalga oshiradi, shunda CI mumkin
Har bir quruvchida GPU uskunasini talab qilmasdan Stage7 evristikasini isbotlang,
va GPU-ga xos testlar takroriy o'yin qamrovini saqlab qolish uchun fan-out funksiyalarini bekor qilishga majbur qiladi
yangi standart sozlamalar bilan sinxronlash.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 Qurilma yorliqlari va ogohlantirish shartnomasi

`scripts/fastpq/wrap_benchmark.py` endi macOS tasvirida `system_profiler`ni tekshiradi
Fleet telemetriyasi uchun har bir o'ralgan ko'rsatkichda apparat belgilarini joylashtiradi va qayd qiladi
va suratga olish matritsasi moslashtirilgan elektron jadvallarsiz qurilma tomonidan aylantirilishi mumkin. A
20000-qatorli Metall ushlash endi quyidagi yozuvlarni o'z ichiga oladi:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

Bu teglar `benchmarks.zero_fill_hotspots` va bilan birga yutiladi
`benchmarks.metal_dispatch_queue`, shuning uchun Grafana surati, matritsani olish
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) va Alertmanager
dalillarning barchasi o'lchovlarni ishlab chiqqan apparat sinfiga rozi. The
`--label` bayrog'i laboratoriya xostida mavjud bo'lmagan hollarda qo'lda bekor qilishga ruxsat beradi
`system_profiler`, lekin avtomatik problangan identifikatorlar endi AppleM1–M4 va
diskret PCIe GPUlari qutidan tashqarida.【scripts/fastpq/wrap_benchmark.py:1】

Linux tasvirlari bir xil ishlovdan o'tadi: `wrap_benchmark.py` endi tekshiradi
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` va `lspci` shuning uchun CUDA va OpenCL ishlaydi
`cpu_model`, `gpu_model` va kanonik `device_class` (`xeon-rtx-sm80`) hosil qiling
Stage7 CUDA xosti uchun, MI300A laboratoriyasi uchun `neoverse-mi300`). Operatorlar mumkin
hali ham avtomatik ravishda aniqlangan qiymatlarni bekor qiladi, ammo Stage7 dalillar to'plami endi to'planmaydi
Xeon/Neoverse tasvirlarini to'g'ri qurilma bilan belgilash uchun qo'lda tahrirlashni talab qiling
metadata.Ishlash vaqtida har bir xost `fastpq.device_class`, `fastpq.chip_family` va
`fastpq.gpu_kind` (yoki mos keladigan `FASTPQ_*` muhit o'zgaruvchilari)
Prometheus eksport qilish uchun suratga olish to'plamida paydo bo'ladigan bir xil matritsa teglari
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` va
FASTPQ Acceleration boshqaruv paneli uchta o'qdan istalgani bo'yicha filtrlashi mumkin. The
Alertmanager qoidalari bir xil yorliqlar to'plami bo'yicha yig'ilib, operatorlarga diagramma berishga imkon beradi
qabul qilish, pasaytirishlar va bitta o'rniga har bir apparat profili
flot bo'ylab nisbati.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【boshqaruv panellari/alerts/fastpq_acceleration_rules.yml:1】

Telemetriya SLO/ogohlantirish shartnomasi endi olingan o'lchovlarni Stage7 bilan bog'laydi
darvozalar. Quyidagi jadvalda signallar va majburlash nuqtalari jamlangan:

| Signal | Manba | Maqsad / Trigger | Amalga oshirish |
| ------ | ------ | ---------------- | ----------- |
| GPU qabul qilish nisbati | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | Har bir (device_class, chip_family, gpu_kind) ruxsatlarining ≥95% `resolved="gpu", backend="metal"` ga tushishi kerak; sahifa har qanday triplet 15m dan 50% dan pastga tushganda | `FastpqMetalDowngrade` alert (sahifa)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:1】 |
| Orqa tomondagi bo'shliq | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Har bir uchlik uchun 0 da qolishi kerak; doimiy (>10m) portlashdan keyin ogohlantiring | `FastpqBackendNoneBurst` ogohlantirish (ogohlantirish)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:21】 |
| CPU qaytarilish nisbati | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | GPU tomonidan so'ralgan dalillarning ≤5% har qanday triplet uchun CPU backendiga tushishi mumkin; ≥10m | uchun triplet 5% dan oshganda sahifa `FastpqCpuFallbackBurst` alert (sahifa)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:32】 |
| Metall navbatning ish aylanishi | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | Har safar GPU ishlari navbatda turganda 15 m o'rtacha aylanish ≥50% bo'lishi kerak; GPU so'rovlari davom etayotganda foydalanish maqsaddan pastga tushganda ogohlantiring `FastpqQueueDutyCycleDrop` ogohlantirish (ogohlantirish)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:98】 |
| Navbat chuqurligi va nol toʻldirish byudjeti | O'ralgan benchmark `metal_dispatch_queue` va `zero_fill_hotspots` bloklari | `max_in_flight` `limit` ostida kamida bitta slot qolishi kerak va LDE nol toʻldirish oʻrtacha 20000-qatorli kanonik iz uchun ≤0,4ms (≈80GB/s) boʻlishi kerak; har qanday regressiya tarqatish paketini bloklaydi | `scripts/fastpq/wrap_benchmark.py` chiqishi orqali koʻrib chiqilgan va Stage7 dalillar toʻplamiga biriktirilgan (`docs/source/fastpq_rollout_playbook.md`). |
| Ish vaqti navbati bo'sh joy | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | Har bir uchlik uchun `limit - max_in_flight ≥ 1`; 10 m dan keyin bo'sh joysiz ogohlantiring | `FastpqQueueHeadroomLow` ogohlantirish (ogohlantirish)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:41】 |
| Ish vaqti nol to'ldirish kechikishi | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | Oxirgi nol to‘ldirish namunasi ≤0,40 ms qolishi kerak (Stage7 chegarasi) | `FastpqZeroFillRegression` alert (sahifa)【boshqaruv paneli/ogohlantirishlar/fastpq_acceleration_rules.yml:58】 |O'ram to'g'ridan-to'g'ri nol to'ldirish qatorini amalga oshiradi. O'tish
`--require-zero-fill-max-ms 0.40` - `scripts/fastpq/wrap_benchmark.py` va u
JSON dastgohida nol to'ldirish telemetriyasi bo'lmaganida yoki eng qizg'in bo'lsa muvaffaqiyatsiz bo'ladi
nol toʻldirish namunasi Stage7 budjetidan oshib ketadi, bu esa tarqatish paketlarini oldini oladi
majburiy dalillarsiz jo'natish.【scripts/fastpq/wrap_benchmark.py:1008】

#### 7-1-bosqich ogohlantirish bilan ishlash nazorat ro'yxati

Yuqorida sanab o'tilgan har bir ogohlantirish ma'lum bir qo'ng'iroq bo'yicha matkapni ta'minlaydi, shuning uchun operatorlar to'playdi
chiqarish to'plami talab qiladigan bir xil artefaktlar:

1. **`FastpqQueueHeadroomLow` (ogohlantirish).** Bir zumda Prometheus so‘rovini ishga tushiring
   `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` uchun va
   `fastpq-acceleration` dan Grafana “Queue headroom” panelini oling
   taxta. So'rov natijasini yozib oling
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   ogohlantirish identifikatori bilan birga, shuning uchun chiqarish to'plami ogohlantirish ekanligini isbotlaydi
   navbat och qolmasdan oldin tan olingan.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (sahifa).** Tekshiring
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` va agar metrik bo'lsa
   shovqinli, so'nggi JSON dastgohida `scripts/fastpq/wrap_benchmark.py` ni qayta ishga tushiring
   `zero_fill_hotspots` blokini yangilash uchun. PromQL chiqishini biriktiring,
   ekran tasvirlari va yangilangan dastgoh faylini chiqarish katalogiga; bu hosil qiladi
   `ci/check_fastpq_rollout.sh` chiqarish paytida kutgan bir xil dalillar
   validation.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (sahifa).** Buni tasdiqlang
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` 5% dan oshadi
   qavat, keyin mos keladigan pasaytirish xabarlari uchun `irohad` jurnallaridan namuna oling
   (`telemetry::fastpq.execution_mode resolved="cpu"`). PromQL dumpini saqlang
   plus `metrics_cpu_fallback.prom`/`rollback_drill.log` da jurnaldan parchalar, shuning uchun
   bundle ham ta'sirni, ham operatorning e'tirofini ko'rsatadi.
4. **Dalillarni o‘rash.** Har qanday ogohlantirish o‘chirilgandan so‘ng, Stage7-3 bosqichlarini qayta ishga tushiring.
   ishga tushirish kitobi (Grafana eksporti, ogohlantirish snapshoti, orqaga qaytarish matkapi) va
   to'plamni qayta ulashdan oldin uni `ci/check_fastpq_rollout.sh` orqali qayta tasdiqlang
   chiqish chiptasiga.【docs/source/fastpq_rollout_playbook.md:114】

Avtomatlashtirishni afzal ko'rgan operatorlar ishlashi mumkin
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
Prometheus API-dan navbat balandligi, nol-to'ldirish va protsessor zaxirasi uchun so'rov qilish
yuqorida sanab o'tilgan ko'rsatkichlar; yordamchi qo'lga olingan JSONni yozadi (prefiks bilan
original promQL) `metrics_headroom.prom`, `metrics_zero_fill.prom` va
`metrics_cpu_fallback.prom` tanlangan rollout katalog ostida shu fayllar
qo'lda jingalak chaqiruvlarisiz to'plamga biriktirilishi mumkin.`ci/check_fastpq_rollout.sh` endi navbat balandligi va nol toʻldirishni taʼminlaydi
to'g'ridan-to'g'ri byudjet. U tomonidan havola qilingan har bir `metal` dastgohini tahlil qiladi
`fastpq_bench_manifest.json`, tekshiradi
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` va
`benchmarks.zero_fill_hotspots[]` va bo'sh joy tushganda to'plam muvaffaqiyatsiz tugadi
bitta uyasi ostida yoki istalgan LDE hotspot `mean_ms > 0.40` haqida xabar berganda. Bu saqlaydi
CIda 7-bosqich telemetriya qo'riqchisi, qo'lda bajarilgan ko'rib chiqishga mos keladi
Grafana snapshot va dalillarni chiqarish.【ci/check_fastpq_rollout.sh#L1】
Xuddi shu tekshirishning bir qismi sifatida skript endi har bir o'ralganligini ta'kidlaydi
Benchmark avtomatik ravishda aniqlangan apparat belgilarini o'z ichiga oladi (`metadata.labels.device_class`
va `metadata.labels.gpu_kind`). Ushbu teglar mavjud bo'lmagan to'plamlar darhol muvaffaqiyatsizlikka uchraydi,
artefaktlar, Stage7-2 matritsasi manifestlari va ish vaqtini chiqarishni kafolatlaydi
asboblar panelida barchasi aynan bir xil qurilma sinf nomlariga ishora qiladi.

Grafana "So'nggi Benchmark" paneli va tegishli tarqatish to'plami endi iqtibos keltiradi
`device_class`, nol toʻldirish budjeti va navbat chuqurligining surati, shuning uchun qoʻngʻiroq boʻyicha muhandislar
ishlab chiqarish telemetriyasini belgi paytida ishlatiladigan aniq tortishish sinfi bilan bog'lashi mumkin
o'chirilgan. Kelajakdagi matritsa yozuvlari bir xil teglarni oladi, ya'ni Stage7-2 qurilmasi
ro'yxatlar va Prometheus asboblar paneli AppleM4 uchun yagona nomlash sxemasini baham ko'radi,
M3 Max va yaqinlashib kelayotgan MI300/RTX suratlari.

### 7-1-bosqich Fleet telemetriya qo'llanmasi

GPU qatorlarini sukut bo'yicha yoqishdan oldin ushbu nazorat ro'yxatiga rioya qiling, shuning uchun flot telemetriyasi
va Alertmanager qoidalari nashrga tayyorgarlik jarayonida olingan bir xil dalillarni aks ettiradi:

1. **Yorliq olish va ish vaqti xostlari.** `python3 scripts/fastpq/wrap_benchmark.py`
   allaqachon `metadata.labels.device_class`, `chip_family` va `gpu_kind` chiqaradi
   har bir o'ralgan JSON uchun. Ushbu teglarni sinxronlashtiring
   `fastpq.{device_class,chip_family,gpu_kind}` (yoki
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) `iroha_config` ichida
   shuning uchun ish vaqti ko'rsatkichlari nashr etiladi
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   va bir xil identifikatorlarga ega `fastpq_metal_queue_*` o'lchagichlari
   `artifacts/fastpq_benchmarks/matrix/devices/*.txt` da. Yangisini sahnalashtirganda
   sinf, orqali matritsa manifestini qayta tiklang
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   shuning uchun CI va asboblar paneli qo'shimcha yorliqni tushunadi.
2. **Navbat ko‘rsatkichlari va qabul qilish ko‘rsatkichlarini tasdiqlang.** `irohad --features fastpq-gpu` dasturini ishga tushiring
   Metall xostlarda va jonli navbatni tasdiqlash uchun telemetriya so'nggi nuqtasini qirib tashlang
   o'lchagichlar eksport qilinadi:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```Birinchi buyruq semafor namunasi `busy` ni chiqarishini isbotlaydi,
   `overlap`, `limit` va `max_in_flight` seriyalari va ikkinchisi
   har bir qurilma klassi `backend="metal"` ga yechim topmoqda yoki ga qaytmoqda
   `backend="cpu"`. Oldin Prometheus/OTel orqali qirib tashlash nishonini sim bilan o'tkazing
   asboblar panelini import qilish, shuning uchun Grafana flot ko'rinishini darhol chizishi mumkin.
3. **Boshqaruv paneli + ogohlantirish paketini o‘rnating.** Import
   `dashboards/grafana/fastpq_acceleration.json` Grafana ga (ushlang
   o'rnatilgan Device Class, Chip Family va GPU Kind shablonlari o'zgaruvchilari) va yuklang
   `dashboards/alerts/fastpq_acceleration_rules.yml` birgalikda Alertmanagerga
   uning birligi sinov moslamasi bilan. Qoidalar to'plami `promtool` jabduqlarini yuboradi; yugur
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   qoidalar `FastpqMetalDowngrade` isbotlash uchun o'zgarganda va
   `FastpqBackendNoneBurst` hali ham hujjatlashtirilgan chegaralarda yonmoqda.
4. **Dalillar to'plami bilan darvoza relizlar.** Saqlash
   `docs/source/fastpq_rollout_playbook.md` prokat yaratishda qulay
   topshirish, shuning uchun har bir to'plam o'ralgan mezonlarga ega bo'ladi, Grafana eksporti,
   ogohlantirish paketi, navbat telemetriyasini isbotlash va orqaga qaytarish jurnallari. CI allaqachon amal qiladi
   shartnoma: `make check-fastpq-rollout` (yoki chaqirish
   `ci/check_fastpq_rollout.sh --bundle <path>`) to'plamni tasdiqlaydi, qayta ishlaydi
   ogohlantirish sinovdan o'tadi va navbatdagi bo'sh joy yoki nol to'ldirilganda imzo chekishni rad etadi
   byudjetlarning regressi.
5. **Ogohlantirishlarni tuzatishga bog‘lang.** Alertmanager sahifalarini ochganda, Grafana dan foydalaning.
   taxta va 2-bosqichdan boshlab xom Prometheus hisoblagichlari mavjudligini tasdiqlash uchun
   pasaytirishlar navbat ochligi, protsessorning ishlamay qolishi yoki backend = hech qanday portlashdan kelib chiqadi.
Runbook ichida yashaydi
ushbu hujjat plus `docs/source/fastpq_rollout_playbook.md`; yangilang
tegishli `fastpq_execution_mode_total` bilan chiptani chiqarish,
`fastpq_metal_queue_ratio` va `fastpq_metal_queue_depth` parchalari birgalikda
Grafana paneliga havolalar va sharhlovchilar koʻrishi uchun ogohlantirish snapshoti bilan
aynan qaysi SLO ishga tushirilgan.

### WP2-E - Bosqichma-bosqich metall profilini yaratish surati

`scripts/fastpq/src/bin/metal_profile.rs` o'ralgan metall ushlashlarni umumlashtiradi
Shunday qilib, 900ms dan kichik maqsad vaqt o'tishi bilan kuzatilishi mumkin (yurish
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
Yangi Markdown yordamchisi
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
quyidagi bosqich jadvallarini yaratadi (u Markdownni matn bilan birga chop etadi
xulosa, shuning uchun WP2-E chiptalari dalillarni so'zma-so'z joylashtirishi mumkin). Ikkita suratga olish kuzatilmoqda
hozir:

> **Yangi WP2-E asboblari:** `fastpq_metal_bench --gpu-probe ...` endi
> aniqlash oniy tasviri (so'ralgan/hal qilingan bajarish rejimi, `FASTPQ_GPU`
> bekor qilish, aniqlangan backend va sanab o'tilgan metall qurilmalar/ro'yxatga olish kitobi identifikatorlari)
> har qanday yadro ishga tushishidan oldin. Majburiy GPU harakatsiz ishlayotganda ushbu jurnalni yozib oling
> protsessor yo'liga qaytadi, shuning uchun xostlar ko'rgan dalillar to'plami yozuvlari
> `MTLCopyAllDevices` nolni qaytaradi va qaysi bekor qilish amalda edi
> benchmark.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Sahnani suratga olish yordamchisi:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> endi `fastpq_metal_bench` FFT, LDE va Poseidon uchun alohida-alohida boshqaradi,
> xom JSON chiqishlarini bosqichli kataloglar ostida saqlaydi va bitta chiqaradi
> CPU/GPU vaqtlarini, navbat chuqurligini qayd qiluvchi `stage_profile_summary.json` to'plami
> telemetriya, ustunli bosqichlar statistikasi, yadro profillari va tegishli iz
> artefaktlar. `--stage fft --stage lde --stage poseidon` dan kichik to'plamni nishonga oling,
> Muayyan xctrace shablonini tanlash uchun `--trace-template "Metal System Trace"`,
> va `--trace-dir` `.trace` to'plamlarini umumiy manzilga yo'naltirish. ni biriktiring
> Xulosa JSON va har bir WP2-E muammosi uchun yaratilgan kuzatuv fayllari, shuning uchun sharhlovchilar
> navbatning bandligi (`metal_dispatch_queue.*`), bir-birining ustiga chiqish nisbati va
> qo'lda bir nechta spelunkingsiz yugurishlar bo'ylab olingan ishga tushirish geometriyasi
> `fastpq_metal_bench` chaqiruvlari.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Navbat/sahnaviy dalillar yordamchisi (2026-05-09):** hozir `scripts/fastpq/profile_queue.py`
> bir yoki bir nechta `fastpq_metal_bench` JSONni qabul qiladi va Markdown jadvalini chiqaradi va chiqaradi
> mashinada oʻqilishi mumkin boʻlgan xulosa (`--markdown-out/--json-out`), shuning uchun navbat chuqurligi, bir-birining ustiga chiqish nisbati va
> Xost tomonidagi sahnalashtirish telemetriyasi har bir WP2-E artefaktlari bilan bir qatorda harakatlanishi mumkin. Yugurish
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` quyidagi jadvalni yaratdi va arxivlangan Metall ushlagichlar hali ham hisobot beradi deb belgiladi
> `dispatch_count = 0` va `column_staging.batches = 0`—WP2-E.1 metall ochilguncha ochiq qoladi.
> asboblar telemetriya yoqilgan holda qayta qurilgan. Yaratilgan JSON/Markdown artefaktlari jonli
> audit uchun `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` ostida.
> Yordamchi hozir (2026-05-19) Poseidon quvur liniyasi telemetriyasini (`pipe_depth`,
> `batches`, `chunk_columns` va `fallbacks`) Markdown jadvali va JSON xulosasi ichida,
> shuning uchun WP2-E.4/6 sharhlovchilari GPU quvur liniyasi bo'ylab qolgan yoki yo'qligini isbotlashlari mumkin.
> qayta tiklashlar xom tasvirni ochmasdan sodir bo'ldi.【scripts/fastpq/profile_queue.py:1】> **Bosqich profili sarhisobi (2026-05-30):** `scripts/fastpq/stage_profile_report.py` sarflaydi
> `cargo xtask fastpq-stage-profile` tomonidan chiqarilgan `stage_profile_summary.json` to'plami va
> Markdown va JSON xulosalarini taqdim etadi, shuning uchun WP2-E sharhlovchilari dalillarni chiptalarga nusxalashlari mumkin
> vaqtni qo'lda yozmasdan. Chaqirmoq
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> GPU/CPU vositalari, tezlashtirish deltalari, kuzatuv qamrovi va
> har bir bosqichda telemetriya bo'shliqlari. JSON chiqishi jadvalni aks ettiradi va har bir bosqichdagi muammo teglarini yozadi
> (`trace missing`, `queue telemetry missing` va boshqalar) shuning uchun boshqaruvni avtomatlashtirish xostni farq qilishi mumkin
> WP2-E.1 orqali WP2-E.6 da havola qilingan ishlaydi.
> **Xost/qurilmaning bir-biriga o‘xshash himoyasi (2026-06-04):** `scripts/fastpq/profile_queue.py` endi izoh beradi
> FFT/LDE/Poseidon kutish nisbatlari bilan bir qatorda bosqichdagi tekislash/kutish millisekundlari jami va
> `--max-wait-ratio <threshold>` noto'g'ri o'xshashlikni aniqlaganda muammo. Foydalanish
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> WP2-E.5 chiptalarini aniq kutish nisbatlari bilan Markdown jadvali va JSON to'plamini olish uchun
> ikki marta buferlash oynasi GPU-ni oziqlantirganligini ko'rsatishi mumkin. Oddiy matnli konsol chiqishi ham
> qo'ng'iroq bo'yicha tekshiruvlarni osonlashtirish uchun har bir faza nisbatlarini sanab o'tadi.
> **Telemetriya qo'riqchisi + ishga tushirish holati (2026-06-09):** `fastpq_metal_bench` endi `run_status` blokini chiqaradi
> (backend yorlig'i, jo'natmalar soni, sabablar) va yangi `--require-telemetry` bayrog'i ishga tushmaydi
> GPU vaqtlari yoki navbat/staging telemetriyasi yoʻq boʻlganda. `profile_queue.py` ishga tushirishni amalga oshiradi
> ajratilgan ustun holati va muammolar roʻyxatida `ok` boʻlmagan holatlarni koʻrsatadi va
> `launch_geometry_sweep.py` bir xil holatni ogohlantirish/tasnifga kiritadi, shuning uchun matritsalar hech narsa qila olmaydi
> jimgina protsessorga qaytgan yoki navbat asboblarini o'tkazib yuborgan tasvirlarni uzoqroq qabul qiling.
> **Poseidon/LDE avtomatik sozlash (2026-06-12):** `metal_config::poseidon_batch_multiplier()` endi o'lchaydi
> Metall bilan ishlash bo'yicha maslahatlar va `lde_tile_stage_target()` diskret GPU-larda plitka chuqurligini oshiradi.
> Qo'llaniladigan multiplikator va kafel chegarasi `metal_heuristics` blokiga kiritilgan
> `fastpq_metal_bench` chiqadi va `scripts/fastpq/metal_capture_summary.py` tomonidan ko'rsatiladi, shuning uchun WP2-E
> to'plamlar har bir suratga olishda qo'llanilgan aniq quvur liniyasi tugmalarini JSON-ni qazib olmagan holda yozib oladi.【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq_prover:2833】【scripts/fastpq_maryp.

| Yorliq | Jo'natish | band | Qoplash | Maksimal chuqurlik | FFT tekislash | FFT kutish | FFT kutish % | LDE tekislash | LDE kuting | LDE kuting % | Poseydon tekislanadi | Poseydon kuting | Poseidon kuting % | Quvur chuqurligi | Quvur partiyalari | Quvurlarni qayta tiklash |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k surat (oldindan oʻzgartirish)

`fastpq_metal_bench_20k_latest.json`| Bosqich | Ustunlar | Kirish len | GPU o'rtacha (ms) | CPU o'rtacha (ms) | GPU ulushi | Tezlashtirish | D CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130,986 ms (115,761–167,755) | 112,616 ms (95,335–132,929) | 2,4% | 0,860× | −18.370 |
| IFFT | 16 | 32768 | 129,296 ms (111,127–142,955) | 158,144 ms (126,847–237,887) | 2,4% | 1,223× | +28.848 |
| LDE | 16 | 262144 | 1570,656 ms (1544,397–1584,502) | 1752,523 ms (1548,807–2191,930) | 29,2% | 1,116× | +181.867 |
| Poseydon | 16 | 524288 | 3548,329 ms (3519,881–3576,041) | 3642,706 ms (3539,055–3758,279) | 66,0% | 1,027× | +94.377 |

Asosiy kuzatishlar:1. GPU jami 5,379 soniyani tashkil etadi, bu 900 ms maqsadidan **4,48 soniyadan oshib ketdi**. Poseydon
   Xeshlash hali ham ish vaqtida hukmronlik qiladi (≈66%), ikkinchisida LDE yadrosi
   joy (≈29%), shuning uchun WP2-E Poseidon quvurining chuqurligiga ham, ham hujum qilishi kerak.
   CPU zaxiralari yo'qolishidan oldin LDE xotira rezidentligi/plitka rejasi.
2. IFFT skalerdan >1,22× bo‘lsa ham, FFT regressiya (0,86×) bo‘lib qoladi.
   yo'l. Bizga ishga tushirish geometriyasi kerak
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) tushunish uchun
   FFT bandligini allaqachon yaxshiroq - zarar bermasdan qutqarish mumkinmi
   IFFT vaqtlari. `scripts/fastpq/launch_geometry_sweep.py` yordamchisi endi boshqaradi
   bu tajribalar oxirigacha: vergul bilan ajratilgan bekor qilish (masalan,
   `--fft-columns 16,32 --queue-fanout 1,2` va
   `--poseidon-lanes auto,256`) va u chaqiradi
   Har bir kombinatsiya uchun `fastpq_metal_bench`, ostida JSON foydali yuklarni saqlang
   `artifacts/fastpq_geometry/<timestamp>/` va `summary.json` toʻplamini saqlab qolish
   Har bir yugurishning navbat nisbatlarini, FFT/LDE ishga tushirish tanlovlarini, GPU va CPU vaqtlarini tavsiflash,
   va xost meta-ma'lumotlari (xost nomi/yorlig'i, platforma uchligi, aniqlangan qurilma
   sinf, GPU sotuvchisi/modeli) shuning uchun qurilmalar o'rtasidagi taqqoslashlar deterministik xususiyatga ega
   kelib chiqishi. Yordamchi endi yoniga `reason_summary.json` ni ham yozadi
   sukut bo'yicha xulosa, aylantirish uchun geometriya matritsasi bilan bir xil tasniflagichdan foydalanish
   yuqori protsessor zaxiralari va etishmayotgan telemetriya. Belgilash uchun `--host-label staging-m3` dan foydalaning
   umumiy laboratoriyalardan olingan suratlar.
   Yordamchi `scripts/fastpq/geometry_matrix.py` vositasi endi bitta yoki yutadi
   ko'proq umumiy to'plamlar (`--summary hostA/summary.json --summary hostB/summary.json`)
   va har bir ishga tushirish shaklini *barqaror* deb belgilovchi Markdown/JSON jadvallarini chiqaradi
   (FFT/LDE/Poseidon GPU vaqtlari yozib olingan) yoki * beqaror* (vaqt tugashi, protsessorning ishdan chiqishi,
   metall bo'lmagan backend yoki etishmayotgan telemetriya) xost ustunlari yonida. The
   jadvallar endi hal qilingan `execution_mode`/`gpu_backend` va a
   `Reason` ustuni, shuning uchun CPU zaxiralari va etishmayotgan GPU vaqtlari aniq ko'rinadi
   Stage7 matritsalari hatto vaqt bloklari mavjud bo'lganda ham; Xulosa qatori hisoblanadi
   barqaror va umumiy yugurishlar. `--operation fft|lde|poseidon_hash_columns` orqali o'ting
   supurish bitta bosqichni ajratish kerak bo'lganda (masalan, profilga
   Poseidon alohida) va dastgohga xos bayroqlar uchun `--extra-args`-ni bepul saqlang.
   Yordamchi har qanday narsani qabul qiladi
   buyruq prefiksi (standart `cargo run … fastpq_metal_bench`) va ixtiyoriy
   `--halt-on-error` / `--timeout-seconds` qo'riqlaydi, shuning uchun ishlash muhandislari
   solishtirma yig'ish paytida turli xil mashinalarda supurishni takrorlang,
   Stage7 uchun ko'p qurilmali dalillar to'plamlari.
3. `metal_dispatch_queue` `dispatch_count = 0` xabar berdi, shuning uchun navbat bandligi
   GPU yadrolari ishlayotgan bo'lsa ham telemetriya yo'q edi. Metall ish vaqti hozir foydalaniladi
   navbat/ustun bosqichlari uchun panjaralarni olish/bo‘shatish
   asboblar bayroqlarini kuzating va geometriya matritsasi hisoboti chaqiriladi
   FFT/LDE/Poseidon GPU vaqtlari mavjud bo'lmaganda beqaror ishga tushirish shakllari. Saqlash
   Markdown/JSON matritsasini WP2-E chiptalariga biriktirish, shunda sharhlovchilar ko'rishlari mumkin
   Navbatdagi telemetriya mavjud bo'lganda, qaysi kombinatsiyalar hali ham muvaffaqiyatsiz bo'ladi.`run_status` qo'riqchisi va `--require-telemetry` bayrog'i endi suratga olinmaydi
   GPU vaqtlari etishmayotgan yoki navbat/sahna telemetriyasi mavjud bo'lmaganda, shuning uchun
   dispatch_count=0 ta yugurishlar endi WP2-E to'plamlariga e'tiborsiz qolmaydi.
   `fastpq_metal_bench` endi `--require-gpu`ni ochib beradi va
   `launch_geometry_sweep.py` uni sukut bo'yicha yoqadi (o'chirish
   `--allow-cpu-fallback`) shuning uchun protsessorning nosozliklari va metallni aniqlashdagi nosozliklar to'xtatiladi
   Stage7 matritsalarini GPU bo'lmagan telemetriya bilan ifloslantirish o'rniga darhol.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Nol to'ldirish ko'rsatkichlari ilgari xuddi shu sababga ko'ra yo'qolgan; fextavonie tuzatish
   xost asboblarini jonli ushlab turadi, shuning uchun keyingi suratga olish quyidagilarni o'z ichiga olishi kerak
   Sintetik vaqtlarsiz `zero_fill` bloki.

#### `FASTPQ_GPU=gpu` bilan 20k surat

`fastpq_metal_bench_20k_refresh.json`

| Bosqich | Ustunlar | Kirish len | GPU o'rtacha (ms) | CPU o'rtacha (ms) | GPU ulushi | Tezlashtirish | D CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79,951 ms (65,645–93,193) | 83,289 ms (59,956–107,585) | 0,3% | 1,042× | +3.338 |
| IFFT | 16 | 32768 | 78,605 ms (69,986–83,726) | 93,898 ms (80,656–119,625) | 0,3% | 1,195× | +15.293 |
| LDE | 16 | 262144 | 657,673 ms (619,219–712,367) | 669,537 ms (619,716–723,285) | 2,1% | 1,018× | +11.864 |
| Poseydon | 16 | 524288 | 30004,898 ms (27284,117–32945,253) | 29087,532 ms (24969,810–33020,517) | 97,4% | 0,969× | −917.366 |

Kuzatishlar:

1. `FASTPQ_GPU=gpu` bilan ham, bu suratga olish hali ham protsessorning ishdan chiqishini aks ettiradi:
   `metal_dispatch_queue` nolga yopishgan holda iteratsiya uchun ~30s. Qachon
   bekor qilish oʻrnatilgan, lekin xost metall qurilmani topa olmaydi, CLI endi chiqadi
   har qanday yadrolarni ishga tushirishdan oldin va so'ralgan/hal qilingan rejimni chop etishdan oldin
   backend yorlig'i, shuning uchun muhandislar aniqlash, huquqlar yoki
   metallib qidiruvi pasaytirishga sabab bo'ldi. `fastpq_metal_bench --gpu-probe-ni ishga tushiring
   --qatorlar …` with `FASTPQ_DEBUG_METAL_ENUM=1` ro'yxatga olish jurnalini yozib olish va
   Profilerni qayta ishga tushirishdan oldin asosiy aniqlash muammosini hal qiling.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. Nolinchi to‘ldirish telemetriyasi endi haqiqiy namunani (32 MiB dan 18,66 ms) yozib oladi, bu buni isbotlaydi.
   qilichbozlik tuzatish ishlaydi, lekin navbat deltalari GPU yuborilmaguncha yo'q bo'lib qoladi
   muvaffaqiyatga erishing.
3. Backend pasaytirishda davom etganligi sababli Stage7 telemetriya eshigi harakatsiz
   bloklangan: navbat bo'yicha dalil va poseidonning o'xshashligi haqiqiy GPUni talab qiladi
   yugur.

Ushbu suratlar endi WP2-E qoldiqlarini bog'laydi. Keyingi harakatlar: profilerni yig'ing
olovli diagrammalar va navbat jurnallari (backend GPUda bajarilgandan so'ng),
FFTni qayta ko'rib chiqishdan oldin Poseidon/LDE to'siqlari paydo bo'ladi va orqa qismning qaytarilishini blokdan chiqaring
shuning uchun Stage7 telemetriyasi haqiqiy GPU ma'lumotlariga ega.

### Kuchli tomonlar
- Bosqich bosqichlari, birinchi navbatdagi dizayn, shaffof STARK stek.### Yuqori ustuvor harakat elementlari
1. Qadoqlash/buyurtma moslamalarini amalga oshiring va AIR spetsifikatsiyasini yangilang.
2. `3f2b7fe` Poseidon2 ni yakunlang va misol SMT/qidiruv vektorlarini nashr qiling.
3. Ishlagan misollarni (`lookup_grand_product.md`, `smt_update.md`) armatura bilan birga saqlang.
4. Toʻgʻrilik hosilasi va CI rad etish metodologiyasini hujjatlashtiruvchi ilova A ilovasini qoʻshing.

### hal qilingan dizayn qarorlari
- P1da ZK o'chirilgan (faqat to'g'ri); keyingi bosqichda qayta ko'rib chiqing.
- boshqaruv holatidan olingan ruxsat jadvali ildizi; to'plamlar jadvalga faqat o'qish uchun mo'ljallangan va a'zolikni qidirish orqali isbotlaydi.
- Yo'q kalit dalillar kanonik kodlash bilan nol barg va qo'shni guvohdan foydalanadi.
- Semantikani o'chirish = kanonik kalit maydonida nolga o'rnatilgan barg qiymati.

Ushbu hujjatdan kanonik havola sifatida foydalaning; Driftni oldini olish uchun uni manba kodi, moslamalar va qo'shimchalar bilan birga yangilang.

## Ilova A - Sog'lomlik hosilasi

Ushbu ilovada "Soundness & SLOs" jadvali qanday ishlab chiqarilganligi va CI yuqorida aytib o'tilgan ≥128-bitli qavatni qanday amalga oshirishi tushuntiriladi.

### Belgilash
- `N_trace = 2^k` - saralash va to'ldirishdan keyin ikki darajagacha bo'lgan iz uzunligi.
- `b` - portlash omili (`N_eval = N_trace × b`).
- `r` - FRI aritiyasi (kanonik to'plamlar uchun 8 yoki 16).
- `ℓ` - FRI qisqarishlari soni (`layers` ustuni).
- `q` - har bir dalil uchun tekshirgich so'rovlari (`queries` ustuni).
- `ρ` - ustunni rejalashtiruvchi tomonidan bildirilgan samarali kod tezligi: `ρ = max_i(degree_i / domain_i)` birinchi FRI raundidan omon qolgan cheklovlardan.

Goldilocks tayanch maydonida `|F| = 2^64 - 2^32 + 1` mavjud, shuning uchun Fiat-Shamir to'qnashuvlari `q / 2^64` bilan chegaralanadi. Silliqlash ortogonal `2^{-g}` faktorini qo'shadi, `fastpq-lane-balanced` uchun `g = 23` va kechikish profili uchun `g = 21`.【crates/fastpq_isi/src/params.rs:65】

### Analitik bog'langan

Doimiy DEEP-FRI bilan statistik muvaffaqiyatsizlik ehtimoli qondiradi

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

chunki har bir qatlam polinom darajasi va domen kengligini bir xil `r` faktoriga qisqartiradi va `ρ` doimiyligini saqlaydi. Jadvalning `est bits` ustuni hisobotlari `⌊-log₂ p_fri⌋`; Fiat-Shamir va silliqlash qo'shimcha xavfsizlik chegarasi sifatida xizmat qiladi.

### Planner chiqishi va ishlangan hisoblash

Stage1 ustunini rejalashtiruvchini vakillik partiyalarida ishga tushirish quyidagi natijalarni beradi:

| Parametrlar to'plami | `N_trace` | `b` | `N_eval` | `ρ` (rejalashtiruvchi) | Samarali daraja (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | --------------------------------- | --- | --- | ------------------ |
| Balanslangan 20k partiya | `2^15` | 8 | 262144 | 0.077026 | 20192 | 5 | 52 | 190 bit |
| O'tkazish qobiliyati 65k to'plam | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132 bit |
| Kechikish 131k to'plam | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142 bit |Misol (muvozanatlangan 20k to'plam):
1. `N_trace = 2^15`, shuning uchun `N_eval = 2^15 × 8 = 2^18`.
2. Planner asboblari hisobotlari `ρ = 0.077026`, shuning uchun `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, jadval yozuviga mos keladi.
4. Fiat-Shamir to'qnashuvlari maksimal darajada `2^{-58.3}` ni qo'shadi va silliqlash (`g = 23`) boshqa `2^{-23}`ni olib tashlaydi va umumiy mustahkamlikni 160 bitdan yuqori darajada ushlab turadi.

### CI rad etish-namuna olish jabduqlar

Har bir CI ishida empirik o'lchovlar analitik chegaradan ±0,6 bit ichida qolishini ta'minlash uchun Monte-Karlo jabduqlari amalga oshiriladi:
1. Kanonik parametrlar majmuasini chizing va mos keladigan qatorlar soni bilan `TransitionBatch` ni sintez qiling.
2. Izni yarating, tasodifiy tanlangan cheklovni o'zgartiring (masalan, yirik mahsulot yoki SMT birodarini qidirishni buzish) va dalil keltirishga harakat qiling.
3. Fiat-Shamir muammolarini qayta namuna qilib, tekshirgichni qayta ishga tushiring (silliqlash kiradi) va o'zgartirilgan dalilning rad etilganligini yozib oling.
4. Har bir parametr to'plami uchun 16384 urug' uchun takrorlang va kuzatilgan rad etish tezligining 99% Clopper-Pirson pastki chegarasini bitlarga aylantiring.

Agar o'lchangan pastki chegara 128 bitdan pastga tushsa, ish darhol bajarilmaydi, shuning uchun rejalashtiruvchi, katlama halqasi yoki transkript simlaridagi regressiyalar birlashishdan oldin ushlanib qoladi.

## B ilovasi — Domen-ildiz hosilasi

Stage0 kuzatish va baholash generatorlarini Poseidondan olingan konstantalarga o'rnatadi, shuning uchun barcha ilovalar bir xil kichik guruhlarga ega.

### Jarayon
1. **Urug‘ tanlash.** UTF‑8 tegini `fastpq:v1:domain_roots` FASTPQning boshqa joylarida ishlatiladigan Poseidon2 shimgichga singdiring (holat kengligi=3, tezlik=2, to‘rtta to‘liq + 57 qisman tur). Kirishlar `[len, limbs…]` kodlashini `pack_bytes` dan qayta ishlatib, `g_base = 7` tayanch generatorini beradi.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq_pose:18NI00000779X.
2. **Tiz generatori.** `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` ni hisoblang va yarim quvvat 1 bo‘lmaganda `trace_root^{2^{trace_log_size}} = 1` ni tekshiring.
3. **LDE generatori.** `lde_root` hosil qilish uchun `lde_log_size` bilan bir xil darajani takrorlang.
4. **Koset tanlash.** Stage0 asosiy kichik guruhdan foydalanadi (`omega_coset = 1`). Kelajakdagi kosetlar `fastpq:v1:domain_roots:coset` kabi qo'shimcha tegni o'zlashtira oladi.
5. **Oʻzgartirish oʻlchami.** `permutation_size` ni aniq saqlang, shuning uchun rejalashtiruvchilar hech qachon ikkining yashirin vakolatlaridan toʻldirish qoidalarini tushunmaydilar.

### Ko'paytirish va tasdiqlash
- Asboblar: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` Rust parchalari yoki Markdown jadvalini chiqaradi (qarang: `--format table`, `--seed`, `--filter`).【scripts/fastpq/src_gen/poseidon:】
- Sinovlar: `canonical_sets_meet_security_target` kanonik parametrlar toʻplamini nashr etilgan konstantalarga (nol boʻlmagan ildizlar, portlash/arity juftligi, almashtirish oʻlchami) mos holatda saqlaydi, shuning uchun `cargo test -p fastpq_isi` driftni darhol ushlaydi.【crates/fastpq_isi/src/18ms.
- Haqiqat manbai: har safar yangi parametr paketlari kiritilganda Stage0 jadvali va `fastpq_isi/src/params.rs` ni yangilang.

## Ilova C - Majburiyat quvurlari tafsilotlari### Streaming Poseidon majburiyatlari oqimi
Stage2 prover va tekshiruvchi tomonidan baham ko'rilgan deterministik kuzatuv majburiyatini belgilaydi:
1. **O‘tishlarni normallashtiring.** `trace::build_trace` har bir partiyani saralaydi, uni `N_trace = 2^{⌈log₂ rows⌉}` ga joylashtiradi va ustun vektorlarini quyidagi tartibda chiqaradi.【crates/fastpq_prover/src/trace.rs:123】
2. **Xesh ustunlari.** `trace::column_hashes` ustunlarni `fastpq:v1:trace:column:<name>` tegli maxsus Poseidon2 gubkalari orqali uzatadi. `fastpq-prover-preview` funksiyasi faol bo'lsa, xuddi shu o'tish backend tomonidan talab qilinadigan IFFT/LDE koeffitsientlarini qayta ishlaydi, shuning uchun qo'shimcha matritsa nusxalari ajratilmaydi.【crates/fastpq_prover/src/trace.rs:474】
3. **Merkle daraxtiga ko‘taring.** `trace::merkle_root` `fastpq:v1:trace:node` yorlig‘i ostidagi Poseidon tugunlari bilan ustunni buklaydi, bu alohida holatlarning oldini olish uchun sathda g‘alati fan chiqishi bo‘lsa, oxirgi bargni takrorlaydi.【crates/fastpq_prover/src.6/src:
4. **Dajestni yakunlang.** `digest::trace_commitment` domen tegini (`fastpq:v1:trace_commitment`), parametr nomini, toʻldirilgan oʻlchamlarni, ustun dayjestlarini va Merkle ildiziga bir xil `[len, limbs…]` kodlashdan foydalangan holda prefiks qoʻyadi, soʻngra SHA6b-da emlashdan oldin foydali yukni xeshlaydi5. `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

Tasdiqlovchi Fiat-Shamir sinovlarini tanlashdan oldin xuddi shu dayjestni qayta hisoblab chiqadi, shuning uchun har qanday ochilishdan oldin dalillarni bekor qilish mos kelmaydi.

### Poseidon zaxira boshqaruvlari- Prover endi maxsus Poseidon quvur liniyasini bekor qilishni (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) ochib beradi, shuning uchun operatorlar Stagems7 <0 maqsadiga erisha olmagan qurilmalarda GPU FFT/LDE bilan CPU Poseidon xeshingini aralashtirishlari mumkin. Qo'llab-quvvatlanadigan qiymatlar ijro rejimi tugmachasini (`auto`, `cpu`, `gpu`) aks ettiradi, belgilanmagan bo'lsa, standart global rejimga o'tadi. Ish vaqti ushbu qiymatni chiziqli konfiguratsiya (`FastpqPoseidonMode`) orqali o'tkazadi va uni proverga (`Prover::canonical_with_modes`) tarqatadi, shuning uchun bekor qilishlar konfiguratsiyada deterministik va tekshirilishi mumkin. dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- Telemetriya yangi `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` hisoblagichi (va OTLP egizak `fastpq.poseidon_pipeline_resolutions_total`) orqali hal qilingan quvur liniyasi rejimini eksport qiladi. Shunday qilib, `sorafs`/operator asboblar paneli protsessorning majburiy qaytarilishiga (`path="cpu_forced"`) yoki ish vaqtining pasaytirishlariga (`path="cpu_fallback"`) qarshi GPU birlashtirilgan/quvurli xeshlash ishlayotganligini tasdiqlashi mumkin. CLI probi avtomatik ravishda `irohad` da o'rnatiladi, shuning uchun to'plamlarni chiqaring va jonli telemetriya bir xil dalillar oqimiga ega.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- Aralash rejimli dalillar, shuningdek, mavjud qabul qilish eshigi orqali har bir tabloga muhrlanadi: prover har bir partiya uchun hal qilingan rejim + yo'l yorlig'ini chiqaradi va isbot tushganda `fastpq_poseidon_pipeline_total` hisoblagich ijro rejimi hisoblagichi bilan birga oshadi. Bu WP2-E.6 ni qoralamalarni ko'rinadigan qilish va optimallashtirish davom etayotganda deterministik pasaytirish uchun toza kalitni taqdim etish orqali qondiradi.【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` endi Prometheus qirqishlarini (Metal yoki CUDA) tahlil qiladi va har bir o'ralgan to'plam ichiga `poseidon_metrics` xulosasini joylashtiradi. Yordamchi `metadata.labels.device_class` bo'yicha hisoblagich qatorlarini filtrlaydi, mos keladigan `fastpq_execution_mode_total` namunalarini oladi va `fastpq_poseidon_pipeline_total` yozuvlari etishmayotgan bo'lsa, o'rashni amalga oshirmaydi, shuning uchun WP2-E.6 to'plamlari har doim CUDA/ad-Mehoc o'rniga takrorlanadigan dalillarni jo'natadi. eslatmalar.【scripts/fastpq/wrap_benchmark.py:1】【skriptlar/fastpq/tests/test_wrap_benchmark.py:1】

#### Deterministik aralash rejim siyosati (WP2-E.6)1. **GPU tanqisligini aniqlang.** Stage7 suratga olish yoki jonli Grafana surati Poseidon kechikishini ko‘rsatadigan har qanday qurilma sinfini belgilang, bunda FFT/LDE maqsaddan past bo‘lib, jami tekshirish vaqtini >900ms ushlab turadi. Operatorlar suratga olish matritsasiga (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) izoh qo'yadi va `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` to'xtab qolganda, `fastpq_execution_mode_total{backend="metal"}` esa GPU FFT/LDE yozishni davom ettirganda, qo'ng'iroqqa sahifa beradi. jo'natmalar.【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **Poseidon protsessorini faqat zararlangan hostlar uchun aylantiring.** `zk.fastpq.poseidon_mode = "cpu"` (yoki `FASTPQ_POSEIDON_MODE=cpu`) ni mahalliy xost konfiguratsiyasida flot yorliqlari bilan birga `zk.fastpq.execution_mode = "gpu"` ni saqlab, FFT/LDE tezlatgichdan foydalanishda davom eting. Chiqarish chiptasida konfiguratsiya farqini yozib oling va to'plamga `poseidon_fallback.patch` sifatida har bir xostni bekor qilishni qo'shing, shunda sharhlovchilar o'zgarishlarni aniq takrorlashlari mumkin.
3. **Pastlashni isbotlang.** Tugunni qayta ishga tushirgandan so'ng darhol Poseidon hisoblagichini qirib tashlang:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   Chiqindixonada GPU ijro hisoblagichi bilan qulflash bosqichida o'sib borayotgan `path="cpu_forced"` ko'rsatilishi kerak. Skrabni mavjud `metrics_cpu_fallback.prom` surati yonida `metrics_poseidon.prom` sifatida saqlang va `poseidon_fallback.log` da mos keladigan `telemetry::fastpq.poseidon` jurnali chiziqlarini oling.
4. **Monitor va chiqish.** Optimallashtirish ishlari davom etar ekan, `fastpq_poseidon_pipeline_total{path="cpu_forced"}` da ogohlantirishni davom ettiring. Yamoq sinov xostida sinovdan o‘tkaziluvchi ish vaqtini 900 ms dan pastga qaytargandan so‘ng, konfiguratsiyani `auto` ga qaytaring, qirqishni qayta ishga tushiring (yana `path="gpu"` ko‘rsatilgan) va aralash rejimli matkapni yopish uchun to‘plamga oldingi/keyin ko‘rsatkichlarini biriktiring.

**Telemetriya shartnomasi.**

| Signal | PromQL / Manba | Maqsad |
|--------|-----------------|---------|
| Poseidon rejimi hisoblagichi | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Protsessor xeshingi qasddan qilinganligini va belgilangan qurilma sinfiga tegishli ekanligini tasdiqlaydi. |
| Bajarish rejimi hisoblagichi | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | FFT/LDE hali ham Poseidon versiyasini pasaytirganda ham GPUda ishlashini isbotlaydi. |
| Jurnal dalillar | `poseidon_fallback.log` da olingan `telemetry::fastpq.poseidon` yozuvlari | Xost `cpu_forced` sababi bilan protsessor xeshingga qaror qilganiga har bir dalil dalil beradi. |

Boshqaruv FFT/LDE telemetriyasi bilan bir qatorda deterministik qayta tiklash siyosatini tekshirishi uchun aralash rejim faol bo'lganda tarqatish to'plami endi `metrics_poseidon.prom`, konfiguratsiya farqi va jurnaldan ko'chirmani o'z ichiga olishi kerak. `ci/check_fastpq_rollout.sh` allaqachon navbat/nol to'ldirish chegaralarini qo'llaydi; Kuzatuv darvozasi aralash rejimda bo'shatishni avtomatlashtirishga tushgandan so'ng, Poseidon hisoblagichining sog'lig'ini tekshiradi.

Stage7 suratga olish moslamasi allaqachon CUDA bilan ishlaydi: har bir `fastpq_cuda_bench` to‘plamini `--poseidon-metrics` bilan o‘rang (qirib tashlangan `metrics_poseidon.prom` ga ishora qiladi) va chiqish endi Metalify-da qo‘llaniladigan bir xil quvur liniyasi hisoblagichlarini/ravshanlik xulosasini o‘z ichiga oladi. asboblar.【skriptlar/fastpq/wrap_benchmark.py:1】### Ustun tartibi
Xeshlash quvur liniyasi ustunlarni ushbu deterministik tartibda iste'mol qiladi:
1. Selektor bayroqlari: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_active`, I08NI60, `s_perm`.
2. Qadoqlangan limb ustunlari (har biri nol uzunligiga to'ldirilgan): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Yordamchi skalyarlar: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `neighbour_leaf`, I018, I018 `slot`.
4. Har bir daraja uchun siyrak Merkle guvohlari `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` ustunlar bo'ylab aynan shu tartibda yuradi, shuning uchun to'ldiruvchining orqa tomoni va Stage2 STARK ilovasi relizlar bo'ylab barqaror bo'lib qoladi.【crates/fastpq_prover/src/trace.rs:474】

### Transkripsiya domen teglari
Stage2 muammo yaratishni deterministik saqlash uchun quyidagi Fiat-Shamir katalogini tuzatadi:

| teg | Maqsad |
| --- | ------- |
| `fastpq:v1:init` | Protokol versiyasini, parametrlar to'plamini va `PublicIO`ni qabul qiling. |
| `fastpq:v1:roots` | Merkle ildizlarini kuzatib boring va qidiring. |
| `fastpq:v1:gamma` | Katta mahsulot qidirishdan namuna oling. |
| `fastpq:v1:alpha:<i>` | Namuna tarkibi-polinom muammolari (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Baholangan yirik mahsulotni o'z ichiga oling. |
| `fastpq:v1:beta:<round>` | Har bir FRI raundi uchun katlama muammosidan namuna oling. |
| `fastpq:v1:fri_layer:<round>` | Har bir FRI qatlami uchun Merkle ildizini kiriting. |
| `fastpq:v1:fri:final` | So'rovlarni ochishdan oldin oxirgi FRI qatlamini yozib oling. |
| `fastpq:v1:query_index:0` | Tekshirish so'rovi indekslarini aniq hosil qiling. |