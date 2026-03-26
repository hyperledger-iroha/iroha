---
lang: es
direction: ltr
source: docs/portal/docs/sdks/javascript.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Inicio rápido del SDK de JavaScript
Descripción: Cree transacciones, transmita eventos e impulse vistas previas de Connect con `@iroha/iroha-js`.
babosa: /sdks/javascript
---

`@iroha/iroha-js` es el paquete canónico de Node.js para interactuar con Torii. eso
incluye constructores Norito, ayudantes Ed25519, utilidades de paginación y un resistente
Cliente HTTP/WebSocket para que pueda reflejar los flujos CLI desde TypeScript.

## Instalación

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

El paso de compilación incluye `cargo build -p iroha_js_host`. Asegúrese de que la cadena de herramientas
`rust-toolchain.toml` está disponible localmente antes de ejecutar `npm run build:native`.

## Gestión de claves

```ts
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(verifyEd25519(message, signature, publicKey));

const derived = publicKeyFromPrivate(privateKey);
console.assert(Buffer.compare(derived, publicKey) === 0);
```

## Crear transacciones

Los creadores de instrucciones Norito normalizan identificadores, metadatos y cantidades para que
Las transacciones codificadas coinciden con las cargas útiles de Rust/CLI.

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "norito:4e52543000000001",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "soraカタカナ...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "soraカタカナ...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "soraカタカナ...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Configuración del cliente Torii

`ToriiClient` acepta perillas de reintento/tiempo de espera que reflejan `iroha_config`. uso
`resolveToriiClientConfig` para fusionar un objeto de configuración camelCase (normalizar
`iroha_config` primero), anulaciones de entorno y opciones en línea.

```ts
import { ToriiClient, resolveToriiClientConfig } from "@iroha/iroha-js";
import fs from "node:fs";

const rawConfig = JSON.parse(fs.readFileSync("./iroha_config.json", "utf8"));
const config = rawConfig?.torii
  ? {
      ...rawConfig,
      torii: {
        ...rawConfig.torii,
        apiTokens: rawConfig.torii.api_tokens ?? rawConfig.torii.apiTokens,
      },
    }
  : rawConfig;
const clientConfig = resolveToriiClientConfig({
  config,
  overrides: { timeoutMs: 2_000, maxRetries: 5 },
});

const torii = new ToriiClient(
  config?.torii?.address ?? "http://localhost:8080",
  {
    config,
    timeoutMs: clientConfig.timeoutMs,
    maxRetries: clientConfig.maxRetries,
  },
);
```

Variables de entorno para desarrollo local:| Variables | Propósito |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Solicitar tiempo de espera (milisegundos). |
| `IROHA_TORII_MAX_RETRIES` | Número máximo de intentos de reintento. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Retraso del reintento inicial. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Multiplicador de retroceso exponencial. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Retraso máximo de reintento. |
| `IROHA_TORII_RETRY_STATUSES` | Códigos de estado HTTP separados por comas para volver a intentarlo. |
| `IROHA_TORII_RETRY_METHODS` | Métodos HTTP separados por comas para reintentar. |
| `IROHA_TORII_API_TOKEN` | Agrega `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | Agrega el encabezado `Authorization: Bearer …`. |

Los perfiles de reintento reflejan los valores predeterminados de Android y se exportan para realizar comprobaciones de paridad:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. Ver `docs/source/sdk/js/torii_retry_policy.md`
para el mapeo de punto final a perfil y las auditorías de gobernanza de parámetros durante
JS4/JS7.

## Listas iterables y paginación

Los ayudantes de paginación reflejan la ergonomía del SDK de Python para `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFT, saldos, titulares de activos y el
historial de transacciones de la cuenta.

