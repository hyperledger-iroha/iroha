---
lang: ru
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.he.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: Settlement-iso-mapping
заголовок: Урегулирование ↔ Картирование полей ISO 20022
Sidebar_label: Мировое соглашение ↔ ISO 20022
описание: Каноническое сопоставление между потоками расчетов Iroha и мостом ISO 20022.
---

:::обратите внимание на канонический источник
:::

## Расчет ↔ Картирование полей ISO 20022

В этом примечании отражено каноническое сопоставление между инструкциями расчета Iroha.
(`DvpIsi`, `PvpIsi`, потоки обеспечения репо) и используемые сообщения ISO 20022.
у моста. Он отражает структуру сообщений, реализованную в
`crates/ivm/src/iso20022.rs` и служит ссылкой при производстве или
проверка полезных данных Norito.

### Политика справочных данных (идентификаторы и проверка)

Эта политика объединяет предпочтения идентификатора, правила проверки и справочные данные.
обязательства, которые мост Norito ↔ ISO 20022 должен обеспечить перед отправкой сообщений.

**Точки привязки внутри сообщения ISO:**
- **Идентификаторы приборов** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (или эквивалентное поле инструмента).
- **Стороны/агенты** → `DlvrgSttlmPties/Pty` и `RcvgSttlmPties/Pty` для `sese.*`,
  или структуры агентов в `pacs.009`.
- **Счета** → `…/Acct` элементы для счетов депо/кассы; отзеркалить бухгалтерскую книгу
  `AccountId` в `SupplementaryData`.
- **Собственные идентификаторы** → `…/OthrId` с `Tp/Prtry` и зеркально отражены в
  `SupplementaryData`. Никогда не заменяйте регулируемые идентификаторы собственными.

#### Предпочтение идентификатора по семейству сообщений

##### `sese.023` / `.024` / `.025` (расчет по ценным бумагам)

