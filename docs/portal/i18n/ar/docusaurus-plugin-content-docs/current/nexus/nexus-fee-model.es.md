---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-fee-model
title: Actualizaciones del modelo de tarifas de Nexus
description: Espejo de `docs/source/nexus_fee_model.md`, que documenta los recibos de liquidacion de lanes y las superficies de conciliacion.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/nexus_fee_model.md`. Manten ambas copias alineadas mientras las traducciones al japones, hebreo, espanol, portugues, frances, ruso, arabe y urdu migran.
:::

# Actualizaciones del modelo de tarifas de Nexus

El router de liquidacion unificado ahora captura recibos deterministas por lane para que los operadores puedan conciliar los debitos de gas contra el modelo de tarifas de Nexus.

- Para la arquitectura completa del router, la politica de buffer, la matriz de telemetria y la secuencia de despliegue, ve `docs/settlement-router.md`. Esa guia explica como los parametros documentados aqui se vinculan con el entregable del roadmap NX-3 y como los SREs deben monitorear el router en produccion.
- La configuracion del activo de gas (`pipeline.gas.units_per_gas`) incluye un decimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, o `tier3`), y una `volatility_class` (`stable`, `elevated`, `dislocated`). Estas banderas alimentan el router de liquidacion para que la cotizacion XOR resultante coincida con el TWAP canonico y el tier de haircut para el lane.
- Cada transaccion que paga gas registra un `LaneSettlementReceipt`. Cada recibo almacena el identificador de origen provisto por el caller, el micro-monto local, el XOR debido inmediatamente, el XOR esperado despues del haircut, la varianza realizada (`xor_variance_micro`), y la marca de tiempo del bloque en milisegundos.
- La ejecucion de bloques agrega recibos por lane/dataspace y los publica via `lane_settlement_commitments` en `/v1/sumeragi/status`. Los totales exponen `total_local_micro`, `total_xor_due_micro`, y `total_xor_after_haircut_micro` sumados sobre el bloque para las exportaciones nocturnas de conciliacion.
- Un nuevo contador `total_xor_variance_micro` rastrea cuanto margen de seguridad se consumio (diferencia entre el XOR debido y la expectativa post-haircut), y `swap_metadata` documenta los parametros de conversion determinista (TWAP, epsilon, liquidity profile, y volatility_class) para que los auditores puedan verificar las entradas de la cotizacion independientemente de la configuracion en runtime.

Los consumidores pueden observar `lane_settlement_commitments` junto con los snapshots existentes de commitments de lane y dataspace para verificar que los buffers de tarifas, los niveles de haircut y la ejecucion del swap coinciden con el modelo de tarifas de Nexus configurado.
