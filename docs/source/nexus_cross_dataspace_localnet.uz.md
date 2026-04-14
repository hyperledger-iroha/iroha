<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Cross-Ma'lumotlar fazosi Localnet Proof

Ushbu runbook Nexus integratsiyasini tasdiqlaydi:

- ikkita cheklangan shaxsiy ma'lumotlar maydoni (`ds1`, `ds2`) bilan 4-peerli mahalliy tarmoqni ishga tushiradi,
- har bir ma'lumot maydoniga hisobdagi trafikni yo'naltiradi,
- har bir ma'lumot maydonida aktiv yaratadi,
- har ikki yo'nalishda ham ma'lumotlar maydonlari bo'ylab atom almashinuvini amalga oshiradi;
- kam moliyalashtirilgan oyog'ini taqdim etish va balanslarni tekshirish orqali orqaga qaytish semantikasini isbotlaydi.

Kanonik test:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Tez yugurish

O'ram skriptidan ombor ildizidan foydalaning:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Standart xatti-harakatlar:

- faqat o'zaro faoliyat ma'lumotlar fazosini isbotlash testini o'tkazadi,
- `NORITO_SKIP_BINDINGS_SYNC=1` to'plamlari,
- `IROHA_TEST_SKIP_BUILD=1` to'plamlari,
- `--test-threads=1` dan foydalanadi,
- `--nocapture` dan o'tadi.

## Foydali variantlar

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` sud ekspertizasi uchun vaqtinchalik peer kataloglarini (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) saqlaydi.
- `--all-nexus` faqat isbot testini emas, `mod nexus::` (to'liq Nexus integratsiya kichik to'plami) bilan ishlaydi.

## CI darvozasi

CI yordamchisi:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Maqsad qo'ying:

```bash
make check-nexus-cross-dataspace
```

Bu darvoza deterministik isbot o'ramini bajaradi va agar o'zaro ma'lumotlar fazosi atomik bo'lsa, ishni bajarmaydi.
almashtirish stsenariysi orqaga suriladi.

## Qo'lda ekvivalent buyruqlar

Maqsadli isbotlash testi:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

To'liq Nexus kichik to'plami:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Kutilayotgan isbot signallari- Test o'tadi.
- Bir kutilgan ogohlantirish qasddan muvaffaqiyatsiz to'lanmagan hisob-kitoblar uchun paydo bo'ladi:
  `settlement leg requires 10000 but only ... is available`.
- Yakuniy balansni tasdiqlash quyidagi hollarda muvaffaqiyatli bo'ladi:
  - muvaffaqiyatli forvard almashinuvi,
  - muvaffaqiyatli teskari almashtirish,
  - muvaffaqiyatsiz moliyalashtirilmagan svop (o'zgarmagan qoldiqlarni qaytarish).

## Joriy tasdiqlash surati

**2026-yil 19-fevral** holatiga ko‘ra, ushbu ish jarayoni quyidagi bilan o‘tdi:

- maqsadli test: `1 passed; 0 failed`,
- to'liq Nexus to'plami: `24 passed; 0 failed`.