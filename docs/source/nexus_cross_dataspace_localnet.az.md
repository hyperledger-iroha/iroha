<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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

Bu runbook Nexus inteqrasiya sübutunu yerinə yetirir ki:

- iki məhdud şəxsi məlumat məkanı (`ds1`, `ds2`) olan 4-peer lokal şəbəkəni işə salır,
- hər bir məlumat məkanına hesab trafiki marşrutları,
- hər bir məlumat məkanında aktiv yaradır,
- hər iki istiqamətdə məlumat məkanları arasında atom mübadiləsi hesablaşmasını həyata keçirir,
- kifayət qədər maliyyələşdirilməmiş bir ayağı təqdim etməklə və balansları yoxlamaqla, geriyə dönmə semantikasını sübut edir.

Kanonik test belədir:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Sürətli qaçış

Repozitor kökündən sarğı skriptindən istifadə edin:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Defolt davranış:

- yalnız cross-dataspace sübut testini həyata keçirir,
- dəstlər `NORITO_SKIP_BINDINGS_SYNC=1`,
- dəstlər `IROHA_TEST_SKIP_BUILD=1`,
- `--test-threads=1` istifadə edir,
- `--nocapture` keçir.

## Faydalı Seçimlər

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` məhkəmə ekspertizası üçün müvəqqəti həmyaşıd kataloqlarını (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) saxlayır.
- `--all-nexus` yalnız sübut testi deyil, `mod nexus::` (tam Nexus inteqrasiya alt dəsti) ilə işləyir.

## CI Qapısı

CI köməkçisi:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Hədəf qoyun:

```bash
make check-nexus-cross-dataspace
```

Bu qapı deterministik sübut sarğısını yerinə yetirir və çarpaz verilənlər məkanı atomik
mübadilə ssenarisi geriləyir.

## Manual Ekvivalent Əmrlər

Məqsədli sübut testi:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Tam Nexus alt dəsti:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Gözlənilən sübut siqnalları- Sınaqdan keçir.
- Bir gözlənilən xəbərdarlıq bilərəkdən uğursuz maliyyələşdirilməmiş hesablaşma mərhələsi üçün görünür:
  `settlement leg requires 10000 but only ... is available`.
- Yekun balans təsdiqləri aşağıdakılardan sonra uğur qazanır:
  - uğurlu forvard mübadiləsi,
  - uğurlu tərs mübadilə,
  - uğursuz maliyyələşdirilməmiş svop (dəyişməmiş qalıqların geri alınması).

## Cari Doğrulama Snapshot

**19 fevral 2026-cı il** tarixinə bu iş axını aşağıdakılarla keçdi:

- hədəflənmiş test: `1 passed; 0 failed`,
- tam Nexus alt dəsti: `24 passed; 0 failed`.