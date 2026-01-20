---
id: address-display-guidelines
lang: ru
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Канонический источник
Эта страница отражает `docs/source/sns/address_display_guidelines.md` и теперь
служит канонической копией портала. Исходный файл остается для PR переводов.
:::

Кошельки, обозреватели и примеры SDK должны относиться к адресам аккаунтов как
к неизменяемым payload. Пример Android-кошелька в
`examples/android/retail-wallet` теперь демонстрирует требуемый UX-паттерн:

- **Две цели копирования.** Предоставьте две явные кнопки копирования: IH58
  (предпочтительно) и сжатая Sora-only форма (`snx1...`, второй по предпочтению). IH58 всегда безопасно
  делиться наружу и он используется в QR payload. Сжатая форма должна включать
  встроенное предупреждение, потому что работает только в Sora-aware приложениях.
  Пример Android подключает обе кнопки Material и их tooltips в
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, а
  демо iOS SwiftUI отражает тот же UX через `AddressPreviewCard` в
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, выбираемый текст.** Отрисовывайте обе строки моноширинным
  шрифтом и с `textIsSelectable="true"`, чтобы пользователи могли проверять
  значения без вызова IME. Избегайте редактируемых полей: IME может переписать
  kana или внедрить нулевой ширины кодовые точки.
- **Подсказки для неявного домена по умолчанию.** Когда селектор указывает на
  неявный домен `default`, показывайте подпись, напоминающую операторам, что
  суффикс не требуется. Обозреватели также должны выделять канонический доменный
  ярлык, когда селектор кодирует digest.
- **QR для IH58.** QR-коды должны кодировать строку IH58. Если генерация QR
  провалилась, покажите явную ошибку вместо пустого изображения.
- **Сообщение буфера обмена.** После копирования сжатой формы отправьте toast
  или snackbar, напоминающий пользователям, что она Sora-only и подвержена
  искажению IME.

Следование этим guardrails предотвращает повреждение Unicode/IME и удовлетворяет
критериям приемки дорожной карты ADDR-6 для UX кошельков/обозревателей.

## Скриншоты для сверки

Используйте следующие фикстуры при локализационных проверках, чтобы подписи
кнопок, tooltips и предупреждения оставались согласованными между платформами:

- Справка Android: `/img/sns/address_copy_android.svg`

  ![Справка Android двойного копирования](/img/sns/address_copy_android.svg)

- Справка iOS: `/img/sns/address_copy_ios.svg`

  ![Справка iOS двойного копирования](/img/sns/address_copy_ios.svg)

## Помощники SDK

