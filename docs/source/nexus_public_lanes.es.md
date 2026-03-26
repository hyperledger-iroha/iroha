---
lang: es
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

# Staking de lanes publicas de Nexus (NX-9)

Estado: En progreso -> **runtime + docs de operadores alineados** (Abr 2026)
Responsables: Economics WG / Governance WG / Core Runtime
Referencia de roadmap: NX-9 - staking de lane publico y modulo de recompensas

Esta nota captura el modelo de datos canonico, la superficie de instrucciones, los controles de gobernanza
y los hooks operativos para el programa de staking de lanes publicas de Nexus. El objetivo es permitir que
validadores permissionless se unan a las lanes publicas, bloqueen stake, sirvan bloques y reciban recompensas
mientras la gobernanza mantiene palancas deterministas de slashing/runbook.

El andamiaje de codigo ahora vive en:

- Tipos del modelo de datos: `crates/iroha_data_model/src/nexus/staking.rs`
- Definiciones ISI: `crates/iroha_data_model/src/isi/staking.rs`
- Stub del ejecutor core (devuelve un error determinista de guardia hasta que llegue la logica NX-9):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs pueden comenzar a cablear los payloads Norito antes de la implementacion completa del runtime;
las instrucciones de stake ahora bloquean el asset de staking configurado retirando desde `stake_account`/`staker`
a una cuenta escrow en bonded (`nexus.staking.stake_escrow_account_id`). Los slashes debitan el escrow y acreditan
el sink configurado (`nexus.staking.slash_sink_account_id`), y los unbonds devuelven fondos a la cuenta de origen
cuando expira el temporizador.

## 1. Estado del ledger y tipos

### 1.1 Registros de validadores

`PublicLaneValidatorRecord` rastrea el estado canonico de cada validador:

| Campo | Descripcion |
|-------|-------------|
| `lane_id: LaneId` | Lane que atiende el validador. |
| `validator: AccountId` | Cuenta que firma mensajes de consenso. |
| `stake_account: AccountId` | Cuenta que aporta el auto-bond (puede diferir de la identidad del validador). |
| `total_stake: Numeric` | Stake propio + delegaciones aprobadas. |
| `self_stake: Numeric` | Stake aportado por el validador. |
| `metadata: Metadata` | Comision %, ids de telemetria, flags de jurisdiccion, info de contacto. |
| `status: PublicLaneValidatorStatus` | Ciclo de vida (pending/active/jailed/exiting/etc.). El payload `PendingActivation` codifica la epoca objetivo. |
| `activation_epoch: Option<u64>` | Epoca cuando el validador se activo (se fija en activacion). |
| `activation_height: Option<u64>` | Altura de bloque registrada en activacion. |
| `last_reward_epoch: Option<u64>` | Epoca que produjo el ultimo pago. |

`PublicLaneValidatorStatus` enumera fases del ciclo de vida:

- `PendingActivation(epoch)` - esperando la epoca de activacion especificada por gobernanza; el payload de tupla
  almacena la primera epoca de activacion calculada como `current_epoch + 1` (genesis bootstrap uses `current_epoch`)
  (las epocas se derivan de `epoch_length_blocks`).
- `Active` - participa en consenso y puede cobrar recompensas.
- `Jailed { reason }` - suspendido temporalmente (downtime, breach de telemetria, etc.).
- `Exiting { releases_at_ms }` - unbonding; las recompensas dejan de acumularse.
- `Exited` - removido del conjunto.
- `Slashed { slash_id }` - evento de slashing registrado para auditorias.

Los metadatos de activacion son monotonicos: `activation_epoch`/`activation_height` se fijan la primera vez
que un validador pendiente se activa y cualquier intento de reactivar en una epoca/altura anterior se rechaza.
Los validadores pendientes se promueven automaticamente al inicio del primer bloque cuya epoca cumple el
limite programado, y el contador de metricas de activacion (`nexus_public_lane_validator_activation_total`)
registra la promocion junto con el cambio de estado.

Los despliegues permissioned mantienen el roster de peers genesis activo incluso antes de que exista stake de
validadores de lane publica: mientras los peers tengan llaves de consenso activas, el runtime recurre a los peers
genesis para el set de validadores. Esto evita un deadlock de bootstrap mientras la admision de staking esta
inhabilitada o aun se despliega.

### 1.2 Shares de stake y unbonding

Delegadores (y validadores que aumentan su propio bond) se modelan via `PublicLaneStakeShare`:

