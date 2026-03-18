---
lang: mn
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama Docs Index

Энэ индекс нь IVM, Kotodama, IVM-эхний шугам хоолойн үндсэн загвар, лавлагааны баримт бичгүүдийг холбодог. 日本訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM архитектур ба хэлний зураглал: `../../ivm.md`
- IVM Syscall ABI: `ivm_syscalls.md`
- Үүсгэсэн системийн дуудлагын тогтмолууд: `ivm_syscalls_generated.md` (шинэчлэхийн тулд `make docs-syscalls` ажиллуулна уу)
- IVM байт кодын гарчиг: `ivm_header.md`
- Kotodama дүрмийн болон семантик: `kotodama_grammar.md`
- Kotodama жишээ болон системийн зураглал: `kotodama_examples.md`
- Гүйлгээ дамжуулах хоолой (IVM‑эхний): `../../new_pipeline.md`
- Torii Гэрээний API (манифест): `torii_contracts_api.md`
- Universal account/UAID үйлдлийн гарын авлага: `universal_accounts_guide.md`
- JSON асуулгын дугтуй (CLI / багаж хэрэгсэл): `query_json.md`
- Norito урсгал модулийн лавлагаа: `norito_streaming.md`
- Ажиллах үеийн ABI дээж: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (хавсралт, нотолгоо, саналын тоо): `zk_app_api.md`
- Torii ZK хавсралт/prover runbook: `zk/prover_runbook.md`
- Torii ZK App API операторын гарын авлага (хавсралт/провер; хайрцагны баримт): `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/proof амьдралын мөчлөг (бүртгэл, баталгаажуулалт, телеметр): `zk/lifecycle.md`
- Torii Операторын туслах хэрэгсэл (харагдахуйц эцсийн цэгүүд): `references/operator_aids.md`
- Nexus анхдагч эгнээний хурдан эхлүүлэх: `quickstart/default_lane.md`
- MOCHI супервайзер хурдан эхлүүлэх ба архитектур: `mochi/index.md`
- JavaScript SDK гарын авлага (хурдан эхлүүлэх, тохиргоо, нийтлэх): `sdk/js/index.md`
- Swift SDK парити/CI хяналтын самбар: `references/ios_metrics.md`
- Засаглал: `../../gov.md`
- Домэйн баталгаажуулалт (хороонд, бодлого, баталгаажуулалт): `domain_endorsements.md`
- JDG баталгаажуулалт (офлайн баталгаажуулалтын хэрэгсэл): `jdg_attestations.md`
- Тодруулга зохицуулалтын сануулгууд: `coordination_llm_prompts.md`
- Замын зураг: `../../roadmap.md`
- Docker бүтээгчийн зургийн хэрэглээ: `docker_build.md`

Хэрэглэх зөвлөмжүүд
- Гадны хэрэгслүүдийг (`koto_compile`, `ivm_run`) ашиглан `examples/` дээр жишээ үүсгэж ажиллуулна уу:
  - `make examples-run` (мөн `ivm_tool` боломжтой бол `make examples-inspect`)
- Жишээ болон толгой хэсгийг шалгах нэмэлт интеграцийн тестүүд (анхдагчаар үл тоомсорлодог) `integration_tests/tests/` дээр ажилладаг.Дамжуулах хоолойн тохиргоо
- Ажиллах үеийн бүх горимыг `iroha_config` файлаар тохируулсан. Орчны хувьсагчдыг операторуудад ашигладаггүй.
- Мэдрэмжтэй өгөгдмөл үзүүлэлтүүдийг өгсөн; ихэнх байршуулалтад өөрчлөлт оруулах шаардлагагүй болно.
- `[pipeline]` доорх холбогдох түлхүүрүүд:
  - `dynamic_prepass`: хандалтын багцыг гаргахын тулд IVM зөвхөн уншигдах урьдчилсан дамжуулалтыг идэвхжүүлнэ (өгөгдмөл: үнэн).
  - `access_set_cache_enabled`: `(code_hash, entrypoint)`-ийн кэшээс авсан хандалтын багц; зөвлөмжийг дибаг хийхийг идэвхгүй болгох (өгөгдмөл: үнэн).
  - `parallel_overlay`: давхаргыг зэрэгцүүлэн бүтээх; commit нь тодорхойлогч хэвээр байна (анхдагч: үнэн).
  - `gpu_key_bucket`: `(key, tx_idx, rw_flag)` дээр тогтвортой радикс ашиглан хуваарь гаргагчийн урьдчилсан дамжуулалтын нэмэлт түлхүүр хуваалт; deterministic CPU-ийн буцаалт үргэлж идэвхтэй байдаг (анхдагч: худал).
  - `cache_size`: дэлхийн IVM код тайлахын өмнөх кэшийн багтаамж (код тайлагдсан урсгалуудыг хадгалсан). Өгөгдмөл: 128. Өсгөх нь давтан гүйцэтгэлийн код тайлах хугацааг багасгаж чадна.

Docs синк шалгалт
- Syscall тогтмолууд (docs/source/ivm_syscalls_generated.md)
  - Дахин сэргээх: `make docs-syscalls`
  - Зөвхөн шалгах: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI хүснэгт (crates/ivm/docs/syscalls.md)
  - Зөвхөн шалгах: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Үүсгэсэн хэсгийг шинэчлэх (мөн кодын баримтын хүснэгт): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Заагч-ABI хүснэгтүүд (crates/ivm/docs/pointer_abi.md болон ivm.md)
  - Зөвхөн шалгах: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Шинэчлэх хэсгүүд: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM толгойн бодлого болон ABI хэшүүд (docs/source/ivm_header.md)
  - Зөвхөн шалгах: `cargo run -p ivm --bin gen_header_doc -- --check` болон `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Шинэчлэх хэсгүүд: `cargo run -p ivm --bin gen_header_doc -- --write` болон `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Үйлдлийн ажлын урсгал `.github/workflows/check-docs.yml` нь түлхэх/PR бүрт эдгээр шалгалтуудыг хийдэг бөгөөд хэрэв үүсгэсэн баримтууд хэрэгжилтээс хазайвал амжилтгүй болно.
- [Засаглалын сурах бичиг](governance_playbook.md)
