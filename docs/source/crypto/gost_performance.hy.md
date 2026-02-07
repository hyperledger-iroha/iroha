---
lang: hy
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ԳՕՍՏ-ի կատարողականի աշխատանքային հոսք

Այս նշումը փաստում է, թե ինչպես ենք մենք հետևում և կիրառում կատարողականի ծրարը
TC26 ԳՕՍՏ ստորագրման հետնամաս.

## Վազում տեղական

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Կուլիսների հետևում երկու թիրախներն էլ կանչում են `scripts/gost_bench.sh`, որը.

1. Կատարում է `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`:
2. Աշխատում է `gost_perf_check`-ը `target/criterion`-ի դեմ՝ ստուգելով մեդիանները
   մուտքի ելակետ (`crates/iroha_crypto/benches/gost_perf_baseline.json`):
3. Ներարկում է Markdown ամփոփագիրը `$GITHUB_STEP_SUMMARY`-ում, երբ այն հասանելի է:

Հետադարձը/բարելավումը հաստատելուց հետո բազային գիծը թարմացնելու համար գործարկեք՝

```bash
make gost-bench-update
```

կամ ուղղակիորեն.

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh`-ը գործարկում է bench + checker-ը, վերագրում է բազային JSON-ը և տպում
նոր միջինները. Միշտ միացրեք թարմացված JSON-ը որոշման գրառման հետ մեկտեղ
`crates/iroha_crypto/docs/gost_backend.md`.

### Ընթացիկ հղման միջինները

| Ալգորիթմ | Միջին (մկվ) |
|----------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml`-ն օգտագործում է նույն սկրիպտը և նաև գործարկում է dudect-ի ժամանակի պահակը:
CI-ն ձախողվում է, երբ չափված մեդիանը գերազանցում է բազային գիծը ավելի շատ, քան կազմաձևված հանդուրժողականությունը
(20% լռելյայն) կամ երբ ժամանակի պահակը հայտնաբերում է արտահոսք, ուստի ռեգրեսիաներն ինքնաբերաբար բռնվում են:

## Ամփոփ ելք

`gost_perf_check`-ը տպում է համեմատության աղյուսակը տեղում և միևնույն բովանդակությունը կցում է դրան
`$GITHUB_STEP_SUMMARY`, ուստի CI-ի աշխատանքի տեղեկամատյանները և գործարկվող ամփոփագրերը կիսում են նույն թվերը: