<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: es
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

# FAQ de settlement de Nexus

**Enlace del roadmap:** NX-14 - documentacion de Nexus y runbooks de operadores  
**Estado:** Redactado 2026-03-24 (refleja settlement router y especificaciones del playbook CBDC)  
**Audiencia:** Operadores, autores de SDK y revisores de gobernanza que preparan el lanzamiento de
Nexus (Iroha 3).

Este FAQ responde las preguntas que surgieron durante la revision NX-14 sobre routing de
settlement, conversion de XOR, telemetria y evidencia de auditoria. Consulte
`docs/source/settlement_router.md` para la especificacion completa y
`docs/source/cbdc_lane_playbook.md` para los knobs de politica especificos de CBDC.

> **TL;DR:** Todos los flujos de settlement pasan por Settlement Router, que
> debita buffers XOR en lanes publicas y aplica tarifas especificas por lane. Los operadores
> deben mantener la config de routing (`config/config.toml`), los dashboards de telemetria y
> los logs de auditoria en sync con los manifests publicados.

## Preguntas frecuentes

### Que lanes manejan settlement y como se donde encaja mi DS?

- Cada dataspace declara un `settlement_handle` en su manifest. Los handles por defecto se asignan
  asi:
  - `xor_global` para lanes publicas por defecto.
  - `xor_lane_weighted` para lanes publicas personalizadas que obtienen liquidez en otro lugar.
  - `xor_hosted_custody` para lanes privadas/CBDC (buffer XOR en escrow).
  - `xor_dual_fund` para lanes hibridas/confidenciales que mezclan flujos shielded + publicos.
- Consulta `docs/source/nexus_lanes.md` para clases de lane y
  `docs/source/project_tracker/nexus_config_deltas/*.md` para las aprobaciones mas recientes del
  catalogo. `irohad --sora --config ... --trace-config` imprime el catalogo efectivo en runtime
  para auditorias.

### Como determina el Settlement Router las tasas de conversion?

- El router aplica una sola ruta con precios deterministas:
  - Para lanes publicas usamos el pool de liquidez XOR on-chain (DEX publica). Los oraculos de
    precio hacen fallback al TWAP aprobado por gobernanza cuando la liquidez es baja.
  - Las lanes privadas pre-financian buffers XOR. Cuando debitan un settlement, el router registra
    la tupla de conversion `{lane_id, source_token, xor_amount, haircut}` y aplica haircuts
    aprobados por gobernanza (`haircut.rs`) si los buffers se desalinean.
- La configuracion vive bajo `[settlement]` en `config/config.toml`. Evita ediciones personalizadas
  salvo que la gobernanza lo indique. Consulta `docs/source/settlement_router.md` para descripciones
  de campos.

### Como se aplican tarifas y reembolsos?

- Las tarifas se expresan por lane en el manifest:
  - `base_fee_bps` - se aplica a cada debito de settlement.
  - `liquidity_haircut_bps` - compensa a proveedores de liquidez compartida.
  - `rebate_policy` - opcional (p. ej., rebates promocionales CBDC).
- El router emite eventos `SettlementApplied` (formato Norito) con desgloses de tarifas para que
  SDKs y auditores concilien entradas del ledger.

### Que telemetria demuestra que los settlements estan saludables?

- Metricas Prometheus (exportadas via `iroha_telemetry` y settlement router):
  - `nexus_settlement_latency_seconds{lane_id}` - P99 debe mantenerse por debajo de 900 ms para
    lanes publicas / 1200 ms para lanes privadas.
  - `settlement_router_conversion_total{source_token}` - confirma volumenes de conversion por token.
  - `settlement_router_haircut_total{lane_id}` - alertar cuando sea distinto de cero sin una nota de
    gobernanza asociada.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` - muestra debitos XOR en vivo por lane
    (micro unidades). Alertar cuando <25 %/10 % de Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` - variacion de haircut realizada para conciliar
    con P&L de tesoreria.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` - epsilon efectivo aplicado en el ultimo
    bloque; auditores comparan contra la politica del router.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` - uso de linea de credito
    sponsor/MM; alertar por encima de 80 %.
- `nexus_lane_block_height{lane,dataspace}` - ultima altura de bloque observada para el par
  lane/dataspace; mantener peers vecinos dentro de pocos slots entre si.
- `nexus_lane_finality_lag_slots{lane,dataspace}` - slots de separacion entre el head global y el
  bloque mas reciente de esa lane; alertar cuando >12 fuera de drills.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` - backlog esperando settlement en XOR; gatear
  cargas CBDC/privadas antes de superar umbrales regulatorios.

`settlement_router_conversion_total` lleva las etiquetas `lane_id`, `dataspace_id` y
`source_token` para demostrar que activo de gas impulso cada conversion.
`settlement_router_haircut_total` acumula unidades XOR (no micro cantidades brutas), permitiendo a
Tesoreria conciliar el ledger de haircut directamente desde Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` muestra si el router aplico el
  bucket de margen `stable`, `elevated` o `dislocated`. Las entradas elevated/dislocated deben
  enlazar al log de incidente o nota de gobernanza.
- Dashboards: `dashboards/grafana/nexus_settlement.json` mas el overview
  `nexus_lanes.json`. Enlaza alertas a `dashboards/alerts/nexus_audit_rules.yml`.
- Cuando la telemetria de settlement se degrada, registra el incidente segun el runbook en
  `docs/source/nexus_operations.md`.

### Como exporto telemetria de lane para reguladores?

