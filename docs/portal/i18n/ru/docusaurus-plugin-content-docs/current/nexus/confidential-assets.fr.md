---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Конфиденциальные акты и переводы ZK
описание: Фаза C проекта для слепой циркуляции, регистрации и управления операторами.
слаг: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Дизайн конфиденциальных и переводных документов ZK

## Мотивация
- Пользователь потока действий слепо соглашается, чтобы домены сохраняли конфиденциальность транзакций без модификатора прозрачности обращения.
- Гардерация выполнения определяется гетерогенным аппаратным обеспечением валидаторов и консерватором Norito/Kotodama ABI v1.
- Не используйте аудиторов и операторов управления циклом жизни (активация, ротация, отзыв) для схем и криптографических параметров.

## Модель угрозы
- Les validateurs sont честные, но любопытные: они выполняют согласованную верность в главном реестре/состоянии инспектора.
- Наблюдатели следят за участниками блока и сплетнями о транзакциях; есть гипотезы об этих каналах сплетен.
- Дополнительные возможности: анализ трафика за пределами реестра, количественные данные о противниках (в соответствии с дорожной картой PQ), атаки на доступ к реестру.

## Внешний вид ансамбля дизайна
- Действия могут быть объявлены в *экранированном пуле* и плюс существующие прозрачные балансы; Слепое обращение является представителем посредством криптографических обязательств.
- Примечания к инкапсулятивному `(asset_id, amount, recipient_view_key, blinding, rho)` с:
  - Обязательство: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Обнулитель: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, независимо от порядка нот.
  - Шифр ​​полезной нагрузки: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Транзакции, транспортирующие полезные нагрузки, `ConfidentialTransfer` кодируют контент Norito:
  - Публичные входы: якорь Меркла, обнулители, новые обязательства, идентификатор актива, версия схемы.
  - Полезные нагрузки для получателей и опций аудиторов.
  - Предварительный сертификат с нулевым разглашением, подтверждающий сохранение ценности, права собственности и авторизации.
- Проверка ключей и наборов параметров для управления через регистры в бухгалтерской книге с активацией; les noeuds отказываются от валидации доказательств, которые ссылаются на недействующие или отозванные въезды.
- Заголовки консенсуса, задействованные в функциональном конфиденциальном обзоре, действуют до тех пор, пока блоки Soient не примут такое значение, если соответствующий реестр/параметры соответствуют.
- При построении доказательств используется стопка Halo2 (Plonkish) без надежной настройки; Groth16 или другие варианты SNARK, которые не поддерживаются в v1.

### Крепления детерминированы

Конверты конфиденциальной информации хранятся в каноническом приспособлении `fixtures/confidential/encrypted_payload_v1.json`. Набор данных захватывает позитивную оболочку v1, а также уродливые изменения в том, что SDK мощно подтверждает часть синтаксического анализа. Тесты модели данных Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) и пакета Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) заряжают устройство, гарантируя, что кодирование Norito, поверхности с ошибками и пути регрессии остаются неизменными в ходе эволюции кодека.Les SDKs Swift может поддерживаться с помощью инструкций Shield sans Glue JSON на заказ: сконструировать
`ShieldRequest` с 32-байтовым обязательством, изменением полезной нагрузки и дебетовыми метаданными,
puis appeler `IrohaSDK.submit(shield:keypair:)` (или `submitAndWait`) для подписанта и ретранслятора
транзакция через `/v2/pipeline/transactions`. Le helper valide les longueurs de обязательства,
Вставьте `ConfidentialEncryptedPayload` в кодировщик Norito и отобразите макет `zk::Shield`.
ci-dessous afin que les кошельки синхронизируются с Rust.

