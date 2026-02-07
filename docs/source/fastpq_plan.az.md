---
lang: az
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ Prover İşinin Dağılması

Bu sənəd istehsala hazır FASTPQ-ISI proverinin çatdırılması və məlumat məkanının planlaşdırılması boru kəmərinə qoşulması üçün mərhələli planı əks etdirir. Aşağıdakı hər bir tərif TODO kimi qeyd olunmadığı halda normativdir. Təxmini sağlamlıq Qahirə tipli DEEP-FRI sərhədlərindən istifadə edir; Ölçülmüş həddi 128 bitdən aşağı düşərsə, CI-də avtomatik rədd etmə-nümunə alma testləri uğursuz olur.

## Mərhələ 0 — Hash Yer Tutucusu (eniş)
- BLAKE2b öhdəliyi ilə deterministik Norito kodlaşdırması.
- `BackendUnavailable` qaytaran yer tutucu arxa ucu.
- `fastpq_isi` tərəfindən təmin edilən kanonik parametrlər cədvəli.

## Mərhələ 1 — Trace Builder Prototipi

> **Status (2025-11-09):** `fastpq_prover` indi kanonik qablaşdırmanı ifşa edir
> köməkçilər (`pack_bytes`, `PackedBytes`) və deterministik Poseidon2
> Goldilocks üzərində sifariş öhdəliyi. Sabitlər bərkidilir
> `ark-poseidon2` `3f2b7fe` öhdəliklərini yerinə yetirir, müvəqqəti BLAKE2-nin dəyişdirilməsi ilə bağlı təqibi bağlayır
> yer tutucu bağlıdır. Qızıl armaturlar (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) indi reqressiya dəstini bağlayır.

### Məqsədlər
- KV yeniləmə AIR üçün FASTPQ iz qurucusunu tətbiq edin. Hər bir sıra kodlamalıdır:
  - `key_limbs[i]`: kanonik açar yolunun baza-256 üzvü (7 bayt, az-endian).
  - `value_old_limbs[i]`, `value_new_limbs[i]`: əvvəlki/sonrakı dəyərlər üçün eyni qablaşdırma.
  - Seçici sütunlar: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, I100NI670, `s_perm`.
  - Köməkçi sütunlar: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Aktiv sütunları: `asset_id_limbs[i]` 7 baytlıq hissələrdən istifadə edir.
  - `ℓ` səviyyəsi üzrə SMT sütunları: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, üstəgəl üzv olmayanlar üçün `neighbour_leaf`.
  - Metadata sütunları: `dsid`, `slot`.
- **Deterministik sıralama.** Sabit çeşidləmədən istifadə edərək sətirləri leksikoqrafik olaraq `(key_bytes, op_rank, original_index)` ilə sıralayın. `op_rank` xəritəçəkmə: `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, Prometheus. `original_index` çeşidləmədən əvvəl 0 əsaslı indeksdir. Yaranan Poseidon2 sifariş hashını davam etdirin (domen teqi `fastpq:v1:ordering`). Uzunluqlar u64 sahə elementləri olduğu üçün heş preimajını `[domain_len, domain_limbs…, payload_len, payload_limbs…]` kimi kodlayın, beləliklə arxada qalan sıfır bayt fərqlənə bilər.
- Axtarış şahidi: saxlanılan `s_perm` sütunu (`s_role_grant` və `s_role_revoke` məntiqi OR) 1 olduqda `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` istehsal edin. Rol/icazə identifikatorları sabit eni 32 bayt LE; dövr 8 bayt LE-dir.
- Həm AIR-dən əvvəl, həm də AIR daxilində invariantları tətbiq edin: seçicilər bir-birini istisna edən, hər aktivin qorunması, dsid/slot sabitləri.
- `N_trace = 2^k` (sətir sayının `pow2_ceiling`); `N_eval = N_trace * 2^b` burada `b` partlama eksponentidir.
- Qurğular və əmlak testlərini təmin edin:
  - Qablaşdırma gediş-gəliş (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Sifariş sabitlik hash (`tests/fixtures/ordering_hash.json`).
  - Partiya qurğuları (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### AIR Sütun Sxemi
| Sütun Qrupu | Adlar | Təsvir |
| ----------------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Fəaliyyət | `s_active` | Həqiqi sıralar üçün 1, doldurma üçün 0.                                                                                       |
| Əsas | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Paketlənmiş Goldilocks elementləri (kiçik endian, 7 bayt əzalar).                                                             |
| Aktiv | `asset_id_limbs[i]` | Qablaşdırılmış kanonik aktiv identifikatoru (az-endian, 7 baytlıq hissələr).                                                      |
| Seçicilər | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Məhdudiyyət: Σ seçicilər (`s_perm` daxil olmaqla) = `s_active`; `s_perm` rol qrant/ləğv sətirlərini əks etdirir.              |
| Köməkçi | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | Məhdudiyyətlər, konservasiya və audit yolları üçün istifadə edilən dövlət.                                                           |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Səviyyə başına Poseidon2 giriş/çıxışları və üzv olmamaq üçün qonşu şahid.                                         |
| Axtar | `perm_hash` | İcazə axtarışı üçün Poseidon2 hash (yalnız `s_perm = 1` zamanı məhdudlaşdırılır).                                            |
| Metadata | `dsid`, `slot` | Satırlar arasında sabit.                                                                                                 |### Riyaziyyat və Məhdudiyyətlər
- **Sahə qablaşdırması:** baytlar 7 baytlıq hissələrə bölünür (az-endian). Hər bir üzv `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; əzalarını rədd etmək ≥ Goldilocks modulu.
- **Balans/konservasiya:** icazə verin `δ = value_new - value_old`. Sətirləri `asset_id` ilə qruplaşdırın. Hər aktiv qrupunun birinci cərgəsində `r_asset_start = 1` təyin edin (əks halda 0) və məhdudlaşdırın
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  Hər bir aktiv qrupunun sonuncu sətirində təsdiq edin
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  Transferlər məhdudiyyəti avtomatik təmin edir, çünki onların δ dəyərləri qrup üzrə sıfıra bərabərdir. Nümunə: əgər `value_old = 100` və `value_new = 120` nanə cərgəsindədirsə, δ = 20, beləliklə, nanə məbləği +20 verir və heç bir yanıq baş vermədikdə yekun yoxlama sıfıra bərabər olur.
- ** Doldurma:** `s_active` təqdim edin. Bütün sıra məhdudiyyətlərini `s_active`-ə vurun və bitişik prefiksi tətbiq edin: `s_active[i] ≥ s_active[i+1]`. Doldurma sətirləri (`s_active=0`) sabit dəyərləri saxlamalıdır, lakin başqa cür məhdudlaşdırılmamalıdır.
- **Sifariş hashı:** Sıra kodlaşdırmaları üzərində Poseidon2 hash (domen `fastpq:v1:ordering`); yoxlanılabilirlik üçün İctimai IO-da saxlanılır.

## Mərhələ 2 — STARK Prover Core