```ts
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({
  pageSize: 50,
  maxItems: 200,
})) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const assetId = "norito:4e52543000000001";
const balances = await torii.listAccountAssets("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Permisos sin conexión y metadatos de veredictoLas respuestas a las asignaciones sin conexión exponen los metadatos del libro mayor enriquecidos por adelantado:
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` e `remaining_amount` se devuelven junto con el archivo sin formato.
registre para que los paneles no tengan que decodificar las cargas útiles Norito integradas. el nuevo
ayudantes de cuenta atrás (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) resalta la próxima fecha límite de vencimiento (actualizar → política
→ certificado) para que las insignias de UI puedan advertir a los operadores cada vez que se haya asignado un permiso
Quedan <24 h. El SDK
refleja los filtros REST expuestos por `/v1/offline/reserve/topup`:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` y el
`requireVerdict` / `onlyMissingVerdict` booleanos. Combinaciones no válidas (para
ejemplo `onlyMissingVerdict` + `verdictIdHex`) se rechazan localmente antes de Torii
se llama.

```ts
const { items: allowances } = await torii.listOfflineAllowances({
  limit: 25,
  policyExpiresBeforeMs: Date.now() + 86_400_000,
  requireVerdict: true,
});

for (const entry of allowances) {
  console.log(
    entry.controller_display,
    entry.remaining_amount,
    entry.verdict_id_hex,
    entry.refresh_at_ms,
  );
}
```

## Recargas sin conexión (emitir + registrarse)

Utilice los ayudantes de recarga cuando desee emitir un certificado e inmediatamente
registrarlo en el libro mayor. El SDK verifica el certificado emitido y registrado.
Los ID coinciden antes de regresar y la respuesta incluye ambas cargas útiles. hay
sin punto final de recarga dedicado; el ayudante encadena la emisión + registra llamadas. si
ya tiene un certificado firmado, llame a `registerOfflineAllowance` (o
`renewOfflineAllowance`) directamente.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_i105>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_i105>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii consultas y streaming (WebSockets)Los asistentes de consulta exponen el estado, las métricas Prometheus, las instantáneas de telemetría y los eventos
transmisiones utilizando la gramática de filtro Norito. La transmisión se actualiza automáticamente a
WebSockets y se reanuda cuando el presupuesto de reintento lo permite.

```ts
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

