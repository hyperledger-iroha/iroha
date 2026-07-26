---
lang: ru
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
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

Исходный route TRON использует точный ABI
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. Успешное выполнение
требует `expectedNonce == transferNonce`; затем до увеличения storage то же
значение записывается в канонический payload. Native admission восстанавливает
полный ABI call из recipient в payload, масштабированной суммы и nonce. Поэтому
устаревший selector с двумя аргументами, старый или будущий nonce и исчерпанный
`uint64` nonce безопасно отклоняются.

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

## Outbound commitment, хранение и обнаружение

Каждое успешно созданное outbound message получает плотный `commitment_index` в
порядке исполнения блока (`0..=511`). Неизменяемые пределы V1 — 512 сообщений на
блок и 4 096 байт канонического payload на сообщение. `[zk.sccp]` совместно
ограничивает ожидающие payload через `max_pending_outbound_messages` (по умолчанию
`65536`) и `max_pending_outbound_payload_bytes` (по умолчанию `268435456`).

До публикации finality или удаления тела блока Kura неизменно сохраняет точный
канонический header и аутентифицированный корнем архив SCCP. Восстановление proof,
bundle, proof request и недавней истории не читает историческое тело блока или
изменяемую копию payload из WSV. После принятия destination proof ожидающий payload
и его учёт удаляются атомарно, а фиксированный terminal descriptor остаётся вместе
с locator/index. Ожидающее состояние ограничено; terminal records и неизменяемая
история Kura намеренно растут для постоянной защиты от replay.
`GET /v1/sccp/messages/recent` использует составной cursor
`{ from, after_index }`. Неизменяемые evidence учитываются в общем/операторском
использовании диска, но исключены из бюджета удаляемых тел.

## Ограничения Torii

`/v1/bridge/proofs/submit` и `/v1/bridge/messages` применяют отдельный лимит
HTTP body для каждого endpoint. Аутентификация, rate limit и `Content-Length`
проверяются до чтения тела; chunked body читается только до жёсткой границы.
Слишком большой запрос возвращает `413`, а некорректный transport/JSON —
отдельный `400`. Detached transaction payload ограничен 16 MiB, signature
payload — 16 KiB.
