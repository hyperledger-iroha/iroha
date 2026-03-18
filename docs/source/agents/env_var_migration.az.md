---
lang: az
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Konfiqurasiya Miqrasiya İzləyicisi

Bu izləyici istehsalla bağlı ətraf mühit dəyişkənliyinin aşkar edilmiş keçidlərini ümumiləşdirir
`docs/source/agents/env_var_inventory.{json,md}` və nəzərdə tutulan miqrasiya ilə
`iroha_config`-ə yol (və ya açıq inkişaf/yalnız test əhatəsi).


Qeyd: `ci/check_env_config_surface.sh` indi yeni **istehsal** env olduqda uğursuz olur
`ENV_CONFIG_GUARD_ALLOW=1` olmadığı halda şimlər `AGENTS_BASE_REF`-ə nisbətən görünür
dəst; ləğvetmədən istifadə etməzdən əvvəl qəsdən əlavələri burada sənədləşdirin.

## Tamamlanmış köçlər- **IVM ABI-dən çıxmaq** — Silindi `IVM_ALLOW_NON_V1_ABI`; kompilyator indi rədd edir
  səhv yolunu qoruyan vahid testi ilə qeyd-şərtsiz qeyri-v1 ABI.
- **IVM debug banner env shim** — `IVM_SUPPRESS_BANNER` env-dən imtina etdi;
  bannerin sıxışdırılması proqramatik təyinedici vasitəsilə əlçatan olaraq qalır.
- **IVM keş/ölçü** — Yivli keş/prover/GPU ölçüləri vasitəsilə
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) və iş vaxtı env şimlərini çıxarın. Ev sahibləri indi zəng edirlər
  `ivm::ivm_cache::configure_limits` və `ivm::zk::set_prover_threads`, istifadə testləri
  env əvəzləmə əvəzinə `CacheLimitsGuard`.
- **Növbə kökünü birləşdirin** — `connect.queue.root` əlavə edildi (defolt:
  `~/.iroha/connect`) müştəri konfiqurasiyasına daxil edin və onu CLI və
  JS diaqnostikası. JS köməkçiləri konfiqurasiyanı (və ya açıq-aşkar `rootDir`) həll edir və
  yalnız `allowEnvOverride` vasitəsilə inkişaf/sınaqda `IROHA_CONNECT_QUEUE_ROOT`-i qiymətləndirin;
  şablonlar düyməni sənədləşdirir, belə ki, operatorlar artıq env ləğv etmələrinə ehtiyac duymurlar.
- **Izanami şəbəkəyə qoşulma** — Üçün açıq-aydın `allow_net` CLI/konfiqurasiya bayrağı əlavə edildi
  Izanami xaos aləti; çalışır indi `allow_net=true`/`--allow-net` tələb edir və
- **IVM banner səsi** — `IROHA_BEEP` env şimini konfiqurasiya ilə əvəz etdi
  `ivm.banner.{show,beep}` dəyişir (defolt: doğru/doğru). Başlanğıc banneri/bip
  məftil indi konfiqurasiyanı yalnız istehsalda oxuyur; dev/test hələ də şərəf yaradır
  manuel keçidlər üçün env ləğvi.
- **DA spool ləğvi (yalnız testlər)** — `IROHA_DA_SPOOL_DIR` ləğvi indidir
  `cfg(test)` köməkçilərinin arxasında hasarlanmış; istehsal kodu həmişə makaradan qaynaqlanır
  konfiqurasiyadan gələn yol.
- **Kripto intrinsics** — `IROHA_DISABLE_SM_INTRINSICS` dəyişdirildi /
  `IROHA_ENABLE_SM_INTRINSICS` konfiqurasiya ilə idarə olunur
  `crypto.sm_intrinsics` siyasəti (`auto`/`force-enable`/`force-disable`) və
  `IROHA_SM_OPENSSL_PREVIEW` qoruyucunu çıxardı. Ev sahibləri siyasəti burada tətbiq edir
  başlanğıc, dəzgahlar/testlər `CRYPTO_SM_INTRINSICS` və OpenSSL vasitəsilə daxil ola bilər
  önizləmə indi yalnız konfiqurasiya bayrağına hörmət edir.
  Izanami artıq `--allow-net`/davamlı konfiqurasiya tələb edir və testlər indi buna əsaslanır
  mühit mühitini dəyişdirməkdən daha çox bu düymə.
- **FastPQ GPU tuning** — `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}` əlavə edildi
  konfiqurasiya düymələri (defolt: `None`/`None`/`false`/`false`/`false`) və onları CLI təhlili vasitəsilə keçirin
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` şimləri indi inkişaf/test ehtiyatları kimi davranır və
  konfiqurasiya yükləndikdə nəzərə alınmır (hətta konfiqurasiya onları təyin olunmamış qoysa da); sənədlər/inventar idi
  miqrasiyanı qeyd etmək üçün yeniləndi.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) indi paylaşılan proqram vasitəsilə sazlama/sınaq quruluşlarının arxasındadır.
  köməkçi, buna görə istehsal binariləri yerli diaqnostika üçün düymələri qoruyarkən onlara məhəl qoymur. Env
  inventar yalnız inkişaf/test sahəsini əks etdirmək üçün yenidən yaradıldı.- **FASTPQ qurğu yeniləmələri** — `FASTPQ_UPDATE_FIXTURES` indi yalnız FASTPQ inteqrasiyasında görünür
  testlər; istehsal mənbələri artıq env keçidini oxumur və inventar yalnız testi əks etdirir
  əhatə dairəsi.
- **İnventar yeniləməsi + əhatə dairəsinin aşkarlanması** — Env inventar aləti indi `build.rs` fayllarını aşağıdakı kimi teq edir
  əhatə dairəsini qurmaq və `#[cfg(test)]`/inteqrasiya qoşqu modullarını izləmək, beləliklə, yalnız test üçün keçid edir (məsələn,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) və CUDA qurma bayraqları istehsal sayından kənarda görünür.
  Env-config qoruyucu fərqini yaşıl saxlamaq üçün inventar 07 dekabr 2025-ci ildə (518 refs / 144 vars) bərpa edildi.
- **P2P topologiyası env şim buraxılış qoruyucusu** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` indi deterministi işə salır
  buraxılış quruluşlarında başlanğıc xətası (yalnız debug/testdə xəbərdar olun) belə ki, istehsal qovşaqları yalnız
  `network.peer_gossip_period_ms`. Env inventar mühafizəçi və əks etdirmək üçün bərpa edilmişdir
  yenilənmiş klassifikator indi `cfg!` ilə qorunan keçidləri debug/test kimi əhatə edir.

## Yüksək prioritet miqrasiya (istehsal yolları)

- _Heç biri (inventar cfg!/debug aşkarlanması ilə yenilənib; P2P şiminin bərkidilməsindən sonra env-config qoruyucusu yaşıl)._

## Yalnız inkişaf/test üçün hasara keçir

- Cari yoxlama (07 dekabr 2025-ci il): yalnız qurulan CUDA bayraqları (`IVM_CUDA_*`) `build` və
  qoşqu keçidləri (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) indi olaraq qeydiyyatdan keçin
  İnventarda olan `test`/`debug` (`cfg!` ilə qorunan şimlər daxil olmaqla). Əlavə hasar tələb olunmur;
  şimlər müvəqqəti olduqda TODO markerləri ilə `cfg(test)`/yalnız dəzgah köməkçilərinin arxasında gələcək əlavələri saxlayın.

## Quraşdırma müddəti (olduğu kimi buraxın)

- Yük/xüsusiyyət əhatələri (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD` və s.) qalır
  build-script narahatlıqları və icra zamanı konfiqurasiya miqrasiyası üçün əhatə dairəsindən kənardır.

## Növbəti hərəkətlər

1) Yeni istehsal env şimlərini tutmaq üçün konfiqurasiya səthi yeniləmələrindən sonra `make check-env-config-surface`-i işə salın
   erkən və alt sistem sahiblərini/ETA-ları təyin edin.  
2) Hər taramadan sonra inventarı yeniləyin (`make check-env-config-surface`).
   izləyici yeni qoruyucu barmaqlıqlarla uyğunlaşdırılır və env-konfiqurasiya qoruyucu fərqi səs-küysüz qalır.