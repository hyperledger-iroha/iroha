---
lang: ru
direction: ltr
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# Пакетирование релиза NoritoBridge

В этом документе описаны шаги, необходимые для публикации Swift‑биндингов
`NoritoBridge` в виде XCFramework, который можно подключать из Swift Package Manager и
CocoaPods. Workflow держит Swift‑артефакты в строгом соответствии с релизами Rust‑crate’а,
поставляющего Norito‑codec Iroha. Для пошаговых инструкций по подключению этих артефактов
в приложение (настройка Xcode‑проекта, использование ChaChaPoly и т.д.) см.
`docs/connect_swift_integration.md`.

> **Примечание.** Автоматизация CI для этого потока будет добавлена после появления
> macOS‑builders с необходимым Apple‑инструментарием (задача отслеживается в backlog’е
> macOS‑builders команды Release Engineering). До тех пор шаги ниже необходимо выполнять
> вручную на машине разработчика под macOS.

## Предварительные условия

- macOS‑хост с установленными актуальными Xcode Command Line Tools.
- Установленный Rust‑toolchain, совпадающий с `rust-toolchain.toml` в корне workspace’а.
- Swift‑toolchain версии 5.7 или новее.
- CocoaPods (через Ruby‑gems), если вы публикуете в центральный репозиторий specs.
- Доступ к ключам подписи релизов Hyperledger Iroha для тегирования Swift‑артефактов.

## Модель версионирования

1. Определите версию Rust‑crate’а для Norito‑codec’а (`crates/norito/Cargo.toml`).
2. Проставьте тег workspace’у с идентификатором релиза (например, `v2.1.0`).
3. Используйте ту же семантическую версию для Swift‑пакета и CocoaPods‑podspec’а.
4. При увеличении версии Rust‑crate’а повторяйте процесс и публикуйте соответствующий
   Swift‑артефакт. В тестовых релизах допускаются суффиксы metadata (например,
   `-alpha.1`).

## Шаги сборки

1. Из корня репозитория вызовите helper‑скрипт для сборки XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Скрипт компилирует библиотеку‑bridge на Rust для целевых платформ iOS и macOS и
   упаковывает получившиеся статические библиотеки в единый каталог XCFramework.

2. Заархивируйте XCFramework для распространения:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Обновите manifest Swift‑пакета (`IrohaSwift/Package.swift`), указав новую версию и
   checksum:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Запишите полученный checksum в `Package.swift` при объявлении binary‑target’а.

4. Обновите `IrohaSwift/IrohaSwift.podspec`, указав новую версию, checksum и URL архива.

5. **Перегенерируйте заголовки, если bridge получил новые экспортируемые символы.**
   Swift‑bridge теперь экспортирует `connect_norito_set_acceleration_config`, чтобы
   `AccelerationSettings` мог включать Metal/GPU‑backends. Убедитесь, что
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` соответствует
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` перед упаковкой.

6. Запустите Swift‑валидацию перед выставлением тега:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   Первая команда проверяет, что Swift‑пакет (включая `AccelerationSettings`) успешно
   собирается; вторая — что fixtures находятся в паритете, дашборды паритет/CI
   отрендерены, а также что выполняются те же проверки телеметрии, что и в Buildkite
   (включая наличие metadata `ci/xcframework-smoke:<lane>:device_tag`).

7. Закоммитьте сгенерированные артефакты в release‑ветку и проставьте тег на коммит.

## Публикация

### Swift Package Manager

- Отправьте тег в публичный Git‑репозиторий.
- Убедитесь, что тег доступен из пакетного индекса (Apple или community‑mirror).
- Подключение со стороны потребителя:
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. Локально проверьте pod:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Опубликуйте обновлённый podspec:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Убедитесь, что новая версия появилась в CocoaPods‑index.

## Особенности CI

- Создайте macOS‑job, который запускает packaging‑скрипт, архивирует артефакты и
  публикует вычисленный checksum как output workflow’а.
- Считайте релиз «готовым» только после того, как Swift‑demo‑приложение успешно
  соберётся против свежесобранного framework’а.
- Сохраняйте build‑логи для дальнейшей диагностики сбоев.

## Дополнительные идеи по автоматизации

- Использовать `xcodebuild -create-xcframework` напрямую, когда все необходимые таргеты
  будут доступны.
- Интегрировать подпись/нотаризацию для распространения за пределами dev‑машин.
- Держать интеграционные тесты в полном соответствии с упакованной версией, закрепляя
  зависимость SPM на релизном теге.

