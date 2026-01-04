---
lang: es
direction: ltr
source: docs/settlement-router.md
status: complete
translator: manual
source_hash: a00e929ef6088863db93ea122b7314ee921b9b37bea2c6e2c536082c29038f97
source_last_modified: "2025-11-12T08:35:29.309807+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/settlement-router.md (Deterministic Settlement Router) -->

# Router de Liquidación Determinista (NX-3)

**Estado:** Planificado (NX-3)  
**Responsables:** Economics WG / Core Ledger WG / Treasury / SRE  
**Alcance:** Implementa la política unificada de conversión a XOR y de búfer descrita en
la entrada de la hoja de ruta “Unified lane settlement & XOR conversion (NX-3)”.

Este documento formaliza el algoritmo, la telemetría y los contratos operativos del router
de liquidación unificado. Complementa el resumen de comisiones de
`docs/source/nexus_fee_model.md` explicando cómo el router deriva los débitos en XOR,
interactúa con el DEX AMM/CLMM canónico (o el RFQ gobernado de respaldo) y expone recibos
deterministas para cada lane.

## Objetivos

   Nexus y flujos AMX cross-DS—usan el mismo pipeline de liquidación, de modo que los
   cierres nocturnos sólo necesitan un modelo.
2. **Conversión determinista:** Las deudas en XOR se derivan de parámetros on‑chain
   (TWAP, épsilon de volatilidad, tier de liquidez) y se redondean hacia arriba a
   micro‑XOR para que cada nodo produzca el mismo recibo.
3. **Búferes de seguridad:** Cada dataspace/lane mantiene un búfer XOR P95 de 72 h. El
   router debita primero los búferes, aplica compuertas de “throttle/XOR‑only” cuando los
   niveles bajan y exporta métricas para que los operadores puedan recargar de forma
   proactiva.
4. **Liquidez gobernada:** Las conversiones prefieren el DEX AMM/CLMM de lane público
   canónico; los RFQ/swap‑line gobernados sólo se activan cuando cae la profundidad o se
   disparan los circuit breakers.

## Arquitectura

| Componente | Ubicación | Responsabilidad |
|-----------|-----------|-----------------|
| `crates/settlement_router/` | Crate del router (nuevo) | Cálculo de shadow price, haircuts, contabilidad de búfer, generador de schedule, seguimiento de exposición P&L. |
| `crates/oracle_adapter/` | Middleware de oráculo | Mantiene la ventana TWAP (60 s por defecto) + volatilidad móvil, emite precio de circuito si las fuentes se quedan obsoletas. |
| `crates/iroha_core/src/fees` y `LaneBlockCommitment` | Core ledger | Persiste recibos de lane, varianza en XOR y metadatos de swaps en pruebas de bloque para auditores. |
| DEX AMM/CLMM canónico y RFQ gobernado | Equipo DEX/AMM | Ejecuta swaps deterministas; proporciona profundidad + tiers de haircut (tier1=profundo → 0 bp, tier2=medio → 25 bp, tier3=delgado → 75 bp). |
| Líneas de swap de Tesorería / APIs de sponsors | Financial Services WG | Permiten que sponsors o MM autorizados (≤25 % de Bmin) recarguen los búferes sin esperar a swaps de mercado. |
| Telemetría + dashboards | `iroha_telemetry`, `dashboards/…` | Emite `iroha_settlement_buffer_xor`, `iroha_settlement_pnl_xor`, `iroha_settlement_haircut_bp`, etc.; los dashboards avisan al cruzar umbrales de búfer. |

## Pipeline de conversión

1. **Agregación de recibos**
   - Cada transacción/leg AMX emite un `LaneSettlementReceipt` con importe local en micro,
     XOR debido, tier de haircut, variación realizada (`xor_variance_micro`) y metadatos del
     llamador (definidos ya en `docs/source/nexus_fee_model.md`).
   - El router agrupa recibos por `(lane, dataspace)` durante la ejecución del bloque.

2. **Shadow price (`shadow_price.rs`)**
   - Obtiene TWAP de 60 s (`twap_local_per_xor`) y una muestra de volatilidad de 30 s.
   - Calcula épsilon: base 25 bp + (bucket de volatilidad × 25 bp, con límite en 75 bp).
   - Persiste el `volatility_class` resultante (`stable`, `elevated`, `dislocated`) dentro
     de `LaneSwapMetadata` para que auditores y dashboards puedan vincular el margen extra
     con el régimen de oráculo capturado en `pipeline.gas.units_per_gas`.
   - Calcula `xor_due = ceil(local_micro / twap × (1 + epsilon))`.
   - Guarda la tupla `(twap, epsilon, tier)` en los metadatos del swap para auditoría.