const abort = new AbortController();
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
  signal: abort.signal,
})) {
  console.log(event.id, event.data);
  break;
}
abort.abort(); // closes the underlying WebSocket cleanly
```

Utilice `streamBlocks`, `streamTransactions` o `streamTelemetry` para el otro
Puntos finales de WebSocket. Todos los asistentes de transmisión muestran intentos de reintento, así que conecte el
Devolución de llamada `onReconnect` para alimentar paneles y alertas.

## Instantáneas de Explorer y cargas útiles QR

La telemetría de Explorer proporciona ayudas escritas para `/v1/explorer/metrics` y
puntos finales `/v1/explorer/accounts/{account_id}/qr` para que los paneles puedan reproducir el
Las mismas instantáneas que alimentan el portal. `getExplorerMetrics()` normaliza el
carga útil y devuelve `null` cuando la ruta está deshabilitada. Combínalo con
`getExplorerAccountQr()` siempre que necesite literales i105 (preferido)/sora (segundo mejor) más en línea
SVG para botones de compartir.

```ts
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("last block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("soraカタカナ...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

Pasar `i105` refleja el comprimido predeterminado de Explorer
selectores; omita la anulación para la salida i105 preferida o solicite `i105_qr`
cuando necesite la variante segura para QR. El literal comprimido es el segundo mejor.
Opción exclusiva de Sora para UX. El ayudante siempre devuelve el identificador canónico,
el literal seleccionado y los metadatos (prefijo de red, versión/módulos QR, error
nivel de corrección y SVG en línea), por lo que CI/CD puede publicar las mismas cargas útiles que
El Explorer emerge sin necesidad de llamar a convertidores hechos a medida.## Conectar sesiones y hacer cola

Los ayudantes de Connect reflejan `docs/source/connect_architecture_strawman.md`. el
la ruta más rápida para una sesión lista para vista previa es `bootstrapConnectPreviewSession`,
que une la generación determinista SID/URI y el Torii
convocatoria de inscripción.

```ts
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(
  torii,
  {
    chainId: "sora-mainnet",
    node: "https://torii.nexus.example",
    sessionOptions: { node: "https://torii.backup.example" },
  },
);

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- Pase `register: false` cuando solo necesite URI deterministas para QR/enlace profundo
  vistas previas.
- `generateConnectSid` permanece disponible cuando necesita derivar identificadores de sesión
  sin acuñar URI.
- Las claves direccionales y los sobres de texto cifrado provienen del puente nativo; cuando
  no disponible, el SDK recurre al códec JSON y arroja
  `ConnectQueueError.bridgeUnavailable`.
- Los buffers sin conexión se almacenan como blobs Norito `.to` en IndexedDB. Cola de monitor
  estado a través del `ConnectQueueError.overflow(limit)` emitido /
  Errores `.expired(ttlMs)` y telemetría de alimentación `connect.queue_depth` como se describe
  en la hoja de ruta.

### Conectar instantáneas de registro y políticas

Los operadores de plataformas pueden realizar una introspección y actualizar el registro de Connect sin
dejando Node.js. `iterateConnectApps()` páginas a través del registro, mientras
`getConnectStatus()` y `getConnectAppPolicy()` exponen los contadores de tiempo de ejecución y
dotación política actual. `updateConnectAppPolicy()` acepta campos camelCase,
para que pueda preparar la misma carga útil JSON que espera Torii.

```ts
const status = await torii.getConnectStatus();
console.log("connect enabled:", status?.enabled ?? false);
console.log("active sessions:", status?.sessionsActive ?? 0);
console.log("buffered bytes:", status?.totalBufferBytes ?? 0);

for await (const app of torii.iterateConnectApps({ limit: 100 })) {
  console.log(app.appId, app.namespaces, app.policy?.relayEnabled ? "relay" : "wallet-only");
}

const policy = await torii.getConnectAppPolicy();
if ((policy.wsPerIpMaxSessions ?? 0) < 5) {
  await torii.updateConnectAppPolicy({
    wsPerIpMaxSessions: 5,
    pingIntervalMs: policy.pingIntervalMs ?? 30_000,
    pingMissTolerance: policy.pingMissTolerance ?? 3,
  });
}
```

Capture siempre la última instantánea `getConnectStatus()` antes de aplicar
mutaciones: la lista de verificación de gobernanza requiere evidencia de que las actualizaciones de políticas comienzan
de los límites actuales de la flota.### Conectar marcación WebSocket

`ToriiClient.openConnectWebSocket()` ensambla el canónico
URL `/v1/connect/ws` (incluidos `sid`, `role` y parámetros de token), actualizaciones
`http→ws` / `https→wss` y entrega la URL final a cualquier WebSocket
implementación que usted proporciona. Los navegadores reutilizan automáticamente el global
`WebSocket`. Las personas que llaman a Node.js deben pasar un constructor como `ws`:

```ts
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL ?? "https://torii.nexus.example");
const preview = await torii.createConnectSessionPreview({ chainId: "sora-mainnet" });
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});

socket.addEventListener("message", (event) => {
  console.log("Connect payload", event.data);
});
socket.addEventListener("close", () => {
  console.log("Connect socket closed");
});

socket.binaryType = "arraybuffer";
socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind);
    return;
  }
  pendingFrames.enqueue(new Uint8Array(event.data));
});
```

Cuando solo necesite la URL, llame a `torii.buildConnectWebSocketUrl(params)` o al
ayudante de nivel superior `buildConnectWebSocketUrl(baseUrl, params)` y reutilizar el
cadena resultante en un transporte/cola personalizado.

¿Busca una muestra completa orientada a CLI? el
[Receta de vista previa de conexión](./recipes/javascript-connect-preview.md) incluye una
script ejecutable más guía de telemetría que refleja la hoja de ruta entregable para
documentar el flujo de cola de conexión + WebSocket.

### Telemetría y alertas en cola

Conecte las métricas de la cola directamente a las superficies auxiliares para que los paneles puedan reflejarse
los KPI de la hoja de ruta.

```ts
import { bootstrapConnectPreviewSession, ConnectQueueError } from "@iroha/iroha-js";

async function dialWithTelemetry(client: ToriiClient) {
  try {
    const { session } = await bootstrapConnectPreviewSession(client, { chainId: "sora-mainnet" });
    queueDepthGauge.record(session.queue_depth ?? 0);
    // …open the WebSocket here…
  } catch (error) {
    if (error instanceof ConnectQueueError) {
      if (error.kind === ConnectQueueError.KIND.OVERFLOW) {
        queueOverflowCounter.add(1, { limit: error.limit ?? 0 });
      } else if (error.kind === ConnectQueueError.KIND.EXPIRED) {
        queueExpiryCounter.add(1, { ttlMs: error.ttlMs ?? 0 });
      }
      return;
    }
    throw error;
  }
}
```

`ConnectQueueError#toConnectError()` convierte las fallas de la cola en genéricas
Taxonomía `ConnectError` para que los interceptores HTTP/WebSocket compartidos puedan emitir la
estándar `connect.queue_depth`, `connect.queue_overflow_total` y
Métricas `connect.queue_expired_total` a las que se hace referencia en toda la hoja de ruta.