Каждый SDK предоставляет удобный helper, возвращающий формы IH58 и сжатую, а
также строку предупреждения, чтобы UI-слои оставались согласованными:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` возвращает предупреждение о
  сжатой форме и добавляет его в `warnings`, когда вызывающие передают literal
  `snx1...`, чтобы обозреватели/панели кошельков могли показывать предупреждение
  Sora-only во время вставки/валидации, а не только при генерации сжатой формы.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Используйте эти helpers вместо переимплементации логики encode в UI-слоях.
JavaScript helper также раскрывает payload `selector` в `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`), чтобы UIs могли указать, является
ли селектор Local-12 или подкреплен registry, не парся сырой payload повторно.

## Демонстрация инструментирования обозревателя

<ExplorerAddressCard />

Обозреватели должны отражать телеметрию и доступность кошелька:

- Добавляйте `data-copy-mode="ih58|compressed|qr"` на кнопки копирования, чтобы
  фронтенды могли эмитить счетчики использования рядом с метрикой Torii
  `torii_address_format_total`. Демонстрационный компонент выше отправляет
  событие `iroha:address-copy` с `{mode,timestamp}` - подключите его к своему
  аналитическому/телеметрийному пайплайну (например, в Segment или NORITO
  коллектор), чтобы dashboards могли коррелировать использование форматов
  адресов на сервере с режимами копирования клиента. Также зеркальте счетчики
  доменов Torii (`torii_address_domain_total{domain_kind}`) в том же фиде, чтобы
  проверки вывода Local-12 могли экспортировать 30-дневное доказательство
  `domain_kind="local12"` напрямую из панели Grafana `address_ingest`.
- Сопоставляйте каждому контролу отдельные `aria-label`/`aria-describedby`,
  объясняющие, безопасен ли literal для обмена (IH58) или только для Sora
  (сжатый). Включайте подпись неявного домена в описание, чтобы вспомогательные
  технологии показывали тот же контекст.
- Экспонируйте live-регион (например, `<output aria-live="polite">...</output>`),
  который объявляет результаты копирования и предупреждения, совпадая с
  поведением VoiceOver/TalkBack, уже настроенным в примерах Swift/Android.

Это инструментирование удовлетворяет ADDR-6b, показывая, что операторы могут
наблюдать и прием Torii, и режимы копирования клиента до отключения Local
селекторов.

## Набор миграции Local -> Global

Используйте [Local -> Global toolkit](local-to-global-toolkit.md) для
автоматизации аудита и конверсии устаревших Local селекторов. Helper выводит и
JSON-отчет аудита, и конвертированный список IH58/сжатых значений, который
операторы прикладывают к readiness тикетам, а сопутствующий runbook связывает
панели Grafana и правила Alertmanager, которые ограничивают strict cutover.

## Быстрый справочник бинарной раскладки (ADDR-1a)

Когда SDK показывают продвинутые инструменты адресов (inspectors, подсказки
валидации, manifest builders), направляйте разработчиков к каноническому
wire-формату из `docs/account_structure.md`. Раскладка всегда
`header · selector · controller`, где биты header следующие:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) сегодня; ненулевые значения зарезервированы и
  должны приводить к `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` различает single (`0`) и multisig (`1`) контроллеры.
- `norm_version = 1` кодирует правила селектора Norm v1. Будущие нормы будут
  переиспользовать то же 2-битовое поле.
- `ext_flag` всегда `0`; установленные биты означают неподдерживаемые расширения
  payload.

Селектор следует сразу после header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI и SDK должны быть готовы показывать тип селектора:

- `0x00` = неявный домен по умолчанию (без payload).
- `0x01` = локальный digest (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = запись глобального registry (`registry_id:u32` big-endian).

Канонические hex-примеры, которые инструменты кошельков могут ссылать или
встраивать в docs/tests:

| Тип селектора | Канонический hex |
|---------------|---------------|
| Неявный по умолчанию | `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201` |
| Локальный digest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Глобальный registry (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

См. `docs/source/references/address_norm_v1.md` для полной таблицы selector/state
и `docs/account_structure.md` для полного байтового диаграммы.

## Принуждение канонических форм

Операторы, конвертирующие устаревшие Local кодировки в канонический IH58 или
сжатые строки, должны следовать CLI-флоу из ADDR-5:

1. `iroha address inspect` теперь выдает структурированное JSON-резюме с IH58,
   compressed и каноническими hex payload. Резюме также включает объект `domain`
   с полями `kind`/`warning` и отражает любой переданный домен через
   `input_domain`. Когда `kind` равен `local12`, CLI печатает предупреждение в
   stderr, а JSON-резюме отражает ту же подсказку, чтобы CI пайплайны и SDK могли
   ее показывать. Передавайте `--append-domain`, если хотите воспроизвести
   конвертированный encoding как `<ih58>@<domain>`.
2. SDK могут показать тот же warning/summary через JavaScript helper:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("snx1...@wonderland");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Helper сохраняет IH58 префикс, извлеченный из literal, если только вы явно не
  указали `networkPrefix`, поэтому резюме для не-default сетей не перерисовываются
  тихо с дефолтным префиксом.

3. Конвертируйте канонический payload, повторно используя `ih58.value` или
   `compressed` из резюме (или запросите другой encoding через `--format`). Эти
   строки уже безопасны для внешнего обмена.
4. Обновите manifests, registries и клиентские документы канонической формой и
   уведомите контрагентов, что Local селекторы будут отклоняться после cutover.
5. Для больших наборов данных запустите
   `iroha address audit --input addresses.txt --network-prefix 753`. Команда
   читает literals, разделенные переводом строки (комментарии с `#` игнорируются,
   а `--input -` или отсутствие флага использует STDIN), выдает JSON-отчет с
   каноническими/IH58/сжатыми резюме для каждой записи и считает ошибки парсинга
   и предупреждения Local домена. Используйте `--allow-errors` при аудите
   `--fail-on-warning`, когда операторы готовы запрещать Local селекторы в CI.
