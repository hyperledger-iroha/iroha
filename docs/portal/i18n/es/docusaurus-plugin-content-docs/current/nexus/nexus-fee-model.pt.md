---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: Actualizados del modelo de taxas del Nexus
descripción: Espelho de `docs/source/nexus_fee_model.md`, documentando recibos de liquidacao de lanes e superficies de conciliacao.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_fee_model.md`. Mantenha as duas copias alinhadas enquanto as traducoes japonesas, hebraicas, espanholas, portuguesas, francesas, russas, arabes e urdu migram.
:::

# Actualizados del modelo de taxas del Nexus

El roteador de liquidacao unificado agora captura recibos deterministas por carril para que los operadores possam conciliar débitos de gas con el modelo de taxas do Nexus.- Para arquitetura completa do roteador, política de buffer, matriz de telemetría y secuenciamento de rollout, veja `docs/settlement-router.md`. Esta guía explica cómo los parámetros documentados aquí se conectan a la entrega del roadmap NX-3 y cómo los SRE deben monitorear o rotar en la producción.
- La configuración del activo de gas (`pipeline.gas.units_per_gas`) incluye un decimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, o `tier3`), y un `volatility_class` (`stable`, `elevated`, `dislocated`). Estos sinalizadores alimentan el rotor de liquidacao para que a cotacao XOR resultante corresponda al TWAP canonico y al nivel de corte de pelo del carril.
- Cada transacción que paga gas registra un `LaneSettlementReceipt`. Cada recibo armazena o identificador de origem fornecido pelo chamador, o micro-valor local, o XOR devido inmediatamente, o XOR esperado apos o haircut, a variancia realizada (`xor_variance_micro`) y o timestamp do bloco em milissegundos.
- La ejecución de bloques agrega recibos por carril/espacio de datos y los públicos a través de `lane_settlement_commitments` en `/v2/sumeragi/status`. Os totais mostram `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro` somados no bloco para exportacoes notturnas de conciliacao.- Un nuevo contador `total_xor_variance_micro` rastrea la cantidad de margen de seguridad consumida (diferencia entre el XOR devido y el esperado pos-haircut), y `swap_metadata` documenta los parámetros de conversación determinística (TWAP, épsilon, liquidity perfil y volatility_class) para que los auditores puedan verificar las entradas de Cotacao independiente de la configuración en tiempo de ejecución.

Los consumidores pueden acompañar `lane_settlement_commitments` junto con las instantáneas existentes de compromisos de carril y espacio de datos para verificar que los buffers de taxas, los niveles de corte de pelo y la ejecución de intercambio corresponden al modelo de taxas de Nexus configurado.