## Observadores de streaming y cursores de eventos`ToriiClient.streamEvents()` expone `/v1/events/sse` como un iterador asíncrono con función automática
reintentos, por lo que las CLI de Nodo/Bun pueden seguir la actividad de la canalización de la misma manera que lo hace la CLI de Rust.
Mantenga el cursor `Last-Event-ID` junto a los artefactos de su runbook para que los operadores puedan
reanudar una secuencia sin omitir eventos cuando se reinicia un proceso.

```ts
import fs from "node:fs/promises";
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const cursorFile = process.env.STREAM_CURSOR_FILE ?? ".cache/torii.cursor";
const resumeId = await fs
  .readFile(cursorFile, "utf8")
  .then((value) => value.trim())
  .catch(() => null);
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  lastEventId: resumeId || undefined,
  signal: controller.signal,
})) {
  if (event.id) {
    await fs.writeFile(cursorFile, `${event.id}\n`, "utf8");
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] id=${event.id ?? "∅"} status=${status ?? "n/a"}`);
}
```

- Cambie `PIPELINE_STATUS` (por ejemplo, `Pending`, `Applied` o `Approved`) o configure
  `STREAM_FILTER_JSON` para reproducir los mismos filtros que acepta la CLI.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` mantiene vivo el iterador hasta que
  se recibe la señal; pase `STREAM_MAX_EVENTS=25` cuando solo necesite los primeros eventos
  para una prueba de humo.
- `ToriiClient.streamSumeragiStatus()` refleja la misma interfaz para
  `/v1/sumeragi/status/sse` para que la telemetría de consenso se pueda seguir por separado, y el
  El iterador respeta `Last-Event-ID` de la misma manera.
- Consulte `javascript/iroha_js/recipes/streaming.mjs` para obtener una CLI llave en mano (persistencia del cursor,
  anulaciones de filtro env-var y registro `extractPipelineStatusKind`) utilizados en JS4
  entregable de la hoja de ruta de streaming/WebSocket.

## Portafolios UAID y Directorio Espacial

Las API de Space Directory muestran el ciclo de vida del ID de cuenta universal (UAID). el
los ayudantes aceptan literales `uaid:<hex>` o resúmenes sin formato de 64 hexadecimales (LSB=1) y
canonicalizarlos antes de enviar solicitudes:- `getUaidPortfolio(uaid, { assetId })` agrega saldos por espacio de datos,
  agrupar tenencias de activos por ID de cuenta canónica; pase `assetId` para filtrar el
  cartera hasta una sola instancia de activo.
- `getUaidBindings(uaid)` enumera cada espacio de datos ↔ cuenta
  enlace (`i105` devuelve los literales `i105`).
- `getUaidManifests(uaid, { dataspaceId })` devuelve cada manifiesto de capacidad,
  estado del ciclo de vida y cuentas vinculadas para auditoría.

Para paquetes de evidencia de operador, flujos de publicación/revocación de manifiesto y migración de SDK
orientación, siga la Guía de cuenta universal (`docs/source/universal_accounts_guide.md`)
junto con estos ayudantes del cliente para que el portal y la documentación fuente permanezcan sincronizados.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "norito:4e52543000000002",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, {} );
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

