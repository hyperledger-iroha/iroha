---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: Actualizaciones del modelo de tarifas de Nexus
descripción: Espejo de `docs/source/nexus_fee_model.md`, que documenta los recibos de liquidación de carriles y las superficies de conciliacion.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_fee_model.md`. Manten ambas copias alineadas mientras las traducciones al japonés, hebreo, español, portugues, francés, ruso, árabe y urdu migran.
:::

# Actualizaciones del modelo de tarifas de Nexus

El router de liquidación unificado ahora captura recibos deterministas por carril para que los operadores puedan conciliar los débitos de gas contra el modelo de tarifas de Nexus.- Para la arquitectura completa del enrutador, la política de buffer, la matriz de telemetría y la secuencia de implementación, ve `docs/settlement-router.md`. Esa guía explica como los parámetros documentados aquí se vinculan con el entregable del roadmap NX-3 y como los SRE deben monitorear el enrutador en producción.
- La configuración del activo de gas (`pipeline.gas.units_per_gas`) incluye un decimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, o `tier3`), y un `volatility_class`. (`stable`, `elevated`, `dislocated`). Estas banderas alimentan el router de liquidación para que la cotización XOR resultante coincida con el TWAP canonico y el tier de haircut para el lane.
- Cada transacción que paga gas registra un `LaneSettlementReceipt`. Cada recibo almacena el identificador de origen provisto por el llamante, el micro-monto local, el XOR debido inmediatamente, el XOR esperado después del corte de pelo, la varianza realizada (`xor_variance_micro`), y la marca de tiempo del bloque en milisegundos.
- La ejecucion de bloques agrega recibos por lane/dataspace y los publica via `lane_settlement_commitments` en `/v1/sumeragi/status`. Los totales exponen `total_local_micro`, `total_xor_due_micro`, y `total_xor_after_haircut_micro` sumados sobre el bloque para las exportaciones nocturnas de conciliación.- Un nuevo contador `total_xor_variance_micro` rastrea cuanto margen de seguridad se consume (diferencia entre el XOR debido y la expectativa post-haircut), y `swap_metadata` documenta los parámetros de conversión determinista (TWAP, epsilon, liquidity perfil, y volatility_class) para que los auditores puedan verificar las entradas de la cotización independientemente de la configuración en tiempo de ejecución.

Los consumidores pueden observar `lane_settlement_commitments` junto con los snapshots existentes de compromisos de lane y dataspace para verificar que los buffers de tarifas, los niveles de haircut y la ejecución del swap coinciden con el modelo de tarifas de Nexus configurado.