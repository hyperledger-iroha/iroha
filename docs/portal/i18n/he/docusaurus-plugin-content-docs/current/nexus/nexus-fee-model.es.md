---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-fee-model
כותרת: Actualizaciones del Modelo de Tarifas de Nexus
תיאור: Espejo de `docs/source/nexus_fee_model.md`, que documenta los recibos de liquidacion de lanes y las superficies de conciliacion.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/nexus_fee_model.md`. Manten ambas copias alineadas mientras las traducciones al japones, עברית, אספנול, פורטוגז, פרנסס, רוסו, ערב ואורדו מיגראן.
:::

# Actualizaciones del modelo de tarifas de Nexus

נתב ה-Liquidacion Unificado ahora captura recibos deterministas por lane para que los operadores puedan conciliar los debitos de gas contra el modelo de tarifas de Nexus.

- Para la arquitectura completa del router, la politica de buffer, la matriz de telemetria y la secuencia de despliegue, ve `docs/settlement-router.md`. זה הסבר כמו מסמכים של הפרמטרים האפשריים עם מפת הדרכים המרתקת NX-3 y como los SREs deben monitorear el router in produccion.
- La configuracion del active de gas (`pipeline.gas.units_per_gas`) כולל אחד עשרוני `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, או I1801una `volatility_class` (`stable`, `elevated`, `dislocated`). Estas banderas alimentan el router de liquidacion para que la cotizacion XOR resultante coincida con el TWAP canonico y el tier de hair de para el lane.
- Cada transaccion que paga gas registra un `LaneSettlementReceipt`. Cada recibo almacena el identificador de origen provisto por el caller, el micro-monto local, el XOR debido inmediatamente, el XOR esperado despues del hair, la varianza realizada (`xor_variance_micro`), y la marca de tiempo del bloque en milisegundos.
- La ejecucion de bloques agrega recibos por lane/dataspace y los publica via `lane_settlement_commitments` en `/v1/sumeragi/status`. Los totals exponen `total_local_micro`, `total_xor_due_micro`, y `total_xor_after_haircut_micro` sumados sobre el bloque para las exportaciones nocturnas de conciliacion.
- Un nuevo contador `total_xor_variance_micro` rastrea cuanto margen de seguridad se consumio (diferencia entre el XOR debido y la expectativa post-haircut), y `swap_metadata` documenta los parametros de conversion determinista (TWAP, lucide, psique) profile parame auditores puedan verificar las entradas de la cotizacion independientemente de la configuracion בזמן ריצה.

Los consumidores pueden observar `lane_settlement_commitments` junto con los fotos existentes de commitments de lane and dataspace para validar que los buffers de tarifas, los niveles de hair and la eecucion of swap coincided with el modelo de tarifas de configurado0003X00.