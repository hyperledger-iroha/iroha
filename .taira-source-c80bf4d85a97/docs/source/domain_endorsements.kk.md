---
lang: kk
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Домен мақұлдаулары

Домен мақұлдаулары операторларға доменді құруға және комитет қол қойған мәлімдеме бойынша қайта пайдалануға мүмкіндік береді. Индоссамент пайдалы жүктемесі тізбекте жазылған Norito нысаны болып табылады, осылайша клиенттер қай доменді кім және қашан растағанын тексере алады.

## Пайдалы жүк пішіні

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: канондық домен идентификаторы
- `committee_id`: адам оқи алатын комитет белгісі
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: блок биіктігін шектеу жарамдылығы
- `scope`: қосымша деректер кеңістігі және қосымша `[block_start, block_end]` терезесі (қоса алғанда), ол **қабылданатын блок биіктігін жабуы керек**
- `signatures`: `body_hash()` үстіндегі қолдар (`signatures = []` арқылы растау)
- `metadata`: қосымша Norito метадеректері (ұсыныс идентификаторлары, аудит сілтемелері, т.б.)

## Орындау

- Мақұлдаулар Nexus қосылғанда және `nexus.endorsement.quorum > 0` болғанда немесе доменге арналған саясат доменді қажет деп белгілегенде қажет.
- Валидация домен/мәлімдеме хэшті байланыстыруды, нұсқаны, блок терезесін, деректер кеңістігінің мүшелігін, жарамдылық мерзімін/жасын және комитет кворумын қамтамасыз етеді. Қол қоюшылардың `Endorsement` рөлі бар тікелей консенсус кілттері болуы керек. Қайталауларды `body_hash` қабылдамайды.
- Доменді тіркеуге тіркелген растаулар `endorsement` метадеректер кілтін пайдаланады. Дәл сол тексеру жолы `SubmitDomainEndorsement` нұсқауымен пайдаланылады, ол жаңа доменді тіркеусіз тексеру үшін растауларды жазады.

## Комитеттер мен саясаттар

- Комитеттер тізбекте тіркелуі мүмкін (`RegisterDomainCommittee`) немесе әдепкі конфигурациядан алынған (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- Домендік саясаттар `SetDomainEndorsementPolicy` (комитет идентификаторы, `max_endorsement_age`, `required` жалауы) арқылы конфигурацияланады. Жоқ кезде Nexus әдепкі мәндері пайдаланылады.

## CLI көмекшілері

- Мақұлдауды құру/қол қою (Norito JSON stdout-қа шығарады):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Индоссамент жіберу:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Басқаруды басқару:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Тексеру сәтсіздігі тұрақты қате жолдарын қайтарады (кворум сәйкессіздігі, ескірген/мерзімі өткен растау, ауқым сәйкессіздігі, белгісіз деректер кеңістігі, жетіспейтін комитет).