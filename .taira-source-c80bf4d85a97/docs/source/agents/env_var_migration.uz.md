---
lang: uz
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Migratsiya kuzatuvchisini sozlash

Ushbu treker ishlab chiqarishga qaratilgan atrof-muhit o'zgaruvchan o'zgarishlarni umumlashtiradi
`docs/source/agents/env_var_inventory.{json,md}` tomonidan va mo'ljallangan migratsiya
`iroha_config` ga yo'l (yoki aniq ishlab chiquvchi/faqat sinov qamrovi).


Eslatma: `ci/check_env_config_surface.sh` endi yangi **ishlab chiqarish** envda ishlamay qoladi
Shimlar `AGENTS_BASE_REF` ga nisbatan paydo bo'ladi, agar `ENV_CONFIG_GUARD_ALLOW=1` bo'lmasa
to'plam; bekor qilishni ishlatishdan oldin bu yerda qasddan qoʻshimchalarni hujjatlashtiring.

## Tugallangan migratsiya- **IVM ABI rad etish** — `IVM_ALLOW_NON_V1_ABI` olib tashlandi; kompilyator endi rad etadi
  xato yo'lini himoya qiluvchi birlik testi bilan shartsiz v1 bo'lmagan ABI.
- **IVM disk raskadrovka banneri env shim** — `IVM_SUPPRESS_BANNER` env dan voz kechish bekor qilindi;
  bannerni bostirish dasturiy sozlagich orqali mavjud bo'lib qoladi.
- **IVM kesh/oʻlchami** — tishli kesh/prover/GPU oʻlchami orqali
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) va ish vaqti env shimlarini olib tashladi. Xostlar endi qo'ng'iroq qilishmoqda
  `ivm::ivm_cache::configure_limits` va `ivm::zk::set_prover_threads`, testlardan foydalanish
  Env bekor qilish o'rniga `CacheLimitsGuard`.
- ** Navbat ildizini ulash** — `connect.queue.root` qo‘shildi (standart:
  `~/.iroha/connect`) mijoz konfiguratsiyasiga o'tkazing va uni CLI va
  JS diagnostikasi. JS yordamchilari konfiguratsiyani (yoki aniq `rootDir`) hal qiladi va
  faqat `allowEnvOverride` orqali ishlab/sinovda `IROHA_CONNECT_QUEUE_ROOT` ni hurmat qiling;
  andozalar tugmani hujjatlashtiradi, shuning uchun operatorlar endi env bekor qilishlariga muhtoj emas.
- **Izanami tarmog‘iga kirish** — Aniq `allow_net` CLI/config bayrog‘i qo‘shildi
  Izanami xaos vositasi; ishga tushirish uchun endi `allow_net=true`/`--allow-net` talab qilinadi va
- **IVM banner signali** — `IROHA_BEEP` env simi konfiguratsiyaga asoslangan holda almashtirildi
  `ivm.banner.{show,beep}` almashtiriladi (standart: rost/true). Ishga tushirish banneri/bip
  simlar endi konfiguratsiyani faqat ishlab chiqarishda o'qiydi; dev/test quradi hali ham hurmat
  qo'lda almashtirishlar uchun env bekor qilish.
- **DA spoolni bekor qilish (faqat testlarda)** — `IROHA_DA_SPOOL_DIR` bekor qilish endi
  `cfg(test)` yordamchilari orqasida o'ralgan; ishlab chiqarish kodi har doim g'altakning manbai hisoblanadi
  konfiguratsiyadan yo'l.
- **Kripto intrinsics** - `IROHA_DISABLE_SM_INTRINSICS` almashtirildi /
  `IROHA_ENABLE_SM_INTRINSICS` konfiguratsiyaga asoslangan
  `crypto.sm_intrinsics` siyosati (`auto`/`force-enable`/`force-disable`) va
  `IROHA_SM_OPENSSL_PREVIEW` qo'riqchisini olib tashladi. Xostlar siyosatni quyidagi manzilda qo'llaydi:
  ishga tushirish, dastgohlar/testlar `CRYPTO_SM_INTRINSICS` va OpenSSL orqali ulanishi mumkin
  oldindan ko'rish endi faqat konfiguratsiya bayrog'ini hurmat qiladi.
  Izanami allaqachon `--allow-net`/doimiy konfiguratsiyani talab qiladi va sinovlar endi
  muhit muhitini almashtirishdan ko'ra bu tugma.