### Məqsədlər
- İzləmə və axtarış qiymətləndirmə vektorları üzərində Poseidon2 Merkle öhdəlikləri yaradın. Parametrlər: dərəcə=2, tutum=1, tam dövrələr=8, qismən dövrələr=57, `ark-poseidon2`-ə sabitlənmiş sabitlər `3f2b7fe` (v0.3.0).
- Aşağı dərəcəli genişləndirmə: `D = { g^i | i = 0 .. N_eval-1 }` domenində hər bir sütunu qiymətləndirin, burada `N_eval = 2^{k+b}` Goldilocks-un 2-adik tutumunu bölür. Qoy `g = ω^{(p-1)/N_eval}` ilə `ω` Goldilocks-un sabit primitiv kökü və `p` modulu; əsas alt qrupdan istifadə edin (koset yoxdur). Transkriptdə `g` yazın (teq `fastpq:v1:lde`).
- Kompozisiya polinomları: hər bir məhdudiyyət `C_j` üçün, aşağıda sadalanan dərəcə kənarları ilə `F_j(X) = C_j(X) / Z_N(X)` forması.
- Axtarış arqumenti (icazələr): transkriptdən nümunə `γ`. İz məhsulu `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Cədvəl məhsulu `T = ∏_j (table_perm_j - γ)`. Sərhəd məhdudiyyəti: `Z_final / T = 1`.
- Arity `r ∈ {8, 16}` ilə DEEP-FRI: hər təbəqə üçün `fastpq:v1:fri_layer_ℓ` etiketi ilə kökü udmaq, nümunə `β_ℓ` (teq `fastpq:v1:beta_ℓ`) və `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` vasitəsilə qatlayın.
- Sübut obyekti (Norito kodlu):
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
- Doğrulayıcı güzgülər prover; qızıl transkriptlərlə 1k/5k/20k sıra izlərində reqressiya dəstini işlədin.

### Mühasibatlıq dərəcəsi
| Məhdudiyyət | Bölmədən əvvəl dərəcə | Seçicilərdən sonra dərəcə | Margin vs `deg(Z_N)` |
|------------|------------------------|------------------------|----------------------|
| Transfer/nanə/yanıq konservasiyası | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Rol verilməsi/ləğv axtarışı | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Metadata dəsti | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT hash (səviyyəyə görə) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Böyük məhsulu axtarın | məhsul əlaqəsi | Yoxdur | Sərhəd məhdudiyyəti |
| Sərhəd kökləri / tədarük cəmi | 0 | 0 | dəqiq |

Doldurma sıraları `s_active` vasitəsilə idarə olunur; dummy sıralar məhdudiyyətləri pozmadan izi `N_trace`-ə qədər genişləndirir.## Kodlaşdırma və Transkript (Qlobal)
- **Bayt qablaşdırma:** baza-256 (7 bayt əzalar, kiçik endian). `fastpq_prover/tests/packing.rs`-də testlər.
- **Sahə kodlaması:** kanonik Goldilocks (little-endian 64-bit limb, rədd ≥ p); Poseidon2 çıxışları/SMT kökləri 32 baytlıq kiçik endian massivlər kimi seriallaşdırılır.
- **Transkript (Fiat–Şamir):**
  1. BLAKE2b `protocol_version`, `params_version`, `parameter_set`, `public_io` və Poseidon2 əmsal etiketini (`fastpq:v1:init`) udur.
  2. `trace_root`, `lookup_root` (`fastpq:v1:roots`) udmaq.
  3. Axtarış çağırışını əldə edin `γ` (`fastpq:v1:gamma`).
  4. Kompozisiya problemlərini əldə edin `α_j` (`fastpq:v1:alpha_j`).
  5. Hər FRI təbəqəsinin kökü üçün `fastpq:v1:fri_layer_ℓ` ilə udmaq, `β_ℓ` (`fastpq:v1:beta_ℓ`) əldə etmək.
  6. Sorğu indekslərini əldə edin (`fastpq:v1:query_index`).

  Teqlər kiçik ASCII hərfləridir; yoxlayıcılar seçmə problemlərindən əvvəl uyğunsuzluqları rədd edir. Qızıl transkript qurğusu: `tests/fixtures/transcript_v1.json`.
- **Versiya:** `protocol_version = 1`, `params_version` `fastpq_isi` parametr dəstinə uyğun gəlir.

## Axtarış Arqumenti (İcazələr)
- Təqdim edilmiş cədvəl leksikoqrafik olaraq `(role_id_bytes, permission_id_bytes, epoch_le)` tərəfindən çeşidlənmiş və Poseidon2 Merkle ağacı (`PublicIO`-də `perm_root`) vasitəsilə tərtib edilmişdir.
- İzləmə şahidi `perm_hash` və `s_perm` selektorundan istifadə edir (və ya rolun verilməsi/ləğv edilməsi). Tuple sabit genişliklərlə (32, 32, 8 bayt) `role_id_bytes || permission_id_bytes || epoch_u64_le` kimi kodlaşdırılıb.
- Məhsul əlaqəsi:
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Sərhəd təsdiqi: `Z_final / T = 1`. Beton akkumulyatorla tanış olmaq üçün baxın `examples/lookup_grand_product.md`.

## Seyrək Merkle Ağacı Məhdudiyyətləri
- `SMT_HEIGHT` (səviyyələrin sayı) təyin edin. `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` sütunları bütün `ℓ ∈ [0, SMT_HEIGHT)` üçün görünür.
- `ark-poseidon2` üçün bağlanmış Poseidon2 parametrləri `3f2b7fe` (v0.3.0); domen teqi `fastpq:v1:poseidon_node`. Bütün qovşaqlar kiçik endian sahə kodlaşdırmasından istifadə edir.
- Hər səviyyə üçün qaydaları yeniləyin:
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- Əlavələr dəsti `(node_in_0 = 0, node_out_0 = value_new)`; `(node_in_0 = value_old, node_out_0 = 0)` dəstini silir.
- Qeyri-üzvlük sübutları sorğulanan intervalın boş olduğunu göstərmək üçün `neighbour_leaf` təmin edir. İşlənmiş nümunə və JSON tərtibatı üçün `examples/smt_update.md`-ə baxın.
- Sərhəd məhdudiyyəti: yekun hash əvvəlki sətirlər üçün `old_root` və sonrakı sətirlər üçün `new_root`-ə bərabərdir.

## Səs parametrləri və SLO
| N_trace | partlatmaq | FRI arity | qatlar | sorğular | est bit | Sübut ölçüsü (≤) | RAM (≤) | P95 gecikmə (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1.5 GB | 0,40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2.5 GB | 0,75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3,5 GB | 1,20 s (A100) |

Törəmələr Əlavə A-ya uyğundur. CI qoşqu düzgün tərtib edilməmiş sübutlar yaradır və təxmin edilən bit <128 olarsa uğursuz olur.## İctimai IO Sxemi
| Sahə | Bayt | Kodlaşdırma | Qeydlər |
|----------------|-------|---------------------------------------|-------------------------------------|
| `dsid` | 16 | kiçik endian UUID | Giriş zolağı üçün Dataspace ID (defolt zolaq üçün qlobal), `fastpq:v1:dsid` teqi ilə heşlənmişdir. |
| `slot` | 8 | little-endian u64 | Epoxadan bəri nanosaniyələr.            |
| `old_root` | 32 | kiçik endian Poseidon2 sahə baytları | Partiyadan əvvəl SMT kökü.              |
| `new_root` | 32 | kiçik endian Poseidon2 sahə baytları | Partiyadan sonra SMT kökü.               |
| `perm_root` | 32 | kiçik endian Poseidon2 sahə baytları | Yuva üçün icazə cədvəlinin kökü. |
| `tx_set_hash` | 32 | BLAKE2b | Sıralanmış təlimat identifikatorları.     |
| `parameter` | var | UTF-8 (məsələn, `fastpq-lane-balanced`) | Parametr dəstinin adı.                 |
| `protocol_version`, `params_version` | hər biri 2 | little-endian u16 | Versiya dəyərləri.                      |
| `ordering_hash` | 32 | Poseidon2 (kiçik-endian) | Sıralanmış cərgələrin sabit hashı.         |

Silinmə sıfır dəyər üzvləri ilə kodlanır; olmayan açarlar sıfır yarpaq + qonşu şahiddən istifadə edir.

`FastpqTransitionBatch.public_inputs` `dsid`, `slot` və kök öhdəlikləri üçün kanonik daşıyıcıdır;
toplu metadata giriş hash/transkript hesabının uçotu üçün qorunur.

## Kodlaşdırma Haşları
- Sifariş hash: Poseidon2 (teq `fastpq:v1:ordering`).
- Toplu artefakt hash: `PublicIO || proof.commitments` üzərində BLAKE2b (teq `fastpq:v1:artifact`).

## Bitmiş Mərhələ Tərifləri (DoD)
- **Mərhələ 1 DoD**
  - Qablaşdırma gediş-dönüş testləri və qurğular birləşdirildi.
  - AIR spesifikasiyasına (`docs/source/fastpq_air.md`) `s_active`, aktiv/SMT sütunları, seçici tərifləri (`s_perm` daxil olmaqla) və simvolik məhdudiyyətlər daxildir.
  - Sifariş hash PublicIO-da qeydə alınmış və qurğular vasitəsilə təsdiq edilmişdir.
  - Üzvlük və qeyri-üzv vektorları ilə həyata keçirilən SMT/lookup şahid nəsli.
  - Konservasiya testləri transfer, nanə, yandırma və qarışıq partiyaları əhatə edir.
- **Mərhələ 2 DoD**
  - Transkript spesifikasiyası həyata keçirilir; qızıl transkript (`tests/fixtures/transcript_v1.json`) və domen teqləri təsdiqləndi.
  - Poseidon2 parametri `3f2b7fe` arxitekturalar arasında indianness testləri ilə sübut və yoxlayıcıda sabitlənmişdir.
  - Sağlamlıq CI qoruyucusu aktivdir; sübut ölçüsü/RAM/gecikmə SLOları qeydə alınıb.
- **Mərhələ 3 DoD**
  - Planlaşdırıcı API (`SubmitProofRequest`, `ProofResult`) identifikasiya açarları ilə sənədləşdirilmişdir.
  - Yenidən cəhd/backoff ilə məzmun ünvanlı şəkildə saxlanılan sübut artefaktları.
  - Telemetriya növbənin dərinliyi, növbə gözləmə müddəti, sübutun icra gecikməsi, təkrar cəhd sayları, arxa uç uğursuzluqları və GPU/CPU istifadəsi üçün tablosuna və hər bir metrik üçün xəbərdarlıq həddi ilə ixrac edilmişdir.## Mərhələ 5 — GPU sürətləndirilməsi və optimallaşdırılması
- Hədəf nüvələri: LDE (NTT), Poseidon2 hashing, Merkle ağacının qurulması, FRI qatlama.
- Determinizm: sürətli riyaziyyatı söndürün, CPU, CUDA, Metal arasında bit-eyni nəticələri təmin edin. CI sübut köklərini cihazlar arasında müqayisə etməlidir.
- İstinad aparatında (məsələn, Nvidia A100, AMD MI210) CPU və GPU-nu müqayisə edən benchmark dəsti.
- Metal arxa hissə (Apple Silicon):
  - Quraşdırma skripti nüvə dəstini (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) `fastpq.metallib`-ə `xcrun metal`/`xcrun metallib` vasitəsilə tərtib edir; macOS developer alətlərinə Metal alətlər silsiləsi daxil olduğundan əmin olun (`xcode-select --install`, sonra tələb olunarsa `xcodebuild -downloadComponent MetalToolchain`).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - CI istiləşmələri və ya deterministik qablaşdırma üçün əl ilə yenidən qurulma (güzgülər `build.rs`):
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Uğurlu konstruksiyalar `FASTPQ_METAL_LIB=<path>` buraxır, beləliklə iş vaxtı metallibi deterministik şəkildə yükləyə bilər.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - LDE ləpəsi indi qiymətləndirmə buferinin hostda sıfır başlanğıc olduğunu qəbul edir. Yenidən istifadə edərkən mövcud `vec![0; ..]` ayırma yolunu və ya açıq şəkildə sıfır tamponları saxlayın.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:14:
  - Əlavə keçiddən qaçmaq üçün Coset vurma son FFT mərhələsinə birləşdirilir; LDE quruluşuna edilən hər hansı dəyişiklik həmin invariantı qorumalıdır.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - Paylaşılan yaddaş FFT/LDE nüvəsi indi plitə dərinliyində dayanır və qalan kəpənəkləri üstəgəl hər hansı tərs miqyasla xüsusi `fastpq_fft_post_tiling` keçidinə verir. Rust hostu eyni sütun qruplarını hər iki nüvədən keçir və yalnız `log_len` kafel limitini keçdikdə post-kafel göndərilməsini işə salır, beləliklə, GPU geniş mərhələli işi tamamilə idarə edərkən növbə dərinliyi telemetriyası, kernel statistikası və ehtiyat davranışı deterministik olaraq qalır. cihazda.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - Başlatma formaları ilə sınaqdan keçirmək üçün `FASTPQ_METAL_THREADGROUP=<width>` təyin edin; dispetçer yolu dəyəri cihaz limitinə sıxışdırır və ləğvetməni qeyd edir ki, profilləmə işləri yenidən tərtib etmədən mövzu qrupunun ölçülərini süpürə bilsin.【crates/fastpq_prover/src/metal.rs:321】- FFT plitəsini birbaşa tənzimləyin: host indi ip qrupları zolaqlarını əldə edir (qısa izlər üçün 16, `log_len ≥ 6` bir dəfə, 64 dəfə `log_len ≥ 10`, 128 dəfə `log_len ≥ 14` və Prometheus və kiçik mərhələlərdə 256) izlər, `log_len ≥ 12` zaman 4 və domen `log_len ≥ 18/20/22`-ə çatdıqdan sonra paylaşılan yaddaş keçidi indi nəzarəti post-kafel nüvəsinə ötürməzdən əvvəl 12/14/16 mərhələdə işləyir) üstəgəl cihazın icra eni/maksimum ipləri. Xüsusi işə salma formalarını bərkitmək üçün `FASTPQ_METAL_FFT_LANES` (8 və 256 arasında ikinin gücü) və `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) ilə əvəz edin; hər iki dəyər `FftArgs` vasitəsilə axır, dəstəklənən pəncərəyə bərkidilir və profil yaratmaq üçün qeyd olunur süpürür.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT və LDE sütunlarının yığılması indi həll edilmiş mövzu qrupunun genişliyindən əldə edilir: host hər komanda buferinə təxminən 4096 məntiqi ipi hədəfləyir, dairəvi bufer plitə quruluşu ilə eyni vaxtda 64-ə qədər sütunu qoruyur və yalnız 64→32→16→16→8→4 domenini kəsişdirir. 2¹⁶/2¹⁸/2²⁰/2²² həddlər. Bu, uzun kosetlərin hələ də deterministik şəkildə tamamlanmasını təmin edərkən, 20k-cərgə çəkilişi hər göndəriş üçün ≥64 sütunda saxlayır. Adaptiv planlaşdırıcı hələ də göndərişlər ≈2ms hədəfinə yaxınlaşana qədər sütun enini ikiqat artırır və indi nümunə götürülmüş göndəriş həmin hədəfi ≥30% üstələdikdə partiyanı avtomatik olaraq yarıya endirir, beləliklə, hər sütun üçün qiyməti artıran zolaq/kafel keçidləri əl ilə ləğv edilmədən geri düşür. Poseidon permütasiyaları eyni adaptiv planlaşdırıcını paylaşır və `fastpq_metal_bench`-dəki `metal_heuristics.batch_columns.poseidon` bloku indi həll edilmiş vəziyyət sayını, qapağını, son müddətini və ləğvetmə bayrağını qeyd edir, beləliklə növbə dərinliyi telemetriyası birbaşa Poseidon tənzimləməsinə bağlana bilər. Deterministik bir FFT toplu ölçüsünü təyin etmək üçün `FASTPQ_METAL_FFT_COLUMNS` (1–64) ilə əvəz edin və sabit sütun sayına hörmət etmək üçün LDE dispetçerinə ehtiyacınız olduqda `FASTPQ_METAL_LDE_COLUMNS` (1–64) istifadə edin; Metal dəzgah hər çəkilişdə həll edilmiş `kernel_profiles.*.columns` girişlərini üzə çıxarır, beləliklə tuning təcrübələri qalır təkrar istehsal edilə bilər.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:128- Çox növbəli göndəriş indi diskret Mac-larda avtomatikdir: host `is_low_power`, `is_headless` və cihazın yerini yoxlayır, iki Metal əmr növbəsini fırlatmaq qərarına gəlir, yalnız iş yükü ən azı 16 sütunu daşıyanda fanlar çıxır (həll edilmiş fan-basaçalar tərəfindən miqyaslanır və hər ikisi uzun müddət saxlanılır), GPU zolaqları determinizmdən ödün vermədən məşğuldur. Komanda-bufer semaforu indi “hər növbədə iki uçuş” mərtəbəsini tətbiq edir və növbə telemetriyası ümumi ölçmə pəncərəsini (`window_ms`) və qlobal semafor üçün normallaşdırılmış məşğulluq əmsallarını (`busy_ratio`) qeyd edir və hər növbəyə daxil olan hər bir giriş beləliklə sübut oluna bilər≥0 eyni vaxtda məşğuldur. `FASTPQ_METAL_QUEUE_FANOUT` (1–4 zolaqlar) və `FASTPQ_METAL_COLUMN_THRESHOLD` (fan-outdan əvvəl minimum ümumi sütunlar) ilə defoltları ləğv edin; Metal paritet testləri, çox GPU-lu Mac-ların qapalı qalması üçün ləğvləri məcbur edir və həll edilmiş siyasət növbə dərinliyi telemetriyası və yeni `metal_dispatch_queue.queues[*]` ilə birlikdə qeyd olunur. blok.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【sandıq s/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- Metal aşkarlama indi `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices`-i birbaşa yoxlayır (başsız mərmilərdə CoreGraphics-i qızdırır) və `system_profiler`-ə qayıtmazdan əvvəl `FASTPQ_DEBUG_METAL_ENUM` sadalanan cihazları çap edir və başsız işə salındıqda niyə başsız olduğunu izah edə bilər. `FASTPQ_GPU=gpu` hələ də CPU yoluna endirilib. Ləğvetmə `gpu` olaraq təyin edildikdə, lakin heç bir sürətləndirici aşkarlanmadıqda, `fastpq_metal_bench` indi CPU-da səssizcə davam etmək əvəzinə, sazlama düyməsinin göstəricisi ilə dərhal səhv edir. Bu, WP2‑E-də çağırılan "səssiz CPU geri qaytarılması" sinfini daraldır və operatorlara bükülmüş içəridə siyahı qeydlərini tutmaq üçün düymə verir. benchmarks.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU vaxtları indi CPU ehtiyatlarını “GPU” məlumatları kimi qəbul etməkdən imtina edir. `hash_columns_gpu` sürətləndiricinin həqiqətən işlədiyini bildirir, boru kəməri geri düşəndə ​​`measure_poseidon_gpu` nümunələri düşürür (və xəbərdarlıq edir) və GPU hashing mümkün olmadıqda Poseidon mikrobench uşağı xəta ilə çıxır. Nəticədə, `gpu_recorded=false` Metal icrası geri düşəndə, növbənin xülasəsi hələ də uğursuz göndərmə pəncərəsini qeyd edir və tablosunun xülasələri dərhal reqressiyanı qeyd edir. Qapağı (`scripts/fastpq/wrap_benchmark.py`) indi `metal_dispatch_queue.poseidon.dispatch_count == 0` zaman uğursuz olur, ona görə də Stage7 paketləri real GPU Poseidon göndərişi olmadan imzalana bilməz sübut.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_bench:912.- Poseidon hashing indi həmin quruluş müqaviləsini əks etdirir. `PoseidonColumnBatch` yastılaşdırılmış faydalı yük buferləri üstəgəl ofset/uzunluq deskriptorları istehsal edir, host hər partiya üçün həmin deskriptorları yenidən əsaslandırır və `COLUMN_STAGING_PIPE_DEPTH` ikiqat buferi işlədir, beləliklə faydalı yük + deskriptor yükləmələri GPU işi ilə üst-üstə düşür və hər iki Metal/CUDA ləpələri birbaşa olaraq bütün deskriptləri bloklayır, beləliklə, bütün deskriptləri bloklayır. sütun həzmlərini yaymadan əvvəl cihazda. `hash_columns_from_coefficients` indi diskret GPU-larda defolt olaraq 64+ sütunu uçuşda saxlayaraq (`FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH` vasitəsilə tənzimlənə bilər) GPU işçisi vasitəsilə həmin partiyaları axın edir. Metal dəzgah həll edilmiş boru kəməri parametrlərini + `metal_dispatch_queue.poseidon_pipeline` altında partiya sayılarını qeyd edir və `kernel_profiles.poseidon.bytes` deskriptor trafikini ehtiva edir, beləliklə Stage7 ələ keçirməsi yeni ABI-ni sübut edir end-to-end.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_prover/cuda/fastpq_51】cuda.
- Stage7-P2 əridilmiş Poseidon hashing indi hər iki GPU arxa ucuna düşür. Axın işçisi bitişik `PoseidonColumnBatch::column_window()` dilimlərini `hash_columns_gpu_fused`-ə ötürür, bu da onları `poseidon_hash_columns_fused`-ə ötürür, beləliklə, hər bir göndəriş kanonik Prometheus ilə `leaf_digests || parent_digests` yazır. `ColumnDigests` hər iki dilimi saxlayır və `merkle_root_with_first_level` ana təbəqəni dərhal istehlak edir, ona görə də CPU heç vaxt dərinlik-1 qovşaqlarını yenidən hesablamır və Stage7 telemetriyası GPU-nun əridilmiş ləpə hər dəfə sıfır “geri qayıtma” hesabatını tutduğunu iddia edə bilər. uğur qazanır.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】 【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` indi Metal cihazın adı, reyestr identifikatoru, `low_power`/`headless` bayraqları, yer (daxili, yuva, xarici), diskret göstərici, `device_profile`, `device_profile` bloku, `device_profile`, `headless` işarəsi yayır. (məsələn, “M3 Max”). Stage7 idarə panelləri bu sahəni host adlarını təhlil etmədən M4/M3 və diskret GPU-lar ilə ələ keçirmək üçün istifadə edir və JSON növbənin/evristik sübutun yanında göndərilir, beləliklə hər buraxılış artefaktı hansı donanma sinfinin qaçışa səbəb olduğunu sübut edir.- FFT host/cihazın üst-üstə düşməsi indi ikiqat buferli hazırlama pəncərəsindən istifadə edir: *n* toplusu `fastpq_fft_post_tiling` daxilində tamamlanarkən, host *n+1* toplusunu ikinci mərhələ buferinə düzəldir və yalnız bufer təkrar emal edilməli olduqda fasilə verir. Backend neçə partiyanın düzəldildiyini üstəgəl GPU-nun tamamlanmasını gözləməyə qarşı düzləşdirməyə sərf olunan vaxtı qeyd edir və `fastpq_metal_bench` yığılmış `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` blokunu üzə çıxarır, beləliklə sərbəst buraxılan artefaktlar səssiz ana dayanacaqlar əvəzinə üst-üstə düşdüyünü sübut edə bilər. JSON hesabatı indi də `column_staging.phases.{fft,lde,poseidon}` altında hər faza üzrə yekunları bölərək, Stage7-nin FFT/LDE/Poseidon quruluşunun hosta bağlı olduğunu və ya GPU-nun tamamlanmasını gözlədiyini sübut etməyə imkan verir. Poseidon permutasiyaları eyni birləşdirilmiş səhnələşdirmə buferlərini təkrar istifadə edir, buna görə də `--operation poseidon_hash_columns` çəkilişləri indi sifarişli alətlər olmadan növbə dərinliyi sübutları ilə yanaşı Poseidon-a məxsus `column_staging` deltalarını yayır. Yeni `column_staging.samples.{fft,lde,poseidon}` massivləri `batch/flatten_ms/wait_ms/wait_ratio` dəstlərini qeyd edərək, `COLUMN_STAGING_PIPE_DEPTH` üst-üstə düşməsinin saxlandığını sübut etməyi (və ya ev sahibinin GPU-nu gözləməyə başladığını aşkar etməyi mənasız edir. tamamlamalar).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fa tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:126】- Poseidon2 sürətləndirilməsi indi yüksək doluluqlu Metal nüvəsi kimi işləyir: hər bir mövzu qrupu dəyirmi sabitləri və MDS cərgələrini mövzu qrupunun yaddaşına köçürür, tam/qismən dövrələri açır və hər zolağa birdən çox vəziyyəti gəzdirir ki, hər göndəriş ən azı 4096 məntiqi ipi işə salsın. `fastpq.metallib`-i yenidən qurmadan profilləşdirmə təcrübələrini təkrar etmək üçün `FASTPQ_METAL_POSEIDON_LANES` (32 və 256 arasında iki güc, cihaz limitinə qədər sıxılmış) və `FASTPQ_METAL_POSEIDON_BATCH` (hər zolağa 1–32 vəziyyət) vasitəsilə işə salma formasını ləğv edin; Rust hostu göndərilməzdən əvvəl həll edilmiş tənzimləməni `PoseidonArgs` vasitəsilə ötürür. Host indi hər açılışda bir dəfə `MTLDevice::{is_low_power,is_headless,location}` snapshotları alır və avtomatik olaraq diskret GPU-ları VRAM səviyyəli işə salmağa istiqamətləndirir (≥48GiB hissələrində `256×24`, 32GiB-də `256×20`, digəri isə aşağı gücdə I03NI05) SoC-lər `256×8`-ə yapışır (128/64 zolaqlı aparat üçün geri dönmələr hər zolaqda 8/6 vəziyyətdən istifadə etməyə davam edir), beləliklə operatorlar env varyasyonlarına toxunmadan >16 ştatlı boru kəmərinin dərinliyini əldə edirlər. `fastpq_metal_bench` `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` altında skalyar zolağı çox dövlət nüvəsi ilə müqayisə edən xüsusi `poseidon_microbench` blokunu ələ keçirmək üçün özünü yenidən həyata keçirir, beləliklə, buraxılış artefaktları konkret sürətlənmədən sitat gətirə bilər. Eyni `poseidon_pipeline` səth telemetriyasını çəkir (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`) beləliklə Stage7 sübutları hər GPU-da üst-üstə düşən pəncərəni sübut edir. iz.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【crates tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:19
  - LDE kafel quruluşu indi FFT evristikasını əks etdirir: ağır izlər `log₂(len) ≥ 18` dəfə paylaşılan yaddaş keçidində yalnız 12 mərhələni yerinə yetirir, log₂20-də 10 mərhələyə enir və log₂22-də səkkiz mərhələyə sıxışdırılır, beləliklə geniş kəpənəklər dirək postuna keçir. Deterministik dərinliyə ehtiyacınız olduqda `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) ilə ləğv edin; ev sahibi yalnız evristik erkən dayandıqda, növbə dərinliyi və nüvə telemetriyası deterministik olaraq qaldıqda, plitədən sonrakı göndərişi işə salır.【crates/fastpq_prover/src/metal.rs:827】
  - Kernel mikro-optimallaşdırılması: paylaşılan yaddaş FFT/LDE plitələri indi hər bir kəpənək üçün `pow_mod*`-ni yenidən qiymətləndirmək əvəzinə, hər zolaqlı fırlanma və koset addımlarını təkrar istifadə edir. Hər bir zolaq `w_seed`, `w_stride` və (tələb olunduqda) hər blokda bir dəfə koset addımını əvvəlcədən hesablayır, sonra ofsetlərdən keçir, `apply_stage_tile`/Prometheus və L18NI00000342-ə endirən skalyar çarpmaları azaldır. Ən son evristika ilə ~1,55 s (hələ 950 ms hədəfdən yuxarıdır, lakin yalnız yığım üçün nəzərdə tutulmuş çimdik üzərində daha da ~ 50 ms təkmilləşdirmə).【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_benchson_run:11【- Kernel paketində indi hər bir giriş nöqtəsini sənədləşdirən xüsusi istinad (`docs/source/fastpq_metal_kernels.md`), `fastpq.metallib`-də tətbiq olunan mövzu qrupu/kafel limitləri və metallibin əl ilə tərtib edilməsi üçün reproduksiya addımları var.【docs/source/fastpq_metal_kernels.md:1s.
  - Qiymətləndirmə hesabatı indi `post_tile_dispatches` obyekti buraxır ki, bu da xüsusi döşənmə sonrası nüvədə neçə FFT/IFFT/LDE partiyasının işlədiyini qeyd edir (növ üzrə göndərilmə sayıları və mərhələ/log₂ sərhədləri). `scripts/fastpq/wrap_benchmark.py` bloku `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`-ə köçürür və manifest qapısı sübutları buraxan GPU-dan imtina edir, beləliklə hər 20k-sətir artefakt çox keçidli nüvənin işlədiyini sübut edir. cihazda.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Alətlər/Metal izi korrelyasiyası üçün hər göndəriş üzrə debug jurnallarını (boru xətti etiketi, mövzu qrupunun eni, işə salma qrupları, keçən vaxt) buraxmaq üçün `FASTPQ_METAL_TRACE=1`-i təyin edin.【crates/fastpq_prover/src/metal.rs:346】
- Göndərmə növbəsi indi cihazlaşdırılıb: `FASTPQ_METAL_MAX_IN_FLIGHT` eyni vaxtda olan Metal əmr buferlərini əhatə edir (avtomatik defolt `system_profiler` vasitəsilə aşkar edilmiş GPU nüvəsinin sayından əldə edilir, ən azı macOS cihazından imtina edən zaman host-paralellik geriləməsi ilə növbənin çıxış mərtəbəsinə bərkidilir). Dəzgah növbə dərinliyində nümunə götürməyə imkan verir, beləliklə, ixrac edilmiş JSON `metal_dispatch_queue` obyektini `limit`, `dispatch_count`, `max_in_flight`, `busy_ms`, `busy_ms` və `busy_ms` ilə `metal_dispatch_queue` obyekti daşıyır, və `busy_ms`, və `busy_ms`, və `busy_ms`, və `busy_ms`, buraxılış601 üçün dəlil601 əlavə edir. yalnız Poseidon tutma (`--operation poseidon_hash_columns`) işlədikdə və həll edilmiş komanda-bufer limitini təsvir edən `metal_heuristics` bloku üstəgəl FFT/LDE toplu sütunları (o cümlədən, qiymətləri nəzərdən keçirməyə məcbur olub-olmadığını yoxlayan qərarlar üzərində yoxlama aparan) bir Poseidon tutma (`--operation poseidon_hash_columns`) işlədiyi zaman yuvalanmış `metal_dispatch_queue.poseidon` bloku telemetriya ilə yanaşı. Poseidon ləpələri həmçinin ləpə nümunələrindən distillə edilmiş xüsusi `poseidon_profiles` blokunu qidalandırır, beləliklə bayt/iplik, doluluq və göndərilmə həndəsəsi artefaktlar arasında izlənilir. Birincil əməliyyat növbənin dərinliyini və ya LDE sıfır doldurma statistikasını toplaya bilmirsə (məsələn, GPU göndərilməsi səssizcə CPU-ya düşdüyündə), qoşqu çatışmayan telemetriyanı toplamaq üçün avtomatik olaraq tək bir zond göndərişini işə salır və indi GPU onlara hesabat verməkdən imtina etdiyi zaman hostun sıfır doldurma vaxtlarını sintez edir, buna görə də sübutlar dərc olunur I68016 blok.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq _prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Metal alətlər silsiləsi olmadan çarpaz tərtib edərkən `FASTPQ_SKIP_GPU_BUILD=1` təyin edin; xəbərdarlıq ötürməni qeyd edir və planlaşdırıcı CPU yolunda davam edir.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- Runtime aşkarlanması Metal dəstəyini təsdiqləmək üçün `system_profiler` istifadə edir; çərçivə, cihaz və ya metallib yoxdursa, qurma skripti `FASTPQ_METAL_LIB`-i təmizləyir və planlayıcı deterministik CPU-da qalır path.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover.3】4:s
  - Operator yoxlama siyahısı (Metal hostlar):
    1. Alətlər zəncirinin mövcud olduğunu və tərtib edilmiş `.metallib`-də `FASTPQ_METAL_LIB` nöqtələrinin olduğunu təsdiqləyin (`echo $FASTPQ_METAL_LIB`, `cargo build --features fastpq-gpu`-dən sonra boş olmamalıdır).【crates/fastpq_prover:18bu
    2. GPU zolaqlarını aktivləşdirərək paritet testlərini həyata keçirin: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. Bu, Metal ləpələri işlədir və aşkarlama uğursuz olarsa, avtomatik olaraq geri düşür.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. İdarə panelləri üçün standart nümunə götürün: tərtib edilmiş Metal kitabxananı tapın
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), vasitəsilə ixrac edin
       `FASTPQ_METAL_LIB` və işə salın
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       Kanonik `fastpq-lane-balanced` dəsti indi hər çəkilişi 32,768 cərgəyə qədər yerləşdirir, beləliklə
       JSON həm tələb olunan 20k sıraları, həm də GPU-nu idarə edən dolğun domeni əks etdirir
       ləpələr. JSON/logu sübut mağazanıza yükləyin; gecə macOS iş axını güzgüləri
      bu işlədir və istinad üçün artefaktları arxivləşdirir. Hesabat qeyd edir
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` hər əməliyyatın `speedup` ilə yanaşı,
     LDE bölməsi `zero_fill.{bytes,ms,queue_delta}` əlavə edir, beləliklə sərbəst buraxılan artefaktlar determinizmi sübut edir,
     host sıfır doldurma yükü və artan GPU növbəsi istifadəsi (limit, göndərmə sayı,
     pik uçuş zamanı, məşğul/üst-üstə düşən vaxt) və yeni `kernel_profiles` bloku hər nüvəni çəkir
     Məşğulluq əmsalları, təxmin edilən bant genişliyi və müddət diapazonları beləliklə idarə panelləri GPU-nu qeyd edə bilsin
       xam nümunələri təkrar emal etmədən reqressiyalar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Metal LDE yolunun 950ms-dən aşağı qalmasını gözləyin (Apple M seriyalı aparatında `<1 s` hədəfi);
4. İdarəetmə panellərinin transfer qadcetini qrafikləşdirə bilməsi üçün real ExecWitness-dən sıra istifadəsi telemetriyasını çəkin
   övladlığa götürmə. Torii-dən şahid gətirin
  (`iroha_cli audit witness --binary --out exec.witness`) və kodunu deşifrə edin
  `iroha_cli audit witness --decode exec.witness` (istəyə görə əlavə edin
  Gözlənilən parametr dəstini təsdiq etmək üçün `--fastpq-parameter fastpq-lane-balanced`; FASTPQ dəstələri
  default olaraq yaymaq; yalnız çıxışı kəsmək lazımdırsa, `--no-fastpq-batches` keçir).
   Hər toplu giriş indi `row_usage` obyekti (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, hər seçici hesabları və `transfer_ratio`). Həmin JSON parçasını arxivləşdirin
   xam transkriptlərin yenidən işlənməsi.【crates/iroha_cli/src/audit.rs:209】 Yeni çəkilişi digərləri ilə müqayisə edin
   `scripts/fastpq/check_row_usage.py` ilə əvvəlki baza xətti, belə ki, köçürmə nisbətləri və ya
   cəmi sıra reqress:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```Tüstü testləri üçün nümunə JSON blobları `scripts/fastpq/examples/`-də yaşayır. Yerli olaraq siz `make check-fastpq-row-usage`-i işlədə bilərsiniz
   (`ci/check_fastpq_row_usage.sh`-i əhatə edir) və CI eyni skripti `.github/workflows/fastpq-row-usage.yml` vasitəsilə yerinə yetirir
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` anlıq görüntülər alır ki, sübut paketi istənilən vaxt uğursuz olur
   transfer sıraları geri sürünür. Maşınla oxuna bilən fərq (CI işi `fastpq_row_usage_summary.json` yükləyir) istəyirsinizsə, `--summary-out <path>`-i keçin.
   ExecWitness əlverişsiz olduqda, `fastpq_row_bench` ilə reqressiya nümunəsini sintez edin
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), eyni `row_usage` yayan
   konfiqurasiya edilə bilən seçici sayları üçün obyekt (məsələn, 65536 sıra stress testi):

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Mərhələ7-3 yayma paketləri də `scripts/fastpq/validate_row_usage_snapshot.py`-dən keçməlidir.
   hər bir `row_usage` girişində selektor saylarını ehtiva etdiyini və
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` köməkçini çağırır
   avtomatik olaraq bu invariantları olmayan paketlər GPU zolaqları məcburi olana qədər uğursuz olur.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       dəzgah manifest qapısı bunu `--max-operation-ms lde=950` vasitəsilə həyata keçirir, ona görə də yeniləyin
       dəlilləriniz bu həddi aşdıqda ələ keçirin.
      Alətlər sübuta ehtiyacınız olduqda, qoşqu ilə `--trace-dir <dir>` keçirin
      `xcrun xctrace record` (defolt "Metal Sistem İzi" şablonu) vasitəsilə özünü yenidən işə salır və
      JSON ilə birlikdə vaxt möhürülənmiş `.trace` faylını saxlayır; siz hələ də yeri ləğv edə bilərsiniz /
      şablon əl ilə `--trace-output <path>` plus isteğe bağlı `--trace-template` /
      `--trace-seconds`. Nəticədə JSON `metal_trace_{template,seconds,output}`-i reklam edir
      artefakt paketləri həmişə tutulan izi müəyyən edir.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Hər çəkilişi ilə sarın
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (imza identifikasiyasını bağlamaq lazımdırsa, `--gpg-key <fingerprint>` əlavə edin) beləliklə paket uğursuz olur
       GPU LDE ortalaması 950 ms hədəfi pozduqda, Poseidon 1 saniyədən çox olduqda və ya
       Poseidon telemetriya blokları yoxdur, `row_usage_snapshot` yerləşdirir
      JSON-un yanında, `benchmarks.poseidon_microbench` altında Poseidon mikrobench xülasəsini göstərir,
      və hələ də runbooks və Grafana idarə paneli üçün metadata daşıyır
    (`dashboards/grafana/fastpq_acceleration.json`). JSON indi `speedup.ratio` / yayır
     Hər əməliyyat üçün `speedup.delta_ms`, buna görə də sübutlar GPU-ya qarşı GPU-nu sübut edə bilər
     CPU xam nümunələri təkrar emal etmədən qazanır və sarğı hər ikisini kopyalayır
     sıfır doldurma statistikası (plus `queue_delta`) `zero_fill_hotspots` (bayt, gecikmə, əldə edilmiş)
     GB/s), `metadata.metal_trace` altında Alətlərin metadatasını qeyd edir, isteğe bağlı
     `metadata.row_usage_snapshot` `--row-usage <decoded witness>` təchiz edildikdə bloklayır və
     nüvə başına sayğaclar `benchmarks.kernel_summary`-ə daxil olur, beləliklə darboğazları doldurur, Metal növbə
     istifadə, nüvə doluluğu və bant genişliyi reqressiyaları bir baxışda görünə bilər.
     xammaldan bəhs edir hesabat.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark .py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Sətir-istifadə snapshot indi bükülmüş artefakt ilə səyahət etdiyi üçün, sadəcə olaraq satış biletləri
     ikinci JSON parçasını əlavə etmək əvəzinə paketə istinad edin və CI daxil edilmiş faylı fərqləndirə bilər
    Mərhələ 7 təqdimatlarını təsdiq edərkən birbaşa sayılır. Mikrobench məlumatlarını öz başına arxivləşdirmək üçün,
    `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json` işə salın
    və nəticədə faylı `benchmarks/poseidon/` altında saxlayın. İlə ümumiləşdirilmiş manifest təzə saxlayın
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    beləliklə, tablosuna/CI hər bir faylı əl ilə gəzdirmədən tam tarixi fərqləndirə bilər.4. `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus son nöqtəsi) bükməklə və ya `telemetry::fastpq.execution_mode` jurnallarını axtarmaqla telemetriyanı təsdiqləyin; gözlənilməz `resolved="cpu"` girişləri hostun GPU niyyətinə baxmayaraq geri çəkildiyini göstərir.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Baxım zamanı CPU icrasını məcbur etmək üçün `FASTPQ_GPU=cpu` (və ya konfiqurasiya düyməsini) istifadə edin və ehtiyat qeydlərinin hələ də göründüyünü təsdiqləyin; bu, SRE runbook-larını deterministik yola uyğun saxlayır.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetriya və ehtiyat:
  - İcra rejimi qeydləri (`telemetry::fastpq.execution_mode`) və sayğaclar (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) tələb olunan və həll edilmiş rejimi ifşa edir ki, səssiz geri dönüşlər burada görünsün. tablosuna.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ Acceleration Overview` Grafana lövhəsi (`dashboards/grafana/fastpq_acceleration.json`) Metal qəbuletmə dərəcəsini vizuallaşdırır və standart artefaktlarla əlaqələndirilir, eyni zamanda qoşa edilmiş xəbərdarlıq qaydaları (`dashboards/alerts/fastpq_acceleration_rules.yml`) darvazaların endirilməsini davam etdirir.
  - `FASTPQ_GPU={auto,cpu,gpu}` ləğvetmələri dəstəklənir; naməlum dəyərlər xəbərdarlıqları artırır, lakin hələ də audit üçün telemetriyaya yayılır.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - GPU paritet testləri (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) CUDA və Metal üçün keçməlidir; Metallib olmadıqda və ya aşkarlama uğursuz olduqda CI zərif şəkildə ötür.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Metal hazırlığının sübutu (hər təqdimatla aşağıdakı artefaktları arxivləşdirin ki, yol xəritəsi auditi determinizmi, telemetriya əhatəsini və ehtiyat davranışı sübut edə bilsin):| Addım | Məqsəd | Əmr / Sübut |
    | ---- | ---- | ------------------ |
    | Metallib qurmaq | `xcrun metal`/`xcrun metallib`-in mövcud olduğundan əmin olun və bu öhdəlik üçün deterministik `.metallib` buraxın | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; ixrac `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | env var | doğrulayın Quraşdırma skripti | tərəfindən qeydə alınmış env var-ı yoxlayaraq Metalların aktiv qaldığını təsdiqləyin `echo $FASTPQ_METAL_LIB` (mütləq yolu qaytarmalıdır; boş arxa tərəfin deaktiv edildiyini bildirir).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU paritet dəsti | Göndərmədən əvvəl ləpələrin yerinə yetirildiyini (və ya deterministik endirmə qeydlərini yaydığını) sübut edin | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` və ya `backend="metal"` və ya ehtiyat xəbərdarlığını göstərən nəticə jurnal parçasını saxlayın.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:1
    | Benchmark nümunəsi | `speedup.*` və FFT tənzimləməsini qeyd edən JSON/log cütünü çəkin ki, idarə panelləri sürətləndirici sübutları qəbul edə bilsin | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; Grafana lövhəsi Metal qaçışını götürməsi üçün JSON, vaxt damğası vurulmuş `.trace` və stdout-u arxivləşdirin (hesabat tələb olunan 20k cərgəni üstəgəl doldurulmuş 32,768 sıra domeni qeyd edir ki, rəyçilər L1045000 təsdiq edə bilsinlər. hədəf).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Hesabatı sarın və imzalayın | GPU LDE ortalaması 950ms-i pozarsa, Poseidon 1s-i keçərsə və ya Poseidon telemetriya blokları çatışmırsa və imzalanmış artefakt paketi istehsal edərsə, buraxılışı uğursuz edin | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; həm bükülmüş JSON, həm də yaradılan `.json.asc` imzasını göndərin ki, auditorlar iş yükünü yenidən işə salmadan ikinci saniyəlik ölçüləri yoxlaya bilsinlər.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:73 |
    | İmzalanmış bench manifest | Metal/CUDA paketləri üzrə `<1 s` LDE sübutlarını tətbiq edin və buraxılış təsdiqi üçün imzalanmış digestləri ələ keçirin | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; manifest + imzasını buraxılış biletinə əlavə edin ki, aşağı axın avtomatlaşdırması sub-ikinci sübut göstəricilərini doğrulaya bilsin.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA paketi | Manifestlərin hər iki GPU sinifini əhatə etməsi üçün SM80 CUDA ələ keçirməni Metal sübut ilə kilid addımında saxlayın. | Xeon+RTX hostunda `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; bükülmüş yolu `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`-ə əlavə edin, `.json`/`.asc` cütünü Metal paketinin yanında saxlayın və auditorlara arayış lazım olduqda əsas `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`-ə istinad edin layout.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Telemetriya yoxlaması | Prometheus/OTEL səthlərinin `device_class="<matrix>", backend="metal"` əksini təsdiqləyin (və ya endirməni qeyd edin) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` və başlanğıcda buraxılan `telemetry::fastpq.execution_mode` jurnalını kopyalayın.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| Məcburi ehtiyat qazma | SRE oyun kitabları üçün deterministik CPU yolunu sənədləşdirin | `FASTPQ_GPU=cpu` və ya `zk.fastpq.execution_mode = "cpu"` ilə qısa bir iş yükünü işə salın və operatorların geri qaytarma prosedurunu təkrarlaya bilməsi üçün endirmə jurnalını çəkin.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_confimeter.7s/s/s/iroha_config.
    | İz tutma (isteğe bağlı) | Profil hazırlayarkən, göndərmə izlərini çəkin ki, nüvə zolağı/kafel ləğvləri daha sonra nəzərdən keçirilsin | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ilə bir paritet testini təkrar edin və istehsal edilmiş iz jurnalını buraxılış artefaktlarınıza əlavə edin.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Buraxılış bileti ilə sübutları arxivləşdirin və eyni yoxlama siyahısını `docs/source/fastpq_migration_guide.md`-də əks etdirin ki, səhnələşdirmə/məhsul buraxılışları eyni kitabçaya əməl etsin.【docs/source/fastpq_migration_guide.md:1】

### Yoxlama siyahısının icrasını buraxın

Hər FASTPQ buraxılış biletinə aşağıdakı qapıları əlavə edin. Buraxılışlar bütün elementlər bitənə qədər bloklanır
imzalanmış artefaktlar kimi tamamlanır və əlavə olunur.

1. **Sub-saniyədə sübut göstəriciləri** — Kanonik Metal benchmark ələ keçirmə
   (`fastpq_metal_bench_*.json`) 20000 sıra iş yükünün (32768 doldurulmuş cərgə) tamamlandığını sübut etməlidir
   <1s. Konkret olaraq, `benchmarks.operations` girişi, burada `operation = "lde"` və uyğunluğu
   `report.operations` nümunəsi `gpu_mean_ms ≤ 950`-i göstərməlidir. Tavanı aşan qaçışlar tələb olunur
   təhqiqat və yoxlama siyahısı imzalanmazdan əvvəl yenidən tutma.
2. **İmzalanmış benchmark manifest** — Təzə Metal + CUDA paketlərini qeyd etdikdən sonra qaçın
   Emissiya etmək üçün `cargo xtask fastpq-bench-manifest … --signing-key <path>`
   `artifacts/fastpq_bench_manifest.json` və ayrılmış imza
   (`artifacts/fastpq_bench_manifest.sig`). Hər iki faylı və açıq açar barmaq izini əlavə edin
   bilet buraxın ki, rəyçilər həzmi və imzanı müstəqil şəkildə yoxlaya bilsinlər.【xtask/src/fastpq.rs:1】
3. **Sübut qoşmaları** — xam etalon JSON, stdout log (və ya Alətlər izi,
   tutuldu) və buraxılış bileti ilə manifest/imza cütü. Yoxlama siyahısı yalnızdır
   Bilet həmin artefaktlara bağlandıqda və çağırış üzrə rəyçi təsdiq etdikdə yaşıl sayılır
   `fastpq_bench_manifest.json`-də qeydə alınan həzm yüklənmiş fayllara uyğun gəlir.【artifacts/fastpq_benchmarks/README.md:1】

## Mərhələ 6 — Sərtləşdirmə və Sənədləşdirmə
- Yer tutucunun arxa hissəsi istifadədən çıxarıldı; istehsal boru kəməri heç bir funksiya keçidi olmadan defolt olaraq göndərilir.
- Təkrarlana bilən quruluşlar (pin alət zəncirləri, konteyner şəkilləri).
- İz, SMT, axtarış strukturları üçün fuzzers.
- Prover səviyyəli tüstü testləri tam IVM buraxılışlarından əvvəl Stage6 qurğularını sabit saxlamaq üçün idarəetmə bülletenləri və pul köçürmələrini əhatə edir.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Xəbərdarlıq hədləri, remediasiya prosedurları, potensialın planlaşdırılması qaydaları ilə Runbooks.
- CI-də çarpaz memarlıq sübut təkrarı (x86_64, ARM64).

### Dəzgah manifest və buraxma qapısı

Buraxılış sübutları indi həm Metal, həm də əhatə edən deterministik manifest daxildir
CUDA benchmark paketləri. Qaçış:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```Komanda bükülmüş paketləri təsdiqləyir, gecikmə/sürətləndirmə hədlərini tətbiq edir,
BLAKE3 + SHA-256 həzmləri yayır və (isteğe bağlı olaraq) manifestə imza atır
Ed25519 açarı ki, buraxılış alətləri mənşəyi yoxlaya bilsin. Bax
Tətbiq üçün `xtask/src/fastpq.rs`/`xtask/src/main.rs` və
Əməliyyat rəhbərliyi üçün `artifacts/fastpq_benchmarks/README.md`.

> **Qeyd:** `benchmarks.poseidon_microbench` buraxan metal paketlər indi səbəb olur
> uğursuz nəsil. `scripts/fastpq/wrap_benchmark.py`-i yenidən işə salın
> (və müstəqil ehtiyacınız varsa `scripts/fastpq/export_poseidon_microbench.py`
> xülasə) nə vaxt Poseidon sübutu yoxdursa, onu buraxın
> həmişə skalyar-defolt müqayisəsini çəkin.【xtask/src/fastpq.rs:409】

