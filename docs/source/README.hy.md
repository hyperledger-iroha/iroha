---
lang: hy
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama Փաստաթղթերի ինդեքս

Այս ինդեքսը կապում է IVM, Kotodama և IVM-առաջին խողովակաշարի հիմնական նախագծային և տեղեկատու փաստաթղթերը: 日本語訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM ճարտարապետություն և լեզվի քարտեզագրում՝ `../../ivm.md`
- IVM syscall ABI՝ `ivm_syscalls.md`
- Ստեղծված syscall հաստատուններ՝ `ivm_syscalls_generated.md` (գործարկեք `make docs-syscalls`՝ թարմացնելու համար)
- IVM բայթ կոդի վերնագիր՝ `ivm_header.md`
- Kotodama քերականություն և իմաստաբանություն՝ `kotodama_grammar.md`
- Kotodama օրինակներ և syscall քարտեզագրումներ՝ `kotodama_examples.md`
- Գործարքի խողովակաշար (IVM‑առաջին): `../../new_pipeline.md`
- Torii Contracts API (դրսևորումներ)՝ `torii_contracts_api.md`
- Ունիվերսալ հաշվի/UAID գործառնությունների ուղեցույց՝ `universal_accounts_guide.md`
- JSON հարցման ծրար (CLI / գործիքավորում): `query_json.md`
- Norito հոսքային մոդուլի հղում՝ `norito_streaming.md`
- Runtime ABI նմուշներ՝ `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (կցորդներ, պրովեր, ձայների հաշվարկ)՝ `zk_app_api.md`
- Torii ZK հավելվածներ/պրովերի մատյան՝ `zk/prover_runbook.md`
- Torii ZK App API օպերատորի ուղեցույց (կցորդներ/պրովեր; տուփի փաստաթուղթ)՝ `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/proof կյանքի ցիկլ (գրանցամատյան, ստուգում, հեռաչափություն)՝ `zk/lifecycle.md`
- Torii Օպերատորի օժանդակություն (տեսանելիության վերջնակետեր)՝ `references/operator_aids.md`
- Nexus լռելյայն արագ մեկնարկ՝ `quickstart/default_lane.md`
- MOCHI վերահսկիչի արագ մեկնարկ և ճարտարապետություն՝ `mochi/index.md`
- JavaScript SDK ուղեցույցներ (արագ մեկնարկ, կազմաձևում, հրապարակում)՝ `sdk/js/index.md`
- Swift SDK պարիտետ/CI վահանակներ՝ `references/ios_metrics.md`
- Կառավարում՝ `../../gov.md`
- Դոմենի հաստատումներ (հանձնաժողովներ, քաղաքականություն, վավերացում)՝ `domain_endorsements.md`
- JDG հավաստագրեր (անցանց վավերացման գործիքավորում)՝ `jdg_attestations.md`
- Պարզաբանումների համակարգման հուշումներ՝ `coordination_llm_prompts.md`
- Ճանապարհային քարտեզ՝ `../../roadmap.md`
- Docker շինարարի պատկերի օգտագործումը՝ `docker_build.md`

Օգտագործման խորհուրդներ
- Կառուցեք և գործարկեք օրինակներ `examples/`-ում՝ օգտագործելով արտաքին գործիքներ (`koto_compile`, `ivm_run`):
  - `make examples-run` (և `make examples-inspect`, եթե `ivm_tool` հասանելի է)
- Կամընտիր ինտեգրման թեստերը (անտեսվում են լռելյայնորեն) օրինակների և վերնագրերի ստուգումների համար՝ ապրում են `integration_tests/tests/`-ում:Խողովակաշարի կոնֆիգուրացիա
- Գործարկման ամբողջ պահվածքը կազմաձևված է `iroha_config` ֆայլերի միջոցով: Շրջակա միջավայրի փոփոխականները չեն օգտագործվում օպերատորների համար:
- Տրամադրվում են խելամիտ կանխադրումներ; տեղակայումների մեծ մասը փոփոխությունների կարիք չի ունենա:
- Համապատասխան ստեղներ `[pipeline]`-ի ներքո.
  - `dynamic_prepass`. միացրեք IVM միայն կարդալու նախնական անցումը՝ մուտքի հավաքածուներ ստանալու համար (կանխադրված՝ ճշմարիտ):
  - `access_set_cache_enabled`. քեշից ստացված մուտքի հավաքածուներ ըստ `(code_hash, entrypoint)`-ի; անջատել վրիպազերծման ակնարկները (կանխադրված՝ ճշմարիտ):
  - `parallel_overlay`. զուգահեռ կառուցել ծածկույթներ; commit-ը մնում է դետերմինիստական ​​(կանխադրված՝ ճշմարիտ):
  - `gpu_key_bucket`. կամընտիր ստեղնաշարի դույլեր ժամանակացույցի նախնական անցման համար՝ օգտագործելով կայուն ռադիքս `(key, tx_idx, rw_flag)`-ում; դետերմինիստական ​​CPU-ի հետադարձ կապը միշտ ակտիվ է (լռելյայն՝ կեղծ):
  - `cache_size`. գլոբալ IVM նախնական վերծանման քեշի հզորությունը (ապակոդավորված հոսքերը պահպանվում են): Կանխադրված՝ 128. Մեծացումը կարող է կրճատել վերծանման ժամանակը կրկնվող կատարումների համար:

Փաստաթղթերի համաժամացման ստուգումներ
- Syscall հաստատուններ (docs/source/ivm_syscalls_generated.md)
  - Վերականգնել՝ `make docs-syscalls`
  - Ստուգեք միայն՝ `bash scripts/check_syscalls_doc.sh`
- Syscall ABI աղյուսակ (crates/ivm/docs/syscalls.md)
  - Ստուգեք միայն՝ `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Թարմացրեք ստեղծված բաժինը (և կոդերի փաստաթղթերի աղյուսակը)՝ `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer-ABI աղյուսակներ (crates/ivm/docs/pointer_abi.md և ivm.md)
  - Ստուգեք միայն՝ `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Թարմացրեք բաժինները՝ `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM վերնագրի քաղաքականություն և ABI հեշեր (docs/source/ivm_header.md)
  - Ստուգեք միայն՝ `cargo run -p ivm --bin gen_header_doc -- --check` և `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Թարմացրեք բաժինները՝ `cargo run -p ivm --bin gen_header_doc -- --write` և `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions-ի աշխատանքային հոսքը `.github/workflows/check-docs.yml`-ն իրականացնում է այս ստուգումները յուրաքանչյուր push/PR-ում և չի հաջողվի, եթե ստեղծվող փաստաթղթերը հեռացնեն իրականացումից:
- [Կառավարման գրքույկ] (governance_playbook.md)
