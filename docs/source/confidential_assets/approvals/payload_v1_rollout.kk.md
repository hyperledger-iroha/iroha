---
lang: kk
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Пайдалы жүктеме v1 шығаруды мақұлдау (SDK Кеңесі, 2026-04-28).
//!
//! `roadmap.md:M1` талап ететін SDK кеңесінің шешім жазбасын түсіреді, осылайша
//! шифрланған пайдалы жүктеме v1 шығарылымында тексерілетін жазба бар (жеткізілетін M1.4).

# Пайдалы жүктеме v1 шығару шешімі (28.04.2026)

- **Төраға:** ҚДК Кеңесінің жетекшісі (М. Такемия)
- **Дауыс беруші мүшелер:** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Бақылаушылар:** Mgmt бағдарламасы, Telemetry Ops

## Енгізулер қаралды

1. **Жылдам байланыстырулар және жіберушілер** — `ShieldRequest`/`UnshieldRequest`, асинхронды жіберушілер және Tx құрастырушы көмекшілері паритет сынақтарымен және құжаттар.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI эргономикасы** — `iroha app zk envelope` көмекшісі жол картасының эргономика талаптарына сәйкестендірілген кодтау/тексеру жұмыс үрдістерін және ақаулық диагностикасын қамтиды.【crates/iroha_cli/src/zk.rs:1256】
3. **Детерминистік қондырғылар және паритет люкстері** — Norito байттар/қате беттерін сақтау үшін ортақ қондырғы + Rust/Swift валидациясы тураланған.【Fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Шешім

- **SDK және CLI үшін пайдалы жүктің v1 шығарылымын мақұлдаңыз**, бұл Swift әмияндарына арнайы сантехникасыз құпия конверттерді шығаруға мүмкіндік береді.
- **Шарттары:** 
  - Паритет құрылғыларын CI дрейф ескертулері астында сақтаңыз (`scripts/check_norito_bindings_sync.py` байланыстырылған).
  - `docs/source/confidential_assets.md` (Swift SDK PR арқылы жаңартылған) ішінде операциялық оқу кітапшасын құжаттаңыз.
  - Кез келген өндіріс жалаушаларын аудармас бұрын калибрлеуді + телеметриялық дәлелдемелерді жазыңыз (M2 астында қадағаланады).

## Әрекет элементтері

| Иесі | Элемент | Мерзімі |
|-------|------|-----|
| Swift Lead | GA қолжетімділігін хабарлаңыз + README үзінділері | 2026-05-01 |
| CLI Maintainer | `iroha app zk envelope --from-fixture` көмекшісін қосыңыз (қосымша) | Артқа қалдыру (блоктау емес) |
| DevRel WG | Пайдалы жүктеме v1 нұсқауларымен әмиянның жылдам іске қосылуын жаңартыңыз | 05.05.2026 |

> **Ескертпе:** Бұл жадынама `roadmap.md:2426` ішіндегі «кеңестің мақұлдауын күтуде» деген уақытша шақыруды ауыстырады және M1.4 трекер тармағын қанағаттандырады. Келесі әрекеттер элементтері жабылған сайын `status.md` жаңартыңыз.