- **Прибор (`FinInstrmId`)**
  - Предпочтительно: **ISIN** под номером `…/ISIN`. Это канонический идентификатор CSD/T2S.[^anna]
  - Резервные варианты:
    - **CUSIP** или другой NSIN под `…/OthrId/Id` с `Tp/Cd`, установленным из внешнего ISO.
      список кодов (например, `CUSP`); включите эмитента в `Issr`, если это необходимо.[^iso_mdr]
    - **Идентификатор актива Norito** как собственность: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` и
      запишите то же значение в `SupplementaryData`.
  - Дополнительные дескрипторы: **CFI** (`ClssfctnTp`) и **FISN**, если они поддерживаются для упрощения.
    сверка.[^iso_cfi][^iso_fisn]
- **Стороны (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  – Предпочтительно: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Резервный вариант: **LEI**, когда версия сообщения предоставляет выделенное поле LEI; если
    отсутствуют, имеют собственные идентификаторы с четкими метками `Prtry` и включают BIC в метаданные.[^iso_cr]
- **Место расчетов/место проведения** → **MIC** для места проведения и **BIC** для ЦДЦБ.[^iso_mic]

##### `colr.010` / `.011` / `.012` и `colr.007` (управление обеспечением)

- Следуйте тем же правилам для инструментов, что и для `sese.*` (предпочтительно ISIN).
- Стороны используют **BIC** по умолчанию; **LEI** допускается, если он указан в схеме.[^swift_bic]
– В денежных суммах должны использоваться коды валют **ISO 4217** с правильными второстепенными единицами.[^iso_4217]

##### `pacs.009` / `camt.054` (финансирование и заявления PvP)- **Агенты (`InstgAgt`, `InstdAgt`, агенты должника/кредитора)** → **BIC** с дополнительным
  LEI, где это разрешено.[^swift_bic]
- **Аккаунты**
  - Межбанковские операции: идентифицируются по **BIC** и внутренним ссылкам на счета.
  - Заявления для клиентов (`camt.054`): включите **IBAN**, если он присутствует, и подтвердите его.
    (длина, правила страны, контрольная сумма mod-97).[^swift_iban]
- **Валюта** → **3-буквенный код ISO 4217**, с учетом округления до младших единиц.[^iso_4217]
- **Прием Torii** → Отправьте ветки финансирования PvP через `POST /v2/iso20022/pacs009`; мост
  требует `Purp=SECU` и теперь обеспечивает перекрестные переходы BIC при настройке справочных данных.

#### Правила проверки (применяются перед отправкой)

| Идентификатор | Правило проверки | Заметки |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` и контрольная цифра Луна (mod-10) согласно ISO 6166, Приложение C | Отклонить до выброса моста; предпочитают обогащение вверх по течению.[^anna_luhn] |
| **КУСИП** | Regex `^[A-Z0-9]{9}$` и модуль-10 с 2 весами (символы сопоставляются с цифрами) | Только когда ISIN недоступен; карта через переход ANNA/CUSIP после получения источника.[^cusip] |
| **ЛЕИ** | Regex `^[A-Z0-9]{18}[0-9]{2}$` и контрольная цифра mod-97 (ISO 17442) | Перед принятием проверьте сверку ежедневных дельта-файлов GLEIF.[^gleif] |
| **БИК** | Регулярное выражение `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Необязательный код филиала (последние три символа). Подтвердите активный статус в файлах RA.[^swift_bic] |
| **МИК** | Сохранение из файла ISO 10383 RA; убедитесь, что площадки активны (нет флага завершения `!`) | Пометить выведенные из эксплуатации MIC перед выпуском.[^iso_mic] |
| **ИБАН** | Длина для конкретной страны, прописные буквы и цифры, mod-97 = 1 | Используйте реестр, поддерживаемый SWIFT; отклонять структурно недействительные номера IBAN.[^swift_iban] |
| **Идентификаторы собственных учетных записей/участников** | `Max35Text` (UTF-8, ≤35 символов) с обрезанными пробелами | Применяется к полям `GenericAccountIdentification1.Id` и `PartyIdentification135.Othr/Id`. Отклоняйте записи длиной более 35 символов, чтобы полезные данные моста соответствовали схемам ISO. |
| **Идентификаторы прокси-аккаунтов** | Непустой `Max2048Text` под `…/Prxy/Id` с дополнительными кодами типа в `…/Prxy/Tp/{Cd,Prtry}` | Хранится вместе с основным номером IBAN; для проверки по-прежнему требуются номера IBAN, при этом принимаются дескрипторы прокси (с дополнительными кодами типов) для зеркалирования PvP-рельсов. |
| **КФИ** | Шестизначный код, прописные буквы с использованием таксономии ISO 10962 | Необязательное обогащение; убедитесь, что символы соответствуют классу инструмента.[^iso_cfi] |
| **ФИСН** | До 35 символов, прописные буквы и цифры плюс ограниченное количество знаков препинания | Необязательный; усекать/нормализовать в соответствии с рекомендациями ISO 18774.[^iso_fisn] |
| **Валюта** | Трехбуквенный код ISO 4217, масштаб определяется второстепенными единицами | Суммы необходимо округлять до разрешенных десятичных знаков; применить на стороне Norito.[^iso_4217] |

#### Обязательства по сохранению пешеходного перехода и данных– Поддерживайте перекрестные переходы **ISIN ↔ Norito** и **CUSIP ↔ ISIN**. Обновление каждую ночь от
  Каналы ANNA/DSB и версии контролируют снимки, используемые CI.[^anna_crosswalk]
- Обновите сопоставления **BIC ↔ LEI** из файлов связей с общественностью GLEIF, чтобы мост мог
  выдавать оба, когда это необходимо.[^bic_lei]
– Храните **определения MIC** вместе с метаданными моста, чтобы обеспечить возможность проверки места проведения.
  детерминирован, даже если файлы RA изменяются в полдень.[^iso_mic]
- Запишите происхождение данных (метка времени + источник) в метаданных моста для аудита. Настойчиво
  идентификатор моментального снимка вместе с выдаваемыми инструкциями.
- Настройте `iso_bridge.reference_data.cache_dir` для сохранения копии каждого загруженного набора данных.
  наряду с метаданными происхождения (версия, источник, временная метка, контрольная сумма). Это позволяет аудиторам
  и операторы могут различать исторические каналы даже после ротации исходных снимков.
- Снимки пешеходного перехода ISO принимаются `iroha_core::iso_bridge::reference_data` с использованием
  блок конфигурации `iso_bridge.reference_data` (пути + интервал обновления). Датчики
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` и
  `iso_reference_refresh_interval_secs` раскрывает работоспособность среды выполнения для оповещений. Torii
  мост отклоняет отправку `pacs.008`, чьи BIC агента отсутствуют в настроенном
  пешеходный переход, выявление детерминированных ошибок `InvalidIdentifier`, когда контрагент
  неизвестно.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- Привязки IBAN и ISO 4217 применяются на одном уровне: теперь потоки pacs.008/pacs.009.
  выдает ошибки `InvalidIdentifier`, когда в IBAN должника/кредитора отсутствуют настроенные псевдонимы или когда
  валюта расчета отсутствует в `currency_assets`, что предотвращает искажение моста
  инструкции по достижению реестра. Проверка IBAN также применима к конкретной стране.
  длины и числовые контрольные цифры до прохождения ISO 7064 mod‑97, поэтому структурно недействительны.
  значения отклоняются досрочно.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- Помощники расчета CLI наследуют одни и те же защитные ограждения: pass
  `--iso-reference-crosswalk <path>` вместе с `--delivery-instrument-id`, чтобы иметь DvP
  Предварительный просмотр проверьте идентификаторы инструментов перед отправкой XML-снимка `sese.023`.【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (и оболочка CI `ci/check_iso_reference_data.sh`) ворс
  снимки и приспособления для пешеходного перехода. Команда принимает `--isin`, `--bic-lei`, `--mic` и
  `--fixtures` помечает и при запуске возвращается к образцам наборов данных в `fixtures/iso_bridge/`.
  без аргументов.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- Помощник IVM теперь принимает настоящие XML-конверты ISO 20022 (head.001 + `DataPDU` + `Document`).
  и проверяет заголовок бизнес-приложения через схему `head.001`, поэтому `BizMsgIdr`,
  Агенты `MsgDefIdr`, `CreDt` и BIC/ClrSysMmbId сохраняются детерминированно; XMLDSig/XAdES
  блоки остаются намеренно пропущенными. 

