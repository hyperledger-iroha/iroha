---
lang: ru
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Таксономия ошибок подключения (базовый план Swift)

В этой заметке отслеживается IOS-CONNECT-010 и документируется таксономия общих ошибок для
Nexus Подключите SDK. Swift теперь экспортирует каноническую оболочку `ConnectError`,
который относит все сбои, связанные с Connect, к одной из шести категорий, таким образом, телеметрия,
информационные панели и текст UX остаются согласованными на разных платформах.

> Последнее обновление: 15 января 2026 г.  
> Владелец: руководитель Swift SDK (распорядитель таксономии)  
> Статус: реализации Swift + Android + JavaScript **прибыли**; Ожидается интеграция очереди.

## Категории

| Категория | Цель | Типичные источники |
|----------|---------|-----------------|
| `transport` | Сбои транспорта WebSocket/HTTP, приводящие к завершению сеанса. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Сбои сериализации/моста при кодировании/декодировании кадров. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Сбои TLS/аттестации/политики, требующие исправления пользователем или оператором. | `URLError(.secureConnectionFailed)`, Torii 4xx ответов |
| `timeout` | Срок действия простоя/оффлайн и сторожевые таймеры (время жизни очереди, тайм-аут запроса). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | Сигналы обратного давления FIFO позволяют приложениям корректно сбрасывать нагрузку. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Все остальное: неправильное использование SDK, отсутствие моста Norito, поврежденные журналы. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Каждый SDK публикует тип ошибки, соответствующий таксономии, и раскрывает
атрибуты структурированной телеметрии: `category`, `code`, `fatal` и необязательные.
метаданные (`http_status`, `underlying`).

## Быстрое сопоставление

Swift экспортирует `ConnectError`, `ConnectErrorCategory` и вспомогательные протоколы в
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Ошибка всех общедоступных подключений
типы соответствуют `ConnectErrorConvertible`, поэтому приложения могут вызывать `error.asConnectError()`
и переслать результат на уровни телеметрии/регистрации.| Ошибка Свифта | Категория | Код | Заметки |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Указывает двойной `start()`; ошибка разработчика. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Поднимается при отправке/получении после закрытия. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket доставлял текстовую полезную нагрузку, ожидая двоичного кода. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Контрагент неожиданно закрыл трансляцию. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Приложение забыло настроить симметричные ключи. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | В полезной нагрузке Norito отсутствуют обязательные поля. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Будущая полезная нагрузка, видимая старым SDK. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Мост Norito отсутствует или не удалось закодировать/декодировать байты кадра. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Мост недоступен или несоответствующая длина ключей. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Длина автономной очереди превысила настроенную границу. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Обнаружено `URLSessionWebSocketTask`. |
| `URLError` Случаи TLS | `authorization` | `network.tls_failure` | Сбои согласования ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | Ошибка декодирования/кодирования JSON в другом месте SDK; сообщение использует контекст декодера Swift. |
| Любой другой `Error` | `internal` | `unknown_error` | Гарантированный всеобъемлющий охват; зеркала сообщений `LocalizedError`. |

Блокировка модульных тестов (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`)
сопоставление, чтобы будущие рефакторинги не могли незаметно изменять категории или коды.

### Пример использования

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Телеметрия и информационные панели

Swift SDK предоставляет `ConnectError.telemetryAttributes(fatal:httpStatus:)`.
который возвращает карту канонических атрибутов. Экспортерам следует направлять эти
атрибуты в событиях `connect.error` OTEL с дополнительными опциями:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Панели мониторинга коррелируют счетчики `connect.error` с глубиной очереди (`connect.queue_depth`).
и переподключите гистограммы, чтобы обнаружить регрессии без анализа журналов.

## Сопоставление AndroidAndroid SDK экспортирует `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` и вспомогательные утилиты в разделе
`org.hyperledger.iroha.android.connect.error`. Помощники в стиле Builder преобразуют любой `Throwable`
в полезную нагрузку, соответствующую таксономии, выводить категории на основе исключений транспорта/TLS/кодека,
и предоставлять детерминированные атрибуты телеметрии, чтобы стеки OpenTelemetry/выборки могли использовать
результат без специальных адаптеров.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` уже реализует `ConnectErrorConvertible`, выдавая очередь очередиOverflow/таймаут.
категории для условий переполнения/истечения срока действия, чтобы инструменты автономной очереди могли подключаться к одному и тому же потоку.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
README Android SDK теперь ссылается на таксономию и показывает, как оборачивать транспортные исключения.
перед отправкой телеметрии, сохраняя соответствие рекомендаций dApp базовым показателям Swift.【java/iroha_android/README.md:167】

## Сопоставление JavaScript

Клиенты Node.js/браузера импортируют `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` и
`connectErrorFrom()` из `@iroha/iroha-js`. Общий помощник проверяет коды состояния HTTP,
Коды ошибок узла (сокет, TLS, тайм-аут), имена `DOMException` и ошибки кодека для выдачи
те же категории/коды, которые описаны в этой заметке, а определения TypeScript моделируют телеметрию.
переопределение атрибутов, поэтому инструменты могут генерировать события OTEL без ручного приведения типов.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README документирует рабочий процесс и содержит ссылки на эту таксономию, чтобы команды разработчиков могли
скопируйте фрагменты инструментов дословно.【javascript/iroha_js/README.md:1387】

## Следующие шаги (кросс-SDK)

– **Интеграция очереди:** после отправки автономной очереди обеспечьте логику удаления из очереди.
  поверхности значений `ConnectQueueError`, поэтому телеметрия переполнения остается заслуживающей доверия.