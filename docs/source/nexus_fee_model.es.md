---
lang: es
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e02872dbcb6d92d8be4d40fc2864f28fc6564391640a6ea67768a1f837b57e0f
source_last_modified: "2025-11-15T20:09:59.438546+00:00"
translation_last_reviewed: 2026-01-01
---

# Actualizaciones del modelo de tarifas de Nexus

El router de settlement unificado ahora captura recibos deterministas por lane para que los operadores
puedan reconciliar los debitos de gas con el modelo de tarifas de Nexus.

- Para la arquitectura completa del router, la politica de buffers, la matriz de telemetria y la
  secuencia de rollout, ver `docs/settlement-router.md`. Esa guia explica como los parametros
  documentados aqui se conectan con el entregable de roadmap NX-3 y como los SREs deben monitorear
  el router en produccion.
- La configuracion del asset de gas (`pipeline.gas.units_per_gas`) incluye un decimal `twap_local_per_xor`,
  un `liquidity_profile` (`tier1`, `tier2`, o `tier3`), y un `volatility_class` (`stable`, `elevated`,
  `dislocated`). Estas banderas alimentan el settlement router para que la cotizacion XOR resultante
  coincida con el TWAP canonico y el tier de haircut de la lane.
- Cada transaccion que paga gas registra un `LaneSettlementReceipt`. Cada recibo almacena el
  identificador de origen provisto por el llamante, el micro-monto local, el XOR debido
  inmediatamente, el XOR esperado despues del haircut, el margen de seguridad realizado
  (`xor_variance_micro`), y el timestamp del bloque en milisegundos.
- La ejecucion del bloque agrega recibos por lane/dataspace y los publica via `lane_settlement_commitments`
  en `/v1/sumeragi/status`. Los totales exponen `total_local_micro`, `total_xor_due_micro`, y
  `total_xor_after_haircut_micro` sumados sobre el bloque para exportes nocturnos de conciliacion.
- Un nuevo contador `total_xor_variance_micro` rastrea cuanto margen de seguridad se consumio
  (diferencia entre el XOR debido y la expectativa post-haircut), y `swap_metadata` documenta los
  parametros deterministas de conversion (TWAP, epsilon, liquidity profile, y volatility_class)
  para que los auditores puedan verificar los insumos de la cotizacion independientemente de la
  configuracion de runtime.

Los consumidores pueden observar `lane_settlement_commitments` junto con los snapshots existentes de
commitments de lane y dataspace para verificar que los buffers de tarifas, los tiers de haircut y la
execucion de swap coincidan con el modelo de tarifas de Nexus configurado.
