---
lang: az
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama Sənəd İndeksi

Bu indeks IVM, Kotodama və IVM-birinci boru kəməri üçün əsas dizayn və istinad sənədlərini əlaqələndirir. 訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM memarlıq və dil xəritəsi: `../../ivm.md`
- IVM sistem zəngi ABI: `ivm_syscalls.md`
- Yaradılmış sistem çağırışı sabitləri: `ivm_syscalls_generated.md` (yeniləmək üçün `make docs-syscalls`-i işə salın)
- IVM bayt kodu başlığı: `ivm_header.md`
- Kotodama qrammatika və semantika: `kotodama_grammar.md`
- Kotodama nümunələri və sistem zəngi xəritələri: `kotodama_examples.md`
- Əməliyyat boru kəməri (IVM‑birinci): `../../new_pipeline.md`
- Torii Müqavilələr API (manifestlər): `torii_contracts_api.md`
- Universal hesab/UAID əməliyyat təlimatı: `universal_accounts_guide.md`
- JSON sorğu zərfi (CLI / alətlər): `query_json.md`
- Norito axın modulu arayışı: `norito_streaming.md`
- İcra zamanı ABI nümunələri: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (qoşmalar, sübut, səslərin sayı): `zk_app_api.md`
- Torii ZK əlavələri/prover runbook: `zk/prover_runbook.md`
- Torii ZK App API operator təlimatı (qoşmalar/prover; sandıq sənədi): `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/proof həyat dövrü (reyestr, yoxlama, telemetriya): `zk/lifecycle.md`
- Torii Operator Köməkçiləri (görünmə üçün son nöqtələr): `references/operator_aids.md`
- Nexus standart zolaqlı sürətli başlanğıc: `quickstart/default_lane.md`
- MOCHI supervayzerinin sürətli başlanğıcı və arxitekturası: `mochi/index.md`
- JavaScript SDK təlimatları (sürətli başlanğıc, konfiqurasiya, nəşr): `sdk/js/index.md`
- Swift SDK pariteti/CI tablosları: `references/ios_metrics.md`
- İdarəetmə: `../../gov.md`
- Domen təsdiqləri (komitələr, siyasətlər, doğrulama): `domain_endorsements.md`
- JDG sertifikatları (oflayn doğrulama alətləri): `jdg_attestations.md`
- Aydınlaşdırma koordinasiya tələbləri: `coordination_llm_prompts.md`
- Yol xəritəsi: `../../roadmap.md`
- Docker qurucu təsvirindən istifadə: `docker_build.md`

İstifadə məsləhətləri
- Xarici alətlərdən (`koto_compile`, `ivm_run`) istifadə edərək `examples/`-də nümunələr yaradın və işlədin:
  - `make examples-run` (və `ivm_tool` varsa, `make examples-inspect`)
- Nümunələr və başlıq yoxlamaları üçün əlavə inteqrasiya testləri (standart olaraq nəzərə alınmır) `integration_tests/tests/`-də işləyir.Boru kəmərinin konfiqurasiyası
- Bütün iş zamanı davranışı `iroha_config` faylları vasitəsilə konfiqurasiya edilir. Ətraf mühit dəyişənləri operatorlar üçün istifadə edilmir.
- Ağıllı defoltlar təmin edilir; əksər yerləşdirmələr dəyişikliklərə ehtiyac duymayacaq.
- `[pipeline]` altında müvafiq açarlar:
  - `dynamic_prepass`: giriş dəstlərini əldə etmək üçün IVM yalnız oxunur ön keçidi aktivləşdirin (defolt: doğru).
  - `access_set_cache_enabled`: `(code_hash, entrypoint)` üçün keşdən əldə edilmiş giriş dəstləri; göstərişləri sazlamaq üçün deaktiv edin (defolt: doğru).
  - `parallel_overlay`: paralel olaraq örtüklər qurmaq; commit deterministik olaraq qalır (defolt: doğru).
  - `gpu_key_bucket`: `(key, tx_idx, rw_flag)`-də sabit radixdən istifadə edərək planlayıcının hazır keçidi üçün isteğe bağlı açar qutusu; deterministik CPU geri qaytarılması həmişə aktivdir (defolt: yanlış).
  - `cache_size`: qlobal IVM deşifrədən öncəki keşin tutumu (şifrlənmiş axınlar saxlanılır). Defolt: 128. Artırma təkrar icralar üçün deşifrə vaxtını azalda bilər.

Sənədlərin sinxronizasiyasını yoxlayır
- Syscall sabitləri (docs/source/ivm_syscalls_generated.md)
  - Yenidən yarat: `make docs-syscalls`
  - Yalnız yoxlayın: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI cədvəli (cates/ivm/docs/syscalls.md)
  - Yalnız yoxlayın: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Yeni yaradılan bölmə (və kod sənədləri cədvəli): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer‑ABI cədvəlləri (cates/ivm/docs/pointer_abi.md və ivm.md)
  - Yalnız yoxlayın: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Yeniləmə bölmələri: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM başlıq siyasəti və ABI hashları (docs/source/ivm_header.md)
  - Yalnız yoxlayın: `cargo run -p ivm --bin gen_header_doc -- --check` və `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Yeniləmə bölmələri: `cargo run -p ivm --bin gen_header_doc -- --write` və `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Fəaliyyətləri iş axını `.github/workflows/check-docs.yml` bu yoxlamaları hər təkan/PR-də həyata keçirir və yaradılan sənədlər icradan kənara çıxsa uğursuz olacaq.
- [İdarəetmə Kitabı](governance_playbook.md)
