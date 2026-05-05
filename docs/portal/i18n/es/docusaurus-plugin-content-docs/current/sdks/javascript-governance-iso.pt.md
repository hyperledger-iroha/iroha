---
lang: es
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ejemplos de puentes ISO y de gobernanza
Descripción: Impulse flujos de trabajo avanzados Torii con `@iroha/iroha-js`.
slug: /sdks/javascript/governance-iso-examples
---

Esta guía de campo amplía el inicio rápido al demostrar la gobernanza y
Flujos puente ISO 20022 con `@iroha/iroha-js`. Los fragmentos se reutilizan igual.
ayudantes de tiempo de ejecución que se envían con `ToriiClient`, para que pueda copiarlos directamente en
Herramientas CLI, arneses de CI o servicios de larga duración.

Recursos adicionales:

- `javascript/iroha_js/recipes/governance.mjs`: script ejecutable de un extremo a otro para
  propuestas, votaciones y rotaciones del consejo.
- `javascript/iroha_js/recipes/iso_bridge.mjs`: ayuda CLI para envío
  Cargas útiles pacs.008/pacs.009 y estado determinista de sondeo.
- `docs/source/finance/settlement_iso_mapping.md`: mapeo de campos ISO canónico.

## Ejecutando las recetas incluidas

Estos ejemplos dependen de los scripts en `javascript/iroha_js/recipes/`. correr
`npm install && npm run build:native` de antemano para que los enlaces generados sean
disponible.

### Tutorial del asistente de gobernanza

Configure las siguientes variables de entorno antes de invocar
`recipes/governance.mjs`:- `TORII_URL` — Punto final Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX`: cuenta y clave del firmante (hexadecimal). Guarde las llaves en un
  almacén secreto seguro.
- `CHAIN_ID`: identificador de red opcional.
- `GOV_SUBMIT=1`: envía las transacciones generadas a Torii.
- `GOV_FETCH=1`: recupera propuestas/bloqueos después del envío.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID`: se utilizan búsquedas opcionales
  cuando `GOV_FETCH=1`.

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=<i105-account-id> \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

Los hashes se registran para cada paso y las respuestas Torii aparecen cuando
`GOV_SUBMIT=1` para que los trabajos de CI puedan fallar rápidamente debido a errores de envío.

### Ayudante de puente ISO

`recipes/iso_bridge.mjs` envía un mensaje pacs.008 o pacs.009 y realiza una encuesta
el puente ISO hasta que el estado se estabilice. Configurarlo con:- `TORII_URL`: punto final Torii que expone las API del puente ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (predeterminado) o `pacs.009`. El ayudante utiliza el
  generador de muestra coincidente (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  cuando no proporciona su propio XML.
- `ISO_MESSAGE_SUFFIX`: sufijo opcional añadido a los ID de carga útil de muestra para
  mantenga únicos los ensayos repetidos (el valor predeterminado es los segundos de la época actual en hexadecimal).
- `ISO_CONTENT_TYPE`: anula el encabezado `Content-Type` para envíos
  (por ejemplo `application/pacs009+xml`); ignorado cuando solo sondeas un
  ID de mensaje existente.
- `ISO_MESSAGE_ID`: omita el envío por completo y solo sondee lo proporcionado
  identificador a través de `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — ajusta la estrategia de espera para
  Implementaciones de puentes ruidosas o lentas.
- `ISO_RESOLVE_ON_ACCEPTED=1`: salga tan pronto como Torii devuelva `Accepted`,
  incluso si el hash de la transacción aún está pendiente (útil durante el mantenimiento del puente)
  cuando se retrasa la confirmación del libro mayor).

```bash
# Submit a pacs.009 message and wait for completion.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_KIND=pacs.009 \
ISO_POLL_ATTEMPTS=20 \
ISO_POLL_INTERVAL_MS=1500 \
node javascript/iroha_js/recipes/iso_bridge.mjs

# Poll an existing message id without re-submitting XML.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_ID=iso-demo-1 \
node javascript/iroha_js/recipes/iso_bridge.mjs
```

Ambos scripts salen con el código de estado `1` si Torii nunca informa una terminal
transición, haciéndolos adecuados para trabajos de puerta de CI.

### Ayudante de alias ISO`recipes/iso_alias.mjs` apunta a los puntos finales de alias ISO para que los ensayos puedan cubrir
Búsquedas de alias y hash de elementos ciegos sin necesidad de escribir herramientas personalizadas. eso
llamadas `ToriiClient.evaluateAliasVoprf` más `resolveAlias` / `resolveAliasByIndex`
e imprime el backend, el resumen, el enlace de cuenta, la fuente y el índice determinista.
devuelto por Torii.

Variables de entorno:

- `TORII_URL`: punto final Torii que expone los ayudantes de alias.
- `ISO_VOPRF_INPUT`: elemento ciego con codificación hexadecimal (el valor predeterminado es `deadbeef`).
- `ISO_SKIP_VOPRF=1`: omita la llamada VOPRF cuando solo pruebe las búsquedas.
- `ISO_ALIAS_LABEL`: alias literal a resolver (por ejemplo, cadenas de estilo IBAN).
- `ISO_ALIAS_INDEX`: índice decimal o con prefijo `0x` pasado a `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN`: encabezados opcionales para implementaciones seguras de Torii.

```bash
# Evaluate a blinded element and resolve an alias literal + deterministic index.
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeefcafebabe \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node javascript/iroha_js/recipes/iso_alias.mjs

# Only perform literal resolution.
TORII_URL=https://torii.testnet.sora \
ISO_SKIP_VOPRF=1 \
ISO_ALIAS_LABEL="iso:demo:alpha" \
node javascript/iroha_js/recipes/iso_alias.mjs
```

El asistente refleja el comportamiento de Torii: muestra 404 cuando faltan alias
y trata los errores deshabilitados en tiempo de ejecución como omisiones suaves para que los flujos de CI puedan tolerar el puente.
ventanas de mantenimiento.

## Flujos de trabajo de gobernanza

### Inspeccionar instancias de contrato y propuestas

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const instances = await torii.listGovernanceInstances("apps", {
  contains: "ledger",
  hashPrefix: "deadbeef",
  order: "hash_desc",
  limit: 5,
});
for (const entry of instances.instances) {
  console.log(`${entry.contract_address} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### Enviar propuestas y boletas

Utilice un `AbortController` cuando necesite cancelar o enviar envíos de gobernanza con plazos determinados: el SDK
acepta un objeto `{ signal }` opcional para cada asistente POST que se muestra a continuación.

```ts
const authority = "<i105-account-id>";
const privateKey = Buffer.alloc(32, 0xaa);

// All governance writes accept optional `{ signal }` options for cancellation.
const writeController = new AbortController();
const deployDraft = await torii.governanceProposeDeployContract({
  namespace: "apps",
  contractId: "calc.v1",
  codeHash: "hash:7B38...#ABCD",
  abiHash: Buffer.alloc(32, 0xbb),
  abiVersion: "1",
  window: { lower: 12_345, upper: 12_500 },
  mode: "Plain",
}, { signal: writeController.signal });
console.log("draft instructions", deployDraft.tx_instructions.length);

const ballot = await torii.governanceSubmitPlainBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  referendumId: "ref-plain",
  owner: authority,
  amount: "5000",
  durationBlocks: 7_200,
  direction: "Aye",
}, { signal: writeController.signal });
if (!ballot.accepted) {
  console.warn("ballot rejected", ballot.reason);
}

const zkOwner = "<i105-account-id>"; // canonical I105 account id for ZK public inputs
await torii.governanceSubmitZkBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  electionId: "ref-zk",
  proof: Buffer.alloc(96, 0xcd),
  public: {
    owner: zkOwner,
    amount: "5000",
    duration_blocks: 7_200,
    direction: "Aye",
  },
}, { signal: writeController.signal });
```

### Consejo VRF y promulgación

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "<i105-account-id>",
      variant: "Normal",
      pk: validatorPk,
      proof: validatorProof,
    },
  ],
}, { signal: writeController.signal });
await torii.governancePersistCouncil({
  committeeSize: derived.members.length,
  candidates: derived.members.map((member) => ({
    accountId: member.account_id,
    variant: "Normal",
    pk: validatorPk,
    proof: validatorProof,
  })),
  authority,
  privateKey,
}, { signal: writeController.signal });