6. Когда нужна построчная перепись, используйте
  Для таблиц remediation Local селекторов используйте
  для экспорта CSV `input,status,format,...`, который подсвечивает канонические
  кодировки, предупреждения и ошибки парсинга за один проход.
   Helper по умолчанию пропускает не-Local строки, конвертирует каждую оставшуюся
   запись в запрошенный encoding (IH58/сжатый/hex/JSON) и сохраняет исходный домен
   при `--append-domain`. Совмещайте с `--allow-errors`, чтобы продолжать скан даже
   если dump содержит поврежденные literals.
7. Автоматизация CI/lint может запускать `ci/check_address_normalize.sh`, которая
   извлекает Local селекторы из `fixtures/account/address_vectors.json`,
   конвертирует их через `iroha address normalize`, и повторно запускает
   `iroha address audit --fail-on-warning`, чтобы доказать, что релизы больше не
   эмитят Local digests.

`torii_address_local8_total{endpoint}` вместе с
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` и панель Grafana
`dashboards/grafana/address_ingest.json` дают сигнал enforcement: когда
продакшн-дашборды показывают ноль легитимных Local отправок и ноль Local-12
коллизий в течение 30 дней подряд, Torii переведет Local-8 gate в hard-fail на
mainnet, а затем Local-12, когда глобальные домены получат соответствующие записи
registry. Считайте вывод CLI операторским уведомлением об этом замораживании -
та же строка предупреждения используется в SDK tooltips и автоматизации, чтобы
сохранять паритет с критериями выхода дорожной карты. Torii теперь по умолчанию
кластерах при диагностике регрессий. Продолжайте зеркалировать
`torii_address_domain_total{domain_kind}` в Grafana
(`dashboards/grafana/address_ingest.json`), чтобы пакет доказательств ADDR-7
мог подтвердить, что `domain_kind="local12"` оставался нулевым в течение
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) добавляет три
guardrails:

- `AddressLocal8Resurgence` пейджит, когда контекст сообщает о свежем инкременте
  Local-8. Остановите rollouts strict-mode, найдите проблемный SDK в дашборде и,
  возврата сигнала к нулю - затем восстановите дефолт (`true`).
- `AddressLocal12Collision` срабатывает, когда два Local-12 лейбла хэшируются в
  один digest. Приостановите promotions manifests, запустите Local -> Global
  toolkit для аудита mapping digests и координируйте с Nexus governance перед
  перевыпуском registry entry или повторным включением downstream rollouts.
- `AddressInvalidRatioSlo` предупреждает, когда флотский invalid ratio (исключая
  отказы Local-8/strict-mode) превышает SLO 0.1% в течение десяти минут. Используйте
  `torii_address_invalid_total` для поиска ответственного контекста/причины и
  согласуйте с владельцем SDK прежде чем включать strict-mode снова.

### Фрагмент релизной заметки (кошелек и обозреватель)

Включите следующий bullet в release notes кошелька/обозревателя при cutover:

> **Адреса:** Добавлен helper `iroha address normalize --only-local --append-domain`
> и подключен в CI (`ci/check_address_normalize.sh`), чтобы пайплайны кошелька/
> обозревателя могли конвертировать устаревшие Local селекторы в канонические
> IH58/сжатые формы до блокировки Local-8/Local-12 на mainnet. Обновите любые
> кастомные экспорты, чтобы запускать команду и прикладывать нормализованный
> список к evidence bundle релиза.
