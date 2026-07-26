---
lang: az
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hesablama zolağı (SSC-1)

Hesablama zolağı deterministik HTTP üslublu zəngləri qəbul edir, onları Kotodama-ə uyğunlaşdırır
giriş nöqtələri və hesablama və idarəetmənin nəzərdən keçirilməsi üçün ölçmə/qəbzləri qeyd edir.
Bu RFC manifest sxemini, zəng/qəbul zərflərini, sandbox qoruyucularını,
və ilk buraxılış üçün konfiqurasiya defoltları.

## Manifest

- Sxem: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` `1`-ə bərkidilib; fərqli versiyaya malik manifestlər rədd edilir
  doğrulama zamanı.
- Hər bir marşrut bəyan edir:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama giriş nöqtəsinin adı)
  - kodeklərin icazə siyahısı (`codecs`)
  - TTL/qaz/sorğu/cavab qapaqları (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - determinizm/icra sinfi (`determinism`, `execution_class`)
  - SoraFS giriş/model deskriptorları (`input_limits`, isteğe bağlı `model`)
  - qiymət ailəsi (`price_family`) + resurs profili (`resource_profile`)
  - autentifikasiya siyasəti (`auth`)
- Sandbox qoruyucuları manifest `sandbox` blokunda yaşayır və hamı tərəfindən paylaşılır
  marşrutlar (rejim/təsadüfilik/saxlama və deterministik olmayan sistem çağırışının rədd edilməsi).

Misal: `fixtures/compute/manifest_compute_payments.json`.

## Zənglər, sorğular və qəbzlər

- Sxem: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome`
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` kanonik sorğu hashını yaradır (başlıqlar saxlanılır
  deterministik `BTreeMap`-də və faydalı yük `payload_hash` kimi daşınır).
- `ComputeCall` ad sahəsi/marşrut, kodek, TTL/qaz/cavab qapağı,
  resurs profili + qiymət ailəsi, auth (`Public` və ya UAID ilə bağlıdır
  `ComputeAuthn`), determinizm (`Strict` vs `BestEffort`), icra sinfi
  göstərişlər (CPU/GPU/TEE), elan edilmiş SoraFS giriş baytları/parçaları, isteğe bağlı sponsor
  büdcə və kanonik sorğu zərfi. sorğu hash üçün istifadə olunur
  təkrar qorunma və marşrutlaşdırma.
- Marşrutlar isteğe bağlı SoraFS model arayışlarını və giriş limitlərini daxil edə bilər
  (daxili/parça başlıqları); manifest sandbox qaydaları qapısı GPU/TEE göstərişləri.
- `ComputePriceWeights::charge_units` ölçmə məlumatlarını hesablanmış hesablamaya çevirir
  dövrlər və çıxış baytları üzrə tavan bölməsi vasitəsilə vahidlər.
- `ComputeOutcome` hesabatları `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, və ya `InternalError` və isteğe bağlı olaraq cavab heşləri daxildir/
  audit üçün ölçülər/kodek.

Nümunələr:
- Zəng edin: `fixtures/compute/call_compute_payments.json`
- Qəbz: `fixtures/compute/receipt_compute_payments.json`

## Sandbox və resurs profilləri- `ComputeSandboxRules` standart olaraq icra rejimini `IvmOnly`-a kilidləyir,
  sorğu hash-dən toxumların deterministik təsadüfiliyi, yalnız oxunmağa imkan verir SoraFS
  daxil olur və deterministik olmayan sistem çağırışlarını rədd edir. GPU/TEE göstərişləri tərəfindən təmin edilir
  İcranı deterministik saxlamaq üçün `allow_gpu_hints`/`allow_tee_hints`.
- `ComputeResourceBudget` dövrlərdə, xətti yaddaşda, yığında hər profil üçün qapaqlar təyin edir
  ölçüsü, IO büdcəsi və çıxış, üstəgəl GPU göstərişləri və WASI-lite köməkçiləri üçün keçidlər.
- Defolt olaraq iki profil göndərilir (`cpu-small`, `cpu-balanced`)
  `defaults::compute::resource_profiles` deterministik geri dönüşlərlə.

## Qiymətləndirmə və hesablama vahidləri

- Qiymət ailələri (`ComputePriceWeights`) dövrlərin xəritəsini və hesablamaya çıxış baytlarını
  vahidlər; default ödəniş `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` ilə
  `unit_label = "cu"`. Ailələr manifestlərdə `price_family` ilə açar və
  qəbul zamanı tətbiq edilir.
