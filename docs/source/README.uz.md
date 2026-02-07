---
lang: uz
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama Hujjatlar indeksi

Ushbu indeks IVM, Kotodama va IVM-birinchi quvur liniyasi uchun asosiy dizayn va ma'lumotnoma hujjatlarini bog'laydi. Đ18NI00000023X (./README.ja.md)

- IVM arxitekturasi va til xaritasi: `../../ivm.md`
- IVM tizimi chaqiruvi ABI: `ivm_syscalls.md`
- Yaratilgan tizim konstantalari: `ivm_syscalls_generated.md` (yangilash uchun `make docs-syscalls` ni ishga tushiring)
- IVM bayt kodi sarlavhasi: `ivm_header.md`
- Kotodama grammatika va semantika: `kotodama_grammar.md`
- Kotodama misollari va tizim diagrammasi: `kotodama_examples.md`
- Tranzaksiya quvuri (IVM‑birinchi): `../../new_pipeline.md`
- Torii Contracts API (manifestlar): `torii_contracts_api.md`
- Universal hisob/UAID operatsiyalari bo'yicha qo'llanma: `universal_accounts_guide.md`
- JSON so'rov konverti (CLI / asboblar): `query_json.md`
- Norito oqim moduli ma'lumotnomasi: `norito_streaming.md`
- Ish vaqti ABI namunalari: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (ilovalar, prover, ovozlar soni): `zk_app_api.md`
- Torii ZK qo'shimchalari/prover runbook: `zk/prover_runbook.md`
- Torii ZK App API operator qo'llanmasi (ilovalar/prover; sandiq hujjati): `../../crates/iroha_torii/docs/zk_app_api.md`
- VK/proof hayot aylanishi (ro'yxatga olish, tekshirish, telemetriya): `zk/lifecycle.md`
- Torii Operator yordamchilari (ko'rinish uchun oxirgi nuqtalar): `references/operator_aids.md`
- Nexus standart chiziqli tezkor ishga tushirish: `quickstart/default_lane.md`
- MOCHI supervayzerining tezkor ishga tushirilishi va arxitekturasi: `mochi/index.md`
- JavaScript SDK qo'llanmalari (tezkor ishga tushirish, konfiguratsiya, nashr qilish): `sdk/js/index.md`
- Swift SDK pariteti/CI asboblar paneli: `references/ios_metrics.md`
- Boshqaruv: `../../gov.md`
- Domenni tasdiqlash (qo'mitalar, siyosatlar, tekshirish): `domain_endorsements.md`
- JDG attestatsiyalari (oflayn tekshirish vositalari): `jdg_attestations.md`
- Tushuntirishni muvofiqlashtirish bo'yicha ko'rsatmalar: `coordination_llm_prompts.md`
- Yo'l xaritasi: `../../roadmap.md`
- Docker quruvchi tasviridan foydalanish: `docker_build.md`

Foydalanish bo'yicha maslahatlar
- Tashqi asboblar (`koto_compile`, `ivm_run`) yordamida `examples/` da misollar yaratish va ishga tushirish:
  - `make examples-run` (va agar `ivm_tool` mavjud bo'lsa, `make examples-inspect`)
- Misollar va sarlavha tekshiruvlari uchun ixtiyoriy integratsiya testlari (sukut bo'yicha e'tiborga olinmaydi) `integration_tests/tests/` da ishlaydi.Quvur liniyasi konfiguratsiyasi
- Ish vaqtining barcha xatti-harakatlari `iroha_config` fayllari orqali sozlangan. Atrof-muhit o'zgaruvchilari operatorlar uchun ishlatilmaydi.
- Sensible sukut taqdim etiladi; ko'pgina joylashtirishlar o'zgartirishni talab qilmaydi.
- `[pipeline]` ostida tegishli kalitlar:
  - `dynamic_prepass`: kirish to'plamlarini olish uchun IVM faqat o'qish uchun oldindan o'tishni yoqing (standart: rost).
  - `access_set_cache_enabled`: `(code_hash, entrypoint)` uchun keshdan olingan kirish to'plamlari; maslahatlarni disk raskadrovka qilishni o'chirib qo'ying (standart: rost).
  - `parallel_overlay`: parallel ravishda qoplamalarni qurish; commit deterministik bo'lib qoladi (standart: true).
  - `gpu_key_bucket`: `(key, tx_idx, rw_flag)` da barqaror radisdan foydalangan holda rejalashtiruvchi oldindan o'tish uchun kalitlarni ixtiyoriy paqirlash; deterministik protsessorni qayta tiklash har doim faol (standart: noto'g'ri).
  - `cache_size`: global IVM dekodlashdan oldingi kesh hajmi (dekodlangan oqimlar saqlanadi). Standart: 128. Ko'paytirish takroriy bajarilishlar uchun dekodlash vaqtini qisqartirishi mumkin.

Hujjatlarni sinxronlashtirish tekshiruvlari
- Syscall konstantalari (docs/source/ivm_syscalls_generated.md)
  - Qayta tiklash: `make docs-syscalls`
  - Faqat tekshirish: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI jadvali (crates/ivm/docs/syscalls.md)
  - Faqat tekshirish: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Yangilangan bo'lim (va kodlar jadvali): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer‑ABI jadvallari (crates/ivm/docs/pointer_abi.md va ivm.md)
  - Faqat tekshirish: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Yangilash bo'limlari: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM sarlavha siyosati va ABI xeshlari (docs/source/ivm_header.md)
  - Faqat tekshirish: `cargo run -p ivm --bin gen_header_doc -- --check` va `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Yangilash bo'limlari: `cargo run -p ivm --bin gen_header_doc -- --write` va `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions ish jarayoni `.github/workflows/check-docs.yml` bu tekshiruvlarni har bir surish/PRda amalga oshiradi va agar yaratilgan hujjatlar amalga oshirishdan chetga chiqsa, muvaffaqiyatsiz bo'ladi.
- [Boshqaruv kitobi](governance_playbook.md)