#### Вопросы регулирования и рыночной структуры- **Расчет Т+1**: фондовые рынки США и Канады перешли на Т+1 в 2024 году; отрегулировать Norito
  соответствующее планирование и оповещения по SLA.[^sec_t1][^csa_t1]
- **штрафы CSDR**: правила расчетной дисциплины предусматривают денежные штрафы; убедитесь, что Norito
  метаданные фиксируют ссылки на штрафы для сверки.[^csdr]
- **Пилотные расчеты в тот же день**: регулирующий орган Индии постепенно переходит к расчету T0/T+0; держать
  календари мостиков обновляются по мере расширения пилотов.[^india_t0]
- **Залоговые бай-ины/удержания**: отслеживайте обновления ESMA о сроках бай-инов и дополнительных удержаниях.
  поэтому условная доставка (`HldInd`) соответствует последним рекомендациям.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Доставка против оплаты → `sese.023`| Поле DvP | Путь ISO 20022 | Заметки |
|--------------------------------------------------------|----------------------------------------|-------|
| `settlement_id` | `TxId` | Стабильный идентификатор жизненного цикла |
| `delivery_leg.asset_definition_id` (безопасность) | `SctiesLeg/FinInstrmId` | Канонический идентификатор (ISIN, CUSIP, …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Десятичная строка; награждает точность активов |
| `payment_leg.asset_definition_id` (валюта) | `CashLeg/Ccy` | Код валюты ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | Десятичная строка; округлено согласно числовой спецификации |
| `delivery_leg.from` (продавец/поставщик) | `DlvrgSttlmPties/Pty/Bic` | БИК участника-доставщика *(канонический идентификатор аккаунта в настоящее время экспортируется в метаданные)* |
| `delivery_leg.from` идентификатор учетной записи | `DlvrgSttlmPties/Acct` | Свободная форма; Метаданные Norito содержат точный идентификатор учетной записи |
| `delivery_leg.to` (покупатель/принимающая сторона) | `RcvgSttlmPties/Pty/Bic` | БИК принимающего участника |
| `delivery_leg.to` идентификатор учетной записи | `RcvgSttlmPties/Acct` | Свободная форма; соответствует идентификатору получающего аккаунта |
| `plan.order` | `Plan/ExecutionOrder` | Перечисление: `DELIVERY_THEN_PAYMENT` или `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Перечисление: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Цель сообщения** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (доставка) или `RECE` (получение); отражает, какую ногу выполняет подающая сторона. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (платно) или `FREE` (без оплаты). |
| И18НИ00000170Х, И18НИ00000171Х | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Необязательный Norito JSON в кодировке UTF‑8 |

> **Квалификаторы расчета** — мост отражает рыночную практику путем копирования кодов условий расчета (`SttlmTxCond`), индикаторов частичного расчета (`PrtlSttlmInd`) и других необязательных квалификаторов из метаданных Norito в `sese.023/025`, если они присутствуют. Обеспечьте соблюдение перечислений, опубликованных в списках внешних кодов ISO, чтобы целевой CSD распознавал значения.

### Финансирование «платеж против платежа» → `pacs.009`

Ветви «наличные за наличные», которые финансируют PvP-инструкцию, выдаются в виде кредита «FI-FI».
трансферы. Мост аннотирует эти платежи, поэтому последующие системы распознают
они финансируют расчет по ценным бумагам.| Поле финансирования PvP | Путь ISO 20022 | Заметки |
|------------------------------------------------|------------------------------------------------------|-------|
| `primary_leg.quantity` / {сумма, валюта} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Сумма/валюта, списанная с инициатора. |
| Идентификаторы агентов контрагентов | `InstgAgt`, `InstdAgt` | BIC/LEI агентов-отправителей и получателей. |
| Цель поселения | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Установите `SECU` для PvP-финансирования, связанного с ценными бумагами. |
| Norito Метаданные (идентификаторы счетов, данные FX) | `CdtTrfTxInf/SplmtryData` | Содержит полный идентификатор аккаунта, временные метки валютных операций, подсказки по плану выполнения. |
| Идентификатор инструкции/связывание жизненного цикла | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Соответствует Norito `settlement_id`, поэтому денежная часть согласовывается со стороной ценных бумаг. |

Мост ISO JavaScript SDK соответствует этому требованию, по умолчанию
Назначение категории `pacs.009` — `SECU`; вызывающие абоненты могут переопределить его другим
действительный код ISO при осуществлении кредитовых переводов, не связанных с ценными бумагами, но недействительный
значения отклоняются заранее.

Если инфраструктура требует явного подтверждения ценных бумаг, мост
продолжает выдавать `sese.025`, но это подтверждение отражает часть ценных бумаг
статус (например, `ConfSts = ACCP`), а не «цель» PvP.

### Подтверждение платежа против платежа → `sese.025`

| PvP-поле | Путь ISO 20022 | Заметки |
|-----------------------------------------------|---------------------------|-------|
| `settlement_id` | `TxId` | Стабильный идентификатор жизненного цикла |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Код валюты для основного этапа |
| `primary_leg.quantity` | `SttlmAmt` | Сумма, поставленная инициатором |
| `counter_leg.asset_definition_id` | `AddtlInf` (полезная нагрузка JSON) | Код валюты счетчика, встроенный в дополнительную информацию |
| `counter_leg.quantity` | `SttlmQty` | Сумма счетчика |
| `plan.order` | `Plan/ExecutionOrder` | Тот же набор перечислений, что и DvP |
| `plan.atomicity` | `Plan/Atomicity` | Тот же набор перечислений, что и DvP |
| Статус `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` при совпадении; мост выдает коды отказа при отклонении |
| Идентификаторы контрагентов | `AddtlInf` JSON | Текущий мост сериализует полные кортежи AccountId/BIC в метаданных |

### Замена обеспечения РЕПО → `colr.007`| Поле/контекст репо | Путь ISO 20022 | Заметки |
|-------------------------------------------------|-----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Идентификатор контракта репо |
| Замена обеспечения Tx идентификатор | `TxId` | Генерируется за замену |
| Первоначальное количество залога | `Substitution/OriginalAmt` | Матчи под залог перед заменой |
| Исходная валюта залога | `Substitution/OriginalCcy` | Код валюты |
| Замещающее количество залога | `Substitution/SubstituteAmt` | Сумма замены |
| Заменить залоговую валюту | `Substitution/SubstituteCcy` | Код валюты |
| Дата вступления в силу (график маржи управления) | `Substitution/EffectiveDt` | Дата ISO (ГГГГ-ММ-ДД) |
| Классификация стрижек | `Substitution/Type` | В настоящее время `FULL` или `PARTIAL` в зависимости от политики управления |
| Причина управления / Примечание о стрижке | `Substitution/ReasonCd` | Необязательно, содержит обоснование управления |

### Финансирование и отчеты

| Контекст Iroha | Сообщение ISO 20022 | Расположение на карте |
|----------------------------------|-------------------|------------------|
| Зажигание / размотка денежных операций репо | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` заполнены из веток DvP/PvP |
| Заявления после расчетов | `camt.054` | Движение этапов платежа зарегистрировано под `Ntfctn/Ntry[*]`; мост вводит метаданные реестра/счета в `SplmtryData` |

### Примечания по использованию* Все суммы сериализуются с использованием числовых помощников Norito (`NumericSpec`).
  для обеспечения соответствия масштаба определениям активов.
* Значения `TxId` — `Max35Text` — перед этим необходимо обеспечить длину UTF‑8 ≤35 символов.
  экспорт в сообщения ISO 20022.
* BIC должен состоять из 8 или 11 буквенно-цифровых символов в верхнем регистре (ISO9362); отклонить
  Метаданные Norito, которые не прошли проверку перед отправкой платежей или расчетов.
  подтверждения.
* Идентификаторы аккаунтов (AccountId/ChainId) экспортируются в дополнительные
  метаданные, чтобы принимающие участники могли сверить данные со своими локальными реестрами.
* `SupplementaryData` должен быть каноническим JSON (UTF‑8, отсортированные ключи, собственный JSON).
  побег). Помощники SDK обеспечивают это, поэтому подписи, хэши телеметрии и ISO
  Архивы полезной нагрузки остаются детерминированными при перестройках.
* Суммы валют соответствуют дробным цифрам ISO4217 (например, JPY имеет 0).
  десятичные дроби, в долларах США их 2); мост зажимает числовую точность Norito соответственно.
* Помощники расчета CLI (`iroha app settlement ... --atomicity ...`) теперь выдают
  Инструкции Norito, планы выполнения которых соответствуют 1:1 `Plan/ExecutionOrder` и
  `Plan/Atomicity` выше.
* Помощник ISO (`ivm::iso20022`) проверяет поля, перечисленные выше, и отклоняет
  сообщения, в которых этапы DvP/PvP нарушают числовые спецификации или взаимность контрагентов.

### Помощники SDK Builder

- JavaScript SDK теперь предоставляет `buildPacs008Message`/
  `buildPacs009Message` (см. `javascript/iroha_js/src/isoBridge.js`), поэтому клиент
  автоматизация может конвертировать структурированные метаданные расчетов (BIC/LEI, IBAN,
  коды назначения, дополнительные поля Norito) в детерминированные пакеты XML
  без повторной реализации правил сопоставления из этого руководства.
- Для обоих помощников требуется явный `creationDateTime` (ISO‑8601 с часовым поясом).
  поэтому вместо этого операторы должны использовать детерминированную временную метку из своего рабочего процесса.
  разрешить SDK по умолчанию использовать время настенных часов.
- `recipes/iso_bridge_builder.mjs` демонстрирует, как подключить эти помощники к
  CLI, который объединяет переменные среды или файлы конфигурации JSON, печатает
  сгенерированный XML и, при необходимости, отправляет его в Torii (`ISO_SUBMIT=1`), повторно используя
  та же частота ожидания, что и в рецепте моста ISO.


### Ссылки

- Примеры расчетов LuxCSD/Clearstream ISO 20022, показывающие `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) и `Pmt`. (`APMT`/`FREE`).[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
– Спецификации Clearstream DCP, охватывающие квалификаторы расчетов (`SttlmTxCond`, `PrtlSttlmInd`).[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- Руководство SWIFT PMPG рекомендует `pacs.009` и `CtgyPurp/Cd = SECU` для PvP-финансирования, связанного с ценными бумагами.[3](https://www.swift.com/swift-resource/251897/download)
– Отчеты об определениях сообщений ISO 20022 для ограничений длины идентификатора (BIC, Max35Text).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf).
- Руководство ANNA DSB по формату ISIN и правилам контрольной суммы.[5](https://www.anna-dsb.com/isin/)

### Советы по использованию- Всегда вставляйте соответствующий фрагмент Norito или команду CLI, чтобы LLM могла проверить.
  точные имена полей и числовые шкалы.
- Запросите цитаты (`provide clause references`), чтобы сохранить документальный след
  соответствие требованиям и аудиторская проверка.
- Запишите сводку ответов в `docs/source/finance/settlement_iso_mapping.md`.
  (или связанные приложения), чтобы будущим инженерам не приходилось повторять запрос.

## Учебники по упорядочению событий (ISO 20022 ↔ Norito Bridge)

### Сценарий А — Замена обеспечения (РЕПО/Залог)

**Участники:** залогодатель/получатель (и/или агенты), хранитель(и), CSD/T2S  
**Время:** в зависимости от рыночных ограничений и дневных/ночных циклов T2S; организовать два этапа так, чтобы они завершились в рамках одного расчетного окна.

#### Хореография сообщений
1. `colr.010` Запрос на замену обеспечения → залогодержатель/получатель или агент.  
2. `colr.011` Ответ на замену обеспечения → принять/отклонить (необязательная причина отклонения).  
3. `colr.012` Подтверждение замены обеспечения → подтверждает договор замены.  
4. Инструкция `sese.023` (две ножки):  
   - Вернуть первоначальное обеспечение (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Предоставьте замещающее обеспечение (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Соедините пару (см. ниже).  
5. Сообщения о статусе `sese.024` (принято, согласовано, ожидает, не выполнено, отклонено).  
6. Подтверждения `sese.025` после бронирования.  
7. Необязательная дельта денежных средств (комиссия/дисконт) → `pacs.009` Кредитный перевод из FI в FI с `CtgyPurp/Cd = SECU`; статус через `pacs.002`, возвращается через `pacs.004`.

#### Необходимые подтверждения/статусы
- Транспортный уровень: шлюзы могут выдавать `admi.007` или отклонять запросы перед бизнес-обработкой.  
- Жизненный цикл расчета: `sese.024` (статусы обработки + коды причин), `sese.025` (окончательный).  
- Кассовая сторона: `pacs.002` (`PDNG`, `ACSC`, `RJCT` и т. д.), `pacs.004` для возврата.

#### Условность/отмена полей
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) для связывания двух инструкций.  
- `SttlmParams/HldInd` удерживать до тех пор, пока не будут выполнены критерии; выпуск через `sese.030` (статус `sese.031`).  
- `SttlmParams/PrtlSttlmInd` для управления частичным расчетом (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` для рыночных условий (`NOMC` и т. д.).  
- Дополнительные правила условной доставки ценных бумаг T2S (CoSD), если они поддерживаются.

#### Ссылки
- Управление обеспечением SWIFT MDR (`colr.010/011/012`).  
- Руководства по использованию CSD/T2S (например, DNB, ECB Insights) для связывания и статусов.  
- Практика расчетов SMPG, руководства Clearstream DCP, семинары ASX ISO.

### Сценарий B — Нарушение окна обмена валют (отказ финансирования PvP)

**Участники:** контрагенты и кассовые агенты, депозитарий ценных бумаг, ЦД/Т2С  
**Время:** Окна FX PvP (CLS/двусторонние) и ограничения CSD; держать части ценных бумаг на удержании до подтверждения денежных средств.#### Хореография сообщений
1. `pacs.009` Кредитный перевод между FI и FI по валюте с `CtgyPurp/Cd = SECU`; статус через `pacs.002`; вызвать/отменить через `camt.056`/`camt.029`; если уже урегулировано, возврат `pacs.004`.  
2. `sese.023` Инструкции DvP с `HldInd=true`, чтобы часть ценных бумаг ожидала подтверждения денежных средств.  
3. Уведомления жизненного цикла `sese.024` (принято/согласовано/ожидается).  
4. Если обе ножки `pacs.009` достигают `ACSC` до истечения срока действия окна → отпустите с помощью `sese.030` → `sese.031` (состояние мода) → `sese.025` (подтверждение).  
5. Если окно FX нарушено → отмените/отзовите денежные средства (`camt.056/029` или `pacs.004`) и аннулируйте ценные бумаги (`sese.020` + `sese.027` или разворот `sese.026`, если это уже подтверждено правилами рынка).

