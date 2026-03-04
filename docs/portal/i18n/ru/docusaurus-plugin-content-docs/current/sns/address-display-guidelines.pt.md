---
lang: ru
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импортируйте ExplorerAddressCard из @site/src/comComponents/ExplorerAddressCard;

:::примечание Fonte canonica
Эта страница написана `docs/source/sns/address_display_guidelines.md` и агора служит
как копия канонического портала. Постоянный архив для PR-де
перевод.
:::

Картины, исследования и примеры SDK для разработки исходного текста как
полезные нагрузки imutaveis. Пример карты розничной торговли Android на
`examples/android/retail-wallet` перед демонстрацией или подготовкой UX exigido:

- **Dois alvos de copyia.** Envie dois botoes de copyia явно: IH58
  (предпочтительно) и форма заключения соглашения (`sora...`, второй вариант). IH58 и всегда в безопасности
  соберите внешние и пищевые продукты или полезную нагрузку для QR. Вариант компромисса
  я должен включить в список встроенных приложений, чтобы они могли работать с совместимыми приложениями.
  Сора. Примеры розничных карт для Android, включая ботоматериалы и свои собственные
  всплывающие подсказки
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, и а
  демо-версия SwiftUI для iOS или создание пользовательского интерфейса через `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, выбор текста.** Рендеринг заголовков в виде строк с использованием шрифта.
  monospace e `textIsSelectable="true"` для проверки пользователей
  значения, которые необходимо вызвать IME. Evite Campos Editaveis: IMEs Podem Reescrever Kana
  или введите понтос-де-кодиго-де-ларгура ноль.
- **Dicas de dominio Padrao Implicito.** Когда вы выбираете способ доминио
  неявно `default`, самая легенда лембрандо ос операдорс де дие ненум
  суфиксо и необходимо. Exploradores также должны быть удалены или лейблом dominio
  canonico quando o seletor codifica um дайджест.
- **QR IH58.** Код QR позволяет кодифицировать строку IH58. Посмотрите, как работает QR
  Фальхар, большая часть явной ошибки в каждом изображении на начальном этапе.
- **Mensageria da area de Transferencia.** Депонирование копии по форме сделки,
  выбрасывайте тосты или закусочные, лембрандо или обычные пользователи, которые делают это и когда-нибудь Сора и
  возможно искажение IME.

Следите за тем, чтобы предотвратить повреждение Unicode/IME и учитывать критерии
aceitacao do roadmap ADDR-6 для пользовательского интерфейса карт/исследователей.

## Захваты справочных данных

Используйте в качестве справочной информации, чтобы гарантировать, что вы внесете изменения в локализацию.
вращающиеся боты, всплывающие подсказки и уведомления о том, что они установлены на платформах:

- Справочная версия Android: `/img/sns/address_copy_android.svg`.

  ![Справочник по Android-копии](/img/sns/address_copy_android.svg)

- Ссылка iOS: `/img/sns/address_copy_ios.svg`.

  ![Ссылка на дублированную копию iOS](/img/sns/address_copy_ios.svg)

## Помощники SDK

Cada SDK предоставляет вспомогательные средства, которые возвращаются в форматах IH58 и компримируются.
Добавьте строку, указывающую на то, что пользовательский интерфейс соответствует стандарту:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (И18НИ00000028Х)
- Инспектор JavaScript: `inspectAccountId(...)` возвращает строку предупреждения.
  comprimida e a anexa a `warnings` quando chamadores fornecem um literal
  `sora...`, для того, чтобы панели управления картой/исследователем могли быть показаны или предупреждены
  somente Sora durante fluxos de colagem/validacao em vez de apenas quando geram
  Форма компримида по делу о собственной собственности.
- Питон: `AccountAddress.display_formats(network_prefix: int = 753)`
- Свифт: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Котлин: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (И18НИ00000035Х)

Используйте эти помощники для переопределения логики кодирования в графическом интерфейсе пользователя. О
помощник JavaScript также выставляет полезную нагрузку `selector` в `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) для того, чтобы пользовательский интерфейс был указан для выбора и
Local-12 или измененный реестр для повторной обработки или грубой обработки полезной нагрузки.

## Демо-версия инструмента для исследования



Разработчики могут изучить работу по телеметрии и доступу к ней
Картайра:

- Aplique `data-copy-mode="ih58|compressed|qr"` как ботинки для копирования, которые нужны
  внешние интерфейсы possam emitir contadores de uso junto com a metrica Torii
  `torii_address_format_total`. Демо-компоненты не совпадают с событиями
  `iroha:address-copy` com `{mode,timestamp}`; Подключиться к своему трубопроводу
  аналитика/телеметрия (например, зависть к сегменту или множеству NORITO)
  для того, чтобы панели мониторинга могли коррелировать или использовать форматы endereco do
  сервидор в режиме копирования для клиентов. Реплика tambem os contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) нет параметра Mesmo Feed
  que revisoes de aposentadoria Local-12 может быть экспортирован за 30 дней
  `domain_kind="local12"` непосредственно нанесите краску `address_ingest` на Grafana.
- Emparelhe cada controle com pistas `aria-label`/`aria-describedby` отдельные
  que expliquem se um um буквально e seguro para compartilhar (IH58) или somente sora
  (компримидо). Включена легенда о господстве, подразумеваемая в описании для того, что
  tecnologia Assistiva Mostre или Mesmo Contexto Exibido Visualmente.
- Exponha uma regiao viva (например, `<output aria-live="polite">...</output>`)
  сообщать о результатах копирования и уведомления, а также сообщить о результатах
  VoiceOver/TalkBack и подключены к нашим примерам Swift/Android.

Этот инструмент удовлетворяет требованиям ADDR-6b и позволяет использовать его для наблюдения
после приема Torii, каковы способы копирования, сделанные клиентом до того, как он
выберите Местные desativados.

## Инструментарий для миграции Локальный -> Глобальный

Используйте [toolkit Local -> Global](local-to-global-toolkit.md) для автоматизации
пересмотр и обмен выбранными местными альтернативами. О помощник, излучай танто или родственник
JSON аудитории в списке конвертированных IH58/comprimida que Operadores
экзамен и билеты готовности, экзамен или сводные таблицы runbook associado vincula
Grafana и Regras Alertmanager, который контролирует или переключает режим записи.

## Быстрая ссылка на бинарный макет (ADDR-1a)Когда SDK использует инструменты для проверки (инспекторов,
validacao, строители манифеста), апонте desenvolvedores для формата провода
canonico em `docs/account_structure.md`. O макет всегда e
`header · selector · controller`, биты операционной системы делают заголовок sao:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (биты 7-5) нормально; значения не равны нулю, резервы и разработки
  левантар `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` отличает простые элементы управления (`0`) от многоподписных (`1`).
- `norm_version = 1` кодируется как исправление селектора Norm v1. Нормы будущего
  повторно использовать или снова использовать 2 бита.
- `ext_flag` и всегда `0`; Bits ativos indicam extensoes de payload nao
  поддержка.

Немедленно выберите переключатель заголовка:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Пользовательские интерфейсы и SDK должны быть разработаны для показа или выбора типа:

- `0x00` = dominio Padrao Implicito (полезная нагрузка SEM).
- `0x01` = локальный дайджест (12-байтовый `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = глобальный вход в реестр (`registry_id:u32` с прямым порядком байтов).

Шестнадцатеричные канонические примеры, которые позволяют установить карту или установить ее
 документы/тесты:

| Тип выбора | Шестнадцатеричный канонико |
|---------------|---------------|
| Неявное падение | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Локальный дайджест (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Глобальный реестр (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` для полной таблицы
селектор/установка и `docs/account_structure.md` для полной байтовой диаграммы.

## Импорт канонических форм

Операции по преобразованию кодифицированных локальных альтернатив для канонического или канонического IH58
Строки должны быть включены в документацию CLI рабочего процесса в ADDR-5:

1. `iroha tools address inspect` назад выпустить резюме в формате JSON, созданное с помощью IH58,
   comprimido e payloads hex canonicos. Резюме также включает в себя объект
   `domain` с кампусом `kind`/`warning` и ecoa qualquer dominio fornecido через o
   кампо `input_domain`. Когда `kind` и `local12`, интерфейс командной строки доступен для просмотра
   Stderr и резюме JSON Ecoa и большая ориентация для конвейеров CI и SDK
   поссам эксиби-ла. Passe `--append-domain` всегда, что нужно воспроизвести
   конвертированное кодирование как `<ih58>@<domain>`.
2. SDK можно открыть или отправить сообщение/возобновить с помощью вспомогательного JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  О помощнике по сохранению или префиксе IH58, обнаруженном в буквальном смысле слова, которое я сказал
  явное `networkPrefix`, резюме для Redes Nao Padrao Nao Sao
  повторно рендерятся без звука с префиксом падрао.3. Преобразование или повторное использование полезных данных Canon `ih58.value` или
   `compressed` сделайте резюме (или запросите дополнительное кодирование через `--format`). Эссас
   Струны должны быть безопасными для внешнего использования.
4. Актуализация манифестов, реестров и документов, входящих в состав клиента, в форме.
   canonica e notifique as contrapartes de que seletores Local serao rejeitados
   когда или переключение для заключения.
5. Чтобы объединиться с массой, казните
   `iroha tools address audit --input addresses.txt --network-prefix 753`. О командо
   le literais separados por nova linha (начавшиеся комментарии от `#` sao)
   игнорируйте, и `--input -` или ненужный флаг США STDIN), выдайте отношение JSON
   com резюме canonicos/IH58/comprimidos для каждого входа и ошибок
   анализировать и получать сведения о местном домене. Используйте `--allow-errors` или альтернативные дампы аудита.
   com linhas lixo, e trave a automacao com `--fail-on-warning` quando os
   Operadores estiverem prontos для блокировки выбранных локальных значений без CI.
6. Когда вы напишете линху и линху, используйте
  Планы восстановления селекторов Местное, используйте
  для экспорта в CSV `input,status,format,...`, который необходимо кодировать
  Канонические сообщения, обратите внимание и начните анализировать свой уникальный пассад.
   О помощник, игнорируй линхас нао, местный житель, конвертируй каждый раз до востребования
   для запрошенного кодирования (IH58/comprimido/hex/JSON) и сохранения домена
   Оригинальный Quando `--append-domain` и определенный. Комбинат ком `--allow-errors`
   Чтобы продолжить варредуру, когда вы сбросите некачественную литературу.
7. Автоматический запуск режима CI/lint `ci/check_address_normalize.sh`, что дополнительно
   selectores Local de `fixtures/account/address_vectors.json`, конвертировать через
   `iroha tools address normalize`, и повторное выполнение
   `iroha tools address audit --fail-on-warning` для проверки того, что релизы будут выпущены
   mais дайджесты Local.

`torii_address_local8_total{endpoint}` junto com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, и краска Grafana
`dashboards/grafana/address_ingest.json` fornecem или синал де принудительного исполнения: quando
OS Dashboards de Producao Mostram Zero Envios Local legitimos e Zero colisoes
Local-12 для 30 последовательных дней, Torii для доступа к шлюзу Local-8 для аварийного сбоя
в основной сети перейдите к Local-12, когда всемирные домены будут введены
регистр корреспондентов. Обратите внимание на CLI, как на предупреждение оператора
Это загустевшее сообщение — основная строка уведомления и использование всплывающих подсказок SDK и
автоматический выбор в соответствии с критериями, указанными в дорожной карте. Torii назад
кластеры разработки/тестирования или диагностические регрессии. Продолжить эспельхандо
`torii_address_domain_total{domain_kind}` нет Grafana
(`dashboards/grafana/address_ingest.json`) для пакета доказательств ADDR-7
Докажите, что `domain_kind="local12"` постоянно на нуле, требуется 30 дней
 Перед подключением к основной сети нужно выбрать альтернативные варианты. О пакете Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) дополнительные ограждения:- `AddressLocal8Resurgence` страница всегда остается контекстным отчетом с приращением
  Локальный-8 ново. Выполните развертывание режима Estrito, локализуйте поверхностный SDK
  Responsavel No Dashboard e, Se necessario, Defina Temporariamente
  падрао (`true`).
- `AddressLocal12Collision` dispara quando dois метки Local-12 fazem hash para
  о, дайджест. Приостановите рекламные объявления манифеста, запустите набор инструментов Local -> Global.
  для проверки карт дайджестов и координации с правительством Nexus раньше
  возврат к входу в реестр или повторное развертывание последующих операций.
- `AddressInvalidRatioSlo` уведомление о том, что часть недействительна во всем мире
  (исключая ограничения Local-8/строгого режима) превышает SLO на 0,1% в каждую минуту.
  Используйте `torii_address_invalid_total` для локализации контекста или ответа на запрос.
  Координируйте свои действия с собственным SDK перед возвращением или режимом проживания.

### Trecho de nota de Release (карта и исследователь)

Включите следующий маркер в примечаниях к выпуску карты/эксплорадора к отправке или
переключение:

> **Enderecos:** Дополнительный или помощник `iroha tools address normalize --only-local --append-domain`
> Подключен CI (`ci/check_address_normalize.sh`) для конвейеров
> Конвертер карт/эксплорадора Поссама: местные альтернативы для форматов
> канонические IH58/comprimidas до Local-8/Local-12 были полностью заблокированы
> Основная сеть. Настройте экспорт персонализированных параметров для просмотра или команды и
> добавьте список нормализации или пакет доказательств освобождения.