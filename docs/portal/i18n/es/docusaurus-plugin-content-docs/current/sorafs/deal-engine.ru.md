---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: motor de acuerdos
título: Движок сделок SoraFS
sidebar_label: programa de televisión
descripción: Обзор движка сделок SF-8, интеграции Torii и телеметрических поверхностей.
---

:::nota Канонический источник
:::

# Движок сделок SoraFS

Mapa SF-8 de un televisor con modelo SoraFS, obsoleto
детерминированный учет соглашений на хранение и извлечение между
clientes y proveedores. Соглашения описываются cargas útiles Norito,
определенными в `crates/sorafs_manifest/src/deal.rs`, покрывая условия сделки,
bonos блокировку, вероятностные микроплатежи и записи расчетов.

Встроенный SoraFS trabajador (`sorafs_node::NodeHandle`) теперь создает
Ejemplo `DealEngine` para el proceso de proceso. Движок:

- validar y registrar el modelo según `DealTermsV1`;
- начисляет cargos en XOR при отчетах об использовании репликации;
- оценивает окна вероятностных микроплатежей с помощью детерминированного
  muestreo en основе BLAKE3; y
- Forme el libro mayor de instantáneas y las cargas útiles según las publicaciones en la gobernanza.Юнит-тесты покрывают валидацию, выбор микроплатежей и расчетные потоки, чтобы
Los operadores pueden proporcionar constantemente API. Расчеты теперь выпускают cargas útiles de gobernanza
`DealSettlementV1`, una versión de la tubería de publicación SF-12 y una serie de novedades
OpenTelemetría `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) para los tableros Torii y
Aplicación de SLO. Следующие шаги фокусируются на автоматизации slashing, инициируемой
Los auditores y las normas semánticas de la gobernanza política.

La configuración del televisor se realiza según la métrica `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, а также счетчики билетов
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эти totales раскрывают вероятностный
лотерейный поток, чтобы операторы могли коррелировать выигрыши микроплатежей и
Créditos permanentes con resultados de búsqueda.

## Integración Torii

Torii muestra los puntos finales más importantes, cuáles son los proveedores que mejoran el uso y la ropa
жизненный цикл сделок без специального cableado:- `POST /v2/sorafs/deal/usage` принимает телеметрию `DealUsageReport` и возвращает
  детерминированные результаты учета (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` завершает текущий ventana, стримя
  Este `DealSettlementRecord` se integra con el codificador base64 `DealSettlementV1`,
  готовым к публикации в gobernancia DAG.
- Лента Torii `/v2/events/sse` теперь транслирует записи `SorafsGatewayEvent::DealUsage`,
  суммирующие каждую отправку uso (época, измеренные GiB-horas, счетчики билетов,
  cargos детерминированные), записи `SorafsGatewayEvent::DealSettlement`,
  Libro mayor de instantáneas de varios archivos adicionales BLAKE3 digest/size/base64
  Arte de gobernanza en disco y alertas `SorafsGatewayEvent::ProofHealth` antes de las ediciones anteriores.
  порогов PDP/PoTR (провайдер, окно, состояние strike/cooldown, сумма штрафа). Потребители могут
  Filtre para comprobar, regire con nuevos televisores, registre o alertas de prueba de salud mediante sondeo.

Оба endpoints участвуют в SoraFS через новое окно
`torii.sorafs.quota.deal_telemetry`, el operador puede instalar el dispositivo de forma predeterminada
частоту отправки для каждого деплоя.