- `bonded: Numeric` - monto bonded activo.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` - retiros pendientes con clave `request_id`
  provista por el cliente.
- `metadata` almacena pistas UX/back-office (p. ej., numeros de referencia de mesa de custodia).

`PublicLaneUnbonding` mantiene el cronograma determinista de retiro (`amount`, `release_at_ms`). Torii ahora
expone las shares activas y los retiros pendientes via `GET /v1/nexus/public_lanes/{lane}/stake` para que
las billeteras muestren temporizadores sin RPCs a medida.

Hooks de ciclo de vida (forzados por runtime):

- Las entradas `PendingActivation(epoch)` cambian automaticamente a `Active` cuando la epoca actual alcanza `epoch`.
  La activacion registra `activation_epoch` y `activation_height`, y las regresiones se rechazan tanto para la
  auto-activacion como para llamadas explicitas a `ActivatePublicLaneValidator`.
- Las entradas `Exiting(releases_at_ms)` transicionan a `Exited` cuando el timestamp del bloque supera
  `releases_at_ms`, limpiando filas de stake-share para que la capacidad del validador se recupere sin limpieza manual.
- El registro de recompensas rechaza shares de validadores salvo que el validador este `Active`, evitando que
  validadores pendientes/salientes/jailed acumulen pagos.

### 1.3 Registros de recompensas

Las distribuciones de recompensas usan `PublicLaneRewardRecord` y `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
  "total_reward": "250.0000",
  "shares": [
    { "account": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe...",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Los registros dan a auditores y dashboards evidencia determinista para cada pago. La estructura de recompensa
fluye al ISI `RecordPublicLaneRewards`.

Guardas de runtime:

- Nexus builds deben estar habilitados; builds offline/stub rechazan el registro de recompensas.
- Las epocas de recompensa avanzan monotonicamente por lane; epocas stale o duplicadas se rechazan.
- Los assets de recompensa deben coincidir con el fee sink configurado (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) y el saldo del sink debe cubrir por completo `total_reward`.
- Cada share debe ser positiva y respetar la especificacion numerica del asset; los totales de shares deben
  igualar `total_reward`.

## 2. Catalogo de instrucciones

Todas las instrucciones viven bajo `iroha_data_model::isi::staking`. Derivan encoders/decoders Norito para que
los SDKs envien los payloads sin codecs a medida.

### 2.1 `RegisterPublicLaneValidator`

Registra un validador y bond un stake inicial:

```norito
{
  "lane_id": 1,
  "validator": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
  "stake_account": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Reglas de validacion:

- `initial_stake` >= `min_self_stake` (parametro de gobernanza).
- Metadata DEBE incluir hooks de contacto/telemetria antes de la activacion.
- La gobernanza aprueba/deniega la entrada; hasta entonces el estado es `PendingActivation` y el runtime
  promueve al validador a `Active` en el siguiente limite de epoca una vez alcanzada la epoca objetivo
  (`current_epoch + 1` (genesis bootstrap uses `current_epoch`) al registrar).

### 2.2 `BondPublicLaneStake`

Bondea stake adicional (auto-bond del validador o contribucion del delegador).

Campos clave: `staker`, `amount`, metadata opcional para statements. El runtime debe forzar limites por lane
(`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Inicia el temporizador de unbonding. Los remitentes proveen un `request_id` determinista
(recomendacion: `blake2b(invoice)`), `amount`, y `release_at_ms`. El runtime debe verificar
que `amount` <= stake bonded y ajustar `release_at_ms` al periodo de unbonding configurado.

### 2.4 `FinalizePublicLaneUnbond`

Despues de expirar el temporizador, este ISI desbloquea el stake pendiente y lo devuelve a `staker`.
El ejecutor valida el request id, asegura que el timestamp de desbloqueo esta en el pasado, emite una
actualizacion de `PublicLaneStakeShare` y registra telemetria.

### 2.5 `SlashPublicLaneValidator`

La gobernanza usa esta instruccion para debitar stake y encarcelar/expulsar validadores.

- `slash_id` vincula el evento con telemetria + docs de incidentes.
- `reason_code` es un string enum estable (p. ej., `double_sign`, `downtime`, `safety_violation`).
- `metadata` almacena hashes de bundles de evidencia, referencias a runbooks o IDs de reguladores.

Los slashes se propagan a delegadores segun la politica de gobernanza (perdida proporcional o
validator-first). La logica de runtime emitira anotaciones `PublicLaneRewardRecord` cuando llegue NX-9.

### 2.6 `RecordPublicLaneRewards`

Registra el pago para una epoca. Campos:

- `reward_asset`: asset distribuido (default `xor#nexus`).
- `total_reward`: total emitido/transferido.
- `shares`: vector de entradas `PublicLaneRewardShare`.
- `metadata`: referencias a transacciones de payout, hashes raiz o dashboards.

Este ISI es idempotente por `(lane_id, epoch)` y sustenta la contabilidad nocturna.

## 3. Operaciones, ciclo de vida y tooling

- **Ciclo de vida + modos:** las lanes stake-elected se habilitan via
  `nexus.staking.public_validator_mode = stake_elected` mientras las lanes restringidas
  permanecen admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`).
  Los despliegues permissioned mantienen peers genesis activos hasta que exista stake; para
  lanes stake-elected aun se requiere un peer registrado con llave de consenso activa presente
  en la topologia de commit antes de que `RegisterPublicLaneValidator` tenga exito. Las
  fingerprints de genesis y `use_stake_snapshot_roster` deciden si el runtime deriva el roster
  desde snapshots de stake o cae a peers genesis.
- **Operaciones de activacion/salida:** las registraciones caen en `PendingActivation` con objetivo
  `current_epoch + 1` (genesis bootstrap uses `current_epoch`) y se auto-promueven en el primer bloque cuya epoca cumple el limite programado
  (`epoch_length_blocks`).
  Los operadores tambien pueden llamar `ActivatePublicLaneValidator` despues del limite para forzar
  la promocion. Las salidas mueven validadores a `Exiting(release_at_ms)` y liberan capacidad solo
  cuando el timestamp del bloque alcanza `release_at_ms`; la re-registracion despues de un slash
  aun requiere salida para que el registro quede `Exited` y la capacidad se recupere. Las verificaciones
  de capacidad usan `nexus.staking.max_validators` y corren despues del finalizador de salida, por lo
  que salidas con fecha futura bloquean nuevas registraciones hasta que expire el temporizador.
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`,
  `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, y los switches de modo de
  validador anteriores. Enlazarlos a traves de `iroha_config::parameters::actual::Nexus` y
  exponerlos en `status.md` una vez que los valores GA queden ratificados.
- **Torii/CLI quickstart:**
  - `iroha app nexus lane-report --summary` muestra entradas del catalogo de lanes, readiness de manifest,
    y modos de validadores (stake-elected vs admin-managed) para que operadores confirmen si la admision
    de staking esta habilitada para una lane.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary]`
    muestra marcadores de ciclo de vida/activacion (epoca objetivo pendiente, `activation_epoch` /
    `activation_height`, release de salida, slash id) junto con stake bonded/self.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator soraカタカナ...] [--summary]`
    refleja el endpoint `/stake` con hints de pending-unbond por par `(validator, staker)`.
  - Snapshots Torii para dashboards y SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` - metadata, estado
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), epoca/altura de activacion,
      temporizadores de release, stake bonded, ultima epoca de recompensa.
      `canonical Katakana i105 literal rendering` controla la renderizacion literal (I105 es preferido; I105 es segunda mejor opcion solo Sora).
    - `GET /v1/nexus/public_lanes/{lane}/stake` - shares de stake (`validator`,
      `staker`, monto bonded) mas temporizadores de pending unbond. `?validator=soraカタカナ...`
      filtra la respuesta para dashboards enfocados en un validador; `canonical Katakana i105 rendering` aplica a
      todos los literales.
  - ISIs de ciclo de vida usan el path de transaccion estandar (Torii
    `/v1/transactions` o el pipeline de instrucciones CLI). Payloads Norito JSON de ejemplo:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetria + runbooks:** metricas exponen conteos de validadores, stake bonded y pendiente,
  totales de recompensa y contadores de slash bajo la familia `nexus_public_lane_*`. Conectar los
  dashboards al mismo set de datos usado por las pruebas de aceptacion NX-9 para que los deltas de
  validadores y la evidencia de recompensa/slash sigan siendo auditables. Las instrucciones de slashing
  siguen siendo solo de gobernanza; el registro de recompensas debe probar totales de payout
  (hash del batch de payout).

## 4. Alineacion con el roadmap

- OK Runtime y storages WSV implementan el ciclo de vida de validadores NX-9; las regresiones
  cubren timing de activacion, prerequisitos de peers, salidas diferidas y re-registracion despues
  de slashes.
- OK Torii expone `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` con Norito JSON
  para que SDKs y dashboards monitoreen el estado de la lane sin RPCs custom.
- OK Config y knobs de telemetria estan documentados; los despliegues mixtos mantienen lanes
  stake-elected y admin-managed aisladas para que los rosters de validadores sigan deterministas.

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
