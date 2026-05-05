---
lang: es
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Runbook de liquidación de repositorios

Esta guía documenta el flujo determinista para acuerdos de repo y repo inverso en Iroha.
Cubre la orquestación de CLI, los asistentes de SDK y los controles de gobierno esperados para que los operadores puedan
iniciar, mantener y cancelar acuerdos sin escribir cargas útiles Norito sin procesar. Para la gobernanza
listas de verificación, captura de evidencia y procedimientos de fraude/reversión ver
[`repo_ops.md`](./repo_ops.md), que satisface el elemento F1 de la hoja de ruta.

## Comandos CLI

El comando `iroha app repo` agrupa ayudas específicas del repositorio:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
  --custodian <i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator <i105-account-id> \
  --counterparty <i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` e `repo unwind` respetan `--input/--output` por lo que el `InstructionBox` generado
  Las cargas útiles se pueden canalizar a otros flujos CLI o enviarse inmediatamente.
* Pase `--custodian <account>` para enrutar la garantía a un custodio tripartito. Cuando se omite, el
  la contraparte recibe la prenda directamente (repo bilateral).
* `repo margin` consulta el libro mayor a través de `FindRepoAgreements` e informa el siguiente margen esperado
  marca de tiempo (en milisegundos) junto con si actualmente se debe realizar una devolución de llamada de margen.
* `repo margin-call` añade una instrucción `RepoMarginCallIsi`, que registra el punto de control del margen y
  Emitiendo eventos para todos los participantes. Las llamadas se rechazan si no ha transcurrido la cadencia o si el
  la instrucción es enviada por un no participante.

## Ayudantes del SDK de Python

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="<i105-account-id>",
    counterparty="<i105-account-id>",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* Ambos ayudantes normalizan cantidades numéricas y campos de metadatos antes de invocar los enlaces de PyO3.
* `RepoAgreementRecord` refleja el cálculo del cronograma de tiempo de ejecución para que la automatización fuera del libro mayor pueda
  determine cuándo vencen las devoluciones de llamada sin volver a calcular la cadencia manualmente.

## Asentamientos DvP / PvP

El comando `iroha app settlement` etapas instrucciones de entrega contra pago y pago contra pago:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from <i105-account-id> \
  --delivery-to <i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from <i105-account-id> \
  --payment-to <i105-account-id> \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from <i105-account-id> \
  --primary-to <i105-account-id> \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from <i105-account-id> \
  --counter-to <i105-account-id> \
  --iso-xml-out trade_pvp.xml
```

* Las cantidades de las piernas aceptan valores integrales o decimales y se validan con la precisión del activo.
* `--atomicity` acepta `all-or-nothing`, `commit-first-leg` o `commit-second-leg`. Utilice estos modos
  con `--order` para expresar qué tramo permanece comprometido si falla el procesamiento posterior (`commit-first-leg`
  mantiene aplicado el partido de ida; `commit-second-leg` conserva el segundo).
* Las invocaciones CLI emiten metadatos de instrucciones vacíos hoy; use los ayudantes de Python cuando esté a nivel de asentamiento
  Es necesario adjuntar metadatos.
* Consulte [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) para conocer la asignación de campos ISO 20022 que
  respalda estas instrucciones (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Pase `--iso-xml-out <path>` para que la CLI emita una vista previa XML canónica junto con Norito.
  instrucción; el archivo sigue el mapeo anterior (`sese.023` para DvP, `sese.025` para PvP`). Pair the
  indicador con `--iso-reference-crosswalk <path>` para que la CLI verifique `--delivery-instrument-id` con el
  La misma instantánea que utiliza Torii durante la admisión en tiempo de ejecución.

Los ayudantes de Python reflejan la superficie CLI:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<i105-account-id>"))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="<i105-account-id>",
    to_account="<i105-account-id>",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="<i105-account-id>",
        to_account="<i105-account-id>",
    ),
)
```

## Determinismo y expectativas de gobernanza

Las instrucciones de repositorio se basan exclusivamente en tipos numéricos codificados con Norito y en el código compartido.
Lógica `RepoGovernance::with_defaults`. Tenga en cuenta las siguientes invariantes:* Las cantidades están serializadas con valores deterministas `NumericSpec`: uso de tramos de efectivo
  `fractional(2)` (dos decimales), los tramos colaterales utilizan `integer()`. no enviar
  valores con mayor precisión: los guardias en tiempo de ejecución los rechazarán y los pares divergirán.
* Los repositorios tripartitos conservan la identificación de la cuenta del custodio en `RepoAgreement`. Eventos de ciclo de vida y margen
  emita una carga útil `RepoAccountRole::Custodian` para que los custodios puedan suscribirse y conciliar el inventario.
* Los recortes están fijados a 10000 bps (100 %) y las frecuencias de margen son segundos completos. proporcionar
  parámetros de gobernanza en esas unidades canónicas para mantenerse alineados con las expectativas de tiempo de ejecución.
* Las marcas de tiempo son siempre milisegundos Unix. Todos los ayudantes los reenvían sin cambios al Norito
  carga útil para que los pares obtengan horarios idénticos.
* Las instrucciones de inicio y cancelación reutilizan el mismo identificador de acuerdo. El tiempo de ejecución rechaza
  identificaciones duplicadas y resoluciones por acuerdos desconocidos; Los ayudantes de CLI/SDK detectan esos errores temprano.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` devuelve la cadencia canónica. siempre
  Consulte esta instantánea antes de activar devoluciones de llamadas para evitar reproducir programaciones obsoletas.