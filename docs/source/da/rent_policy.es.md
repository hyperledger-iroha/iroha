---
lang: es
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Disponibilidad de datos Política de incentivos y alquiler (DA-7)

_Estado: Redacción — Propietarios: Grupo de Trabajo de Economía / Tesorería / Equipo de Almacenamiento_

El elemento de la hoja de ruta **DA-7** introduce una renta explícita denominada XOR para cada blob
enviado a `/v2/da/ingest`, además de bonificaciones que recompensan la ejecución de PDP/PoTR y
La salida servía para buscar clientes. Este documento define los parámetros iniciales,
su representación del modelo de datos y el flujo de trabajo de cálculo utilizado por Torii,
SDK y paneles de tesorería.

## Estructura de políticas

La política está codificada como [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
dentro del modelo de datos. Torii y las herramientas de gobernanza mantienen la política en
Cargas útiles Norito para que se puedan volver a calcular las cotizaciones de alquiler y los libros de incentivos
deterministamente. El esquema expone cinco perillas:

| Campo | Descripción | Predeterminado |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR se cobra por GiB por mes de retención. | `250_000` micro-XOR (0,25 XOR) |
| `protocol_reserve_bps` | Parte del alquiler destinada a la reserva del protocolo (puntos básicos). | `2_000` (20%) |
| `pdp_bonus_bps` | Porcentaje de bonificación por evaluación exitosa del PDP. | `500` (5%) |
| `potr_bonus_bps` | Porcentaje de bonificación por evaluación PoTR exitosa. | `250` (2,5%) |
| `egress_credit_per_gib` | Crédito pagado cuando un proveedor proporciona 1 GiB de datos DA. | `1_500` micro-XOR |

Todos los valores de puntos básicos se validan con `BASIS_POINTS_PER_UNIT` (10000).
Las actualizaciones de políticas deben viajar a través de la gobernanza y cada nodo Torii expone el
política activa a través de la sección de configuración `torii.da_ingest.rent_policy`
(`iroha_config`). Los operadores pueden anular los valores predeterminados en `config.toml`:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

Las herramientas CLI (`iroha app da rent-quote`) aceptan las mismas entradas de política Norito/JSON
y emite artefactos que reflejan el `DaRentPolicyV1` activo sin alcanzar
nuevamente al estado Torii. Proporcione la instantánea de la política utilizada para una ejecución de ingesta para que el
La cita sigue siendo reproducible.

### Artefactos de cotización de alquiler persistentes

Ejecute `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` para
emite tanto el resumen en pantalla como un artefacto JSON bastante impreso. el archivo
registra `policy_source`, la instantánea `DaRentPolicyV1` en línea, la instantánea calculada
`DaRentQuote` y un `ledger_projection` derivado (serializado a través de
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) lo que lo hace adecuado para paneles de tesorería y libro mayor ISI
tuberías. Cuando `--quote-out` apunta a un directorio anidado, la CLI crea cualquier
padres desaparecidos, para que los operadores puedan estandarizar ubicaciones como
`artifacts/da/rent_quotes/<timestamp>.json` junto con otros paquetes de pruebas del DA.
Adjunte el artefacto a las aprobaciones de alquiler o ejecuciones de conciliación para que el XOR
El desglose (alquiler base, reserva, bonificaciones PDP/PoTR y créditos de salida) es
reproducible. Pase `--policy-label "<text>"` para anular automáticamente
descripción derivada `policy_source` (rutas de archivo, valor predeterminado incrustado, etc.) con una
etiqueta legible por humanos, como un ticket de gobernanza o hash de manifiesto; los adornos CLI
este valor y rechaza cadenas vacías/solo espacios en blanco para que la evidencia registrada
sigue siendo auditable.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```La sección de proyección del libro mayor alimenta directamente los ISI del libro mayor de alquileres de DA:
define los deltas XOR destinados a la reserva de protocolo, pagos de proveedores y
los grupos de bonificación por prueba sin necesidad de un código de orquestación personalizado.

### Generación de planes de contabilidad de alquileres

Ejecute `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
para convertir una cotización de alquiler persistente en transferencias de libro mayor ejecutables. el comando
analiza el `ledger_projection` integrado, emite instrucciones Norito `Transfer`
que recaudan el alquiler base hacia la tesorería, encamina la reserva/proveedor
porciones y prefinancia los fondos de bonificación PDP/PoTR directamente del pagador. el
El JSON de salida refleja los metadatos de la cotización para que las herramientas de CI y tesorería puedan razonar.
sobre el mismo artefacto:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

El campo final `egress_credit_per_gib_micro_xor` permite paneles y pagos
Los programadores alinean los reembolsos de salida con la política de alquiler que produjo el
cita sin volver a calcular las matemáticas de políticas en scripting pegamento.

## Cita de ejemplo

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

La cotización se puede reproducir en nodos Torii, SDK e informes de tesorería porque
utiliza estructuras deterministas Norito en lugar de matemáticas ad-hoc. Los operadores pueden
adjunte el código JSON/CBOR `DaRentPolicyV1` a las propuestas de gobernanza o alquile
auditorías para demostrar qué parámetros estaban vigentes para un blob determinado.

## Bonificaciones y reservas

- **Reserva de protocolo:** `protocol_reserve_bps` financia la reserva XOR que respalda
  Replicación de emergencia y reducción de reembolsos. El Tesoro rastrea este cubo
  por separado para garantizar que los saldos del libro mayor coincidan con la tasa configurada.
- **Bonificaciones PDP/PoTR:** Cada evaluación de prueba exitosa recibe una bonificación adicional
  pago derivado de `base_rent × bonus_bps`. Cuando el planificador DA emite pruebas
  recibos incluye las etiquetas de puntos básicos para que los incentivos se puedan reproducir.
- **Crédito de salida:** Los proveedores registran los GiB servidos por manifiesto, multiplicados por
  `egress_credit_per_gib` y envíe los recibos a través de `iroha app da prove-availability`.
  La política de alquiler mantiene la cantidad por GiB sincronizada con la gobernanza.

## Flujo operativo

1. **Ingesta:** `/v2/da/ingest` carga el `DaRentPolicyV1` activo, cotiza el alquiler
   basado en el tamaño y la retención del blob, e incorpora la cotización en Norito
   manifiesto. El remitente firma una declaración que hace referencia al hash de alquiler y
   la identificación del boleto de almacenamiento.
2. **Contabilidad:** Los scripts de ingesta de tesorería decodifican el manifiesto, llaman
   `DaRentPolicyV1::quote` y completar los libros de alquiler (alquiler base, reserva,
   bonificaciones y créditos de salida esperados). Cualquier discrepancia entre el alquiler registrado
   y las cotizaciones recalculadas fallan en CI.
3. **Recompensas de prueba:** Cuando los programadores PDP/PoTR marcan un éxito, emiten un recibo
   que contiene el resumen del manifiesto, el tipo de prueba y el bono XOR derivado de
   la política. La gobernanza puede auditar los pagos recalculando la misma cotización.
4. **Reembolso de salida:** Los orquestadores de búsqueda envían resúmenes de salida firmados.
   Torii multiplica el recuento de GiB por `egress_credit_per_gib` y emite el pago
   instrucciones contra el depósito en garantía del alquiler.

## TelemetríaLos nodos Torii exponen el uso de alquiler a través de las siguientes métricas Prometheus (etiquetas:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-meses citados por `/v2/da/ingest`.
- `torii_da_rent_base_micro_total`: renta base (micro XOR) acumulada en el momento de la ingesta.
- `torii_da_protocol_reserve_micro_total` — contribuciones de reserva de protocolo.
- `torii_da_provider_reward_micro_total`: pagos de alquiler por parte del proveedor.
- `torii_da_pdp_bonus_micro_total` y `torii_da_potr_bonus_micro_total` —
  Los fondos de bonificación de PDP/PoTR provienen de la cotización de ingesta.

Los paneles económicos dependen de estos contadores para garantizar los ISI del libro mayor, los grifos de reserva,
y los cronogramas de bonificación de PDP/PoTR coinciden con los parámetros de política vigentes para cada
clúster y clase de almacenamiento. La placa SoraFS de estado de capacidad Grafana
(`dashboards/grafana/sorafs_capacity_health.json`) ahora muestra paneles dedicados
para distribución de alquileres, acumulación de bonificaciones PDP/PoTR y captura de GiB mensuales, lo que permite
Hacienda para filtrar por clúster Torii o clase de almacenamiento al revisar la ingesta
volumen y pagos.

## Próximos pasos

- ✅ Los recibos `/v2/da/ingest` ahora incorporan `rent_quote` y las superficies CLI/SDK muestran lo citado
  alquiler base, participación de reserva y bonificaciones PDP/PoTR para que los remitentes puedan revisar las obligaciones XOR antes
  comprometer cargas útiles.
- Integre el libro de alquiler con los próximos feeds de reputación/libro de pedidos de DA
  para demostrar que los proveedores de alta disponibilidad están recibiendo los pagos correctos.