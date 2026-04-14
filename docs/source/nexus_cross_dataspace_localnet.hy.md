<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Cross-Dataspace Localnet Proof

Այս runbook-ը կատարում է Nexus ինտեգրման ապացույցը, որ.

- գործարկում է 4-հասակակից լոկալ ցանց երկու սահմանափակ անձնական տվյալների տարածություններով (`ds1`, `ds2`),
- երթուղիներ է բերում հաշվի երթևեկությունը յուրաքանչյուր տվյալների տարածք,
- ստեղծում է ակտիվ յուրաքանչյուր տվյալների տարածքում,
- իրականացնում է ատոմային փոխանակման կարգավորում երկու ուղղություններով տվյալների տարածքներում,
- ապացուցում է վերադարձի իմաստաբանությունը՝ ներկայացնելով թերֆինանսավորվող ոտք և ստուգելով մնացորդները անփոփոխ:

Կանոնական թեստը հետևյալն է.
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Արագ վազում

Օգտագործեք wrapper սցենարը պահեստի արմատից.

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Կանխադրված վարքագիծ.

- գործարկում է միայն տվյալների խաչաձեւ տարածության ապացուցման թեստը,
- սահմանում է `NORITO_SKIP_BINDINGS_SYNC=1`,
- սահմանում է `IROHA_TEST_SKIP_BUILD=1`,
- օգտագործում է `--test-threads=1`,
- անցնում է `--nocapture`:

## Օգտակար տարբերակներ

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs`-ը պահում է ժամանակավոր գործընկերային գրացուցակներ (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) դատաբժշկական փորձաքննության համար:
- `--all-nexus`-ը գործարկում է `mod nexus::` (ամբողջական Nexus ինտեգրման ենթաբազմություն), ոչ միայն ապացուցման թեստը:

## CI դարպաս

CI օգնական.

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Դարձնել թիրախ.

```bash
make check-nexus-cross-dataspace
```

Այս դարպասը գործարկում է դետերմինիստական ապացույցի փաթաթան և ձախողում է աշխատանքը, եթե տվյալների միջակայքը ատոմային
փոխանակման սցենարը հետընթաց է ապրում:

## Ձեռնարկի համարժեք հրամաններ

Նպատակային ապացույցի թեստ.

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Ամբողջական Nexus ենթաբազմություն.

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Ակնկալվող ապացույցների ազդանշաններ-Թեստն անցնում է։
- Մեկ ակնկալվող նախազգուշացում է հայտնվում թերֆինանսավորման միտումնավոր ձախողման համար.
  `settlement leg requires 10000 but only ... is available`.
- Վերջնական մնացորդի պնդումները հաջողվում են այն բանից հետո, երբ.
  - հաջող փոխադարձ փոխանակում,
  - հաջող հակադարձ փոխանակում,
  - ձախողված թերֆինանսավորվող սվոպ (վերադարձ անփոփոխ մնացորդներ):

## Ընթացիկ վավերացման լուսանկար

**2026 թվականի փետրվարի 19-ի դրությամբ** այս աշխատանքային հոսքն անցել է.

- նպատակային փորձարկում՝ `1 passed; 0 failed`,
- ամբողջական Nexus ենթաբազմություն՝ `24 passed; 0 failed`: