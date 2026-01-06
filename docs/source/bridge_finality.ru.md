---
lang: ru
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7236dfe86175ff89f660be4cb4dd2c90df20a05f9606d2707407465f639b1a1c
source_last_modified: "2025-12-05T06:21:36.529838+00:00"
translation_last_reviewed: 2026-01-01
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

Этот документ описывает начальную поверхность bridge finality proofs для Iroha.
Цель - позволить внешним цепочкам или light clients проверять, что блок Iroha
финализирован без off-chain вычислений или доверенных реле.

## Формат proof

`BridgeFinalityProof` (Norito/JSON) содержит:

- `height`: высота блока.
- `chain_id`: идентификатор цепочки Iroha для предотвращения кросс-чейн replay.
- `block_header`: канонический `BlockHeader`.
- `block_hash`: hash заголовка (клиенты пересчитывают для валидации).
- `commit_certificate`: набор валидаторов + подписи, финализирующие блок.

Proof самодостаточен; внешние manifests или непрозрачные blobs не требуются.
Retention: Torii отдает finality proofs для недавнего окна commit-certificate
(ограничено configured history cap; по умолчанию 512 записей через
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Клиенты
должны кешировать или якорить proofs, если нужны более длинные горизонты.
Канонический tuple - `(block_header, block_hash, commit_certificate)`: hash заголовка
должен совпадать с hash внутри commit certificate, а chain id привязывает proof к
одному ledger. Серверы отклоняют и логируют `CommitCertificateHashMismatch`, когда
certificate указывает на другой block hash.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) расширяет базовый proof явным commitment и justification:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: подписи authority set над payload commitment
  (переиспользует подписи commit certificate).
- `block_header`, `commit_certificate`: как в базовом proof.

Текущий placeholder: `mmr_root`/`mmr_peaks` выводятся путем пересчета block-hash MMR в памяти;
inclusion proofs пока не возвращаются. Клиенты все еще могут проверить тот же hash через
payload commitment сегодня.

MMR peaks are ordered left to right. Recompute `mmr_root` by bagging peaks
from right to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

Верификация аналогична базовому proof: пересчитать `block_hash` из header, проверить подписи
commit certificate, и убедиться, что поля commitment соответствуют certificate и hash блока.
Bundle добавляет commitment/justification wrapper для bridge протоколов, предпочитающих разделение.

## Шаги проверки

1. Пересчитать `block_hash` из `block_header`; отклонить при mismatch.
2. Проверить, что `commit_certificate.block_hash` совпадает с пересчитанным `block_hash`;
   отклонить пары header/commit certificate с несовпадением.
3. Проверить, что `chain_id` соответствует ожидаемой цепочке Iroha.
4. Пересчитать `validator_set_hash` из `commit_certificate.validator_set` и проверить совпадение
   с записанным hash/version.
5. Проверить подписи в commit certificate против hash заголовка с использованием публичных ключей
   и индексов валидаторов; применять quorum (`2f+1` когда `n>3`, иначе `n`) и отклонять
   дубли/индексы вне диапазона.
6. Опционально привязать к trusted checkpoint, сравнив hash validator set с anchored значением
   (weak-subjectivity anchor).
7. Опционально привязать к ожидаемому epoch anchor, чтобы proofs из старых/новых epochs
   отклонялись до намеренной ротации anchor.

`BridgeFinalityVerifier` (в `iroha_data_model::bridge`) применяет эти проверки, отклоняя
chain-id/height drift, mismatch hash/version validator set, дубли/индексы вне диапазона,
невалидные подписи и неожиданные epochs до подсчета quorum, чтобы light clients могли
использовать единый verifier.

## Reference verifier

`BridgeFinalityVerifier` принимает ожидаемый `chain_id` плюс опциональные anchors validator set
и epoch. Он обеспечивает tuple header/block-hash/commit-certificate, валидирует hash/version
validator set, проверяет подписи/quorum против объявленного roster валидаторов и отслеживает
последнюю высоту, отклоняя stale/skipped proofs. Когда anchors заданы, он отклоняет replays
между epochs/rosters с ошибками `UnexpectedEpoch`/`UnexpectedValidatorSet`; без anchors он
принимает hash validator set и epoch из первого proof перед продолжением детерминированного
отклонения дубли/вне диапазона/недостаточных подписей.

## Surface API

- `GET /v1/bridge/finality/{height}` - возвращает `BridgeFinalityProof` для запрошенной
  высоты блока. Content negotiation через `Accept` поддерживает Norito или JSON.
- `GET /v1/bridge/finality/bundle/{height}` - возвращает `BridgeFinalityBundle`
  (commitment + justification + header/certificate) для запрошенной высоты.

## Notes and follow-ups

- Proofs сейчас выводятся из сохраненных commit certificates. Ограниченная история следует
  окну retention commit certificate; клиенты должны кешировать anchor proofs для более
  длинных горизонтов. Запросы вне окна возвращают `CommitCertificateNotFound(height)`;
  показывайте ошибку и используйте anchored checkpoint.
- Replay или forged proof с mismatch `block_hash` (header vs. certificate) отклоняется
  с `CommitCertificateHashMismatch`; клиенты должны выполнять ту же tuple проверку до
  проверки подписей и отбрасывать mismatch payloads.
- В будущем можно добавить MMR/authority-set commitment chains для уменьшения размера proofs
  в более богатые commitment envelopes.
