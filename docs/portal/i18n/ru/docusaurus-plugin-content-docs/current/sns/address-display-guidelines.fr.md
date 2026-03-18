---
lang: ru
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импортируйте ExplorerAddressCard из @site/src/comComponents/ExplorerAddressCard;

:::note Источник канонический
Эта страница отражена `docs/source/sns/address_display_guidelines.md` и установлена
обслуживание канонического копирования порта. Le fichier source reste pour les PRs
де перевод.
:::

Портфели, исследователи и примеры SDK, изменяющие адреса
de compte comme des payloads immutables. L'Exemple de portefeuille розничная торговля Android
в `examples/android/retail-wallet` для обслуживания шаблона UX необходимы:

- **Две кнопки для копирования.** Четыре явных кнопки для копирования: I105.
  (предпочитаю) и форму сжатия только для Sora (`sora...`, второй выбор). I105 уже сегодня
  убедитесь, что вы используете внешнюю часть и питаете полезную нагрузку QR. Вариант компресса
  сделайте включение встроенного оповещения, потому что он не работает, что в нем
  Приложения оплачиваются по номиналу Соры. Пример Android-ветви двух бутонов Material et
  Leurs всплывающие подсказки в
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` и т. д.
  демо-версия iOS SwiftUI reflete le meme UX через `AddressPreviewCard` в
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, возможен выбор текста.** Добавьте две цепочки с полицией.
  monospace и `textIsSelectable="true"` для того, чтобы самые сильные пользователи
  инспектор les valeurs sans invoquer un IME. Evitez les champs редактируемые: les
  IME может повторно записывать канаву или ввод точек кодирования большего нуля.
- **Указания домена по умолчанию неявные.** Когда выбран пункт выбора
  le Domaine Implicit `default`, affichez une Legende Rappelant aux Operatingurs
  Суффикс qu'aucun n'est requis. Les explorers doivent aussi mettre en avant
  каноническая метка домена, и селектор кодирует дайджест.
- **QR I105.** QR-коды кодируют цепочку I105. Si la поколение du
  QR-эхо отображает явную ошибку вместо изображения.
- **Сообщение на печатной бумаге.** После того, как вы скопируете форму для сжатия, удалите ее.
  тост или закусочная Rappelant aux utilisateurs qu'elle est Sora-only et sujette
  а-ля искажение IME.

Будьте осторожны с повреждением Unicode/IME и удовлетворяйте критериям
принятие дорожной карты ADDR-6 для порта/исследователя пользовательского интерфейса.

## Захват ссылок

Используйте соответствующие ссылки из обзоров локализации для гарантии
que les libelles de boutons, всплывающие подсказки и рекламные объявления остаются в стороне
пластинчатые формы:

- Эталонный Android: `/img/sns/address_copy_android.svg`

  ![Ссылка на двойную копию Android](/img/sns/address_copy_android.svg)

- Ссылка iOS: `/img/sns/address_copy_ios.svg`

  ![Ссылка на двойную копию iOS](/img/sns/address_copy_ios.svg)

## SDK помощников

Chaque SDK предоставляет помощника по удобству, который возвращает формы I105 и т. д.
Сжатие, которое делает цепочка объявлений для того, чтобы пользовательский интерфейс диванов остался в памяти
когерентес:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (И18НИ00000028Х)
- Инспектор JavaScript: `inspectAccountId(...)` return la Chaine.
  Сжатое объявление и помощь `warnings`, когда апеллянты
  Fournissent - буквальный `sora...`, для тех исследователей/таблиц на границе
  portefeuille puissent afficher l'avertissement подвеска только для Sora les flux de
  коллаж/проверка plutot que seulement lorsqu'ils Generent eux-memes la forme
  компресс.
- Питон: `AccountAddress.display_formats(network_prefix: int = 753)`
- Свифт: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Котлин: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (И18НИ00000035Х)

Используйте эти помощники вместо переопределения логики кодирования в данных
пользовательский интерфейс диванов. Помощник JavaScript предоставляет всю полезную нагрузку `selector` в
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) для этих пользовательских интерфейсов
может быть указан, если выбран Local-12 или добавление без регистрации
reparser le payload brut.

## Демонстрация инструментов исследования



Les explorers doivent воспроизводят тяжелую работу телеметрии и доступа
fait pour le portefeuille:

- Appliquez `data-copy-mode="i105|i105_default|qr"` для бутонов для копирования в ближайшее время
  les front-ends puissent emettre des compteurs d'usage en Paralele de la
  метрика Torii `torii_address_format_total`. Le composant demo ci-dessus
  Отправь вечер `iroha:address-copy` с `{mode,timestamp}` - освободите место
  ваш конвейер аналитики/телеметрии (например, отправитель сегмента или
  Коллекционная база Sur NORITO) для мощного коррелятора использования приборных панелей
  Формат адреса сервера с режимами копирования клиента. Рефлетез
  aussi les Compteurs de Domaine Torii (`torii_address_domain_total{domain_kind}`)
  в потоке мемов для que les revues de retrait Local-12 — мощный экспортер
  предыдущая 30-дневная трансляция `domain_kind="local12"`, направленная на следующую картину
  `address_ingest` от Grafana.
- Ассоциация контроля над индикацией `aria-label`/`aria-describedby`
  Различия, которые объясняются, если буквально являются частью (I105) или только для Соры
  (сжать). Включите неявную легенду о домене в описание для
  que les technology d'assistance refletent le meme contexte que l'affichage.
- Выставьте регион в реальном времени (например, `<output aria-live="polite">...</output>`) здесь
  объявите о результатах копирования и предупреждениях, приведите их в соответствие
  Поддержка VoiceOver/TalkBack по кабелю в примерах Swift/Android.

Этот инструментарий удовлетворяет требованиям ADDR-6b и обеспечивает возможность его использования операторами.
Наблюдатель за приемом пищи Torii и режимами копирования клиента перед тем, как
selecteurs Локальные настройки деактивируются.

## Инструментарий для миграции Локальный -> ГлобальныйИспользуйте [инструментарий Local -> Global] (local-to-global-toolkit.md) для
автоматизатор аудита и преобразования селекторов. Местные наследники. Ле помощник
получить доступ к протоколу аудита JSON и списку преобразований I105/сжатия, который
Совместные операторы с билетами готовности, а также ассоциация Runbook
лежат информационные панели Grafana и правила Alertmanager, которые вызывают опасения
переключение в строгий режим.

## Эталон быстрой компоновки бинарного файла (ADDR-1a)

Когда SDK выставляет на адрес адрес (инспекторы, указания
проверка, конструкторы манифеста), пункты разработки и формата
проводной захват Canonique в `docs/account_structure.md`. Le Layout est Toujours
`header · selector · controller`, или биты заголовка:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (биты 7-5) aujourd'hui; les valeurs Non Zero Sont Reservees
  рычаг et doivent `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` отличает простые контроллеры (`0`) от мультиподписей (`1`).
- `norm_version = 1` кодирует правила выбора Norm v1. Фьючерсы на Лес Норм
  повторно использовать мем чемпиона 2 бита.
- `ext_flag` или `0`; les bits actifs indiquent des Extensions de
  полезная нагрузка не несет ответственности.

Выбор сразу за заголовком:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Для пользовательского интерфейса и SDK необходимо указать тип выбора:

- `0x00` = неявный домен по умолчанию (без полезной нагрузки).
- `0x01` = локальный дайджест (12-байтовый `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = вход в глобальную регистрацию (`registry_id:u32` с прямым порядком байтов).

Примеры шестнадцатеричных канонических символов, которые могут использоваться для переноски или интегратора
дополнительные документы/тесты:

| Тип выбора | Шестнадцатеричный канонический |
|---------------|---------------|
| Неявное по умолчанию | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Локальный дайджест (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Зарегистрироваться глобально (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` для полной таблицы
selecteur/etat et `docs/account_structure.md` для полной диаграммы
байты.

## Импозер канонических форм

Операторы, которые преобразуют коды Local Herites в каноническом I105
или в цепочках сжатий, которые позволяют документировать рабочий процесс CLI в соответствии с ADDR-5:

1. `iroha tools address inspect` поддерживает возобновление структуры JSON с I105,
   сжимать и хранить полезные данные в шестнадцатеричном формате. Резюме включает в себя австралийский объект
   `domain` с чемпионами `kind`/`warning` и reflete Tout Domaine Fourni через
   чемпион `input_domain`. Когда `kind` или `local12`, используйте CLI
   предупреждение о стандартном сообщении и резюме JSON, отражающее мем, отправленный для того, чтобы
   Конвейеры CI и SDK могут быть добавлены. Пассез `legacy  suffix`
   lorsque vous voulez rejouer l'encodage Converti sous la forme `<i105>@<domain>`.
2. Les SDK может аффилировать мемы с предупреждением/возобновить через le helper
   JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  Помощник по сохранению префикса I105 обнаруживает depuis le буквально sauf si vous
  Fournissez Explosion `networkPrefix`, сделайте резюме для des reseaux
  не по умолчанию не требуется повторная передача молчания с префиксом по умолчанию.

3. Преобразуйте каноническую полезную нагрузку в повторно используемую на полях `i105.value` или
   `i105_default` для возобновления (или требуется другое кодирование через `--format`). Цес
   Chaines sont Deja Sures a Partager En Externe.
4. Регистрация манифестов, регистров и документов в течение дня, ориентированных на клиента с учетом
   формировать канон и уведомлять контрагентов, выбранных локальным сервером
   отказывается от завершения переключения.
5. Наливайте игры в массовом порядке, выполняйте
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Ла-команде
   Lit des literaux отделяется от новой линии (les commentaires commencant par
   `#` игнорируется, а `--input -` или установленный флаг используют STDIN), и возникает взаимопонимание.
   JSON с каноническими резюме/I105/сжатием для ввода и т. д.
   Ошибки синтаксического анализа связаны с предупреждениями о локальном домене. Утилисез
   `--allow-errors` lors de l'audit de dumps herites contenant des lignes
   паразиты и блокировка автоматизации с помощью `strict CI post-check` lorsque les
   Операторы могут блокировать локальные параметры выбора в CI.
6. Если вы хотите, чтобы вы переписывались по прямой, используйте
  Pour les feuilles de Calcule de remediation des selecteurs Local, utilisez
  для экспорта в CSV `input,status,format,...`, который доступен в новых кодировках
  каноники, рекламные объявления и уроки разбора в одном месте.
   Помощник игнорирует нелокальные линии по умолчанию, конвертирует отдельные входы
   восстановить требуемую кодировку (I105/compresse/hex/JSON) и сохранить файл
   Оригинальный домен quand `legacy  suffix` активен. Ассоциация
   `--allow-errors` для продолжения анализа мема и сброса содержимого
   литературные малые формы.
7. Автоматизация CI/lint может выполнять `ci/check_address_normalize.sh`, которая
   извлечь локальные файлы выбора `fixtures/account/address_vectors.json`, файлы
   конвертируйте его через `iroha tools address normalize` и радуйтесь
   `iroha tools address audit` для проверки выпусков
   n'emetent плюс дайджесты Local.`torii_address_local8_total{endpoint}` плюс
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` и таблица Grafana
`dashboards/grafana/address_ingest.json` четыре сигнала приложения:
une fois que les Dashboards de Production Montrent Zero Soumissions Local
Законность и отсутствие коллизий Кулон Local-12, 30 дней подряд, Torii
basculera la porte Local-8 в строгом соответствии с основной сетью, suivi par Local-12 une
Для того, чтобы глобальные домены были зарегистрированы корреспондентами.
Рассмотрим вылазку CLI с оператором для этого геля - цепочка мемов
 реклама используется в SDK всплывающих подсказок и для автоматизации
сохраняйте соответствие с критериями вылета дорожной карты. Torii использовать
что касается разработки/тестирования кластеров для диагностики регрессий. Продолжить
миройтер `torii_address_domain_total{domain_kind}` в Grafana
(`dashboards/grafana/address_ingest.json`) для предварительного пакета ADDR-7
puisse montrer que `domain_kind="local12"` - это нулевой кулон la fenetre
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) добавлен втрое
барьеры:

- `AddressLocal8Resurgence` страница, которая дает новый сигнал контекста
  приращение Local-8. Остановите развертывание в строгом режиме, локализуйте
  Surface SDK не работает на приборной панели и, если это необходимо, определено временно
  по умолчанию (`true`).
- `AddressLocal12Collision` se declenche lorsque deux labels Local-12 hashed
  дайджест мемов vers le. Отметьте паузу в рекламных акциях и выполните их.
  набор инструментов Local -> Global для проверки сопоставления дайджестов и координации
  с управлением Nexus перед повторным входом в регистрацию или
  снова сжать развертывания в aval.
- `AddressInvalidRatioSlo` предотвратит недействительную пропорцию в l'echelle de la
  флот (без исключений сбросов Local-8/строгого режима) depasse le SLO de 0,1%
  кулон дикс минут. Используйте `torii_address_invalid_total` для идентификатора файла.
  контекст/причина ответственности и координации с предварительным использованием собственного оборудования SDK
  de reenclencher le mode strict.

### Extrait de note de Release (portefeuille et explorer)

Включите следующую пулю в примечания к выпуску portefeuille/explorateur
lors du Cutover:

> **Адреса:** Адрес помощника `iroha tools address normalize`
> и ветвь в CI (`ci/check_address_normalize.sh`) для этих конвейеров
> portefeuille/explorateur puissent Convertir les selecteurs Local Herites vers
> канонические формы I105/сжатия до того, как Local-8/Local-12 soient
> Блоки в сети. Выполните экспорт персонализированных документов в течение дня для исполнителя
> Commande et joinre la liste Normalisee au Bundle de Preuve de Release.