const finalizeDraft = await torii.governanceFinalizeReferendumTyped({
  referendumId: "ref-mainnet-001",
  proposalId: "0123abcd...beef",
}, { signal: writeController.signal });
console.log("finalize tx count", finalizeDraft.tx_instructions.length);

const enactDraft = await torii.governanceEnactProposalTyped({
  proposalId: "abcd0123...cafe",
  window: { lower: 10, upper: 25 },
}, { signal: writeController.signal });
console.log("enact tx count", enactDraft.tx_instructions.length);
```

## Recetas puente ISO 20022### Construir cargas útiles pacs.008 / pacs.009

```ts
import { buildPacs008Message } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "<i105-account-id>" },
  purposeCode: "SECU",
  supplementaryData: { account_id: "<i105-account-id>", leg: "delivery" },
});
```

Todos los identificadores (BIC, LEI, IBAN, importe ISO) se validan antes de que XML sea
generado. Intercambia `buildPacs008Message` por `buildPacs009Message` para emitir PvP
cargas útiles de financiación.

### Enviar y sondear mensajes ISO

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const status = await torii.submitIsoPacs008AndWait(settlement, {
  wait: {
    maxAttempts: Number(process.env.ISO_POLL_ATTEMPTS ?? 20),
    pollIntervalMs: Number(process.env.ISO_POLL_INTERVAL_MS ?? 3_000),
    resolveOnAccepted: process.env.ISO_RESOLVE_ON_ACCEPTED === "1",
    onPoll: ({ attempt, status: snapshot }) => {
      console.log(`[attempt ${attempt}] status=${snapshot?.status ?? "pending"}`);
    },
  },
});
console.log(status.message_id, status.status, status.transaction_hash);

await torii.waitForIsoMessageStatus(process.env.ISO_MESSAGE_ID!, {
  maxAttempts: 10,
  pollIntervalMs: 2_000,
});

// Build XML on the fly from structured fields (skips the sample payloads).
await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
  },
  {
    kind: "pacs.009",
    wait: { maxAttempts: 5, pollIntervalMs: 1_500 },
  },
);
```

Tanto `resolveOnAccepted` como `resolveOnAcceptedWithoutTransaction` son válidos; usar cualquiera de las banderas
tratar los estados `Accepted` (sin un hash de transacción) como terminal al organizar encuestas.

Los ayudantes lanzan `IsoMessageTimeoutError` si el puente nunca informa un
estado terminal. Utilice el nivel inferior `submitIsoPacs008` / `submitIsoPacs009`
llamadas cuando necesita orquestar una lógica de sondeo personalizada; `getIsoMessageStatus`
expone una búsqueda de un solo disparo.

### Superficies relacionadas

- `torii.getSorafsPorWeeklyReport("2026-W05")` recupera el paquete PoR de la semana ISO
  referenciado en la hoja de ruta y puede reutilizar los ayudantes de espera para alertas.
- `resolveAlias` / `resolveAliasByIndex` exponen enlaces de alias de puente ISO para
  Las herramientas de conciliación pueden demostrar la propiedad de la cuenta antes de emitir un pago.