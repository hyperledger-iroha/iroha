---
lang: kk
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Сенімді әріптестерден Genesis Bootstrap

Iroha жергілікті `genesis.file` құрдастары сенімді құрдастардан қол қойылған генезис блогын ала алады
Norito кодталған bootstrap протоколын пайдалану.

- **Протокол:** құрдастары `GenesisRequest` (метадеректер үшін `Preflight`, пайдалы жүктеме үшін `Fetch`) және
  `request_id` арқылы кілттелген `GenesisResponse` кадрлары. Жауап берушілерге тізбек идентификаторы, қол қоюшы кілті,
  хэш және қосымша өлшем туралы анықтама; пайдалы жүктемелер тек `Fetch` және қайталанатын сұрау идентификаторларында қайтарылады
  `DuplicateRequest` алыңыз.
- **Күзетшілер:** жауап берушілер рұқсат етілген тізімді (`genesis.bootstrap_allowlist` немесе сенімді серіктестер) бекітеді
  жинақ), тізбек идентификаторы/pubkey/хэш сәйкестігі, жылдамдық шектеулері (`genesis.bootstrap_response_throttle`) және
  өлшем қақпағы (`genesis.bootstrap_max_bytes`). Рұқсат етілген тізімнен тыс сұраулар `NotAllowed` алады және
  қате кілтпен қол қойылған пайдалы жүктемелер `MismatchedPubkey` алады.
- **Сұраныс ағыны:** жад бос және `genesis.file` орнатылмаған кезде (және
  `genesis.bootstrap_enabled=true`), түйін сенімді құрдастарды міндетті емес параметрлермен алдын ала іске қосады
  `genesis.expected_hash`, содан кейін пайдалы жүктемені алады, `validate_genesis_block` арқылы қолтаңбаларды тексереді,
  және блокты қолданбас бұрын Курамен бірге `genesis.bootstrap.nrt` сақталады. Bootstrap әрекетін қайталайды
  құрмет `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval`, және
  `genesis.bootstrap_max_attempts`.
- **Сәтсіздік режимдері:** рұқсат етілген тізімді жіберіп алу, тізбек/pubkey/хэш сәйкессіздіктері, өлшем үшін сұраулар қабылданбайды.
  шекті бұзу, мөлшерлеме шектеулері, жергілікті генезис жоқ немесе қайталанатын сұрау идентификаторлары. Қайшылықты хэштер
  қатарластар алуды тоқтатады; ешбір жауап берушілер/тайм-ауттар жергілікті конфигурацияға оралмайды.
- **Оператор қадамдары:** жарамды генезисі бар кем дегенде бір сенімді теңдестің қол жетімді екеніне көз жеткізіңіз, конфигурациялаңыз
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` және қайталау түймелері, және
  сәйкес келмейтін пайдалы жүктемелерді қабылдамау үшін қосымша `expected_hash` түйреуіші. Тұрақты пайдалы жүктемелер болуы мүмкін
  `genesis.file` - `genesis.bootstrap.nrt` көрсету арқылы келесі етіктерде қайта пайдаланылады.