`--matrix` bayrağı (defolt olaraq `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
mövcud olduqda) tərəfindən tutulan cihaz arası medianı yükləyir
`scripts/fastpq/capture_matrix.sh`. Manifest 20000 cərgəlik mərtəbəni kodlayır və
hər cihaz sinfi üçün əməliyyat başına gecikmə/sürətləndirmə limitləri, buna görə sifarişlə
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` ləğvetmələri yoxdur
xüsusi reqressiyanı aradan qaldırmadığınız müddətcə daha uzun müddət tələb olunur.

Bükülmüş benchmark yollarını əlavə etməklə matrisi yeniləyin
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` siyahıları və işləyir
`scripts/fastpq/capture_matrix.sh`. Skript hər bir cihaz üçün median şəkillərini çəkir,
konsolidə edilmiş `matrix_manifest.json` yayır və nisbi yolu çap edir
`cargo xtask fastpq-bench-manifest` istehlak edəcək. AppleM4, Xeon+RTX və
Neoverse+MI300 çəkiliş siyahıları (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) üstəgəl onların bükülmüş
benchmark paketləri
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) indi yoxlanılır
beləliklə, hər buraxılış manifestdən əvvəl eyni cihaz arası medianı tətbiq edir
edir imzalanmışdır.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Tənqid Xülasəsi və Açıq Fəaliyyətlər

## Mərhələ 7 — Donanmanın Qəbul Edilməsi və Yayılma Dəlilləri

Stage7 proveri "sənədləşdirilmiş və müqayisəli" (Mərhələ6) keçir
"istehsal donanmaları üçün defolt hazırdır". Diqqət telemetriya qəbuluna yönəldilir,
cihazlar arası tutma pariteti və operator sübut paketləri beləliklə GPU sürətləndirilməsi
deterministik qaydada mandatlaşdırıla bilər.- **Mərhələ7-1 — Donanma telemetriyasının qəbulu və SLO-lar.** İstehsal panelləri
  (`dashboards/grafana/fastpq_acceleration.json`) yaşamaq üçün simli olmalıdır
  Prometheus/OTel növbə dərinliyi stendləri üçün Alertmanager əhatə dairəsi ilə qidalanır,
  sıfır doldurma reqressiyaları və səssiz CPU geri dönüşləri. Xəbərdarlıq paketi altında qalır
  `dashboards/alerts/fastpq_acceleration_rules.yml` və eyni sübutları təqdim edir
  6-cı Mərhələdə paket tələb olunur.【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  İdarə paneli indi `device_class`, `chip_family` üçün şablon dəyişənlərini ifşa edir,
  və `gpu_kind`, operatorlara metal qəbulunu dəqiq matrislə dəyişməyə imkan verir
  etiket (məsələn, `apple-m4-max`), Apple çip ailəsi və ya diskret və ya diskret ilə.
  sorğuları redaktə etmədən inteqrasiya olunmuş GPU sinifləri.
  `irohad --features fastpq-gpu` ilə qurulmuş macOS qovşaqları indi yayır
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (məşğul/üst-üstə düşmə nisbətləri) və
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (limit, max_in_flight, dispatch_count, window_seconds) beləliklə, tablosuna və
  Alertmanager qaydaları birbaşa Metal semafor vəzifə dövrü/boş yerdən oxuya bilər
  Prometheus benchmark paketini gözləmədən. Hostlar indi ixrac edir
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` və
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` istənilən vaxt
  LDE köməkçisi GPU qiymətləndirmə tamponlarını sıfırlar və Alertmanager əldə etdi
  `FastpqQueueHeadroomLow` (10 m üçün baş boşluğu < 1) və
  `FastpqZeroFillRegression` (15 m-dən çox 0,40ms) qaydalar belədir ki, baş boşluğunu sıralayın və
  Sıfır doldurma reqressləri səhifə operatorlarını dərhal gözləmək yerinə
  növbəti bükülmüş benchmark. Yeni `FastpqCpuFallbackBurst` səhifə səviyyəli xəbərdarlığı izləyir
  GPU, iş yükünün 5% -dən çoxu üçün CPU arxa ucuna enməsini tələb edir,
  operatorları sübut əldə etməyə məcbur etmək və müvəqqəti GPU uğursuzluqlarının kök səbəbini
  təkrar cəhd etməzdən əvvəl rollout.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/al erts/fastpq_acceleration_rules.yml:1】【İdarə panelləri/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  SLO dəsti indi də vasitəsilə ≥50% Metal iş dövrü hədəfini tətbiq edir
  Orta olan `FastpqQueueDutyCycleDrop` qaydası
  `fastpq_metal_queue_ratio{metric="busy"}` yuvarlanan 15 dəqiqəlik pəncərə və
  GPU işi hələ də planlaşdırıldıqda xəbərdarlıq edir, lakin növbə onu saxlaya bilmir
  tələb olunan yaşayış. Bu, canlı telemetriya müqaviləsini uyğun olaraq saxlayır
  GPU zolaqlarından əvvəl sınaq sübutları tələb olunur.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Mərhələ7-2 — Cihazlar arası çəkiliş matrisi.** Yeni
  `scripts/fastpq/capture_matrix.sh` qurur
  Hər cihazdan `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
  `artifacts/fastpq_benchmarks/matrix/devices/` altında siyahıları ələ keçirin. AppleM4,
  Xeon+RTX və Neoverse+MI300 medianları indi onların yanında repoda yaşayırlar
  bükülmüş paketlər, buna görə də `cargo xtask fastpq-bench-manifest` manifest yükləyir
  avtomatik olaraq, 20000 cərgəlik mərtəbəni tətbiq edir və hər cihaza tətbiq edir
  buraxılış paketindən əvvəl sifarişli CLI bayraqları olmadan gecikmə/sürətləndirmə limitləritəsdiq edilmişdir.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】s】rc.
Ümumi qeyri-sabitlik səbəbləri indi matrislə yanaşı göndərilir: keçid
emissiya etmək üçün `--reason-summary-out` - `scripts/fastpq/geometry_matrix.py`
Host etiketi və mənbə ilə əsaslanan uğursuzluq/xəbərdarlığın JSON histoqramı
xülasə, beləliklə, Stage7-2 rəyçiləri CPU geri dönüşlərini və ya çatışmayan telemetriyasını görə bilər
tam Markdown cədvəlini skan etmədən bir baxış. İndi də eyni köməkçi
`--host-label chip_family:Chip` qəbul edir (birdən çox düymə üçün təkrarlayın).
Markdown/JSON çıxışlarına basdırmaq əvəzinə seçilmiş host etiketi sütunları daxildir
xam xülasədə həmin metadata, OS qurmalarını filtrləməyi mənasız edir və ya
Stage7-2 sübut paketini tərtib edərkən metal sürücü versiyaları.【scripts/fastpq/geometry_matrix.py:1】
Həndəsə, həmçinin ISO8601 `started_at` / `completed_at` sahələrinə möhür vurur.
xülasə, CSV və Markdown çıxışları beləliklə, tutma paketləri pəncərəni sübut edə bilər
Mərhələ7-2 matrisləri çoxsaylı laboratoriya işlərini birləşdirdikdə hər bir host.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` indi həndəsə matrisini ilə birlikdə tikir
`row_usage/*.json` anlıq görüntüləri tək Stage7 paketinə (`stage7_bundle.json`)
+ `stage7_geometry.md`), vasitəsilə köçürmə nisbətlərini təsdiqləyir
`validate_row_usage_snapshot.py` və davamlı host/env/səbəb/mənbə xülasələri
beləliklə, satış biletləri hoqqabazlıq əvəzinə bir deterministik artefakt əlavə edə bilər
hər host cədvəlləri.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Mərhələ7-3 — Operatorun mənimsənilməsi sübutu və geri çəkilmə təlimləri.** Yeni
  `docs/source/fastpq_rollout_playbook.md` artefakt paketini təsvir edir
  (`fastpq_bench_manifest.json`, bükülmüş Metal/CUDA tutur, Grafana ixracı,
  Alertmanager snapshot, geri qaytarma qeydləri) hər bir buraxılış biletini müşayiət etməlidir
  üstəgəl mərhələli (pilot → rampa → default) vaxt qrafiki və məcburi bərpa məşqləri.
  `ci/check_fastpq_rollout.sh` bu paketləri təsdiqləyir, beləliklə CI Stage7-i tətbiq edir
  buraxılışlardan əvvəl qapısı irəliyə doğru hərəkət edin. Buraxılış boru kəməri indi eyni şeyi çəkə bilər
  vasitəsilə `artifacts/releases/<version>/fastpq_rollouts/…`-ə paketlər
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, təmin edilməsi
  imzalanmış manifestlər və təqdim edilən sübutlar birlikdə qalır. İstinad paketi yaşayır
  saxlamaq üçün `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` altında
  GitHub iş axını (`.github/workflows/fastpq-rollout.yml`) real olsa da yaşıl
  təqdim edilən təqdimatlar nəzərdən keçirilir.

### Mərhələ 7 FFT növbəsi fan-out`crates/fastpq_prover/src/metal.rs` indi `QueuePolicy`-i yaradır
ev sahibi hər dəfə hesabat verdikdə avtomatik olaraq birdən çox Metal əmr növbəsini yaradır
diskret GPU. İnteqrasiya edilmiş GPU-lar tək növbəli yolu saxlayır
(`MIN_QUEUE_FANOUT = 1`), diskret qurğular defolt olaraq iki növbəyə və yalnız
iş yükü ən azı 16 sütunu əhatə etdikdə havanı söndürün. Hər iki evristik sazlana bilər
yeni `FASTPQ_METAL_QUEUE_FANOUT` və `FASTPQ_METAL_COLUMN_THRESHOLD` vasitəsilə
ətraf mühit dəyişənləri və planlaşdırıcı FFT/LDE qrupları arasında dövrə vurur
eyni növbəyə qoşalaşmış plitədən sonrakı göndərişi verməzdən əvvəl aktiv növbələr
sifariş zəmanətlərini qorumaq üçün.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Node operatorları artıq həmin env varyasyonlarını əl ilə ixrac etməli deyil: the
`iroha_config` profili `fastpq.metal_queue_fanout` və
`fastpq.metal_queue_column_threshold` və `irohad` onları tətbiq edir
Metal backend belə işə salınmazdan əvvəl `fastpq_prover::set_metal_queue_policy`
donanma profilləri sifarişli işə salma paketləri olmadan təkrarlana bilir.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Tərs FFT qrupları indi yalnız iş yükü olduqda tək növbəyə yapışır
ventilyasiya həddinə çatır (məsələn, 16 sütunlu zolaqlı balanslaşdırılmış tutma), hansı
böyük sütunlu FFT/LDE/Poseidondan çıxarkən WP2-D üçün ≥1,0× pariteti bərpa edir
çox növbəli yolda göndərir.【crates/fastpq_prover/src/metal.rs:2018】

Köməkçi testlər CI-nin bacara bilməsi üçün növbə siyasəti qısqaclarını və təhlilçinin doğrulamasını həyata keçirir
hər inşaatçıda GPU avadanlığı tələb etmədən Stage7 evristikasını sübut edin,
və GPU-ya xas testlər təkrar oynatma əhatəsini saxlamaq üçün fan-out funksiyalarını ləğv etməyə məcbur edir
yeni standart parametrlərlə sinxronizasiya edin.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Mərhələ7-1 Cihaz Etiketləri və Xəbərdarlıq Müqaviləsi

`scripts/fastpq/wrap_benchmark.py` indi macOS çəkilişində `system_profiler`-i yoxlayır
Donanma telemetriyası üçün hər bir bükülmüş etalonda aparat etiketlərini saxlayır və qeyd edir
və tutma matrisi sifarişli cədvəllər olmadan cihaz tərəfindən dönə bilər. A
20000 sıralı Metal tutma indi aşağıdakı kimi girişləri daşıyır:

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

Bu etiketlər `benchmarks.zero_fill_hotspots` və ilə birlikdə qəbul edilir
`benchmarks.metal_dispatch_queue` beləliklə Grafana snapshot, tutma matrisi
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) və Alertmanager
sübutların hamısı ölçüləri yaradan aparat sinfi ilə razılaşır. The
`--label` bayrağı laboratoriya sahibi olmadıqda hələ də əl ilə ləğv etməyə icazə verir
`system_profiler`, lakin avtomatik yoxlanılan identifikatorlar indi AppleM1–M4 və
diskret PCIe GPU-ları qutudan çıxarın.【scripts/fastpq/wrap_benchmark.py:1】

Linux çəkilişləri eyni müalicəni alır: `wrap_benchmark.py` indi yoxlayır
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` və `lspci` beləliklə CUDA və OpenCL işləyir
`cpu_model`, `gpu_model` və kanonik `device_class` (`xeon-rtx-sm80`) əldə edin
Stage7 CUDA hostu üçün, MI300A laboratoriyası üçün `neoverse-mi300`). Operatorlar bilər
hələ də avtomatik aşkarlanan dəyərləri ləğv edir, lakin Stage7 sübut paketləri artıq deyil
Xeon/Neoverse çəkilişlərini düzgün cihazla işarələmək üçün əl ilə redaktələr tələb edin
metadata.İş vaxtında hər bir host `fastpq.device_class`, `fastpq.chip_family` və
`fastpq.gpu_kind` (və ya müvafiq `FASTPQ_*` mühit dəyişənləri)
Prometheus ixrac etmək üçün çəkiliş paketində görünən eyni matris etiketləri
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` və
FASTPQ Sürətləndirmə tablosu üç oxdan hər hansı biri ilə filtrasiya edə bilər. The
Alertmanager qaydaları eyni etiket dəsti üzərində cəmləşərək operatorlara diaqram qoymağa imkan verir
tək əvəzinə hər bir avadanlıq profilinə qəbul, endirmələr və ehtiyatlar
donanma geniş nisbəti.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

Telemetriya SLO/xəbərdarlıq müqaviləsi indi çəkilmiş ölçüləri Stage7 ilə əlaqələndirir
qapılar. Aşağıdakı cədvəl siqnalları və icra nöqtələrini ümumiləşdirir:

| Siqnal | Mənbə | Hədəf / Tətik | İcra |
| ------ | ------ | ---------------- | ----------- |
| GPU qəbulu nisbəti | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | Hər (cihaz_sinif, çip_family, gpu_kind) qətnamələrin ≥95%-i `resolved="gpu", backend="metal"`-də yerləşməlidir; hər hansı üçlük 15m | üzərində 50%-dən aşağı düşdüyü zaman səhifə `FastpqMetalDowngrade` xəbərdarlıq (səhifə)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:1】 |
| Arxa uç boşluğu | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Hər üçlük üçün 0-da qalmalıdır; hər hansı davamlı (>10m) partlayışdan sonra xəbərdar edin | `FastpqBackendNoneBurst` xəbərdarlıq (xəbərdarlıq)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU geri qaytarılma nisbəti | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | GPU tərəfindən tələb olunan sübutların ≤5%-i istənilən üçlük üçün CPU arxa ucuna düşə bilər; ≥10m | üçün üçlük 5%-i keçdikdə səhifə `FastpqCpuFallbackBurst` xəbərdarlıq (səhifə)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:32】 |
| Metal növbə vəzifə dövrü | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | GPU işləri növbəyə qoyulduqda 15 m orta yuvarlanma ≥50% qalmalıdır; GPU sorğuları davam edərkən istifadə hədəfdən aşağı düşdükdə xəbərdarlıq edin | `FastpqQueueDutyCycleDrop` xəbərdarlıq (xəbərdarlıq)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:98】 |
| Növbənin dərinliyi və sıfır doldurma büdcəsi | Bükülmüş benchmark `metal_dispatch_queue` və `zero_fill_hotspots` blokları | `max_in_flight` `limit` altında ən azı bir yuva qalmalıdır və LDE sıfır doldurma ortası kanonik 20000 sıra izi üçün ≤0,4ms (≈80GB/s) qalmalıdır; hər hansı bir reqressiya yayım paketini bloklayır | `scripts/fastpq/wrap_benchmark.py` çıxışı vasitəsilə nəzərdən keçirilib və Stage7 sübut paketinə əlavə edilib (`docs/source/fastpq_rollout_playbook.md`). |
| İş vaxtı növbəsi üçün boşluq | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | Hər üçlük üçün `limit - max_in_flight ≥ 1`; baş boşluğu olmadan 10m sonra xəbərdarlıq | `FastpqQueueHeadroomLow` xəbərdarlıq (xəbərdarlıq)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:41】 |
| İş vaxtı sıfır doldurma gecikməsi | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | Ən son sıfır doldurma nümunəsi ≤0.40ms qalmalıdır (Mərhələ7 limiti) | `FastpqZeroFillRegression` xəbərdarlıq (səhifə)【İdarə panelləri/alerts/fastpq_acceleration_rules.yml:58】 |Sarmalayıcı birbaşa sıfır doldurma cərgəsini tətbiq edir. keçir
`--require-zero-fill-max-ms 0.40` - `scripts/fastpq/wrap_benchmark.py` və bu
JSON dəzgahında sıfır doldurma telemetriyası olmadıqda və ya ən isti olduqda uğursuz olacaq
sıfır doldurma nümunəsi Stage7 büdcəsini aşır, bu da paketlərin yayılmasının qarşısını alır
məcburi sübut olmadan göndərmə.【scripts/fastpq/wrap_benchmark.py:1008】

#### Mərhələ 7-1 xəbərdarlığın idarə edilməsinə nəzarət siyahısı

Yuxarıda sadalanan hər bir xəbərdarlıq xüsusi bir çağırış məşqini təmin edir ki, operatorlar məlumatı toplasınlar
buraxılış paketinin tələb etdiyi eyni artefaktlar:

1. **`FastpqQueueHeadroomLow` (xəbərdarlıq).** Ani Prometheus sorğusunu işə salın
   `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` üçün və
   `fastpq-acceleration`-dən Grafana “Queue headroom” panelini çəkin
   lövhə. Sorğunun nəticəsini qeyd edin
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   xəbərdarlıq identifikatoru ilə birlikdə buraxılış paketi xəbərdarlığın olduğunu sübut edir
   növbə ac qalmadan əvvəl qəbul edildi.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (səhifə).** Yoxlayın
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` və əgər metrik belədirsə
   səs-küylü, ən son JSON dəzgahında `scripts/fastpq/wrap_benchmark.py`-i yenidən işə salın
   `zero_fill_hotspots` blokunu yeniləmək üçün. PromQL çıxışını əlavə edin,
   ekran görüntüləri və yenilənmiş dəzgah faylı yayım kataloquna; bu yaradır
   `ci/check_fastpq_rollout.sh`-in buraxılış zamanı gözlədiyi eyni sübut
   validation.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (səhifə).** Təsdiq edin
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` 5%-i keçir
   mərtəbə, sonra müvafiq endirmə mesajları üçün `irohad` jurnallarını seçin
   (`telemetry::fastpq.execution_mode resolved="cpu"`). PromQL zibilini saxlayın
   üstəgəl `metrics_cpu_fallback.prom`/`rollback_drill.log`-də log çıxarışları
   paket həm təsir, həm də operatorun etirafını nümayiş etdirir.
4. **Sübutun qablaşdırılması.** Hər hansı xəbərdarlıq silindikdən sonra Mərhələ7-3 addımlarını yenidən icra edin.
   rollout playbook (Grafana ixracı, xəbərdarlıq snapshot, geri qaytarma matkap) və
   paketi yenidən qoşmazdan əvvəl `ci/check_fastpq_rollout.sh` vasitəsilə yenidən təsdiq edin
   buraxılış biletinə.【docs/source/fastpq_rollout_playbook.md:114】

Avtomatlaşdırmaya üstünlük verən operatorlar işləyə bilər
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
növbə boşluğu, sıfır doldurma və CPU ehtiyatı üçün Prometheus API sorğusu
yuxarıda sadalanan göstəricilər; köməkçi tutulan JSON-u yazır (prefiks ilə
orijinal promQL) `metrics_headroom.prom`, `metrics_zero_fill.prom` və
`metrics_cpu_fallback.prom` seçilmiş rollout kataloqu altında o fayllar
əl ilə qıvrılma çağırışları olmadan paketə əlavə edilə bilər.`ci/check_fastpq_rollout.sh` indi növbə boşluğunu və sıfır doldurmağı tətbiq edir
birbaşa büdcə. O, istinad edilən hər bir `metal` dəzgahını təhlil edir
`fastpq_bench_manifest.json`, yoxlayır
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` və
`benchmarks.zero_fill_hotspots[]` və boşluq düşdüyü zaman paket uğursuz olur
bir yuvanın altında və ya hər hansı LDE qaynar nöqtəsi `mean_ms > 0.40` məlumat verdikdə. Bu saxlayır
CI-də Stage7 telemetriya qoruyucusu, uyğun olaraq əl ilə nəzərdən keçirildi
Grafana snapshot və sübut buraxın.【ci/check_fastpq_rollout.sh#L1】
Eyni doğrulamanın bir hissəsi olaraq skriptin indi hər bükülmüş olduğunu israr edir
benchmark avtomatik aşkarlanan aparat etiketlərini daşıyır (`metadata.labels.device_class`
və `metadata.labels.gpu_kind`). Bu etiketləri olmayan paketlər dərhal uğursuz olur,
artefaktların, Stage7-2 matrix manifestlərinin və iş vaxtının buraxılmasına zəmanət verir
tablosunun hamısı eyni cihaz sinif adlarına istinad edir.

Grafana "Son Benchmark" paneli və əlaqədar təqdimat paketi indi sitat gətirir
`device_class`, sıfır doldurma büdcəsi və növbə dərinliyi snapshot, belə ki, çağırış üzrə mühəndislər
istehsal telemetriyasını işarə zamanı istifadə olunan dəqiq tutma sinfi ilə əlaqələndirə bilər
off. Gələcək matris qeydləri eyni etiketləri miras alır, yəni Stage7-2 cihazı
siyahıları və Prometheus idarə panelləri AppleM4 üçün vahid adlandırma sxemini paylaşır,
M3 Max və qarşıdan gələn MI300/RTX çəkilişləri.

### Mərhələ 7-1 Fleet telemetriya runbook

Defolt olaraq GPU zolaqlarını aktivləşdirməzdən əvvəl bu yoxlama siyahısına əməl edin, beləliklə donanma telemetriyası
və Alertmanager qaydaları buraxılışa hazırlıq zamanı əldə edilən eyni sübutları əks etdirir:

1. **Çəkmə və işləmə vaxtı hostlarını etiketləyin.** `python3 scripts/fastpq/wrap_benchmark.py`
   artıq `metadata.labels.device_class`, `chip_family` və `gpu_kind` yayır
   hər bükülmüş JSON üçün. Bu etiketləri sinxronlaşdırın
   `fastpq.{device_class,chip_family,gpu_kind}` (və ya
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) `iroha_config` daxilində
   beləliklə, iş vaxtı ölçüləri dərc olunur
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   və eyni identifikatorları olan `fastpq_metal_queue_*` ölçü cihazları
   `artifacts/fastpq_benchmarks/matrix/devices/*.txt`-də. Yenisini səhnələşdirərkən
   sinif vasitəsilə matris manifestini bərpa edin
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   beləliklə, CI və idarə panelləri əlavə etiketi başa düşür.
2. **Növbə ölçülərini və qəbul göstəricilərini yoxlayın.** `irohad --features fastpq-gpu`-i işə salın
   Metal hostlarda və canlı növbəni təsdiqləmək üçün telemetriyanın son nöqtəsini kazıyın
   Ölçülər ixrac olunur:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```Birinci əmr semafor seçicinin `busy` yaydığını sübut edir,
   `overlap`, `limit` və `max_in_flight` seriyası və ikincisi olub olmadığını göstərir
   hər bir cihaz sinfi `backend="metal"` ilə həll olunur və ya geri qayıdır
   `backend="cpu"`. Sızma hədəfini əvvəl Prometheus/OTel vasitəsilə keçirin
   Grafana donanma görünüşünü dərhal tərtib edə bilməsi üçün tablosunu idxal edir.
3. **İdxal panelini + xəbərdarlıq paketini quraşdırın.** İdxal edin
   `dashboards/grafana/fastpq_acceleration.json` Grafana daxil edin (saxlayın
   quraşdırılmış Cihaz Sinfi, Çip Ailəsi və GPU Tipi şablon dəyişənləri) və yükləyin
   `dashboards/alerts/fastpq_acceleration_rules.yml` birlikdə Alertmanager daxil edin
   vahid sınaq qurğusu ilə. Qaydalar paketi `promtool` kəmərini göndərir; qaçmaq
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   `FastpqMetalDowngrade` sübut etmək üçün qaydalar dəyişdikdə və
   `FastpqBackendNoneBurst` hələ də sənədləşdirilmiş hədlərdə atəş açır.
4. **Gate sübut paketi ilə buraxılır.** Saxlayın
   `docs/source/fastpq_rollout_playbook.md` yayma yaratarkən əlverişlidir
   təqdim etmə, beləliklə, hər paket bükülmüş benchmarkları daşıyır, Grafana ixracı,
   xəbərdarlıq paketi, növbə telemetriyası sübutu və geri çəkilmə qeydləri. CI artıq tətbiq edir
   müqavilə: `make check-fastpq-rollout` (və ya çağırış
   `ci/check_fastpq_rollout.sh --bundle <path>`) paketi təsdiqləyir, yenidən çalışır
   xəbərdarlıq sınayır və növbə boşluq və ya sıfır dolduqda imza atmaqdan imtina edir
   büdcələr geriləyir.
5. **Xəbərdarlıqları bərpa etmək üçün bağlayın.** Alertmanager səhifələrini açarkən, Grafana istifadə edin.
   board və 2-ci addımdan etibarən xam Prometheus sayğacları olub olmadığını təsdiqləmək üçün
   endirmələr növbə aclığından, CPU geriləmələrindən və ya backend=heç biri partlamadan qaynaqlanır.
Runbook yaşayır
bu sənəd üstəgəl `docs/source/fastpq_rollout_playbook.md`; yeniləyin
müvafiq `fastpq_execution_mode_total` ilə buraxılış bileti,
`fastpq_metal_queue_ratio` və `fastpq_metal_queue_depth` çıxarışlar birlikdə
rəyçilərin görə bilməsi üçün Grafana panelinə və xəbərdarlıq snapshotına keçidlərlə
məhz hansı SLO-nu işə saldı.

### WP2-E — Mərhələ-mərhələ Metal profilləmə şəkli

`scripts/fastpq/src/bin/metal_profile.rs` bükülmüş Metal çəkilişləri ümumiləşdirir
Beləliklə, 900 ms-dən aşağı hədəf zamanla izlənilə bilər (çalış
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
Yeni Markdown köməkçisi
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
aşağıdakı mərhələ cədvəllərini yaradır (mətnlə birlikdə Markdown çap edir
xülasə belə ki, WP2-E biletləri dəlilləri hərfi daxil edə bilsin). İki tutma izlənir
indi:

> **Yeni WP2-E cihazları:** `fastpq_metal_bench --gpu-probe ...` indi
> aşkarlama snapshot (tələb edilən/həll edilmiş icra rejimi, `FASTPQ_GPU`
> ləğvetmələr, aşkar edilmiş arxa uç və sadalanan Metal cihazlar/reyestr idləri)
> hər hansı bir ləpə başlamazdan əvvəl. Məcburi GPU hələ də işlədiyi zaman bu jurnalı çəkin
> CPU yoluna qayıdır ki, hostların gördüyü dəlil paketi qeyd edir
> `MTLCopyAllDevices` sıfır qaytarır və hansı ləğvetmələr bu müddət ərzində qüvvədədir
> etalon.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Mərhələ tutma köməkçisi:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> indi ayrı-ayrılıqda FFT, LDE və Poseidon üçün `fastpq_metal_bench` sürür,
> xam JSON çıxışlarını mərhələli kataloqlar altında saxlayır və tək buraxır
> CPU/GPU vaxtlarını, növbə dərinliyini qeyd edən `stage_profile_summary.json` paketi
> telemetriya, sütun quruluşu statistikası, nüvə profilləri və əlaqəli iz
> artefaktlar. Alt çoxluğu hədəfləmək üçün `--stage fft --stage lde --stage poseidon` keçin,
> `--trace-template "Metal System Trace"` xüsusi xctrace şablonunu seçmək üçün,
> və `--trace-dir` `.trace` paketlərini paylaşılan məkana yönləndirmək üçün. əlavə edin
> xülasə JSON plus hər bir WP2-E məsələsi üçün yaradılan iz faylları
> növbə doluluğu (`metal_dispatch_queue.*`), üst-üstə düşmə nisbətləri və
> çoxlu əl ilə spelunking etmədən qaçışlar arasında ələ keçirilmiş başlanğıc həndəsəsi
> `fastpq_metal_bench` çağırışları.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Növbə/təsdiqləmə sübut köməkçisi (05-09-2026):** indi `scripts/fastpq/profile_queue.py`
> bir və ya daha çox `fastpq_metal_bench` qəbul edir JSON həm Markdown cədvəlini, həm də
> maşın tərəfindən oxuna bilən xülasə (`--markdown-out/--json-out`) növbənin dərinliyi, üst-üstə düşmə nisbətləri və
> host tərəfində səhnələşdirmə telemetriyası hər bir WP2-E artefaktının yanında gəzə bilər. Qaçış
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` aşağıdakı cədvəli hazırladı və qeyd etdi ki, arxivləşdirilmiş Metal hələ də hesabat verir
> `dispatch_count = 0` və `column_staging.batches = 0`—WP2-E.1 Metal açılana qədər açıq qalır
> cihazlar telemetriya aktiv edilməklə yenidən qurulur. Yaradılmış JSON/Markdown artefaktları canlıdır
> audit üçün `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` altında.
> Köməkçi indi (2026-05-19) Poseidon boru kəmərinin telemetriyasını da (`pipe_depth`,
> `batches`, `chunk_columns` və `fallbacks`) həm Markdown cədvəlində, həm də JSON xülasəsində,
> beləliklə, WP2-E.4/6 rəyçiləri GPU-nun boru xəttində qalıb-qalmadığını və hər hansı
> ehtiyatlar xam çəkilişi açmadan baş verdi.【scripts/fastpq/profile_queue.py:1】> **Mərhələ profilinin xülasəsi (30-05-2026):** `scripts/fastpq/stage_profile_report.py` istehlak edir
> `cargo xtask fastpq-stage-profile` tərəfindən buraxılan `stage_profile_summary.json` paketi və
> həm Markdown, həm də JSON xülasəsini təqdim edir ki, WP2-E rəyçiləri sübutları biletlərə köçürə bilsinlər
> vaxtları əl ilə köçürmədən. Çağırın
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> GPU/CPU vasitələrini, sürətləndirmə deltalarını, əhatə dairəsini və
> hər mərhələdə telemetriya boşluqları. JSON çıxışı cədvəli əks etdirir və hər mərhələdə problem teqlərini qeyd edir
> (`trace missing`, `queue telemetry missing` və s.) beləliklə idarəetmənin avtomatlaşdırılması ev sahibini fərqləndirə bilər
> WP2-E.1 vasitəsilə WP2-E.6-da istinad edilən çalışır.
> **Host/cihaz üst-üstə düşmə qoruyucusu (06-04-2026):** `scripts/fastpq/profile_queue.py` indi şərh edir
> FFT/LDE/Poseidon gözləmə əmsalları ilə yanaşı, hər bir mərhələ üzrə düzləşdirmə/gözləmə millisaniyəsi cəmləri və emissiyaları
> `--max-wait-ratio <threshold>` zəif üst-üstə düşmə aşkar etdikdə problem yaranır. istifadə edin
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> həm Markdown cədvəlini, həm də açıq gözləmə nisbətləri ilə JSON paketini çəkmək üçün WP2-E.5 biletləri
> ikiqat tamponlama pəncərəsinin GPU-nu qidalandırıb saxlamadığını göstərə bilər. Düz mətnli konsol çıxışı da
> Zəng üzrə araşdırmaları asanlaşdırmaq üçün hər bir faza nisbətlərini sadalayır.
> **Telemetriya qoruyucusu + işləmə vəziyyəti (06-09-2026):** `fastpq_metal_bench` indi `run_status` blokunu yayır
> (backend etiketi, göndərmə sayı, səbəblər) və yeni `--require-telemetry` bayrağı qaçır
> GPU vaxtları və ya növbə/tədris telemetriyası olmadıqda. `profile_queue.py` qaçışı göstərir
> ayrılmış sütun kimi status və buraxılış siyahısında qeyri-`ok` vəziyyətlərini göstərir və
> `launch_geometry_sweep.py` eyni vəziyyəti xəbərdarlıqlara/klassifikasiyaya daxil edir ki, matrislər
> səssizcə CPU-ya düşən və ya növbə alətlərini atlayan çəkilişləri daha uzun müddətə qəbul edin.
> **Poseidon/LDE avtomatik tuning (2026-06-12):** `metal_config::poseidon_batch_multiplier()` indi tərəzi
> Metal işləmə dəsti göstərişləri və `lde_tile_stage_target()` diskret GPU-larda kafel dərinliyini artırır.
> Tətbiq olunan çarpan və kafel limiti `metal_heuristics` blokuna daxildir.
> `fastpq_metal_bench` çıxışları və `scripts/fastpq/metal_capture_summary.py` tərəfindən göstərildiyi üçün WP2-E
> paketlər xam JSON-u qazmadan hər bir çəkilişdə istifadə olunan dəqiq boru kəməri düymələrini qeyd edir.【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq_prover:2833】【scripts/fastpq_marypture.

| Etiket | Göndər | Məşğul | Üst-üstə düşmə | Maksimum Dərinlik | FFT düzləşdirmək | FFT gözləyin | FFT gözləyin % | LDE düzləşdirmək | LDE gözləyin | LDE gözləyin % | Poseidon düzləşdirmək | Poseidon gözləyin | Poseidon gözləyin % | Boru dərinliyi | Boru partiyaları | Boru ehtiyatları |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k snapshot (əvvəlcədən dəyişdirmə)

`fastpq_metal_bench_20k_latest.json`| Mərhələ | Sütunlar | Daxil et len ​​| GPU orta (ms) | CPU orta (ms) | GPU payı | Sürətləndirmə | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130,986 ms (115,761–167,755) | 112,616 ms (95,335–132,929) | 2,4% | 0,860× | −18.370 |
| IFFT | 16 | 32768 | 129,296 ms (111,127–142,955) | 158,144 ms (126,847–237,887) | 2,4% | 1,223× | +28.848 |
| LDE | 16 | 262144 | 1570,656 ms (1544,397–1584,502) | 1752,523 ms (1548,807–2191,930) | 29,2% | 1,116× | +181.867 |
| Poseydon | 16 | 524288 | 3548,329 ms (3519,881–3576,041) | 3642,706 ms (3539,055–3758,279) | 66,0% | 1,027× | +94.377 |

Əsas müşahidələr:1. Ümumi GPU 5,379 s-dir ki, bu da 900 ms hədəfindən **4,48 s artıqdır. Poseydon
   hashing hələ də icra müddətində (≈66%) üstünlük təşkil edir, ikinci yerdə LDE nüvəsi var
   yer (≈29%), buna görə də WP2-E həm Poseidon boru kəmərinin dərinliyinə həm də hücum etməlidir.
   CPU ehtiyatları yoxa çıxmazdan əvvəl LDE yaddaş rezidentliyi/kafel planı.
2. IFFT skalar üzərində >1,22× olsa belə, FFT reqressiya olaraq qalır (0,86×)
   yol. Bizə bir başlanğıc həndəsəsi lazımdır
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) başa düşmək üçün
   FFT işğalı onsuz da daha yaxşı olana zərər vermədən xilas ola bilərmi?
   IFFT vaxtları. `scripts/fastpq/launch_geometry_sweep.py` köməkçisi indi idarə edir
   bu eksperimentlər başdan-başa: vergüllə ayrılmış əvəzləmələri keçin (məsələn,
   `--fft-columns 16,32 --queue-fanout 1,2` və
   `--poseidon-lanes auto,256`) və o, çağıracaq
   Hər kombinasiya üçün `fastpq_metal_bench`, JSON yüklərini altında saxlayın
   `artifacts/fastpq_geometry/<timestamp>/` və `summary.json` paketini davam etdirin
   hər bir qaçışın növbə nisbətlərini, FFT/LDE başlatma seçimlərini, GPU və CPU vaxtlarını təsvir edən,
   və host metadata (host adı/etiket, platforma üçlü, aşkar edilmiş cihaz
   sinif, GPU satıcısı/modeli) buna görə də cihazlar arası müqayisələr deterministikdir
   mənşə. Köməkçi indi də yanında `reason_summary.json` yazır
   yuvarlanmaq üçün həndəsə matrisi ilə eyni təsnifatçıdan istifadə edərək defolt olaraq xülasə
   CPU-nun geri dönmələri və çatışmayan telemetriya. Etiketləmək üçün `--host-label staging-m3` istifadə edin
   paylaşılan laboratoriyalardan çəkilişlər.
   Yoldaş `scripts/fastpq/geometry_matrix.py` aləti indi bir və ya birini qəbul edir
   daha çox xülasə paketləri (`--summary hostA/summary.json --summary hostB/summary.json`)
   və hər işə salma formasını *stabil* kimi etiketləyən Markdown/JSON cədvəllərini yayır
   (FFT/LDE/Poseidon GPU vaxtları çəkilib) və ya *qeyri-sabit* (vaxt aşımı, CPU geri qaytarılması,
   qeyri-metal backend və ya çatışmayan telemetriya) əsas sütunların yanında. The
   cədvəllərə indi həll edilmiş `execution_mode`/`gpu_backend` və əlavə
   `Reason` sütunu beləliklə CPU-nun geri dönmələri və çatışmayan GPU vaxtları açıq-aşkar görünür.
   Stage7 matrisləri hətta zamanlama blokları mövcud olduqda; xülasə xətti sayılır
   stabil vs ümumi qaçış. `--operation fft|lde|poseidon_hash_columns` keçin
   süpürgə bir mərhələni təcrid etməli olduqda (məsələn, profil üçün
   Poseidon ayrıca) və dəzgahın xüsusi bayraqları üçün `--extra-args`-i pulsuz saxlayın.
   Köməkçi hər hansı birini qəbul edir
   əmr prefiksi (defolt olaraq `cargo run … fastpq_metal_bench`) üstəgəl isteğe bağlıdır
   `--halt-on-error` / `--timeout-seconds` qoruyur ki, performans mühəndisləri
   müqayisəli toplanarkən süpürgəni müxtəlif maşınlarda təkrarlayın,
   Stage7 üçün çox cihaz sübut paketləri.
3. `metal_dispatch_queue` `dispatch_count = 0` məlumat verdi, buna görə də növbənin doluluğu
   GPU nüvələri işləsə də, telemetriya yox idi. Metal iş vaxtı indi istifadə edir
   növbə/sütun quruluşu üçün hasarları əldə edin/buraxın
   cihaz bayraqlarına baxın və həndəsə matrisi hesabatı səslənir
   FFT/LDE/Poseidon GPU vaxtları olmadıqda qeyri-sabit işə salma formaları. Saxla
   rəyçilərin görə bilməsi üçün Markdown/JSON matrisini WP2-E biletlərinə əlavə etmək
   növbə telemetriyası mövcud olduqdan sonra hansı birləşmələr hələ də uğursuz olur.`run_status` qoruyucu və `--require-telemetry` bayrağı indi tutula bilmir
   GPU vaxtları çatışmırsa və ya növbə/səhnə telemetriyası olmadıqda, yəni
   dispatch_count=0 qaçış artıq nəzərə çarpmadan WP2-E paketlərinə daxil ola bilməz.
   `fastpq_metal_bench` indi `--require-gpu`-i ifşa edir və
   `launch_geometry_sweep.py` onu defolt olaraq aktivləşdirir (çıxın
   `--allow-cpu-fallback`) beləliklə, CPU ehtiyatları və Metal aşkarlama uğursuzluqları dayandırılır
   Stage7 matrislərini qeyri-GPU telemetriyası ilə çirkləndirmək əvəzinə dərhal.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Sıfır doldurma göstəriciləri əvvəllər eyni səbəbdən yoxa çıxdı; hasarın düzəldilməsi
   ev sahibi alətlərini canlı saxlayır, ona görə də növbəti çəkiliş daxil olmalıdır
   Sintetik vaxtlar olmadan `zero_fill` bloku.

#### `FASTPQ_GPU=gpu` ilə 20k snapshot

`fastpq_metal_bench_20k_refresh.json`

| Mərhələ | Sütunlar | Daxil et len ​​| GPU orta (ms) | CPU orta (ms) | GPU payı | Sürətləndirmə | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79,951 ms (65,645–93,193) | 83,289 ms (59,956–107,585) | 0,3% | 1,042× | +3.338 |
| IFFT | 16 | 32768 | 78,605 ms (69,986–83,726) | 93,898 ms (80,656–119,625) | 0,3% | 1,195× | +15.293 |
| LDE | 16 | 262144 | 657,673 ms (619,219–712,367) | 669,537 ms (619,716–723,285) | 2,1% | 1,018× | +11.864 |
| Poseydon | 16 | 524288 | 30004,898 ms (27284,117–32945,253) | 29087,532 ms (24969,810–33020,517) | 97,4% | 0,969× | −917.366 |

Müşahidələr:

1. Hətta `FASTPQ_GPU=gpu` ilə belə, bu tutma hələ də CPU-nu əks etdirir:
   `metal_dispatch_queue` sıfırda ilişib qalmış hər iterasiya üçün ~30s. Zaman
   ləğvetmə təyin edilib, lakin ev sahibi Metal cihaz aşkar edə bilmir, CLI indi çıxır
   hər hansı ləpələri işə salmadan və tələb olunan/həll edilmiş rejimi üstəgəl çap etdirməzdən əvvəl
   arxa uç etiketi beləliklə mühəndislər aşkarlama, hüquqlar və ya
   metallib axtarışı endirməyə səbəb oldu. `fastpq_metal_bench --gpu-probe-u işə salın
   --sətirlər …` with `FASTPQ_DEBUG_METAL_ENUM=1` nömrələmə jurnalını və
   Profileri yenidən işə salmazdan əvvəl əsas aşkarlama problemini həll edin.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. Sıfır doldurma telemetriyası indi real nümunəni (32MiB-dən 18,66 ms) qeyd edir və bunu sübut edir.
   hasarın düzəldilməsi işləyir, lakin növbə deltaları GPU göndərənə qədər yoxdur
   uğur qazanmaq.
3. Backend aşağı səviyyəyə enməyə davam etdiyi üçün Stage7 telemetriya qapısı hələ də qalır
   bloklanmışdır: növbə başlığı sübutu və poseidon üst-üstə düşməsi orijinal GPU tələb edir
   qaçmaq.

Bu ələ keçirmələr indi WP2-E geriləməsini birləşdirir. Növbəti hərəkətlər: profilçi toplayın
alov diaqramları və növbə qeydləri (backend GPU-da yerinə yetirildikdən sonra)
FFT-ə yenidən baxmadan əvvəl Poseidon/LDE darboğazları və arxa uçun geri qaytarılmasının qarşısını açın
belə ki, Stage7 telemetriyasında real GPU məlumatı var.

### Güclü tərəflər
- Artan səhnələşdirmə, ilk izləmə dizaynı, şəffaf STARK yığını.### Yüksək Prioritetli Fəaliyyət Maddələri
1. Qablaşdırma/sifariş qurğularını həyata keçirin və AIR spesifikasiyasını yeniləyin.
2. Poseidon2 `3f2b7fe` öhdəliyini yekunlaşdırın və nümunə SMT/axtarış vektorlarını dərc edin.
3. Armaturlarla yanaşı işlənmiş nümunələri (`lookup_grand_product.md`, `smt_update.md`) qoruyun.
4. Dəqiqlik əldə edilməsini və CI rədd metodologiyasını sənədləşdirən Əlavə A əlavə edin.

### Həll edilmiş Dizayn Qərarları
- P1-də ZK əlil (yalnız düzgünlük); gələcək mərhələdə yenidən nəzərdən keçirin.
- İdarəetmə vəziyyətindən əldə edilən icazə cədvəlinin kökü; qruplar cədvəli yalnız oxumaq üçün nəzərdə tutur və axtarış vasitəsilə üzvlüyünü sübut edir.
- Olmayan əsas sübutlar sıfır yarpaq və kanonik kodlaşdırma ilə qonşu şahiddən istifadə edir.
- Semantikanı silin = kanonik düymə boşluğunda sıfıra təyin edilmiş yarpaq dəyəri.

Bu sənədi kanonik istinad kimi istifadə edin; sürüşmənin qarşısını almaq üçün onu mənbə kodu, qurğular və əlavələrlə birlikdə yeniləyin.

## Əlavə A — Sağlamlığın əldə edilməsi

Bu əlavə “Sağlamlıq və SLOs” cədvəlinin necə hazırlandığını və CI-nin daha əvvəl qeyd olunan ≥128-bit mərtəbəni necə tətbiq etdiyini izah edir.

### Qeyd
- `N_trace = 2^k` — iki qüvvəyə qədər çeşidləmə və doldurmadan sonra iz uzunluğu.
- `b` — partlama faktoru (`N_eval = N_trace × b`).
- `r` — FRI arity (kanonik dəstlər üçün 8 və ya 16).
- `ℓ` — FRI azalmalarının sayı (`layers` sütunu).
- `q` — sübuta görə yoxlayıcı sorğular (`queries` sütunu).
- `ρ` — sütun planlayıcısı tərəfindən bildirilən effektiv kod dərəcəsi: `ρ = max_i(degree_i / domain_i)`, birinci FRI raundunda sağ qalan məhdudiyyətlər üzərində.

Goldilocks baza sahəsi `|F| = 2^64 - 2^32 + 1`-ə malikdir, buna görə Fiat-Şamir toqquşmaları `q / 2^64` ilə məhdudlaşır. Taşlama ortoqonal `2^{-g}` faktoru əlavə edir, `fastpq-lane-balanced` üçün `g = 23` və gecikmə profili üçün `g = 21`.【crates/fastpq_isi/src/params.rs:65】

### Analitik bağlıdır

Sabit DEEP-FRI ilə statistik uğursuzluq ehtimalı təmin edilir

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

çünki hər qat polinom dərəcəsini və domen enini eyni `r` faktoru ilə azaldır, `ρ` sabit saxlayır. Cədvəlin `est bits` sütunu `⌊-log₂ p_fri⌋` hesabatları; Fiat-Şamir və daşlama əlavə təhlükəsizlik marjası kimi xidmət edir.

### Planlayıcı çıxışı və işlənmiş hesablama

Mərhələ 1 sütun planlayıcısını təmsilçi qruplar üzrə işə salmaq nəticə verir:

| Parametr dəsti | `N_trace` | `b` | `N_eval` | `ρ` (planlayıcı) | Effektiv dərəcə (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | --------------------------------- | --- | --- | ------------------ |
| Balanslaşdırılmış 20k toplu | `2^15` | 8 | 262144 | 0.077026 | 20192 | 5 | 52 | 190bit |
| Məhsuldarlıq 65k toplu | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132 bit |
| Gecikmə 131k toplu | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142 bit |Nümunə (balanslaşdırılmış 20k toplu):
1. `N_trace = 2^15`, buna görə də `N_eval = 2^15 × 8 = 2^18`.
2. Planner alətləri hesabatları `ρ = 0.077026`, buna görə də `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, cədvəl daxilinə uyğun gəlir.
4. Fiat-Şamir toqquşmaları ən çox `2^{-58.3}` əlavə edir və daşlama (`g = 23`) başqa bir `2^{-23}` çıxarır və ümumi sağlamlığı 160 bitdən çox rahat saxlayır.

### CI imtina üçün nümunə götürmə kəməri

Hər bir CI əməliyyatı empirik ölçmələrin analitik sərhəddən ±0,6 bit ərzində qalmasını təmin etmək üçün Monte Carlo qoşqusunu yerinə yetirir:
1. Kanonik parametrlər dəstini çəkin və uyğun sıra sayı ilə `TransitionBatch` sintez edin.
2. İzi yaradın, təsadüfi seçilmiş məhdudiyyəti çevirin (məsələn, axtarışın böyük məhsulunu və ya SMT qardaşını narahat edin) və sübut yaratmağa çalışın.
3. Fiat-Şamir problemlərini yenidən nümunə götürərək yoxlayıcını yenidən işə salın (daşlama daxildir) və saxtalaşdırılmış sübutun rədd edilib-edilmədiyini qeyd edin.
4. Parametr dəsti başına 16384 toxum üçün təkrarlayın və müşahidə edilən rəddetmə dərəcəsinin 99% Klopper-Pirson aşağı sərhədini bitlərə çevirin.

Ölçülmüş aşağı sərhəd 128 bitin altına düşərsə, iş dərhal uğursuz olur, beləliklə planlayıcıda, qatlanan döngədə və ya transkript naqillərindəki reqressiyalar birləşmədən əvvəl tutulur.

## Əlavə B — Domen-kök törəməsi

Stage0 izləmə və qiymətləndirmə generatorlarını Poseidon mənşəli sabitlərə bağlayır ki, bütün tətbiqlər eyni alt qrupları paylaşsın.

### Prosedur
1. **Toxum seçimi.** UTF‑8 etiketini `fastpq:v1:domain_roots` FASTPQ-da başqa yerdə istifadə olunan Poseidon2 süngərinə daxil edin (vəziyyət eni=3, dərəcə=2, dörd tam + 57 qismən dövrə). Daxiletmələr `pack_bytes` kodlaşdırmasından `[len, limbs…]` kodlamasını təkrar istifadə edərək, `g_base = 7` baza generatorunu verir.
2. **İz generatoru.** Yarım güc 1 olmadığı halda `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` hesablayın və `trace_root^{2^{trace_log_size}} = 1`-i yoxlayın.
3. **LDE generatoru.** `lde_root` əldə etmək üçün eyni eksponentasiyanı `lde_log_size` ilə təkrarlayın.
4. **Coset seçimi.** Mərhələ0 əsas alt qrupdan istifadə edir (`omega_coset = 1`). Gələcək kosetlər `fastpq:v1:domain_roots:coset` kimi əlavə etiketi qəbul edə bilər.
5. **Permutasiya ölçüsü.** `permutation_size`-ni açıq şəkildə saxlayın ki, planlaşdırıcılar heç vaxt ikinin gizli səlahiyyətlərindən doldurma qaydaları çıxarmasın.

### Reproduksiya və doğrulama
- Alətlər: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` ya Rust parçalarını, ya da Markdown cədvəlini yayır (bax: `--format table`, `--seed`, `--filter`).【scripts/fastpq/src_gen/poseidon
- Testlər: `canonical_sets_meet_security_target` kanonik parametr dəstlərini dərc edilmiş sabitlərlə (sıfırdan fərqli köklər, partlama/arity cütləşməsi, permutasiya ölçüsü) uyğunlaşdırır, beləliklə, `cargo test -p fastpq_isi` drifti dərhal tutur.【crates/fastpq_isi/src:18ms.
- Həqiqət mənbəyi: yeni parametr paketləri təqdim edildikdə Stage0 cədvəlini və `fastpq_isi/src/params.rs`-i birlikdə yeniləyin.

## Əlavə C — Öhdəlik boru kəmərinin təfərrüatları### Streaming Poseidon öhdəlik axını
Mərhələ 2 sübut edən və yoxlayıcı tərəfindən paylaşılan deterministik iz öhdəliyini müəyyən edir:
1. **Keçidləri normallaşdırın.** `trace::build_trace` hər partiyanı çeşidləyir, onu `N_trace = 2^{⌈log₂ rows⌉}`-ə köçürür və aşağıdakı ardıcıllıqla sütun vektorlarını buraxır.【crates/fastpq_prover/src/trace.rs:123】
2. **Haş sütunları.** `trace::column_hashes` sütunları `fastpq:v1:trace:column:<name>` etiketli xüsusi Poseidon2 süngərləri vasitəsilə ötürür. `fastpq-prover-preview` funksiyası aktiv olduqda, eyni keçid backend tərəfindən tələb olunan IFFT/LDE əmsallarını təkrar emal edir, ona görə də əlavə matris nüsxələri ayrılmır.【crates/fastpq_prover/src/trace.rs:474】
3. **Merkle ağacına qaldırın.** `trace::merkle_root`, `fastpq:v1:trace:node` etiketli Poseidon qovşaqları ilə sütun həzmlərini qatlayaraq, xüsusi hallardan qaçınmaq üçün səviyyənin tək-tük çıxması zamanı sonuncu yarpağı təkrarlayır.【crates/fastpq_prover/src5/src:
4. **Dəyjesti yekunlaşdırın.** `digest::trace_commitment` eyni `[len, limbs…]` kodlaşdırmasından istifadə edərək domen teqini (`fastpq:v1:trace_commitment`), parametr adını, doldurulmuş ölçüləri, sütun həzmlərini və Merkle kökünü prefiks edir, sonra SHA6b-də emal edilmiş yükü 5-də hesh edir. `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

Doğrulayıcı Fiat-Şamir problemlərini seçməzdən əvvəl eyni həzmi yenidən hesablayır, buna görə də hər hansı bir açılışdan əvvəl sübutları dayandırın.

### Poseidon geri qaytarma nəzarətləri- Prover indi xüsusi Poseidon boru kəmərinin ləğvini (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) ifşa edir ki, operatorlar Stagems7 <0 hədəfinə çata bilməyən cihazlarda GPU FFT/LDE-ni CPU Poseidon heshing ilə qarışdıra bilsinlər. Dəstəklənən dəyərlər icra rejimi düyməsini (`auto`, `cpu`, `gpu`) əks etdirir, təyin edilmədikdə defolt olaraq qlobal rejimə keçir. İş vaxtı bu dəyəri zolaq konfiqurasiyası (`FastpqPoseidonMode`) vasitəsilə ötürür və onu proverə (`Prover::canonical_with_modes`) yayır, beləliklə, ləğvetmələr konfiqurasiyada deterministik və yoxlanıla bilər. zibil qutuları.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- Telemetriya həll edilmiş boru kəməri rejimini yeni `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` sayğacı (və OTLP əkiz `fastpq.poseidon_pipeline_resolutions_total`) vasitəsilə ixrac edir. Buna görə də `sorafs`/operatorun idarə panelləri prosessorun məcburi geri qaytarılmasına (`path="cpu_forced"`) və ya iş vaxtının aşağı salınmasına (`path="cpu_fallback"`) qarşı GPU-nun əridilmiş/borulanmış heşinqlə işlədiyini təsdiqləyə bilər. CLI zondu avtomatik olaraq `irohad`-də quraşdırılır, ona görə də paketləri buraxın və canlı telemetriya eyni sübut axınını paylaşır.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- Qarışıq rejimli dəlillər, həmçinin mövcud qəbul qapısı vasitəsilə hər bir tabloda möhürlənir: prover hər bir partiya üçün həll edilmiş rejim + yol etiketi verir və sübut yerə düşəndə `fastpq_poseidon_pipeline_total` sayğac icra rejimi sayğacının yanında artır. Bu, qaralmaları görünən etmək və optimallaşdırma davam edərkən deterministik endirmələr üçün təmiz keçid təmin etməklə WP2-E.6-nı təmin edir.【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` indi Prometheus qırıntılarını (Metal və ya CUDA) təhlil edir və hər bükülmüş paketin içərisinə `poseidon_metrics` xülasəsini daxil edir. Köməkçi sayğac sıralarını `metadata.labels.device_class` ilə süzür, uyğun gələn `fastpq_execution_mode_total` nümunələrini çəkir və `fastpq_poseidon_pipeline_total` qeydləri çatışmırsa, bağlamanı uğursuz edir, beləliklə, WP2-E.6 paketləri həmişə CUDA ad-Mehoc əvəzinə təkrar istehsal olunan sübutlar göndərir. qeydlər.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Deterministik qarışıq rejim siyasəti (WP2-E.6)1. **GPU çatışmazlığını aşkar edin.** Stage7 çəkilişi və ya canlı Grafana snapşotunda FFT/LDE hədəfin altında qalarkən ümumi sübut müddəti >900ms saxlayan Poseidon gecikməsini göstərən istənilən cihaz sinifini qeyd edin. `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` durğunlaşdıqda, `fastpq_execution_mode_total{backend="metal"}` hələ də GPU FFT/LDE-ni qeyd edərkən operatorlar tutma matrisinə (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) şərh verir və çağırışı səhifələyir. göndərilir.【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **Yalnız təsirə məruz qalan hostlar üçün CPU Poseidon-a çevirin.** `zk.fastpq.poseidon_mode = "cpu"` (və ya `FASTPQ_POSEIDON_MODE=cpu`) donanma etiketləri ilə yanaşı host-yerli konfiqurasiyasında `zk.fastpq.execution_mode = "gpu"`-i saxlayaraq FFT/LDE sürətləndiricidən istifadə etməyə davam edin. Təqdimat biletində konfiqurasiya fərqini qeyd edin və `poseidon_fallback.patch` olaraq paketə hər bir hostun ləğvini əlavə edin ki, rəyçilər dəyişikliyi qəti şəkildə təkrarlaya bilsinlər.
3. **Endirməni sübut edin.** Düyünü yenidən işə saldıqdan dərhal sonra Poseidon sayğacını qırın:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   Dump, GPU icra sayğacı ilə kilid addımında böyüyən `path="cpu_forced"`-i göstərməlidir. Sızıntını mövcud `metrics_cpu_fallback.prom` şəklinin yanında `metrics_poseidon.prom` olaraq saxlayın və `poseidon_fallback.log`-də uyğun `telemetry::fastpq.poseidon` jurnal xətlərini çəkin.
4. **Monitor və çıxın.** Optimallaşdırma işləri davam edərkən `fastpq_poseidon_pipeline_total{path="cpu_forced"}`-də xəbərdar olmağa davam edin. Yamaq sınaq hostunda hər sınaq müddətini 900 ms-dən aşağı qaytardıqdan sonra konfiqurasiyanı yenidən `auto`-ə çevirin, sıyrıntını yenidən işə salın (yenidən `path="gpu"` göstərilir) və qarışıq rejimli qazmağı bağlamaq üçün paketə əvvəl/sonra göstəricilərini əlavə edin.

**Telemetriya müqaviləsi.**

| Siqnal | PromQL / Mənbə | Məqsəd |
|--------|-----------------|---------|
| Poseidon rejimi sayğacı | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | CPU heshinginin qəsdən olduğunu və işarələnmiş cihaz sinfinə aid olduğunu təsdiq edir. |
| İcra rejimi sayğacı | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Poseidon səviyyəsini endirərkən belə FFT/LDE-nin hələ də GPU-da işlədiyini sübut edir. |
| Gündəlik sübut | `poseidon_fallback.log`-də qeydə alınan `telemetry::fastpq.poseidon` qeydləri | Hostun `cpu_forced` Səbəbi ilə CPU hashinginə qərar verdiyini sübut edən hər bir sübut təqdim edir. |

Yayım paketinə indi qarışıq rejim aktiv olduqda `metrics_poseidon.prom`, konfiqurasiya fərqi və jurnaldan çıxarış daxil edilməlidir ki, idarəetmə FFT/LDE telemetriyası ilə yanaşı deterministik geri qaytarma siyasətini də yoxlaya bilsin. `ci/check_fastpq_rollout.sh` artıq növbə/sıfır doldurma məhdudiyyətlərini tətbiq edir; Qarışıq rejim buraxılış avtomatlaşdırmasına düşdükdən sonra izləmə qapısı Poseidon sayğacını ağlı başında yoxlayacaq.

Stage7 ələ keçirmə aləti artıq CUDA-nı idarə edir: hər `fastpq_cuda_bench` paketini `--poseidon-metrics` ilə sarın (sıyrılmış `metrics_poseidon.prom`-ə işarə edir) və çıxış indi Metalify-də istifadə edilən eyni boru kəməri sayğaclarını/rezolyusiya xülasəsini daşıyır. alətlər.【scripts/fastpq/wrap_benchmark.py:1】### Sütun sifarişi
Hashing boru kəməri bu deterministik qaydada sütunları istehlak edir:
1. Seçici bayraqlar: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_active`, I18500 `s_perm`.
2. Qablaşdırılmış ekstremitə sütunları (hər biri iz uzunluğuna sıfır doldurulmuşdur): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Köməkçi skalyarlar: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `neighbour_leaf`, I0108, I018 `slot`.
4. Hər səviyyə üçün seyrək Merkle şahidləri `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` sütunları məhz bu ardıcıllıqla gəzir, beləliklə, yer tutucunun arxa hissəsi və Mərhələ 2 STARK tətbiqi buraxılışlar arasında izləmə sabit qalır.【crates/fastpq_prover/src/trace.rs:474】

### Transkript domen teqləri
Stage2, problem yaratmağı deterministik saxlamaq üçün aşağıdakı Fiat-Şamir kataloqunu düzəldir:

| Tag | Məqsəd |
| --- | ------- |
| `fastpq:v1:init` | Protokol versiyasını, parametrlər dəstini və `PublicIO`-i qəbul edin. |
| `fastpq:v1:roots` | İzləyin və Merkle köklərini axtarın. |
| `fastpq:v1:gamma` | Böyük məhsul axtarış problemini nümunə götürün. |
| `fastpq:v1:alpha:<i>` | Nümunə kompozisiya-polinom problemləri (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Qiymətləndirilmiş axtarış böyük məhsulu mənimsəyin. |
| `fastpq:v1:beta:<round>` | Hər FRI turu üçün qatlama problemini nümunə götürün. |
| `fastpq:v1:fri_layer:<round>` | Hər FRI təbəqəsi üçün Merkle kökünü tətbiq edin. |
| `fastpq:v1:fri:final` | Sorğuları açmazdan əvvəl son FRI qatını qeyd edin. |
| `fastpq:v1:query_index:0` | Doğrulayıcı sorğu indekslərini qəti şəkildə əldə edin. |