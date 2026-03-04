---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: Mises a jour du modele de frais Nexus
descripción: Miroir de `docs/source/nexus_fee_model.md`, documentando los recursos de regulación de carriles y las superficies de reconciliación.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_fee_model.md`. Guarde las dos copias alineadas durante la migración de las traducciones japonesa, hebraica, española, portuguesa, francesa, rusa, árabe y nuestra.
:::

# Mises a jour du modele de frais Nexus

El enrutador de control unifica la captura de mantenimiento de los recursos determinados por el carril para que los operadores puedan reconciliar los débitos de gas con el modelo de frais Nexus.- Para la arquitectura completa del enrutador, la política de buffer, la matriz de telemetría y la secuencia de implementación, ver `docs/settlement-router.md`. Esta guía comenta explícitamente los parámetros documentados aquí se adjuntan al libro de ruta NX-3 y comenta los SRE que vigilan el enrutador en producción.
- La configuración del activo de gas (`pipeline.gas.units_per_gas`) incluye un decimal `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, o `tier3`), y un `volatility_class` (`stable`, `elevated`, `dislocated`). Estos indicadores alimentan el enrutador de control hasta que la conexión XOR corresponde al TWAP canónico y al palier de corte de pelo para la línea.
- Cada transacción que paie du gas se registra en un `LaneSettlementReceipt`. Cada vez que busque la fuente de identificación proporcionada por el apelante, el micromontante local, el XOR a regler inmediato, el XOR asistirá después del corte de pelo, la varianza realizada (`xor_variance_micro`) y la horodata del bloque en milisegundos.
- La ejecución de bloques agrega los recursos por carril/espacio de datos y los públicos a través de `lane_settlement_commitments` en `/v1/sumeragi/status`. Los todos los expuestos `total_local_micro`, `total_xor_due_micro` y `total_xor_after_haircut_micro` se agregan al bloque para las exportaciones nocturnas de reconciliación.- Un nuevo ordenador `total_xor_variance_micro` se adapta al margen de seguridad consumible (diferencia entre el XOR del y el asistente post-haircut), y el `swap_metadata` documenta los parámetros de conversión determinantes (TWAP, épsilon, perfil de liquidez y volatilidad_clase) para que los auditores puedan verificar las entradas de la Cotación independiente de la configuración de ejecución.

Los consumidores pueden buscar `lane_settlement_commitments` en las cuentas de instantáneas existentes de compromisos de carril y de espacio de datos para verificar que los buffers de frais, los paliers de haircut y la ejecución del swap correspondientes al modelo de frais Nexus configuren.