- Ölçmə qeydləri `charged_units` üstəgəl xam dövr/giriş/çıxış/müddəti daşıyır
  uzlaşma üçün cəmi. Ödənişlər icra sinfi ilə gücləndirilir və
  determinizm çarpanları (`ComputePriceAmplifiers`) və qapalı
  `compute.economics.max_cu_per_call`; çıxış tərəfindən sıxılır
  `compute.economics.max_amplification_ratio` bağlı cavab gücləndirilməsi.
- Sponsor büdcələri (`ComputeCall::sponsor_budget_cu`) qarşı tətbiq edilir
  zəng başına/gündəlik limitlər; hesablanmış vahidlər elan edilmiş sponsor büdcəsindən artıq olmamalıdır.
- İdarəetmə qiymət yeniləmələri risk sinfi sərhədlərindən istifadə edir
  `compute.economics.price_bounds` və əsas ailələr qeyd edildi
  `compute.economics.price_family_baseline`; istifadə edin
  Yeniləmədən əvvəl deltaları yoxlamaq üçün `ComputeEconomics::apply_price_update`
  aktiv ailə xəritəsi. Torii konfiqurasiya yeniləmələri istifadə edir
  `ConfigUpdate::ComputePricing` və kiso bunu eyni sərhədlərlə tətbiq edir
  idarəetmə redaktələrini deterministik saxlayın.

## Konfiqurasiya

Yeni hesablama konfiqurasiyası `crates/iroha_config/src/parameters`-də yaşayır:

- İstifadəçi görünüşü: `Compute` (`user.rs`) env ləğvetmələri ilə:
  - `COMPUTE_ENABLED` (defolt `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Qiymətləndirmə/iqtisadiyyat: `compute.economics` çəkir
  `max_cu_per_call`/`max_amplification_ratio`, ödəniş bölgüsü, sponsor qalıqları
  (zəng başına və gündəlik Kİ), qiymət ailəsinin əsas göstəriciləri + risk sinifləri/hüdudları
  idarəetmə yeniləmələri və icra sinfi çarpanları (GPU/TEE/best-fort).
- Faktiki/defolt: `actual.rs` / `defaults.rs::compute` təhlil edilmiş ifşa
  `Compute` parametrləri (ad boşluqları, profillər, qiymət ailələri, sandbox).
- Yanlış konfiqurasiyalar (boş ad məkanları, defolt profil/ailə yoxdur, TTL qapağı
  inversiyalar) təhlil zamanı `InvalidComputeConfig` kimi görünür.

## Testlər və qurğular

- Deterministik köməkçilər (`request_hash`, qiymətlər) və armatur səfərləri burada yaşayır
  `crates/iroha_data_model/src/compute/mod.rs` (bax `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON qurğuları `fixtures/compute/`-də yaşayır və məlumat modeli tərəfindən həyata keçirilir
  reqressiya əhatəsi üçün testlər.

## SLO qoşqu və büdcələri- `compute.slo.*` konfiqurasiyası şlüz SLO düymələrini (uçuş növbəsi) ifşa edir
  dərinlik, RPS qapağı və gecikmə hədəfləri).
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Defolt: 32
  uçuş zamanı, marşrut başına 512 növbə, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- SLO xülasələrini və sorğu/çıxışı əldə etmək üçün yüngül dəzgah kəmərini işə salın
  snapshot: `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iterasiyalar] [koncurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 iterasiya, paralellik 16, çıxışlar altında
  `artifacts/compute_gateway/bench_summary.{json,md}`). Skamya istifadə edir
  deterministik faydalı yüklər (`fixtures/compute/payload_compute_payments.json`) və
  məşq zamanı təkrar toqquşmaların qarşısını almaq üçün hər sorğu başlıqları
  `echo`/`uppercase`/`sha3` giriş nöqtələri.

## SDK/CLI paritet qurğuları

- Kanonik qurğular `fixtures/compute/` altında yaşayır: manifest, zəng, faydalı yük və
  şlüz tipli cavab/qəbz tərtibatı. Yük həşləri zəngə uyğun olmalıdır
  `request.payload_hash`; köməkçi yükü yaşayır
  `fixtures/compute/payload_compute_payments.json`.
- CLI gəmiləri `iroha compute simulate` və `iroha compute invoke`:

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

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` yaşayır
  altında reqressiya testləri ilə `javascript/iroha_js/src/compute.js`
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` eyni qurğuları yükləyir, faydalı yük həşlərini təsdiqləyir,
  və testlərlə giriş nöqtələrini simulyasiya edir
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift köməkçilərinin hamısı eyni Norito qurğularını paylaşır ki, SDK-lar
  a vurmadan sorğu tikinti və hash idarə offline doğrulayın
  çalışan qapı.