## Согласованные обязательства и ограничение возможностей
- Открытые заголовки блоков `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; участие в дайджесте или хеш-де-консенсус и doit egaler la vue locale du реестра для акцептора блока.
- Управление может быть выполнено на сцене обновлений в программном режиме `next_conf_features` с `activation_height` в будущем; Jusqu a cette hauteur, les Producteurs de Blocs doivent продолжают создавать прецедент дайджеста.
- Les noeuds validateurs DOIVENT функционируют с `confidential.enabled = true` и `assume_valid = false`. Чеки демарража отказываются входить в набор валидаторов, если условия отражаются или `conf_features` локально расходятся.
- Метаданные рукопожатия P2P, включая `{ enabled, assume_valid, conf_features }`. Коллеги сообщают о функциях, которые не являются платными, и отклоняются от `HandshakeConfidentialMismatch` и вступают в ротацию консенсуса.
- Результаты рукопожатия между валидаторами, наблюдателями и пирами фиксируются в матрице рукопожатия с помощью [Согласование возможностей узла] (#node-capability-negotiation). Выявление рукопожатия `HandshakeConfidentialMismatch` и садовник за пределами ротации консенсуса просто соответствуют тому, что дайджест соответствует.
- Наблюдатели, не выполняющие валидацию, могут определить `assume_valid = true`; они применяются к конфиденциальным данным, которые больше всего влияют на безопасность консенсуса.## Политика и действия
- Определение отдельного действия при транспортировке `AssetConfidentialPolicy`, определяемое создателем или посредством управления:
  - `TransparentOnly`: режим по умолчанию; прозрачные инструкции (`MintAsset`, `TransferAsset` и т. д.) разрешены, а операции экранированы и запрещены.
  - `ShieldedOnly`: все выбросы и все передачи с использованием конфиденциальных инструкций; `RevealConfidential` является препятствием для того, чтобы балансы, которые не были известны публике.
  - `Convertible`: держатели могут быть заменены значениями между прозрачными и экранированными изображениями с помощью инструкций по включению и съезду с рампы.
- Политика, направленная на сдерживание ФШМ для уничтожения фондов блоков:
  - `TransparentOnly -> Convertible` (немедленная активация экранированного пула).
  - `TransparentOnly -> ShieldedOnly` (требуется подвесной переход и отверстие для преобразования).
  - `Convertible -> ShieldedOnly` (минимальная задержка).
  - `ShieldedOnly -> Convertible` (план миграции, необходимый для защиты заметок, остающихся расходуемыми).
  - `ShieldedOnly -> TransparentOnly` est interdit sauf, если экранированный пул представляет собой видео или управление кодирует миграцию, которая защищает оставшиеся примечания.
- Инструкции по управлению исправлением `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` через ISI `ScheduleConfidentialPolicyTransition` и возможность отмены изменений программ с `CancelConfidentialPolicyTransition`. Мемпул проверки гарантирует, что данная транзакция не будет отменена при переходе и включении в нее детерминированности, если будет проверена политическая смена среды блока.
- Подвесные переходы с автоматическими приложениями в открытии нового блока: для того, чтобы войти в отверстие преобразования (для обновлений `ShieldedOnly`) или обратить внимание на `effective_height`, время выполнения встретилось с `AssetConfidentialPolicy`, удалить метаданные `zk.policy` и удалить подвесной вход. Если поставка прозрачного остатка не равна нулю при поступлении перехода `ShieldedOnly`, изменение во время выполнения аннулируется и записывается в журнал предупреждения, прецедент режима без изменений.
- Ручки конфигурации `policy_transition_delay_blocks` и `policy_transition_window_blocks` установлены с минимальным предварительным и льготным периодом для возможности автоматического преобразования кошельков в переключатель.
- `pending_transition.transition_id` серт aussi de handle d Audit; управление doit le citer для завершения или аннулирования в зависимости от того, что операторы могут эффективно коррелировать взаимосвязи на/вне рампы.
- `policy_transition_window_blocks` по умолчанию — 720 (~ 12 часов с временем блока 60 с). Les noeuds clampent les requetes de control qui tenent un preavis plus Court.
- Genesis Manifes et Flux CLI раскрывает курантскую и подвесную политику. Логика допуска освещает политику в момент исполнения для подтверждения того, что инструкция конфиденциальна и авторизована.
- Контрольный список миграции — для «Последовательности миграции» для планирования обновления на этапах, соответствующих Milestone M0.

#### Мониторинг переходов через ToriiКошельки и аудиторы допрашивают `GET /v2/confidential/assets/{definition_id}/transitions` для инспектора l `AssetConfidentialPolicy` actif. Полезная нагрузка JSON включает в себя канонический идентификатор актива, последний верхний блок наблюдения, политический `current_mode`, режим, эффективный для этой высокой точки (временные отверстия для преобразования `Convertible`), а также идентификаторы, сопровождающие `vk_set_hash`/Посейдон/Педерсен. Когда происходит переход управления, внимание уделяется ответу на встраивание:

- `transition_id` - обработка отправки аудита по параметру `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` и `window_open_height` извлекаются (блок или кошельки начинают преобразование для переключений ShieldedOnly).

Пример ответа:

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Ответ `404` указывает на то, что определение действительно соответствует существующему. Lorsqu Aucune Transition n est planifiee le champ `pending_transition` est `null`.

### Машина политического государства| Текущий режим | Соответствующий режим | Предварительное условие | Эффект высокой эффективности | Заметки |
|----|------------------|-------------------------------------------------------------------------------|--------------|------------|
| ПрозрачныйТолько | Кабриолет | Управление активными входами в реестр верификаторов/параметров. Суметр `ScheduleConfidentialPolicyTransition` с `effective_height >= current_height + policy_transition_delay_blocks`. | La переход s выполняет требование a `effective_height`; Экранированный бассейн может отключиться немедленно.        | Это действие по умолчанию для активизации конфиденциальности и защиты прозрачных потоков. |
| ПрозрачныйТолько | Только экранированный | То же самое, плюс `policy_transition_window_blocks >= 1`.                                                         | Время выполнения автоматически переходит в `Convertible` и `effective_height - policy_transition_window_blocks`; передайте `ShieldedOnly` и `effective_height`. | Условия преобразования определяются до отключения прозрачных инструкций. |
| Кабриолет | Только экранированный | Программа перехода с `effective_height >= current_height + policy_transition_delay_blocks`. Сертификатор управления DEVRAIT (`transparent_supply == 0`) посредством аудита метаданных; время выполнения l аппликация или обрезка. | Семантика идентичности окон. Если прозрачная поставка не равна нулю в `effective_height`, переход будет отменен с `PolicyTransitionPrerequisiteFailed`. | Вернитесь в конфиденциальное обращение.                             |
| Только экранированный | Кабриолет | Программа перехода; aucun retrait d Urgence Actif (`withdraw_height` не определен).                                  | L etat bascule a `effective_height`; Пандусы показывают, что заметки защищены экранированными оставшимися действительными.       | Используйте отверстия для технического обслуживания или проверки аудиторов.                               |
| Только экранированный | ПрозрачныйТолько | Управление doit prouver `shielded_supply == 0` или подготовка плана `EmergencyUnshield` Signe (подписи и требования аудитора). | Время выполнения доступно в одном окне `Convertible` перед `effective_height`; a la hauteur, конфиденциальные инструкции повторяются в течение длительного времени и действуют снова в прозрачном режиме. | Вылазка де Дернье возобновляется. Автоматическое аннулирование перехода, если конфиденциальная информация зависит от того, будет ли это окончание. |
| Любой | То же, что и текущий | `CancelConfidentialPolicyTransition` не производит никаких изменений.                                              | `pending_transition` немедленно уйдет в отставку.                                                                       | Поддерживать статус-кво; Индика для завершения.                                         |Переходы, не включенные в список, являются нежелательными для управления соумиссией. Во время выполнения проверьте все необходимые условия перед применением программы перехода; В случае проверки он ответит на действие в прецедентном режиме и получит `PolicyTransitionPrerequisiteFailed` через телеметрию и события блока.

### Последовательность миграции

1. **Подготовка реестров:** активируйте все входы, проверенные и справочные параметры по политической политике. Les noeuds annoncent le `conf_features` является результатом проверки согласованности одноранговых узлов.
2. **Планировщик перехода:** соответствует `ScheduleConfidentialPolicyTransition` с `effective_height` соответствующим `policy_transition_delay_blocks`. В версии `ShieldedOnly`, более точное отверстие для преобразования (`window >= policy_transition_window_blocks`).
3. **Опубликуйте руководство для оператора:** зарегистрируйте `transition_id`, чтобы вернуть и распространить Runbook на съезде/съезде. Кошельки и аудиторы имеют номер `/v2/confidential/assets/{id}/transitions` для знакомства с высоким уровнем открытия.
4. **Применение окна:** откройте окно, во время выполнения политики выполните `Convertible`, включите `PolicyTransitionWindowOpened { transition_id }` и начните отклонять требования управления в конфликте.
5. **Финализатор или блокировщик:** a `effective_height`, проверка во время выполнения предварительных требований (подача прозрачного нуля, па-де-обратная необходимость и т. д.). В результате, политика прошла в требуемом режиме; Например, `PolicyTransitionPrerequisiteFailed` - это Эмис, подвесной переход - это nettoyee, а политика остается неизменной.
6. **Обновления схемы:** после повторного перехода управление дополняет действующую версию схемы (например, `asset_definition.v2`) и инструментарий CLI exige `confidential_policy` для сериализации манифестов. Документы по обновлению Genesis проинструктируют операторов, а также изменят политические настройки и операционный реестр перед изменением проверяющих.

Les nouveaux reseaux qui demarrent с активным конфиденциальным кодом политики, желаемого направления в зарождении. Ils suivent quand meme la checklist ci-dessus los des des Changes post-launch afin que les fnetres de conversion restent deterministes et que les кошельки aient le temps de s juster.

### Версия и активация манифестов Norito- Генезисные манифесты DOIVENT включают `SetParameter` для пользовательского ключа `confidential_registry_root`. Полезная нагрузка соответствует Norito JSON, соответствующему `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: совместите чемпионский титул (`null`), когда он активен, а также шестнадцатеричную цепочку 32-байтовых (`0x...`), например, хеш-продукта `compute_vk_set_hash` в инструкциях по проверке манифеста. Уведомления отказываются от демаркации, если параметр изменен или хеш расходится с зашифрованными кодами реестра.
- Подключите `ConfidentialFeatureDigest::conf_rules_version` к версии макета манифеста. Для изменений v1 DOIT, оставшихся `Some(1)` и других `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Когда набор правил развивается, увеличивается константа, восстанавливаются манифесты и синхронно развертываются двоичные файлы; изменить версии, которые могут быть отменены блоками по валидаторам с `ConfidentialFeatureDigestMismatch`.
- Активация проявляется в том, что перегруппировщик DEVRAIENT нарушает работу реестра, циклические изменения параметров жизни и политические переходы, которые приводят к тому, что дайджест остается последовательным:
  1. Примените изменения в планах реестра (`Publish*`, `Set*Lifecycle`) в автономном режиме и в калькуляторе дайджеста после активации с `compute_confidential_feature_digest`.
  2. Эмметр `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` использует хэш-вычисление для пиров и замедленного восстановления, чтобы правильно переварить мемы с рейтингом инструкций-посредников.
  3. Установите инструкции `ScheduleConfidentialPolicyTransition`. Инструкция по вводу doit citer le `transition_id` emis par control; Манифесты, которые явно отвергаются во время выполнения.
  4. Сохраните байты манифеста, включите SHA-256 и используйте дайджест в запланированной активации. Операторы проверяют три артефакта перед голосованием за манифест для устранения разделов.
- При необходимости развертывания и обрезке, зарегистрируйте высокий кабель в пользовательском параметре компаньона (например, `custom.confidential_upgrade_activation_height`). Cela Fournit aux Auditeurs une Preuve Norito кодирует то, что валидаторы соблюдают условия перед тем, как произойдет изменение дайджеста.## Цикл проверки верификаторов и параметров
###Регистрация ЗК
- В бухгалтерской книге `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` или `proving_system` есть актуальное исправление `Halo2`.
- Пары `(circuit_id, version)` являются уникальными по всему миру; Реестр поддерживает вторичный индекс для поиска метаданных схемы. Предварительная регистрация и пара дубликатов, которые были отклонены при приеме.
- `circuit_id` doit etre not vide et `public_inputs_schema_hash` doit etre Fourni (тип хэша Blake2b-32 для канонического кодирования общедоступных входных данных верификатора). L допуск отменяет регистрацию на этих чемпионатах.
- Инструкции по управлению включают:
  - `PUBLISH` для добавления ввода `Proposed` с дополнительными метаданными.
  - `ACTIVATE { vk_id, activation_height }` для программатора для активации в ограниченную эпоху.
  - `DEPRECATE { vk_id, deprecation_height }` для обозначения высокого финала или дополнительных ссылок на вход.
  - `WITHDRAW { vk_id, withdraw_height }` для срочного отключения; les actifs touches geleront les depenses конфиденциальные после того, как уберут высоту, просто активируя новые входы.
- Генезис демонстрирует автоматическую настройку пользовательского параметра `confidential_registry_root`, который не соответствует `vk_set_hash` дополнительным активным элементам; проверка croise ce дайджест с местным реестром, прежде чем вы сможете воссоединиться с консенсусом.
- Регистратор или счетчик, требующий проверки `gas_schedule_id`; На проверку необходимо ввести `Active`, представить индекс `(circuit_id, version)`, а доказательства Halo2 Fournissent Un `OpenVerifyEnvelope` не `circuit_id`, `vk_hash` и `public_inputs_schema_hash` корреспондент во входе в реестр.

### Ключи проверки
- Ключи проверки остаются за пределами реестра, а ссылки на адреса идентификаторов по содержанию (`pk_cid`, `pk_hash`, `pk_len`) публикуются с метаданными проверяющего.
- Кошелек SDK восстанавливает файлы PK, проверяет хеши и сохраняет файлы в локальном кэше.

### Параметры Педерсена и Посейдона
- Отделение реестров (`PedersenParams`, `PoseidonParams`) зеркально отображает элементы управления циклом проверки подлинности, содержит `params_id`, хэши генераторов/констант, активацию, прекращение поддержки и высокую степень вывода.
- Обязательства и хэши шрифта разделены по домену по `params_id`, чтобы ротация параметров не использовалась повторно, поскольку битовые шаблоны и ансамбли устаревают; l ID находится в записях об обязательствах и тегах обнуления домена.
- Схемы поддерживают выбор нескольких параметров для проверки; ансамбли параметров устаревают, оставаясь расходуемыми, как `deprecation_height`, и ансамбли удаляются из-за требований `withdraw_height`.## Определенный порядок и обнуляющие
- Активируйте `CommitmentTree` с `next_leaf_index`; блоки регулируют обязательства в рамках детерминированного порядка: повторяют транзакции в порядке блока; в транзакционном цикле транзакций, экранированных по номиналу `output_idx`, сериализовать восходящий.
- `note_position` выводит смещения большего количества **ne** fait pas party du nullifier; он не уверен, что члены членства являются свидетелями предварительного заключения.
- La Stabilite des Nullifiers sous Reorgs est garantie par le Design PRF; l введите PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, и ссылки на якоря исторических корней Меркла ограничиваются номиналом `max_anchor_age_blocks`.

## Поток книги
1. **MintConfidential { assets_id, сумма, получатель_подсказка }**
   - Запросить политику действий `Convertible` или `ShieldedOnly`; l входная проверка l авторит, восстановление `params_id`, echantillonne `rho`, выполнение обязательств, встреча с журналом l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` с новым обязательством, корнем Меркла и хэшем вызова транзакции для журналов аудита.
2. **TransferConfidential { assets_id,proof,circuit_id,версия, обнулители,new_commitments,enc_payloads,anchor_root,memo }**
   - Системный вызов виртуальной машины проверяет подтверждение через вход в реестр; Хост уверяет, что нуллификаторы не используются, что обязательства подчеркивают детерминированность и что якорь самый последний.
   - Леджер регистрирует входы `NullifierSet`, сохраняет полезные данные для получателей/аудиторов и получателей `ConfidentialEvent::Transferred`, возобновляет обнуление, выходные данные, хэш-доказательство и корни Меркла.
3. **RevealConfidential { идентификатор_актива, доказательство, идентификатор_схемы, версия, обнулитель, сумма, учетная запись_получателя, корень_якоря }**
   - Доступная уникальность для действий `Convertible`; la доказательство того, что ценность банкноты равна le Montant Revele, le Ledger Credite le Balance, прозрачный и Brule la Note, защищенная en Marquant le Nullifier Comme Depense.
   - Emet `ConfidentialEvent::Unshielded` с открытым доступом, нуллификаторами, идентификаторами доказательства и хэшем для апелляции транзакции.## Настройка модели данных
- `ConfidentialConfig` (новый раздел конфигурации) с активацией флага d, `assume_valid`, ручки газа/ограничений, якорное отверстие, серверная часть верификатора.
- `ConfidentialNote`, `ConfidentialTransfer`, et `ConfidentialMint` schemas Norito avec byte de version explicite (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` enveloppe des memo bytes AEAD avec `{ version, ephemeral_pubkey, nonce, ciphertext }`, par defaut `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` pour le layout XChaCha20-Poly1305.
- Les vecteurs canoniques de derivation de cle vivent dans `docs/source/confidential_key_vectors.json`; le CLI et l endpoint Torii regressent sur ces fixtures.
- `asset::AssetDefinition` gagne `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste le binding `(backend, name, commitment)` pour les verifiers transfer/unshield; l execution rejette les proofs dont la verifying key referencee ou inline ne correspond pas au commitment enregistre.
- `CommitmentTree` (действующий с пограничными контрольно-пропускными пунктами), `NullifierSet` cle `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` имеются в наличии по всему миру.
- Mempool maintient des structures transitoires `NullifierIndex` et `AnchorIndex` pour detection precoce des doublons et checks d age d anchor.
- Les updates de schema Norito incluent un ordering canonique des public inputs; les tests de round-trip assurent le determinisme d encoding.
- Les roundtrips d encrypted payload sont verrouilles via unit tests (`crates/iroha_data_model/src/confidential.rs`). Des vecteurs wallet de suivi attacheront des transcripts AEAD canoniques pour auditeurs. `norito.md` documente le header on-wire de l envelope.

## Интеграция IVM и системный вызов
- Introduire le syscall `VERIFY_CONFIDENTIAL_PROOF` acceptant:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, et le `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultant.
  - Системный вызов взимает плату за проверку метаданных из реестра, применение пределов хвоста/времени, фактуру газа, определяемую, и т. д. применение дельты, которую можно повторно использовать.
- Le host expose un trait read-only `ConfidentialLedger` pour recuperer des snapshots Merkle root et le statut des nullifiers; la librairie Kotodama fournit des helpers d assembly de witness et de validation de schema.
- Les docs pointer-ABI sont mis a jour pour clarifier le layout du buffer de proof et les handles registry.## Переговоры о возможностях noeud
- Объявление о рукопожатии `feature_bits.confidential` с `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Участие требуемых валидаторов `confidential.enabled=true`, `assume_valid=false`, идентификаторов серверной проверки и дайджестов корреспондентов; несоответствия повторяют рукопожатие с `HandshakeConfidentialMismatch`.
- Конфигурация поддерживает `assume_valid` для отдельных наблюдателей: при деактивации, выполнение конфиденциальных инструкций по продукту `UnsupportedInstruction` определяется без паники; Когда активен, наблюдатели, применяющие дельты, объявляют доказательства без проверки.
- Мемпул отменяет конфиденциальные транзакции, если региональная емкость отключена. Фильтры сплетен, которые являются отправителями транзакций, защищенными от одноранговых узлов без корреспондентов, рекламируются и пересылаются по пути идентификаторов верификаторов, не входящих в пределы ограничений.

### Матрица рукопожатия

| Дистанционное объявление | Результаты для валидаторов | Оператор заметок |
|------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, соответствие серверной части, совпадение дайджеста | Принять | Le Peer Atteint l etat `Ready` и участие в предложении, голосовании и разветвлении RBC. Требуется ручное действие Aucune. |
| `enabled=true`, `assume_valid=false`, соответствие серверной части, дайджест устаревший или отсутствует | Реджете (`HandshakeConfidentialMismatch`) | Удаленное применение реестра/параметров активаций на уровне внимания или внимания `activation_height` planifie. Если внести исправление, то все остальное можно будет разъединить, но только в период ротации консенсуса. |
| `enabled=true`, `assume_valid=true` | Реджете (`HandshakeConfidentialMismatch`) | Les validateurs требуют проверки доказательств; Настройка удаленного доступа наблюдателя с входом Torii только или базовым устройством `assume_valid=false` после завершения активации проверки. |
| `enabled=false`, champs omis (сборка устарела), или другой серверный проверяющий | Реджете (`HandshakeConfidentialMismatch`) | Устаревшие или частичные обновления не могут быть воссоединены с результатами консенсуса. Познакомьтесь с курантным выпуском и убедитесь, что серверная часть кортежа + дайджест соответствуют перед переподключением. |

Наблюдатели, добровольно выполняющие проверку доказательств, не допускаются к консенсусу по связям с действиями проверяющих. Если вы можете загружать блоки через Torii или API-интерфейсы для архивирования, то это может привести к получению консенсуса по запросу, просто потому, что они сообщают о возможностях корреспондентов.

### Политика сокращения Выявление и сохранение обнуляющих

Конфиденциальные бухгалтерские книги должны сохранять историческую оценку для проверки достоверности банкнот и облегчения аудита управления. Политика по умолчанию, аппликация по `ConfidentialLedger`, оценка:- **Сохранение обнулителей:** сохранение обнуляющих значений зависит от *минимума* `730` дней (24 месяца) после более высокой степени зависимости или отступа, наложенного регулятором, если это требуется дольше. Les operateurs peuvent etendre la fenetre via `confidential.retention.nullifier_days`. Обнуляющие плюс юные, которые проходят через окончание DOIVENT, остальные запросы через Torii, чтобы убедиться, что аудиторы доказали отсутствие двойного расходования.
- **Очистка раскрывающихся данных:** Отображаемые прозрачные файлы (`RevealConfidential`) сокращают обязательства, связанные с ассоциациями, сразу после завершения блока, но обнуляющее значение сохраняется в соответствии с правилами сохранения ci-dessus. События `ConfidentialEvent::Unshielded` регистрируются публично, получателем и хэш-доказательством для реконструкции исторических данных, не требующих удаления зашифрованного текста.
- **Пограничные контрольно-пропускные пункты:** границы обязательств поддерживаются контрольно-пропускными пунктами, которые имеют право плюс большой `max_anchor_age_blocks` и место удержания. Les noeuds сжимает контрольно-пропускные пункты плюс anciens seulement после истечения срока действия всех нулификаторов в интервале.
- **Дайджест исправления устаревший:** если `HandshakeConfidentialMismatch` выжил в результате дрейфа дайджеста, операторы делают (1) проверку того, что отверстия удержания обнулителей выровнены в кластере, (2) lancer `iroha_cli app confidential verify-ledger` для регенерации дайджеста по ансамблю обнулителей retenus и т. д. (3) перераспределение манифеста rafraichi. Tout nullifier prune trop tot doit etre restaure depuis le stockage froid avant de rejoindre le reseau.

Documentez les переопределяет локальные операции в runbook d; политика управления, которая обеспечивает возможность хранения в течение дня в конфигурации узлов и планов хранения и архивирования в одно и то же время.

### Порядок выселения и восстановления

1. Подвесной циферблат, `IrohaNetwork`, сравните объявленные мощности. Общий уровень несоответствия `HandshakeConfidentialMismatch`; соединение является фермой и остальным партнером в файле открытия без возможности обмена `Ready`.
2. Выполнение проверки осуществляется через журнал обслуживания (включая удаленный дайджест и серверную часть), а Sumeragi не планируется как одноранговый узел для предложения или голосования.
3. Операторы должны выполнить согласование проверяемых реестров и ансамблей параметров (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) или программировать `next_conf_features` с помощью `activation_height`. Если дайджест выровнен, процепное рукопожатие автоматически повторится.
4. Если одноранговый узел повторно использует диффузор (например, через архив воспроизведения), проверяющие устройства будут отброшены в соответствии с `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, гарантируя, что бухгалтерская книга будет согласована в отчете.

### Безопасный для повторения процесс рукопожатия1. Chaque tentative sortante alloue un nouveau materiel de cle Noise/X25519. Полезная нагрузка рукопожатия (`handshake_signature_payload`) объединяет публичные эфимеры, локаль и удаленность, адрес сокета, объявленный в кодировке Norito, и затем - quand compile avec `handshake_chain_id` - l идентификатор цепочки. Le message est chiffre AEAD avant de quitter le noeud.
2. Le responder recompute le payload avec l ordre de cles peer/local inverse et verifie la signature Ed25519 embarquee dans `HandshakeHelloV1`. Когда два адреса эфимеров и адрес объявленного шрифта стороны домена подписи, радуйтесь захвату сообщения от другого однорангового узла или восстановлению устаревшего эхо-определенного соединения.
3. Les flags de capacite confidentielle et le `ConfidentialFeatureDigest` voyagent dans `HandshakeConfidentialMeta`. Le recepteur compare le tuple `{ enabled, assume_valid, verifier_backend, digest }` a son `ConfidentialHandshakeCaps` local; tout mismatch sort avec `HandshakeConfidentialMismatch` avant que le transport ne passe a `Ready`.
4. Операторы DOIVENT пересчитывают дайджест (через `compute_confidential_feature_digest`) и удаляют данные с реестрами/политиками в течение дня перед переподключением. Les peers annoncant des digests anciens continuent d echouer le handshake, empechant un etat stale de reentrer dans le set de validateurs.
5. Успех и проверка рукопожатия в течение дня по стандарту `iroha_p2p::peer` (`handshake_failure_count`, помощники по таксономии ошибок) и создание структур журналов с идентификатором удаленного партнера и перехватом информации. Surveillez ces indicateurs pour detecter les replays ou les mauvaises configurations pendant le rollout.

## Жесты и полезные нагрузки
- Иерархия деривации номинального счета:
  - `sk_spend` -> `nk` (ключ обнулителя), `ivk` (входящий ключ просмотра), `ovk` (исходящий ключ просмотра), `fvk`.
- Les payloads de notes chiffre es utilisent AEAD avec des shared keys derivees par ECDH; des view keys d auditeur optionnelles peuvent etre attachees aux outputs selon la politique de l actif.
- Интерфейс командной строки: `confidential create-keys`, `confidential send`, `confidential export-view-key`, инструменты аудита для дешифрования заметок и помощник `iroha app zk envelope` для создания/проверки конвертов Norito в автономном режиме. Torii раскрывает поток вывода мемов через `POST /v2/confidential/derive-keyset`, возвращает шестнадцатеричные формы и base64, чтобы кошельки могли эффективно восстановить программные иерархии блоков.## Газ, ограничения и контроль DoS
- График определения газа:
  - Halo2 (Plonkish): базовый газ `250_000` + газ `2_000`, номинальный общедоступный вход.
  - `5` Байт подтверждения номинала газа, плюс обнулитель номинала расходов (`300`) и обязательство по номиналу (`500`).
  - Операторы могут использовать дополнительную плату за константы через узел конфигурации (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); Изменения распространяются на удаление ошибок или на диване конфигурации, горячую перезагрузку и другие детерминированные приложения в кластере.
- Ограничения продолжительности (настраиваемые по умолчанию):
- `max_proof_size_bytes = 262_144`.
- И18НИ00000284Х, И18НИ00000285Х, И18НИ00000286Х.
- И18НИ00000287Х, И18НИ00000288Х. Отказ от прохода `verify_timeout_ms` прерывает определение инструкции (управление бюллетенями `proof verification exceeded timeout`, `VerifyProof` возвращается при ошибке).
- Дополнительные квоты, обеспечивающие жизнеспособность: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` и `max_public_inputs`, созданные для строителей блоков; `reorg_depth_bound` (>= `max_anchor_age_blocks`) управляет удержанием пограничных контрольно-пропускных пунктов.
- L во время выполнения отменяет выполнение транзакций, депассант ces ограничивает номинал транзакции или блок, emettant des erreurs `InvalidParameter` определяет и сохраняет состояние реестра нетронутым.
- Предварительная фильтрация в памяти конфиденциальных транзакций по номеру `vk_id`, длина доказательства и возраст привязки перед обращением к проверяющему устройству для использования ресурсов.
- Проверка не определена по тайм-ауту или нарушению правил; Транзакции повторяются с явными ошибками. Серверные модули SIMD не имеют дополнительных настроек, которые могут быть изменены для совместимости газа.

### Базовые уровни калибровки и приемочные параметры
- **Эталонные пластины.** Калибровочные работы DOIVENT соединяют три профиля аппаратного обеспечения. Les run ne couvrant pas tous les profils sont rejetes en review.

  | Profil | Architecture | ЦП/экземпляр | Flags compilateur | Объектиф |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) или Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Etablit des valeurs plancher без внутренних векторов; используйте резервную настройку резервных таблиц. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | release par defaut | Valide le path AVX2; проверьте, что ускорение SIMD сохраняется в пределах допуска нейтрального газа. |
  | `baseline-neon` | `aarch64` | AWS Гравитон3 (c7g.4xlarge) | release par defaut | Убедитесь, что серверная часть NEON остается определенной и согласованной с расписаниями x86. |

