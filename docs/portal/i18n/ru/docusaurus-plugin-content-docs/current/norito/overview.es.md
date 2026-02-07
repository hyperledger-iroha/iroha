---
lang: ru
direction: ltr
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Резюме Norito

Norito — это бинарная возможность сериализации, используемая в полном объеме Iroha: определите, как кодифицируйте структуры данных в красном цвете, сохраняйте дискотеку и выполняйте межкамбийские контракты между хостами. Ящик рабочего пространства зависит от Norito и `serde`, чтобы одноранговые узлы и аппаратные средства могли создавать разные байты.

Este возобновил синтез центральных участков и переход к каноническим ссылкам.

## Архитектура с видом

- **Cabecera + payload** - Сообщение Norito comienza с кабечером согласования функций (флаги, контрольная сумма), сопровождающим полезную нагрузку без включения. Лос-макеты empaquetados и la compresion se negocian mediante bits de la cabecera.
- **Определенная кодификация** - `norito::codec::{Encode, Decode}` реализует базу кодификации. Неправильная компоновка предполагает повторное использование всех полезных данных в кабечерах для того, чтобы хеширование и фирма были детерминированы.
- **Esquema + наследует** - `norito_derive` — родовые реализации `Encode`, `Decode` и `IntoSchema`. Los structs/secuencias empaquetados estan habilitados por дефекто и документированы в `norito.md`.
- **Мультикодек регистра** - Идентификаторы хэшей, тип клавиатуры и дескрипторы активной нагрузки в `norito::multicodec`. Авторизованная таблица доступна на `multicodec.md`.

## Эррамьентас

| Тарея | Командо / API | Заметки |
| --- | --- | --- |
| Проверка кабечера/разделов | `ivm_tool inspect <file>.to` | Муэстра версии ABI, флагов и точек входа. |
| Кодификация/декодификация на Rust | `norito::codec::{Encode, Decode}` | Реализовано для всех основных типов модели данных. |
| Взаимодействие JSON | `norito::json::{to_json_pretty, from_json}` | JSON определяется по значениям Norito. |
| Общие документы/особые характеристики | `norito.md`, `multicodec.md` | Documentacion fuente de verdad en la Raiz del Repo. |

## Flujo de trabajo de desarrollo

1. **Объединение производных** — Prefiere `#[derive(Encode, Decode, IntoSchema)]` для новых структур данных. Эвита сериализуется и пишет вручную, что море абсолютно необходимо.
2. **Действительные пакеты макетов** — используйте `cargo test -p norito` (и матрицу функций, упакованных в `scripts/run_norito_feature_matrix.sh`), чтобы гарантировать, что новые макеты будут установлены.
3. **Обновить документы** — Когда вы начнете кодировать, актуализировать `norito.md` и мультикодек таблицы, вы увидите страницы портала (`/reference/norito-codec` и это резюме).
4. **Mantener pruebas Norito-first** — процедуры интеграции должны использовать вспомогательные средства JSON из Norito и загрузку `serde_json` для выполнения рутинных операций, которые производятся.

## Связывает рапидос

- Спецификация: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Назначение мультикодека: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Сценарий матрицы функций: `scripts/run_norito_feature_matrix.sh`.
- Пример упаковки: `crates/norito/tests/`.

Компания возобновляет работу с быстрым запуском (`/norito/getting-started`), чтобы выполнить повторную практику компиляции и выдать байт-код, который содержит полезные нагрузки Norito.