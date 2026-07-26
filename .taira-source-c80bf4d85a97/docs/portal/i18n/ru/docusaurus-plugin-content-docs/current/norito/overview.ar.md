---
lang: ru
direction: ltr
source: docs/portal/docs/norito/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Код запроса Norito

Norito в режиме онлайн-обновления Iroha: В случае необходимости Он выступил в роли Сэнсэя в фильме "Страна" и в "Старом городе", а также в Сан-Франциско. Нэнси. Вы можете положить ящик в ящик Norito, чтобы установить `serde`. Он был убит в 2007 году.

Он был выбран в качестве посредника в борьбе за права человека.

## لمحة عن البنية

- **الرأس + الحمولة** – вызывает сообщение Norito برأس تفاوض للميزات (флаги, контрольная сумма) и отображает полезную нагрузку. Вы можете сделать это в ближайшее время.
- **Уведомление об ошибке** – `norito::codec::{Encode, Decode}`. Для получения информации о полезных нагрузках в программе "Полезные нагрузки" в разделе "Полезные нагрузки" Уиллоу Дэвис.
- **المخطط+ выводит** – `norito_derive` для `Encode` и `Decode` и `IntoSchema`. Установите/запустите приложение для `norito.md`.
- **Мультикодек** – используется для создания полезной нагрузки в `norito::multicodec`. Он был создан для `multicodec.md`.

## الادوات

| المهمة | Интерфейс / API | ملاحظات |
| --- | --- | --- |
| فحص الرأس/الاقسام | `ivm_tool inspect <file>.to` | Используйте ABI, флаги и точки входа. |
| Предыдущая/встроенная версия в Rust | `norito::codec::{Encode, Decode}` | Откройте для себя модель данных. |
| взаимодействие JSON | `norito::json::{to_json_pretty, from_json}` | JSON создается в формате Norito. |
| Документы/спецификации | `norito.md`, `multicodec.md` | Он был убит в фильме "Старый мир". |

## سير عمل التطوير

1. **Происходит ** – от `#[derive(Encode, Decode, IntoSchema)]` до исходного кода. Он ответил на вопрос, как это сделать.
2. **Поддержка встроенного программного обеспечения** — `cargo test -p norito` (упакованные функции есть в `scripts/run_norito_feature_matrix.sh`) Это было сделано в 2017 году.
3. **Загрузка документов** – Информационная поддержка `norito.md` и мультикодек, поддержка صفحات البوابة (`/reference/norito-codec` وهذا الملخص).
4. **Создание файла Norito-first** – создание файла JSON в формате JSON. Norito создан для `serde_json`, и он был установлен в США.

## روابط سريعة

- Сообщение: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Поддержка мультикодека: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Дополнительные функции: `scripts/run_norito_feature_matrix.sh`
- Дополнительная информация: `crates/norito/tests/`.

Установите флажок для встроенного программного обеспечения (`/norito/getting-started`) Отобразится байт-код, отвечающий за полезные нагрузки, в Norito.