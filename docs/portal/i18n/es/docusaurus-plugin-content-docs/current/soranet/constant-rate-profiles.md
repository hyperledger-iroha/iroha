---
id: constant-rate-profiles
lang: es
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja `docs/source/soranet/constant_rate_profiles.md`. Manten ambas copias sincronizadas hasta que los docs heredados se retiren.
:::

SNNet-17B introduce carriles de transporte de tasa fija para que los relays muevan trafico en celdas de 1,024 B sin importar el tamano del payload. Los operadores eligen entre tres presets:

- **core** - relays de centro de datos o alojados profesionalmente que pueden dedicar >=30 Mbps para cubrir trafico.
- **home** - operadores residenciales o de bajo uplink que aun necesitan fetches anonimos para circuitos sensibles a la privacidad.
- **null** - preset dogfood SNNet-17A2. Conserva los mismos TLVs/envelope pero estira el tick y el ceiling para staging de bajo ancho de banda.

## Resumen de presets

| Perfil | Tick (ms) | Celda (B) | Tope de lanes | Piso dummy | Payload por lane (Mb/s) | Payload techo (Mb/s) | % de uplink en techo | Uplink recomendado (Mb/s) | Tope de vecinos | Disparador de auto-desactivacion (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Tope de lanes** - maximo de vecinos concurrentes de tasa constante. El relay rechaza circuitos extra cuando se alcanza el tope e incrementa `soranet_handshake_capacity_reject_total`.
- **Piso dummy** - numero minimo de lanes que se mantienen vivas con trafico dummy incluso cuando la demanda real es menor.
- **Payload techo** - presupuesto de uplink dedicado a los carriles de tasa constante despues de aplicar la fraccion de ceiling. Los operadores nunca deben exceder este presupuesto aunque haya ancho de banda adicional.
- **Disparador de auto-desactivacion** - porcentaje de saturacion sostenida (promediado por preset) que hace que el runtime baje al piso dummy. La capacidad se restaura despues del umbral de recuperacion (75% para `core`, 60% para `home`, 45% para `null`).

**Importante:** el preset `null` es solo para staging y dogfooding de capacidades; no cumple las garantias de privacidad requeridas para circuitos de produccion.

## Tabla tick -> bandwidth

Cada celda de payload lleva 1,024 B, por lo que la columna KiB/seg equivale al numero de celdas emitidas por segundo. Usa el helper para extender la tabla con ticks personalizados.

| Tick (ms) | Celdas/seg | Payload KiB/seg | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

Formula:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

Helper de CLI:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` emite tablas estilo GitHub tanto para el resumen de presets como para la tabla opcional de ticks, para que puedas pegar la salida deterministica en el portal. Combinalo con `--json-out` para archivar los datos renderizados como evidencia de gobernanza.

## Configuracion y overrides

`tools/soranet-relay` expone los presets tanto en archivos de configuracion como en overrides en tiempo de ejecucion:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

La clave de configuracion acepta `core`, `home` o `null` (default `core`). Los overrides de CLI son utiles para drills de staging o solicitudes del SOC que reduzcan temporalmente el duty cycle sin reescribir configs.

## Guardrails de MTU

- Las celdas de payload usan 1,024 B mas ~96 B de framing Norito+Noise y los headers minimos de QUIC/UDP, manteniendo cada datagrama por debajo del MTU minimo IPv6 de 1,280 B.
- Cuando tuneles (WireGuard/IPsec) agregan encapsulacion extra **debes** reducir `padding.cell_size` para que `cell_size + framing <= 1,280 B`. El validador del relay aplica `padding.cell_size <= 1,136 B` (1,280 B - 48 B de overhead UDP/IPv6 - 96 B de framing).
- Los perfiles `core` deben fijar >=4 neighbors incluso cuando estan idle para que los lanes dummy siempre cubran un subconjunto de guards PQ. Los perfiles `home` pueden limitar circuitos de tasa constante a wallets/aggregators pero deben aplicar back-pressure cuando la saturacion exceda 70% durante tres ventanas de telemetry.

## Telemetria y alertas

Los relays exportan las siguientes metricas por preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alertar cuando:

1. El ratio dummy se mantiene por debajo del piso del preset (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) por mas de dos ventanas.
2. `soranet_constant_rate_ceiling_hits_total` crece mas rapido que un hit cada cinco minutos.
3. `soranet_constant_rate_degraded` cambia a `1` fuera de un drill planificado.

Registra la etiqueta del preset y la lista de neighbors en los reportes de incidentes para que los auditores puedan demostrar que las politicas de tasa constante cumplieron con los requisitos del roadmap.