- **Эталонный жгут.** Все соединения калибровочного газа DOIVENT и другие продукты имеют:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` для подтверждения определения крепления.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`, когда происходят изменения кода операции виртуальной машины.- **Исправлена ​​случайность.** Экспортер `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` перед использованием скамеек для `iroha_test_samples::gen_account_in` подъёма к детерминированному голосу `KeyPair::from_seed`. Ремень `IROHA_CONF_GAS_SEED_ACTIVE=...` является идеальным вариантом; если переменная ошибка, то отзыв DOIT отображается. Toute nouvelle utilite de calibration doit continuer a honorer cette env var lors de l introduction d alea auxiliaire.

- **Сохранение результатов.**
  - Загрузите резюме по критерию (`target/criterion/**/raw.csv`) для каждого профиля в артефакте выпуска.
  - Сохранение производных показателей (`ns/op`, `gas/op`, `ns/gas`) в [регистре конфиденциальной калибровки газа] (./confidential-gas-calibration) с коммитом git и используемой версией компилятора.
  - Сохранение базовых показателей двух последних дней по профилю; Поддержите снимки, а также старые контакты и недавние подтверждения.

- **Допуски приемки.**
  - Разница газа между `baseline-simd-neutral` и `baseline-avx2` DOIVENT rester <= +/-1,5%.
  - Разница газа между `baseline-simd-neutral` и `baseline-neon` DOIVENT rester <= +/-2,0%.
  - Предложения по калибровке являются обязательными для корректировок расписания или RFC, поясняющих карту и меры по смягчению последствий.

– **Контрольный список проверки.** Ответственные за отправку:
  - Inclure `uname -a`, extraits de `/proc/cpuinfo` (model, stepping), et `rustc -Vv` dans le log de calibration.
  - Устройство проверки `IROHA_CONF_GAS_SEED` на стенде для вылазок (на стендах отображается активное семя).
  - Убедитесь, что функции флажков кардиостимулятора и верификатора достоверны в производстве (`--features confidential,telemetry` на стендах с телеметрией).

## Конфигурация и операции
- `iroha_config` к разделу `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Телеметрия в совокупности метрик: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` и `confidential_policy_transitions_total`, sans jamais разоблачитель des donnees en clair.
- Поверхности РПК:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Стратегия испытаний
- Determinisme: le shuffling aleatoire des transactions dans les blocs donne des Merkle roots et nullifier sets identiques.
- Устойчивость к реорганизации: симулятор реорганизации нескольких блоков с привязками; les nullifiers остаются стабильными и устаревшие якоря Sont Rejetes.
- Invariants de gas: verifier un usage de gas identique entre noeuds avec et sans acceleration SIMD.
- Tests de limite: proofs aux plafonds de taille/gas, max in/out counts, enforcement des timeouts.
- Lifecycle: operations de governance pour activation/deprecation de verificateur et parametres, tests de depense apres rotation.
- Policy FSM: transitions autorisees/interdites, delais de transition pendante, et rejet mempool autour des hauteurs effectives.
- Urgences de registry: retrait d urgence fige les actifs a `withdraw_height` et rejette les proofs apres.
- Capability gating: validateurs avec `conf_features` mismatched rejettent les blocs; observers avec `assume_valid=true` suivent sans affecter le consensus.
- Equivalence d etat: noeuds validator/full/observer produisent des roots d etat identiques sur la chaine canonique.
- Fuzzing negatif: proofs malformees, payloads surdimensionnes, et collisions de nullifier rejetees deterministiquement.

