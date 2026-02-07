---
lang: hy
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] Վերանայեք ապարատային բնութագրերը՝ 8+ միջուկներ, 16 ԳԲ RAM, NVMe պահեստավորում ≥ 500 ՄԲ/վ:
- [ ] Հաստատեք երկու IPv4 + IPv6 հասցեներ և վերին հոսքի թույլտվությունները QUIC/UDP 443:
- [ ] Տրամադրել HSM կամ հատուկ ապահով անկլավ ռելեի նույնականացման բանալիների համար:
- [ ] Համաժամեցեք կանոնական անջատման կատալոգը (`governance/compliance/soranet_opt_outs.json`):
- [ ] Միավորել համապատասխանության բլոկը նվագախմբի կազմաձևում (տես `03-config-example.toml`):
- [ ] Վերցրեք իրավասության/համաշխարհային համապատասխանության հավաստագրերը և լրացրեք `attestations` ցուցակը:
- [ ] Գործարկեք `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json`-ը (կամ ձեր ուղիղ պատկերը) և վերանայեք անցողիկ/անհաջող հաշվետվությունը:
- [ ] Ստեղծեք ռելե ընդունելության ԿՍՊ և ստացեք կառավարման կողմից ստորագրված ծրար:
- [ ] Ներմուծեք պահակային նկարագրիչի սերմերը և ստուգեք հրապարակված հեշ շղթայի դեմ:
- [ ] Գործարկեք `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`:
- [ ] Չոր գործարկվող հեռաչափության արտահանում. ապահովել, որ Prometheus քերծվածքը հաջողվի տեղում:
- [ ] Պլանավորեք փորված փորված պատուհանը և գրանցեք էսկալացիայի կոնտակտները:
- [ ] Ստորագրեք փորված ապացույցների փաթեթը՝ `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`: