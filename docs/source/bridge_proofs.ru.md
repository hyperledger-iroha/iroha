---
lang: ru
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> Это сокращённый локализованный обзор по состоянию на 2026-07-11, а не
> полный нормативный перевод. Точные типы, контракты API и требования к
> выпуску приведены на [канонической странице на английском](bridge_proofs.md).

# Доказательства моста SCCP V1 — краткий обзор

## Граница первого выпуска

- SCCP V1 имеет закрытую поверхность: поддерживаются только Ethereum mainnet,
  BSC mainnet и TRON mainnet, а единственной конечной сетью SORA является
  `sora-taira`. Любой другой профиль сети или идентификатор SORA отклоняется.
- `SubmitBridgeProof` принимает только типизированные доказательства
  `NativeProtocol` и `SccpDestination`, привязанные к маршруту. Отправка общих
  payload `Ics` и `TransparentZk` недоступна и отклоняется по принципу
  fail-closed.

## Типизированный реестр и история

- `SccpRegistryV1` типизирован и работает только на добавление. Для каждой lane
  сохраняется не более 64 ревизий маршрутов и 4 096 native trust anchors.
  Записи не удаляются неявно; следующее добавление сверх лимита отклоняется
  атомарно.
- Интервал anchor использует аутентифицированную координату консенсуса:
  finalized beacon slot для Ethereum и finalized native block height для
  BSC/TRON. Старый anchor действителен включительно до checkpoint-преемника и
  недействителен после него.
- Устойчивое inbound-состояние раздельно хранит event/finality height и
  `anchor_interval_height`. High-water для lane+anchor только увеличивается;
  checkpoint-преемник не может быть ниже него. При загрузке snapshot индекс
  полностью пересчитывается, а отсутствующие, устаревшие и лишние значения
  отклоняются. Повторное использование message id и replay также отклоняются.

## Однократная проверка и детерминированные лимиты

- Каждое native- или destination-доказательство канонически декодируется один
  раз и проходит дорогостоящую криптографическую проверку один раз. До неё
  консенсус резервирует консервативную, независимую от оборудования оценку
  работы.
- `[zk.sccp]` задаёт обязательные ненулевые лимиты на proof, transaction и
  block для количества/байтов доказательств, native headers, обновлений
  Ethereum light client, байтов заголовков, восстановлений secp256k1,
  агрегатных проверок/вкладов BLS и pairing-product проверок BN254. Эти лимиты
  допуска связаны с консенсусом и должны совпадать у всех валидаторов.

## Ограничения Torii

`/v1/bridge/proofs/submit` и `/v1/bridge/messages` применяют отдельный лимит
HTTP body для каждого endpoint. Аутентификация, rate limit и `Content-Length`
проверяются до чтения тела; chunked body читается только до жёсткой границы.
Слишком большой запрос возвращает `413`, а некорректный transport/JSON —
отдельный `400`. Detached transaction payload ограничен 16 MiB, signature
payload — 16 KiB.