## Миграция
- Rollout feature-gated: jusqu a ce que Phase C3 se termine, `enabled` default a `false`; les noeuds annoncent leurs capacites avant de rejoindre le set de validateurs.
- Прозрачные действия не влияют на ситуацию; les instructions confidentielles requierent des entrees registry et une negociation de capacites.
- Les noeuds compiles sans support confidentiel rejettent les blocs pertinents deterministiquement; ils ne peuvent pas rejoindre le set de validateurs mais peuvent fonctionner comme observers avec `assume_valid=true`.
- Генезис включает в себя инициальные записи реестра, ансамбли параметров, конфиденциальную политику для действий и ключи для выбора аудитора.
- Операторы, следящие за публикациями Runbook, для ротации реестра, политических преобразований и срочного восстановления для поддержания детерминированных обновлений.## Оставшиеся роды
- Бенчмаркер ансамблей параметров Halo2 (хвост схемы, стратегия поиска) и регистрация результатов в сборнике калибровок для измерения в течение дня значений по умолчанию газа/тайм-аута с обновлением prochain `confidential_assets_calibration.md`.
- Завершение политики аудита раскрытия информации и API-интерфейсов ассоциаций выборочного просмотра, а также одобрение рабочего процесса в Torii для подписания проекта управления.
- Создайте схему шифрования-свидетеля для обработки пакетов выходных данных для нескольких получателей и заметок в документированном формате конверта для разработчиков SDK.
- Комиссар по проверке внешней безопасности цепей, реестров и процедур ротации параметров и архивированию заключений в качестве основы для взаимопонимания и внутреннего аудита.
- Спецификатор API-интерфейсов сверки расходов для аудиторов и публикации руководства по ключу представления области действия, в котором поставщики кошельков реализуют семантические мемы для аттестации.## Поэтапная реализация d
1. **Фаза M0 – Упрочнение во время стоянки**
   - [x] Получение обнулителя соответствует дизайну Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) с определением порядка обязательств, налагаемым в обновлениях бухгалтерской книги.
   - [x] L применение плафонов для подтверждения и конфиденциальных квот по транзакциям/блокам, отмена транзакций за пределами бюджета с учетом определенных ошибок.
   - [x] Объявление P2P о рукопожатии `ConfidentialFeatureDigest` (бэкэнд-дайджест + реестр отпечатков пальцев) и отображение результатов определения несоответствий через `HandshakeConfidentialMismatch`.
   - [x] Отстранение от паники на пути к конфиденциальному исполнению и добавление ролевых ворот для тех, кто не арестован.
   - [ ] Приложение бюджетов времени ожидания проверки и значений профондера реорганизации для пограничных контрольно-пропускных пунктов.
     - [x] Бюджеты тайм-аутов приложений проверки; les доказательство depassant `verify_timeout_ms` подтверждает детерминированность обслуживания.
     - [x] Пограничные контрольно-пропускные пункты с учетом обслуживания `reorg_depth_bound`, сокращение контрольно-пропускных пунктов плюс старые настройки, которые настраиваются для обеспечения безопасности детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`, политику FSM и шлюзы для выполнения инструкций по выпуску/передаче/раскрытию.
   - Зафиксировать `conf_features` в заголовках блока и отказаться от участия валидаторов, если дайджесты реестра/параметров расходятся.
2. **Фаза M1 – Реестры и параметры**
   - Освобождайте реестры `ZkVerifierEntry`, `PedersenParams` и `PoseidonParams` с управлением операциями, созданием и использованием кэша.
   - Подключение системных вызовов для поиска в реестре, идентификаторов расписания газа, хеширования схемы и дополнительных проверок.
   - Публикация формата полезной нагрузки v1, векторов получения ключей для кошельков и поддержка CLI для управления секретными ключами.
3. **Фаза M2 — Газ и производительность**
   - Реализация расписания определения газа, вычислений по блоку и использования эталонных тестов с телеметрией (задержка проверки, хвосты проверки, сброс памяти).
   - Контрольные точки Durcir CommitmentTree, начисление LRU и индексы обнуления для рабочих нагрузок с несколькими активами.
4. **Этап M3 — кошелек для ротации и оснастки**
   - Активное принятие доказательств с несколькими параметрами и несколькими версиями; Поддержка l пилотная версия активации/устаревания для управления с модулями Runbook де перехода.
   - Открытие потоков миграции SDK/CLI, рабочих процессов проверки аудитора и инструментов сверки расходов.
5. **Фаза M4 – Аудит и операции**
   - Функционирование рабочих процессов ключей аудитора, API выборочного раскрытия информации и операций Runbook.
   - Планируйте обзор внешней криптографии/безопасности и публикуйте выводы в формате `status.md`.

Фаза Chaque включала в себя вехи дорожной карты и ассоциации по тестированию для обеспечения гарантий выполнения, определенных для блокчейна.