Ejecuta el helper de abajo cuando un regulador solicite la tabla de lanes:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` acepta el blob JSON devuelto por `iroha status --format json`.
* `--json-out` captura un array JSON canonico por lane (aliases, dataspace, altura de bloque,
  finality lag, capacidad/utilizacion TEU, contadores de scheduler trigger + utilization, RBC
  throughput, backlog, metadatos de gobernanza, etc.).
* `--parquet-out` escribe el mismo payload como archivo Parquet (schema Arrow), listo para
  reguladores que requieren evidencia columnar.
* `--markdown-out` emite un resumen legible que marca lanes con lag, backlog no cero, evidencia de
  compliance faltante y manifests pendientes; el default es `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` es opcional; cuando se provee debe apuntar al manifest JSON descrito en el doc
  de compliance para que las filas exportadas incrusten la politica de lane correspondiente,
  firmas de revisores, snapshot de metricas y extractos de audit log.

Archiva ambos outputs bajo `artifacts/` con la evidencia routed-trace (capturas de
`nexus_lanes.json`, estado de Alertmanager y `nexus_lane_rules.yml`).

### Que evidencia esperan los auditores?

1. **Snapshot de config** - captura `config/config.toml` con la seccion `[settlement]` y el catalogo
   de lanes referenciado por el manifest actual.
2. **Logs del router** - archiva `settlement_router.log` diariamente; incluye IDs de settlement
   con hash, debitos XOR y donde se aplicaron haircuts.
3. **Exportes de telemetria** - snapshot semanal de las metricas mencionadas arriba.
4. **Reporte de conciliacion** - opcional pero recomendado: exporta entradas `SettlementRecordV1`
   (ver `docs/source/cbdc_lane_playbook.md`) y compara contra el ledger de tesoreria.

### Los SDKs necesitan manejo especial para settlement?

- Los SDKs deben:
  - Proveer helpers para consultar eventos de settlement (`/v1/settlement/records`) e interpretar
    logs `SettlementApplied`.
  - Exponer lane IDs + settlement handles en la configuracion del cliente para que los operadores
    puedan rutear transacciones correctamente.
  - Replicar los payloads Norito definidos en `docs/source/settlement_router.md` (p. ej.,
    `SettlementInstructionV1`) con pruebas end-to-end.
- El quickstart del SDK Nexus (siguiente seccion) detalla snippets por lenguaje para onboarding a
  la red publica.

### Como interactuan los settlements con gobernanza o frenos de emergencia?

- La gobernanza puede pausar handles de settlement especificos via updates de manifest. El router
  respeta el flag `paused` y rechaza nuevos settlements con un error determinista
  (`ERR_SETTLEMENT_PAUSED`).
- Los "haircut clamps" de emergencia limitan los debitos maximos de XOR por bloque para evitar
  drenar buffers compartidos.
- Los operadores deben monitorear `governance.settlement_pause_total` y seguir la plantilla de
  incidentes en `docs/source/nexus_operations.md`.

### Donde reporto bugs o solicito cambios?

- Faltas de funcionalidad -> abre un issue etiquetado `NX-14` y enlaza al roadmap.
- Incidentes urgentes de settlement -> pagina al responsable principal de Nexus (ver
  `docs/source/nexus_operations.md`) y adjunta logs del router.
- Correcciones de documentacion -> presenta PRs contra este archivo y los equivalentes del portal
  (`docs/portal/docs/nexus/overview.md`, `docs/portal/docs/nexus/operations.md`).

### Puedes mostrar ejemplos de flujos de settlement?

Los siguientes snippets muestran lo que los auditores esperan para los tipos de lane mas comunes.
Captura el log del router, hashes del ledger y el export de telemetria correspondiente para cada
escenario, para que los revisores puedan reproducir la evidencia.

#### Lane CBDC privada (`xor_hosted_custody`)

A continuacion hay un log del router recortado para una lane CBDC privada usando el handle de
custodia alojada. El log prueba debitos XOR deterministas, composicion de tarifas e IDs de
telemetria:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

En Prometheus deberias ver las metricas correspondientes:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

Archiva el fragmento de log, el hash de transaccion del ledger y el export de metricas juntos para
que los auditores puedan reconstruir el flujo. Los siguientes ejemplos muestran como registrar la
 evidencia para lanes publicas y hibridas/confidenciales.

#### Lane publica (`xor_global`)

Los data spaces publicos enrutan a traves de `xor_global`, asi que el router debita el buffer DEX
compartido y registra el TWAP en vivo que precio la transferencia. Adjunta el hash de TWAP o nota
 de gobernanza cuando el oraculo haga fallback a un valor cacheado.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

Las metricas prueban el mismo flujo:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

Guarda el registro TWAP, el log del router, el snapshot de telemetria y el hash del ledger en el
mismo bundle de evidencia. Cuando se disparen alertas por latencia de lane 0 o frescura de TWAP,
enlaza el ticket de incidente a este bundle.

#### Lane hibrida/confidencial (`xor_dual_fund`)

Las lanes hibridas mezclan buffers shielded con reservas XOR publicas. Cada settlement debe mostrar
que bucket origino el XOR y como la politica de haircut dividio las tarifas. El log del router
expone esos detalles via el bloque de metadata dual-fund:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

Archiva el log del router junto con la politica dual-fund (extracto del catalogo de gobernanza),
el export `SettlementRecordV1` para la lane y el snippet de telemetria para que los auditores
confirmen que el split shielded/publico respeto los limites de gobernanza.

Mantenga este FAQ actualizado cuando cambie el comportamiento del settlement router o cuando la
 gobernanza introduzca nuevas clases de lane/politicas de tarifas.