#### Необходимые подтверждения/статусы
- Наличные: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` для возврата.  
- Ценные бумаги: `sese.024` (причины ожидания/неудачи, такие как `NORE`, `ADEA`), `sese.025`.  
- Транспорт: `admi.007` / шлюз отклоняет перед бизнес-обработкой.

#### Условность/отмена полей
- `SttlmParams/HldInd` + `sese.030` выпуск/отмена в случае успеха/неудачи.  
- `Lnkgs` для привязки инструкций по ценным бумагам к кассовой части.  
- Правило T2S CoSD при использовании условной доставки.  
- `PrtlSttlmInd` для предотвращения непреднамеренных частичных ошибок.  
- На `pacs.009` `CtgyPurp/Cd = SECU` отмечает финансирование, связанное с ценными бумагами.

#### Ссылки
- Руководство PMPG / CBPR+ для платежей в процессах ценных бумаг.  
- Практика урегулирования SMPG, информация T2S о привязке/удержании.  
- Руководства Clearstream DCP, документация ECMS для сообщений о техническом обслуживании.

### pacs.004 возвращает примечания к сопоставлению

- Фикстуры возврата теперь нормализуют `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) и собственные причины возврата, представленные как `TxInf[*]/RtrdRsn/Prtry`, поэтому потребители моста могут воспроизводить присвоение комиссий и коды операторов без повторного анализа XML. конверт.
— Блоки подписи AppHdr внутри конвертов `DataPDU` остаются игнорируемыми при приеме; аудит должен основываться на происхождении канала, а не на встроенных полях XMLDSIG.

### Контрольный список эксплуатации моста
- Примените вышеописанную хореографию (обеспечение: `colr.010/011/012 → sese.023/024/025`; нарушение FX: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- Рассматривать статусы `sese.024`/`sese.025` и результаты `pacs.002` как стробирующие сигналы; `ACSC` запускает отпускание, `RJCT` заставляет раскручиваться.  
- Кодируйте условную доставку через `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` и дополнительные правила CoSD.  
- При необходимости используйте `SupplementaryData` для сопоставления внешних идентификаторов (например, UETR для `pacs.009`).  
- Параметризация времени удержания/размотки по рыночному календарю/отключениям; выдайте `sese.030`/`camt.056` до истечения крайнего срока отмены, при необходимости отмените возврат.

### Пример полезных данных ISO 20022 (с аннотациями)

#### Пара сопутствующих замен (`sese.023`) со связью инструкций

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```Отправьте связанную инструкцию `SUBST-2025-04-001-B` (получение FoP замещающего обеспечения) с `SctiesMvmntTp=RECE`, `Pmt=FREE` и связью `WITH`, указывающей обратно на `SUBST-2025-04-001-A`. Отпустите обе ножки с помощью соответствующего `sese.030`, как только замена будет одобрена.

#### Часть ценных бумаг приостановлена в ожидании подтверждения валютной операции (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Отпустите, как только обе ножки `pacs.009` достигнут `ACSC`:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` подтверждает освобождение от удержания, а затем `sese.025`, как только часть ценных бумаг забронирована.

#### Часть финансирования PvP (`pacs.009` с целью обеспечения ценных бумаг)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` отслеживает статус платежа (`ACSC` = подтверждено, `RJCT` = отклонено). Если окно нарушено, отзовите через `camt.056`/`camt.029` или отправьте `pacs.004` для возврата оплаченных средств.