- **FastPQ GPU sozlash** — `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}` qo‘shildi
  konfiguratsiya tugmalari (standart: `None`/`None`/`false`/`false`/`false`) va ularni CLI tahlili orqali oʻtkazing.
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` shimlari endi ishlab chiquvchi/sinov zaxiralari sifatida ishlaydi va
  konfiguratsiya yuklangandan keyin e'tiborga olinmaydi (hatto konfiguratsiya ularni o'rnatmagan bo'lsa ham); hujjatlar/inventar edi
  migratsiyani belgilash uchun yangilandi.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) endi birgalikda disk raskadrovka/sinov tuzilmalari orqasida joylashgan.
  yordamchi, shuning uchun ishlab chiqarish ikkiliklari mahalliy diagnostika uchun tugmachalarni saqlab, ularga e'tibor bermaydi. Env
  inventarizatsiya faqat ishlab chiquvchi/sinov doirasini aks ettirish uchun qayta tiklandi.- **FASTPQ armatura yangilanishlari** — `FASTPQ_UPDATE_FIXTURES` endi faqat FASTPQ integratsiyasida paydo bo'ladi
  testlar; ishlab chiqarish manbalari endi env almashtirishni o'qimaydi va inventar faqat sinovni aks ettiradi
  qamrovi.
- **Inventarizatsiyani yangilash + qamrovni aniqlash** — Env inventarizatsiya vositalari endi `build.rs` fayllarini quyidagicha teglar.
  `#[cfg(test)]`/integratsiya jabduqlar modullarini yaratish va treklarni yaratish, shuning uchun faqat sinov o'tkaziladi (masalan,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) va CUDA qurish bayroqlari ishlab chiqarish sonidan tashqarida ko'rinadi.
  Env-config himoyasi farqini yashil rangda saqlash uchun inventarizatsiya 2025-yil 07-dekabrda qayta tiklandi (518 refs / 144 vars).
- **P2P topologiyasi env shim bo'shatish himoyasi** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` endi deterministikni ishga tushiradi
  Relizlar qurishda ishga tushirish xatosi (faqat disk raskadrovka/sinovda ogohlantirish), shuning uchun ishlab chiqarish tugunlari faqat quyidagilarga tayanadi.
  `network.peer_gossip_period_ms`. Env inventarizatsiyasi qo'riqchi va uni aks ettirish uchun qayta tiklandi
  yangilangan klassifikator endi disk raskadrovka/sinov sifatida `cfg!` himoyalangan almashuvlarni qamrab oladi.

## Yuqori ustuvor migratsiya (ishlab chiqarish yo'llari)

- _Yo'q (inventar cfg!/debug aniqlash bilan yangilandi; P2P shim qattiqlashgandan keyin env-config himoyasi yashil rangda)._

## Devorga o'tish uchun ishlab/faqat sinovdan o'tadi

- Joriy tozalash (2025-yil 07-dekabr): faqat tuziladigan CUDA bayroqlari (`IVM_CUDA_*`) `build` va
  jabduqlar (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) endi ro'yxatdan o'ting
  Inventarizatsiyadagi `test`/`debug` (jumladan, `cfg!` himoyalangan shimlar). Qo'shimcha to'siqlar kerak emas;
  Shimlar vaqtinchalik bo'lsa, kelajakdagi qo'shimchalarni TODO markerlari bilan `cfg(test)`/faqat dastgoh yordamchilari orqasida saqlang.

## Qurilish vaqti (hozirgidek qoldiring)

- Yuk/xususiyatlar envs (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD` va boshqalar) qoladi
  build-skript bilan bog'liq va ish vaqti konfiguratsiyasini ko'chirish doirasidan tashqarida.

## Keyingi harakatlar

1) `make check-env-config-surface` ni konfiguratsiya yuzasi yangilangandan so'ng ishga tushiring va yangi ishlab chiqarish env shimlarini ushlang
   erta va quyi tizim egalarini/ETAlarni tayinlang.  
2) Har bir tozalashdan keyin inventarni yangilang (`make check-env-config-surface`).
   treker yangi himoya panjaralari bilan bir xilda qoladi va env-config guard diff shovqinsiz qoladi.