---
lang: kk
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS CLI манифестінің соңына дейін мысалы

Бұл мысал SoraFS нұсқасына құжаттама құрастыруын жариялау арқылы жүреді.
`sorafs_manifest_stub` CLI детерминирленген кесу құрылғыларымен бірге
SoraFS Architecture RFC ішінде сипатталған. Ағын манифест ұрпақты қамтиды,
күту тексерулері, жоспарды алуды тексеру және іздеуді дәлелдеу репетициясы
командалар CI-ге бірдей қадамдарды ендіре алады.

## Алғышарттар

- Жұмыс кеңістігі клондалған және құралдар тізбегі дайын (`cargo`, `rustc`).
- `fixtures/sorafs_chunker` құрылғылары қол жетімді, сондықтан күтілетін мәндер болуы мүмкін
  алынған (өндіріс кезеңдері үшін тасымалдау кітабының жазбасынан мәндерді алыңыз
  артефактпен байланысты).
- Жарияланатын пайдалы жүктеме каталогының үлгісі (бұл мысалда `docs/book` пайдаланылады).

## 1-қадам — Манифест, CAR, қолтаңбалар және алу жоспарын жасаңыз

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

Пәрмен:

- Пайдалы жүктемені `ChunkProfile::DEFAULT` арқылы жібереді.
- CARv2 мұрағатын плюс кесек алу жоспарын шығарады.
- `ManifestV1` жазбасын жасайды, манифест қолтаңбаларын (берілген болса) тексереді және
  конвертті жазады.
- Күту жалаушаларын күшейтеді, сондықтан байттар ауытқыса, іске қосу сәтсіз болады.

## 2-қадам — Бөлшектерді сақтау + PoR репетициясы арқылы шығыстарды тексеріңіз

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Бұл детерминирленген бөлшектер қоймасы арқылы CAR қайта ойнатады, шығарады
Қайта алу мүмкіндігін дәлелдеу сынағы және сәйкес манифест есебін шығарады
басқаруды шолу.

## 3-қадам — Көп провайдерлерді іздеуді имитациялау

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI орталары үшін әр провайдерге бөлек пайдалы жүктеме жолдарын қамтамасыз етіңіз (мысалы, орнатылған
құрылғылар) жаттығулар ауқымын жоспарлау және сәтсіздіктерді өңдеу.

## 4-қадам — Бухгалтерлік кітаптың жазбасын жазу

Жарияланымды `docs/source/sorafs/migration_ledger.md` форматында жазып, мыналарды жазып алыңыз:

- Манифест CID, CAR дайджест және кеңес қол қою хэші.
- Күй (`Draft`, `Staging`, `Pinned`).
- CI жүгірістеріне немесе басқару билеттеріне сілтемелер.

## 5-қадам — Басқару құралдары арқылы бекіту (тізілім белсенді болған кезде)

PIN тізілімі орналастырылғаннан кейін (көшіру жол картасындағы M2 кезең),
манифестті CLI арқылы жіберіңіз:

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

Ұсыныс идентификаторы және кейінгі бекіту транзакция хэштері болуы керек
тексеру мүмкіндігі үшін көшіру кітабының жазбасында жазылған.

## Тазалау

`target/sorafs/` астындағы артефактілерді мұрағаттауға немесе кезеңдік түйіндерге жүктеп салуға болады.
Манифестті, қолтаңбаларды, CAR және алу жоспарын бірге төмен қарай сақтаңыз
операторлар мен SDK топтары орналастыруды анықтауды растай алады.