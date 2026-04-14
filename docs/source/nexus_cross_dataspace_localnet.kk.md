<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

№ Nexus Деректер кеңістігіндегі жергілікті желі дәлелі

Бұл runbook Nexus біріктіру дәлелін орындайды:

- екі шектеулі жеке деректер кеңістігі (`ds1`, `ds2`) бар 4 деңгейлі жергілікті желіні жүктейді.
- әрбір деректер кеңістігіне есептік трафикті бағыттайды,
- әрбір деректер кеңістігінде актив жасайды,
- екі бағытта да деректер кеңістігінде атомдық своп есеп айырысуды орындайды,
- аз қаржыландырылған тармақты жіберу және теңгерімді тексеру арқылы кері семантиканы дәлелдейді.

Канондық сынақ дегеніміз:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Жылдам жүгіру

Репозиторий түбірінен орауыш сценарийін пайдаланыңыз:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Әдепкі әрекет:

- тек кросс-деректер кеңістігінің дәлелдеу сынағымен жұмыс істейді,
- `NORITO_SKIP_BINDINGS_SYNC=1` жиынтықтары,
- `IROHA_TEST_SKIP_BUILD=1` жиынтықтары,
- `--test-threads=1` пайдаланады,
- `--nocapture` тапсырады.

## Пайдалы опциялар

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` криминалистика үшін уақытша тең анықтамалықтарды (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) сақтайды.
- `--all-nexus` тек дәлелдеу сынағы емес, `mod nexus::` (толық Nexus интеграциялық жиын) іске қосады.

## CI қақпасы

CI көмекшісі:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Мақсат қою:

```bash
make check-nexus-cross-dataspace
```

Бұл қақпа детерминирленген дәлелдеу орауышын орындайды және егер кросс-деректер кеңістігі атомдық
своп сценарийі регрессия.

## Қолмен эквивалентті пәрмендер

Мақсатты дәлелдеу сынағы:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Толық Nexus жиыны:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Күтілетін дәлелдеу сигналдары- Тест тапсырады.
- Бір күтілетін ескерту әдейі сәтсіздікке ұшыраған жеткіліксіз қаржыландырылған есеп айырысу кезеңі үшін пайда болады:
  `settlement leg requires 10000 but only ... is available`.
- Қорытынды теңгерім растаулары келесі жағдайларда сәтті болады:
  - сәтті форвард своп,
  - сәтті кері своп,
  - сәтсіз қаржыландырылмаған своп (өзгеріссіз қалдықтарды кері қайтару).

## Ағымдағы тексеру суреті

**2026 жылдың 19 ақпанында** бұл жұмыс процесі келесімен өтті:

- мақсатты тест: `1 passed; 0 failed`,
- толық Nexus жиыны: `24 passed; 0 failed`.