3. **Haircut + varianza (`haircut.rs`)**
   - Aplica haircuts según el perfil de liquidez (tier1=0 bp, tier2=25 bp, tier3=75 bp).
   - Registra `total_xor_after_haircut` y `total_xor_variance` (diferencia entre lo debido
     y lo post‑haircut) para que Tesorería vea cuánto margen de seguridad consume cada
     tier.

4. **Débito del búfer (`buffer.rs`)**
   - Cada búfer de lane/dataspace define Bmin = gasto XOR P95 a 72 h.
   - Estados: **Normal** (≥75 %), **Alert** (<75 %), **Throttle** (<25 %), **XOR‑only**
     (<10 %), **Halt** (<2 % o oráculo obsoleto).
   - El router debita los búferes por recibo; cuando se cruzan umbrales, emite telemetría
     determinista y aplica throttles (por ejemplo, reducir a la mitad la tasa de inclusión
     por debajo del 25 %).
   - Los metadatos de lane deben declarar la reserva de Tesorería mediante:
     - `metadata.settlement.buffer_account` – ID de cuenta que mantiene la reserva
       (por ejemplo, `buffer::cbdc_treasury`).
     - `metadata.settlement.buffer_asset` – Asset que se debita para headroom (normalmente
       `xor#sora`).
     - `metadata.settlement.buffer_capacity_micro` – Capacidad expresada en micro‑XOR
       (cadena decimal).
   - La telemetría expone `iroha_settlement_buffer_xor` (headroom restante),
     `iroha_settlement_buffer_capacity_xor` y `iroha_settlement_buffer_state` (estado
     discreto).

5. **Planificación de swaps (`schedule.rs`)**
   - Agrupa recibos por asset local y tier de liquidez para minimizar el número de swaps.
   - Respeta límites de tamaño/cantidad para cada orden AMM/CLMM (para no romper
     invariantes de ruta).
   - Produce un `LaneSwapSchedule` determinista que describe:
     - el asset local agregado;
     - la cantidad target en XOR;
     - el DEX/tier seleccionado;
     - el haircut aplicado;
     - el lane/dataspace afectados.

6. **Liquidación y pruebas de bloque**
   - El scheduler envía órdenes a la capa DEX. El resultado de cada swap se codifica en
     `LaneSwapMetadata` y se adjunta a `LaneBlockCommitment`.
   - Los auditores pueden reconstruir la conversión XOR‑local a partir de:
     - TWAP y volatilidad;
     - tier de liquidez;
     - haircut aplicado;
     - variación total de XOR.

## Telemetría y alertas

Las métricas mínimas que deben publicarse:

- `iroha_settlement_buffer_xor{dataspace,lane}` – headroom restante en XOR.
- `iroha_settlement_buffer_capacity_xor{dataspace,lane}` – capacidad configurada Bmin.
- `iroha_settlement_buffer_state{dataspace,lane}` – estado discreto (`normal`,
  `alert`, `throttle`, `xor_only`, `halt`).
- `iroha_settlement_pnl_xor{dataspace,lane}` – P&L acumulado en XOR desde el inicio.
- `iroha_settlement_haircut_bp{dataspace,lane,tier}` – haircut efectivo aplicado.

Dashboards y alertas:

- Alertar cuando el estado pasa a `alert` (<75 %) o peor.
- Crear paneles que muestren headroom por lane/dataspace y consumo diario.
- Destacar lanes donde la varianza acumulada (`total_xor_variance`) supere el rango
  esperado.

## Estrategia de despliegue

Se recomienda habilitar el router en fases:

1. **Shadow mode:** habilitar cálculos del router + telemetría, pero dejando
2. **Activación de débitos:** activar `settlement_debit=true` para lanes de bajo riesgo
   (perfil C) y verificar conciliaciones.
3. **Conversión on:** activar TWAMM slices + swaps AMM cuando la telemetría muestre
   estabilidad en los búferes; mantener el RFQ de respaldo preparado.
4. **Integración con AMX:** garantizar que los lanes AMX propaguen recibos y expongan
   `SETTLEMENT_ROUTER_UNAVAILABLE` a los SDKs para reintentos.
   condicionar futuros merges a que la telemetría de liquidación permanezca dentro de los
   umbrales.

Seguir este documento garantiza que la tarea de la hoja de ruta NX‑3 proporcione un
comportamiento determinista, artefactos auditables y una guía clara tanto para
desarrolladores como para operadores.
