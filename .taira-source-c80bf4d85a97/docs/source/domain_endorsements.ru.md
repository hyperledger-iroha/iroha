---
lang: ru
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Подтверждения домена

Подтверждение домена позволяет операторам контролировать создание и повторное использование домена на основании заявления, подписанного комитетом. Полезная нагрузка подтверждения — это объект Norito, записанный в цепочке, чтобы клиенты могли проверять, кто, какой домен и когда подтверждал.

## Форма полезной нагрузки

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: канонический идентификатор домена.
- `committee_id`: удобочитаемая этикетка комитета.
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: высота блока, ограничивающая допустимость
- `scope`: дополнительное пространство данных плюс дополнительное окно `[block_start, block_end]` (включительно), которое **должно** покрывать высоту принимающего блока.
- `signatures`: подписи поверх `body_hash()` (подтверждение `signatures = []`)
- `metadata`: дополнительные метаданные Norito (идентификаторы предложений, ссылки аудита и т. д.).

## Правоприменение

- Подтверждения необходимы, когда Nexus включен и `nexus.endorsement.quorum > 0`, или когда политика для каждого домена помечает домен как обязательный.
- Проверка обеспечивает привязку хеша домена/оператора, версию, окно блокировки, членство в пространстве данных, срок действия/возраст и кворум комитета. Подписывающие стороны должны иметь действующие ключи консенсуса с ролью `Endorsement`. Повторы отклонены `body_hash`.
- В подтверждениях, прилагаемых к регистрации домена, используется ключ метаданных `endorsement`. Тот же путь проверки используется инструкцией `SubmitDomainEndorsement`, которая записывает подтверждения для аудита без регистрации нового домена.

## Комитеты и политика

- Комитеты могут быть зарегистрированы в цепочке (`RegisterDomainCommittee`) или получены из настроек по умолчанию (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
— Политики для каждого домена настраиваются через `SetDomainEndorsementPolicy` (идентификатор комитета, `max_endorsement_age`, флаг `required`). Если он отсутствует, используются значения по умолчанию Nexus.

## помощники CLI

- Создайте/подпишите одобрение (выводит Norito JSON на стандартный вывод):

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

- Оставить одобрение:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Управлять управлением:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

При сбоях проверки возвращаются стабильные строки ошибок (несоответствие кворума, устаревшее или истекшее подтверждение, несоответствие области действия, неизвестное пространство данных, отсутствие комитета).