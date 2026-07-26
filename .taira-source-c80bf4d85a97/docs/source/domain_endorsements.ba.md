---
lang: ba
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Домен раҫлауҙары

Домен раҫлауҙары операторҙар ҡапҡа домен булдырыу һәм комитет ҡултамғаһы аҫтында ҡабаттан ҡулланырға мөмкинлек бирә. Раҫлау файҙалы йөкләмәһе Norito сылбырҙа теркәлгән объект, шулай итеп, клиенттар аудит ала, кем раҫлай, ниндәй домен һәм ҡасан.

## Түләү формаһы

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id` X: канон домен идентификаторы
- `committee_id`: кеше уҡыу комитет лейблы
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: блок бейеклеге сикләү дөрөҫлөгө
- `scope`: опциональ мәғлүмәттәр киңлеге плюс өҫтәмә `[block_start, block_end]` тәҙрә (инклюзив), тип ** тейеш ** ҡаплау ҡабул итеү блок бейеклеге
- `signatures`: `body_hash()`-тан ашыу ҡултамғалар (`signatures = []` менән раҫлау)
- `metadata`: опциональ Norito метамағлүмәттәр (тәҡдим ids, аудит һылтанмалары һ.б.)

##

- Nexus өҫтөндә эшләү һәм `nexus.endorsement.quorum > 0`, йәки домен сәйәсәте кәрәк булғанда доменды билдәләгәндә кәрәк.
- Валидация домен/әйтергә хеш-сута, версия, блок тәҙрәһе, мәғлүмәттәр киңлеге ағзалығы, срогы/йәш, һәм комитет кворум үтәй. Сигнерҙар `Endorsement` роле менән тере консенсус асҡыстары булырға тейеш. Ҡабатлауҙар `body_hash` тарафынан кире ҡағыла.
- Домен теркәүенә беркетелгән раҫлауҙар метамағлүмәттәр асҡысы `endorsement`. Шул уҡ валидация юлы `SubmitDomainEndorsement` инструкцияһы тарафынан ҡулланыла, унда яңы домен теркәмәйенсә аудит өсөн хуплауҙар теркәлә.

## Комитеттар һәм сәйәсәт

- Комитеттарҙы (`RegisterDomainCommittee`X) йәки конфигурациянан алынған (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `domain_id` X).
- Пер-домен сәйәсәте `SetDomainEndorsementPolicy` аша конфигурациялана (комитет id, `max_endorsement_age`, `required` флагы). Ҡасан юҡ, Nexus ғәҙәттәгесә ҡулланыла.

## CLI ярҙамсылары

- Төҙөү/билдәләү раҫлау (сығыштар Norito JSON stdout):

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

- Раҫлауҙы тапшырығыҙ:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Идара итеү идара итеү:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Валидация етешһеҙлектәре тотороҡло хата ептәрен ҡайтара (кворум тап килмәүе, иҫке/ваҡытын раҫлау, өлкә тап килмәүе, билдәһеҙ мәғлүмәттәр киңлеге, юғалған комитет).