Los operadores también pueden rotar manifiestos o ejecutar flujos de emergencia de denegación de beneficios sin
bajando a la CLI. Ambos ayudantes aceptan un objeto `{ signal }` opcional, por lo que
los envíos de larga duración se pueden cancelar con `AbortController`; no objeto
Las opciones o entradas que no son `AbortSignal` generan un `TypeError` síncrono antes de que
la solicitud llega a Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "soraカタカナ...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "soraカタカナ...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
````publishSpaceDirectoryManifest()` acepta JSON de manifiesto sin formato (que coincide con el
accesorios bajo `fixtures/space_directory/`) o cualquier objeto que serialice al
misma estructura. `privateKey`, `privateKeyHex` o `privateKeyMultihash` se asignan a
el campo `ExposedPrivateKey` Torii espera y el valor predeterminado es `ed25519`
algoritmo cuando no se proporciona ningún prefijo. Ambas solicitudes regresan una vez que Torii se pone en cola
la instrucción (`202 Accepted`), momento en el cual el libro mayor emitirá el
coincidente `SpaceDirectoryEvent`.

## Puente de gobernanza e ISO

`ToriiClient` expone las API de gobernanza para inspeccionar contratos y preparar
propuestas, presentar papeletas (simples o ZK), rotar el consejo y convocar
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` sin DTO escritos a mano. Ayudantes de ISO 20022
siga el mismo patrón a través de `buildPacs008Message`/`buildPacs009Message` y el
Trío `submitIso*`/`waitForIsoMessageStatus`.

Consulte la [receta puente de gobernanza e ISO](./recipes/javascript-governance-iso.md)
para muestras listas para CLI y enlaces a la guía de campo completa en
`docs/source/sdk/js/governance_iso_examples.md`.

## Pruebas de muestreo y entrega de eritrocitos

La hoja de ruta de JS también requiere el muestreo del Compromiso de Bloqueo de Roadrunner (RBC) para que los operadores puedan
demuestre que el bloque que obtuvieron a través de Sumeragi coincide con las pruebas de fragmentos que verifican.
Utilice los asistentes integrados en lugar de crear cargas útiles a mano:1. `getSumeragiRbcSessions()` refleja `/v1/sumeragi/rbc/sessions`, y
   `findRbcSamplingCandidate()` selecciona automáticamente la primera sesión entregada con un hash de bloque
   (la suite de integración recurre a él cada vez que
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` no está configurado).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` normaliza `{blockHash,height,view}`
   además de anulaciones opcionales `{count,seed,apiToken}` para que los números hexadecimales o negativos con formato incorrecto nunca
   llegar a Torii.
3. `sampleRbcChunks()` ENVÍA la solicitud a `/v1/sumeragi/rbc/sample`, devolviendo pruebas de fragmentos
   y rutas de Merkle (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) con las que debe archivar
   el resto de su evidencia de adopción.
4. `getSumeragiRbcDelivered(height, view)` captura los metadatos de entrega de la cohorte para que los auditores
   Puede reproducir la prueba de un extremo a otro.

```js
import assert from "node:assert";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080", {
  apiToken: process.env.TORII_API_TOKEN,
});

const candidate =
  (await torii.findRbcSamplingCandidate().catch(() => null)) ??
  (await torii.getSumeragiRbcSessions()).items.find((session) => session.delivered);
if (!candidate) {
  throw new Error("no delivered RBC session available; set IROHA_TORII_INTEGRATION_RBC_SAMPLE");
}

const request = ToriiClient.buildRbcSampleRequest(candidate, {
  count: Number(process.env.RBC_SAMPLE_COUNT ?? 2),
  seed: Number(process.env.RBC_SAMPLE_SEED ?? 0),
  apiToken: process.env.RBC_SAMPLE_API_TOKEN ?? process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks(request);
sample.samples.forEach((chunk) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, "chunk must be hex");
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```

Persista ambas respuestas bajo la raíz del artefacto que somete a la gobernanza. Anular el
sesión seleccionada automáticamente a través de `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
siempre que necesite sondear un bloque específico y tratar las fallas para recuperar instantáneas de RBC como un
error de control previo al vuelo en lugar de bajar silenciosamente al modo directo.

## Pruebas y CI

1. Carga en caché y artefactos npm.
2. Ejecute `npm run build:native`.
3. Ejecute `npm test` (o `node --test` para trabajos de humo).

El flujo de trabajo de referencia de GitHub Actions se encuentra en
`docs/source/examples/iroha_js_ci.md`.

## Próximos pasos

- Revisar los tipos generados en `javascript/iroha_js/index.d.ts`.
- Explore las recetas en `javascript/iroha_js/recipes/`.
- Empareje `ToriiClient` con el inicio rápido Norito para inspeccionar cargas útiles junto
  Llamadas SDK.