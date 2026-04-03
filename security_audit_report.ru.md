<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Отчет об аудите безопасности

Дата: 26 марта 2026 г.

## Резюме

Этот аудит был сосредоточен на поверхностях с самым высоким риском в текущем дереве: Torii потоки HTTP/API/аутентификации, P2P-транспорт, API-интерфейсы обработки секретов, средства защиты транспорта SDK и путь очистки вложений.

Я нашел 6 практических проблем:

- 2 вывода высокой степени тяжести
- 4 результата средней степени тяжести

Наиболее важными проблемами являются:

1. Torii в настоящее время регистрирует заголовки входящих запросов для каждого HTTP-запроса, который может предоставлять токены носителя, токены API, токены сеанса оператора/загрузочной загрузки и пересылать маркеры mTLS в журналы.
2. Несколько общедоступных маршрутов Torii и SDK по-прежнему поддерживают отправку необработанных значений `private_key` на сервер, чтобы Torii мог подписывать от имени вызывающего абонента.
3. Некоторые «секретные» пути рассматриваются как обычные тела запроса, включая получение конфиденциального начального значения и каноническую аутентификацию запроса в некоторых SDK.

## Метод

- Статический анализ путей обработки секретов Torii, P2P, шифрования/VM и SDK.
- Команды целевой проверки:
  - `cargo check -p iroha_torii --lib --message-format short` -> пройти
  - `cargo check -p iroha_p2p --message-format short` -> пройти
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> пройти
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> пройдено, только предупреждения о дублирующихся версиях
- Не выполнено в этом проходе:
  - полная сборка/тестирование/обрезание рабочего пространства
  - Наборы тестов Swift/Gradle
  - Проверка времени выполнения CUDA/Metal.

## Выводы

### SA-001 Высокий: Torii регистрирует конфиденциальные заголовки запросов по всему миру.Воздействие. Любое развертывание, обеспечивающее отслеживание запросов, может привести к утечке токенов носителя/API/оператора и связанных с ними материалов аутентификации в журналы приложений.

Доказательства:

- `crates/iroha_torii/src/lib.rs:20752` включает `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` включает `DefaultMakeSpan::default().include_headers(true)`
- Чувствительные имена заголовков активно используются в других местах одного и того же сервиса:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Почему это важно:

- `include_headers(true)` записывает полные значения входящего заголовка в диапазоны трассировки.
- Torii принимает материалы аутентификации в таких заголовках, как `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` и `x-forwarded-client-cert`.
- Таким образом, компрометация приемника журнала, сбор журнала отладки или пакет поддержки могут стать событием раскрытия учетных данных.

Рекомендуемое исправление:

— Прекратить включение полных заголовков запросов в производственные промежутки.
— Добавьте явное редактирование заголовков, чувствительных к безопасности, если для отладки все еще требуется ведение журнала заголовков.
- Считайте регистрацию запросов/ответов секретной по умолчанию, если только данные не включены в разрешенный список.

### SA-002 High: общедоступные API Torii по-прежнему принимают необработанные закрытые ключи для подписи на стороне сервера.

Воздействие: клиентам рекомендуется передавать необработанные закрытые ключи по сети, чтобы сервер мог подписывать их от их имени, создавая ненужный канал раскрытия секретов на уровнях API, SDK, прокси и памяти сервера.

Доказательство:- Документация маршрута управления явно рекламирует подпись на стороне сервера:
  - `crates/iroha_torii/src/gov.rs:495`
- Реализация маршрута анализирует предоставленный закрытый ключ и подписывает его на стороне сервера:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK активно сериализуют `private_key` в тела JSON:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Примечания:

- Этот шаблон не привязан к одному семейству маршрутов. Текущее дерево содержит одну и ту же модель удобства для управления, офлайн-наличности, подписок и других DTO, ориентированных на приложения.
- Проверки транспорта только по протоколу HTTPS уменьшают случайную передачу открытого текста, но не устраняют риск обработки секретов на стороне сервера или риска ведения журналов/доступа к памяти.

Рекомендуемое исправление:

— Устаревшие все запросы DTO, содержащие необработанные данные `private_key`.
- Требовать от клиентов подписываться локально и отправлять подписи или полностью подписанные транзакции/конверты.
— Удалите примеры `private_key` из OpenAPI/SDK после окна совместимости.

### SA-003 Средний: при получении конфиденциального ключа секретный исходный материал отправляется на Torii и возвращается обратно.

Воздействие. Конфиденциальный API получения ключей превращает исходный материал в обычные полезные данные запроса/ответа, увеличивая вероятность раскрытия начального значения через прокси, промежуточное программное обеспечение, журналы, трассировки, отчеты о сбоях или неправильное использование клиента.

Доказательство:- Заявка принимает семенной материал напрямую:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Схема ответа повторяет начальное значение как в шестнадцатеричном, так и в base64:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- Обработчик явно перекодирует и возвращает начальное значение:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK представляет это как обычный сетевой метод и сохраняет отраженное начальное значение в модели ответа:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Рекомендуемое исправление:

— Отдавайте предпочтение локальному получению ключей в коде CLI/SDK и полностью удалите маршрут удаленного получения.
- Если маршрут необходимо сохранить, никогда не возвращайте семена в ответ и отмечайте семенные тела как чувствительные на всех транспортных ограждениях и путях телеметрии/регистрации.

### SA-004 Medium: обнаружение чувствительности транспорта SDK имеет «слепые зоны» для секретных материалов, не относящихся к `private_key`.

Влияние: некоторые SDK будут применять HTTPS для необработанных запросов `private_key`, но при этом позволять другим материалам, чувствительным к безопасности, передаваться по незащищенному HTTP или на несовпадающие хосты.

Доказательство:— Swift рассматривает канонические заголовки аутентификации запроса как конфиденциальные:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Но Swift по-прежнему соответствует только телу `"private_key"`:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
— Kotlin распознает только заголовки `authorization` и `x-api-token`, а затем возвращается к той же эвристике тела `"private_key"`:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android имеет то же ограничение:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
— Стороны, подписывающие канонические запросы Kotlin/Java, генерируют дополнительные заголовки аутентификации, которые не классифицируются как конфиденциальные их собственными транспортными средствами защиты:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Рекомендуемое исправление:

— Замените эвристическое сканирование тела явной классификацией запросов.
— Считайте канонические заголовки аутентификации, поля начальных/парольных фраз, подписанные заголовки мутаций и любые будущие поля, несущие секрет, как конфиденциальные по контракту, а не по совпадению подстроки.
- Следите за тем, чтобы правила конфиденциальности были согласованы в Swift, Kotlin и Java.

### SA-005 Средний: «песочница» вложения — это всего лишь подпроцесс плюс `setrlimit`Воздействие: средство очистки вложений описано и сообщается как «находящееся в песочнице», но его реализация представляет собой всего лишь ответвление/исполняемый файл текущего двоичного файла с ограничениями ресурсов. Эксплойт парсера или архива по-прежнему будет выполняться с тем же пользователем, представлением файловой системы и привилегиями окружающей сети/процесса, что и Torii.

Доказательства:

- Внешний путь помечает результат как изолированный после создания дочернего элемента:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Дочерний элемент по умолчанию использует текущий исполняемый файл:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Подпроцесс явно переключается обратно на `AttachmentSanitizerMode::InProcess`:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Единственное применяемое усиление — это CPU/адресное пространство `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Рекомендуемое исправление:

- Либо внедрите реальную песочницу ОС (например, изоляцию в стиле namespaces/seccomp/landlock/jail, сброс привилегий, отсутствие сети, ограниченную файловую систему), либо перестаньте помечать результат как `sandboxed`.
— Рассматривайте текущий проект как «изоляцию подпроцесса», а не как «песочницу» в API, телеметрии и документации, пока не будет достигнута настоящая изоляция.

### SA-006 Средний: дополнительные транспорты P2P TLS/QUIC отключают проверку сертификата.Влияние. Когда `quic` или `p2p_tls` включен, канал обеспечивает шифрование, но не проверяет подлинность удаленной конечной точки. Активный злоумышленник на пути все равно может ретранслировать или завершить канал, игнорируя обычные ожидания безопасности, связанные с TLS/QUIC.

Доказательства:

- QUIC явно документирует разрешительную проверку сертификата:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- Верификатор QUIC безоговорочно принимает сертификат сервера:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- Транспорт TLS-over-TCP делает то же самое:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Рекомендуемое исправление:

- Либо проверьте одноранговые сертификаты, либо добавьте явную привязку канала между подписанным рукопожатием более высокого уровня и транспортным сеансом.
- Если текущее поведение является преднамеренным, переименуйте/задокументируйте эту функцию как неаутентифицированный зашифрованный транспорт, чтобы операторы не перепутали ее с полной одноранговой аутентификацией TLS.

## Рекомендуемый порядок исправления1. Немедленно исправьте SA-001, отредактировав или отключив ведение журнала заголовков.
2. Разработайте и внедрите план миграции для SA-002, чтобы необработанные закрытые ключи не пересекали границу API.
3. Удалить или сузить удаленный маршрут получения конфиденциальных ключей и классифицировать тела, несущие семена, как конфиденциальные.
4. Согласуйте правила чувствительности транспорта SDK в Swift/Kotlin/Java.
5. Решите, нужна ли санация вложений в настоящей песочнице или в честном переименовании/изменении области действия.
6. Уточнить и ужесточить модель угроз P2P TLS/QUIC, прежде чем операторы включат эти транспорты, ожидающие аутентифицированного TLS.

## Примечания по проверке

- `cargo check -p iroha_torii --lib --message-format short` пройден.
- `cargo check -p iroha_p2p --message-format short` пройден.
- `cargo deny check advisories bans sources --hide-inclusion-graph` прошёл после запуска за пределами песочницы; он выдавал предупреждения о дублирующихся версиях, но сообщал `advisories ok, bans ok, sources ok`.
- Целенаправленное тестирование Torii для конфиденциального маршрута извлечения набора ключей было начато во время этого аудита, но не было завершено до написания отчета; Независимо от этого этот вывод подтверждается прямой проверкой источника.