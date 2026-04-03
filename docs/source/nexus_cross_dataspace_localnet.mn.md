<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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

Энэхүү runbook нь Nexus интеграцийн баталгааг гүйцэтгэдэг:

- хоёр хязгаарлагдмал хувийн өгөгдлийн орон зай (`ds1`, `ds2`) бүхий 4 үет локал сүлжээг ачаална.
- өгөгдлийн орон зай бүрт дансны урсгалыг чиглүүлэх,
- өгөгдлийн орон зай бүрт хөрөнгө үүсгэх,
- өгөгдлийн орон зайд атомын солилцооны тооцоог хоёр чиглэлд гүйцэтгэдэг;
- Дутуу санхүүжүүлсэн хөлөө илгээж, үлдэгдлийг шалгах замаар буцаах семантикийг нотолж байна.

Каноник тест нь:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Түргэн гүйлт

Repository root-аас ороосон скриптийг ашиглана уу:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Өгөгдмөл зан байдал:

- зөвхөн өгөгдлийн орон зайг баталгаажуулах тестийг ажиллуулдаг,
- `NORITO_SKIP_BINDINGS_SYNC=1` багц,
- `IROHA_TEST_SKIP_BUILD=1` багц,
- `--test-threads=1` ашигладаг,
- `--nocapture` дамжуулдаг.

## Хэрэгтэй сонголтууд

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` нь шүүх эмнэлгийн зориулалттай түр зуурын лавлах (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) хадгалдаг.
- `--all-nexus` нь зөвхөн нотлох тест биш `mod nexus::` (бүтэн Nexus интеграцийн дэд хэсэг) ажилладаг.

## CI хаалга

CI туслах:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Зорилтот болгох:

```bash
make check-nexus-cross-dataspace
```

Энэ хаалга нь детерминистик баталгааны багцыг гүйцэтгэдэг бөгөөд хэрэв хөндлөн өгөгдлийн орон зай атомын
своп хувилбар регресс.

## Гарын авлагын дүйцэх командууд

Зорилтот нотлох тест:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Бүрэн Nexus дэд багц:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Хүлээгдэж буй нотлох дохио-Тест тэнцсэн.
- Санаатайгаар бүтэлгүйтсэн дутуу санхүүжүүлэлтийн үед хүлээгдэж буй нэг анхааруулга гарч ирнэ:
  `settlement leg requires 10000 but only ... is available`.
- Эцсийн тэнцлийн баталгаа дараах дараа амжилттай болно:
  - амжилттай форвард солилцоо,
  - амжилттай урвуу солилцоо,
  - бүтэлгүйтсэн дутуу санхүүжүүлсэн своп (өөрчлөгдөөгүй үлдэгдлийг буцаах).

## Одоогийн баталгаажуулалтын агшин зураг

**2026 оны 2-р сарын 19-ний байдлаар** энэ ажлын урсгал дараах байдлаар дамжсан.

- зорилтот тест: `1 passed; 0 failed`,
- бүрэн Nexus дэд багц: `24 passed; 0 failed`.