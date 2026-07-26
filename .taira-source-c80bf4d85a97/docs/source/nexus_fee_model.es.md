---
lang: es
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
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
- Cada transaccion debe incluir el campo tipado y vinculado a la firma
  `fee_payment` (`FeePaymentIntent`). Este elige como pagador a la autoridad
  o a un programa sponsor exacto con su revision inmutable, e incluye los
  maximos firmados por componente y un limite de gas positivo cuando sea
  necesario. Se rechazan las claves de metadatos heredadas `fee_sponsor`,
  `gas_limit` y `gas_asset_id`.
- Cotiza antes de firmar: crea el payload sin firmar exacto, haz que su
  autoridad autentique `POST /v1/fees/quote`, revisa el intent recomendado,
  sustituye solo `payload.fee_payment` y firma y envia ese mismo payload. La
  cotizacion es una observacion, no una reserva; admission vuelve a comprobar
  el estado actual.
- El settlement directo admite como pagador a la autoridad o a un programa
  sponsor exacto. El settlement por recibos (`lane_relay_burn`) solo admite
  un sponsor exacto: las tarifas Nexus pagadas por la autoridad se rechazan con
  `relay_capacity_unavailable` porque su saldo no es un source lock de recibo
  autenticado.
- Cada transaccion que paga gas registra un `LaneSettlementReceipt`. Cada recibo almacena el
  identificador de origen provisto por el llamante, el micro-monto local, el XOR debido
  inmediatamente, el XOR esperado despues del haircut, el margen de seguridad realizado
  (`xor_variance`), y el timestamp del bloque en milisegundos.
- La ejecucion del bloque agrega recibos por lane/dataspace y los publica via `lane_settlement_commitments`
  en `/v1/sumeragi/status`. Los totales exponen `total_local_amount`, `total_xor_due`, y
  `total_xor_after_haircut` sumados sobre el bloque para exportes nocturnos de conciliacion.
- Un nuevo contador `total_xor_variance` rastrea cuanto margen de seguridad se consumio
  (diferencia entre el XOR debido y la expectativa post-haircut), y `swap_metadata` documenta los
  parametros deterministas de conversion (TWAP, epsilon, liquidity profile, y volatility_class)
  para que los auditores puedan verificar los insumos de la cotizacion independientemente de la
  configuracion de runtime.

Los consumidores pueden observar `lane_settlement_commitments` junto con los snapshots existentes de
commitments de lane y dataspace para verificar que los buffers de tarifas, los tiers de haircut y la
execucion de swap coincidan con el modelo de tarifas de Nexus configurado.
