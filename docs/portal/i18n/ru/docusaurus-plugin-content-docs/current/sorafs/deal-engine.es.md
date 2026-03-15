---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: механизм сделок
Название: Motor de acuerdos de SoraFS
Sidebar_label: Двигатель аккуэрдоса
описание: Резюме электродвигателя SF-8, интеграция с Torii и поверхностями телеметрии.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/deal_engine.md`. Mantén ambas ubicaciones alineadas mientras la documentación hereedada siga active.
:::

# Двигатель электродвигателя SoraFS

На линии движения SF-8 представлен двигатель SoraFS, который работает
Определенные меры по обеспечению безопасности и восстановлению сил между
клиенты и поставщики. Эти сведения описаны с полезными нагрузками Norito
Определено в `crates/sorafs_manifest/src/deal.rs`, что означает завершение этого дела,
Блокировка пособий, вероятностные микропагосы и реестры ликвидаций.

Рабочий элемент SoraFS (`sorafs_node::NodeHandle`) сейчас
`DealEngine` для каждого процесса. Эл мотор:

- проверка и регистрация acuerdos usando `DealTermsV1`;
- накопление грузов с номиналом в XOR, когда вы сообщаете об использовании репликации;
- вероятностная оценка вероятностных микропазов с использованием детерминированного устройства
  базируется на BLAKE3; й
- создавать снимки бухгалтерской книги и ликвидационные данные для публикации
  де Гобернанса.

Проверка унитарных унитарий Las pruebas cubren, выбор микропагосов и потоков
ликвидация для того, чтобы операторы могли получить доступ к API с конфиденциальностью.
Las Liquidaciones Ahora Emiten Payloads de Gobernanza `DealSettlementV1`,
Подключитесь непосредственно к конвейеру публикации SF-12 и реализуйте серию
ОпенТелеметрия `sorafs.node.deal_*`
(И18НИ00000018Х, И18НИ00000019Х, И18НИ00000020Х,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) для панелей мониторинга Torii y
применение SLO. Los siguientes pasos se enfocan en la autotización de slashing
инициация для аудиторов и координатор семантики отмены политики правительства.

Телеметрия для использования также пищевых продуктов с измерительным устройством `sorafs.node.micropayment_*`:
И18НИ00000025Х, И18НИ00000026Х,
И18НИ00000027Х, И18НИ00000028Х,
`micropayment_outstanding_nano`, и контадоры по билетам
(И18НИ00000030Х, И18НИ00000031Х,
`micropayment_tickets_duplicate_total`). Estos Totales Exponen el Flujo de Lotería
Вероятностная вероятность того, что операторы могут коррелировать с победами микропагосов и
перенос кредита с результатами ликвидации.

## Интеграция с Torii

Torii экспонирует конечные точки, выделенные для того, чтобы проверяющие отчеты использовались и проводились в цикле
персонализированная проводка для жизни без греха:- `POST /v1/sorafs/deal/usage` принимает телеметрию `DealUsageReport` и возвращается
  определённые результаты контроля (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` завершает актуальную вентиляцию, передает эл.
  `DealSettlementRecord` — результат объединения с `DealSettlementV1` в base64
  список для публикации в DAG de gobernanza.
- El подача `/v1/events/sse` от Torii сейчас передает регистры `SorafsGatewayEvent::DealUsage`
  que возобновлено каждый раз, когда мы отправляемся (эпоха, GiB-hora medidos, contadores de Tickets,
  грузы детерминированные), регистры `SorafsGatewayEvent::DealSettlement`
  который включает в себя снимок канонической книги учета ликвидации в виде дайджеста/tamaño/base64
  BLAKE3 артефакт правительства на дискотеке, и оповещения `SorafsGatewayEvent::ProofHealth`
  Cada vez que se exceden umbrales PDP/PoTR (проводник, вентиляция, состояние удара/перезарядки,
  «монто де пенализасьон»). Потребители могут использовать фильтр для реакции
  новая телеметрия, ликвидация или оповещение о спасении по результатам опроса.

Конечные точки участвуют в фреймворке SoraFS и проходят по новому каналу.
`torii.sorafs.quota.deal_telemetry`, разрешите работу операторов настроить тарелку отправки
разрешено для деспльега.