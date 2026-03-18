---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/javascript.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: início rápido do SDK JavaScript
description: Crie transações, transmita eventos e gere visualizações do Connect com `@iroha/iroha-js`.
slug: /sdks/javascript
---

`@iroha/iroha-js` é o pacote Node.js canônico para interagir com Torii. Isso
agrupa construtores Norito, auxiliares Ed25519, utilitários de paginação e um resiliente
Cliente HTTP/WebSocket para que você possa espelhar os fluxos CLI do TypeScript.

## Instalação

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

A etapa de construção envolve `cargo build -p iroha_js_host`. Certifique-se de que o conjunto de ferramentas
`rust-toolchain.toml` está disponível localmente antes de executar `npm run build:native`.

## Gerenciamento de chaves

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

## Construir transações

Os construtores de instruções Norito normalizam identificadores, metadados e quantidades para
as transações codificadas correspondem às cargas úteis do Rust/CLI.

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
  destinationAccountId: "i105...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "i105...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "i105...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Configuração do cliente Torii

`ToriiClient` aceita botões de nova tentativa/tempo limite que espelham `iroha_config`. Usar
`resolveToriiClientConfig` para mesclar um objeto de configuração camelCase (normalizar
`iroha_config` primeiro), substituições de ambiente e opções embutidas.

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

Variáveis de ambiente para desenvolvimento local:

| Variável | Finalidade |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Tempo limite da solicitação (milissegundos). |
| `IROHA_TORII_MAX_RETRIES` | Máximo de tentativas de repetição. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Espera inicial de nova tentativa. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Multiplicador de backoff exponencial. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Atraso máximo de nova tentativa. |
| `IROHA_TORII_RETRY_STATUSES` | Códigos de status HTTP separados por vírgula para tentar novamente. |
| `IROHA_TORII_RETRY_METHODS` | Métodos HTTP separados por vírgula para tentar novamente. |
| `IROHA_TORII_API_TOKEN` | Adiciona `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | Adiciona cabeçalho `Authorization: Bearer …`. |

Os perfis de nova tentativa refletem os padrões do Android e são exportados para verificações de paridade:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. Consulte `docs/source/sdk/js/torii_retry_policy.md`
para o mapeamento de endpoint para perfil e auditorias de governança de parâmetros durante
JS4/JS7.

## Listas iteráveis e paginação

Os auxiliares de paginação espelham a ergonomia do Python SDK para `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFTs, saldos, detentores de ativos e o
histórico de transações da conta.

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
const balances = await torii.listAccountAssets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Subsídios off-line e metadados de veredicto

As respostas de subsídios off-line expõem antecipadamente os metadados do razão enriquecido -
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` e `remaining_amount` são retornados junto com o valor bruto
registre para que os painéis não precisem decodificar as cargas úteis Norito incorporadas. O novo
auxiliares de contagem regressiva (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) destaque o próximo prazo de expiração (atualizar → política
→ certificado) para que os crachás da UI possam avisar os operadores sempre que uma permissão for
<24h restantes. O SDK
espelha os filtros REST expostos por `/v1/offline/allowances`:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` e o
`requireVerdict` / `onlyMissingVerdict` booleanos. Combinações inválidas (para
exemplo `onlyMissingVerdict` + `verdictIdHex`) são rejeitados localmente antes de Torii
é chamado.

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

## Recargas offline (emissão + registro)Use os auxiliares de recarga quando quiser emitir um certificado e imediatamente
registre-o no livro-razão. O SDK verifica o certificado emitido e registrado
Os IDs correspondem antes de retornar e a resposta inclui ambas as cargas. Há
nenhum terminal de recarga dedicado; o auxiliar encadeia as chamadas de problema + registro. Se
você já possui um certificado assinado, ligue para `registerOfflineAllowance` (ou
`renewOfflineAllowance`) diretamente.

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

## Consultas e streaming Torii (WebSockets)

Auxiliares de consulta expõem status, métricas Prometheus, instantâneos de telemetria e eventos
fluxos usando a gramática de filtro Norito. O streaming é atualizado automaticamente para
WebSockets e currículos quando o orçamento para novas tentativas permitir.

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

Use `streamBlocks`, `streamTransactions` ou `streamTelemetry` para o outro
Pontos de extremidade WebSocket. Todos os auxiliares de streaming apresentam novas tentativas, então conecte o
Retorno de chamada `onReconnect` para alimentar painéis e alertas.

## Instantâneos do Explorer e cargas QR

A telemetria do Explorer fornece auxiliares digitados para `/v1/explorer/metrics` e
Endpoints `/v1/explorer/accounts/{account_id}/qr` para que os painéis possam reproduzir o
mesmos instantâneos que alimentam o portal. `getExplorerMetrics()` normaliza o
carga útil e retorna `null` quando a rota está desabilitada. Combine com
`getExplorerAccountQr()` sempre que você precisar de literais I105 (preferencial)/sora (segundo melhor) mais inline
SVG para botões de compartilhamento.

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

const qr = await torii.getExplorerAccountQr("i105...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

Passar `I105` espelha o padrão compactado do Explorer
seletores; omita a substituição para a saída I105 preferencial ou solicite `i105_qr`
quando você precisar da variante segura para QR. O literal compactado é o segundo melhor
Opção somente Sora para UX. O auxiliar sempre retorna o identificador canônico,
o literal selecionado e metadados (prefixo de rede, versão/módulos QR, erro
nível de correção e SVG embutido), para que o CI/CD possa publicar as mesmas cargas que
o Explorer surge sem chamar conversores personalizados.

## Conectar sessões e filas

Os auxiliares do Connect espelham `docs/source/connect_architecture_strawman.md`. O
o caminho mais rápido para uma sessão pronta para visualização é `bootstrapConnectPreviewSession`,
que une a geração determinística de SID/URI e o Torii
chamada de registro.

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

- Passe `register: false` quando precisar apenas de URIs determinísticos para QR/deeplink
  prévias.
- `generateConnectSid` permanece disponível quando você precisa derivar IDs de sessão
  sem cunhar URIs.
- Chaves direcionais e envelopes de texto cifrado vêm da ponte nativa; quando
  indisponível, o SDK volta para o codec JSON e lança
  `ConnectQueueError.bridgeUnavailable`.
- Os buffers offline são armazenados como blobs Norito `.to` no IndexedDB. Fila de monitoramento
  estado através do `ConnectQueueError.overflow(limit)` /
  Erros `.expired(ttlMs)` e telemetria de alimentação `connect.queue_depth` conforme descrito
  no roteiro.

### Conecte instantâneos de registro e políticaOs operadores da plataforma podem examinar e atualizar o registro do Connect sem
saindo do Node.js. `iterateConnectApps()` percorre o registro, enquanto
`getConnectStatus()` e `getConnectAppPolicy()` expõem os contadores de tempo de execução e
actual envelope político. `updateConnectAppPolicy()` aceita campos camelCase,
para que você possa preparar a mesma carga JSON que Torii espera.

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

Sempre capture o instantâneo `getConnectStatus()` mais recente antes de aplicar
mutações – a lista de verificação de governança exige evidências de que as atualizações das políticas começam
dos limites atuais da frota.

### Conectar discagem WebSocket

`ToriiClient.openConnectWebSocket()` monta o canônico
URL `/v1/connect/ws` (incluindo `sid`, `role` e parâmetros de token), atualizações
`http→ws` / `https→wss` e entrega o URL final para qualquer WebSocket
implementação que você fornece. Os navegadores reutilizam automaticamente o global
`WebSocket`. Os chamadores do Node.js devem passar um construtor como `ws`:

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

Quando você precisar apenas da URL, ligue para `torii.buildConnectWebSocketUrl(params)` ou para o
auxiliar `buildConnectWebSocketUrl(baseUrl, params)` de nível superior e reutilizar o
string resultante em um transporte/fila personalizado.

Procurando uma amostra completa orientada para CLI? O
[Receita de visualização do Connect](./recipes/javascript-connect-preview.md) inclui um
script executável mais orientação de telemetria que reflete o roteiro entregue para
documentando a fila do Connect + fluxo do WebSocket.

### Telemetria e alertas de fila

Conecte as métricas da fila diretamente às superfícies auxiliares para que os painéis possam espelhar
os KPIs do roteiro.

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

`ConnectQueueError#toConnectError()` converte falhas de fila em genéricas
Taxonomia `ConnectError` para que interceptadores HTTP/WebSocket compartilhados possam emitir o
padrão `connect.queue_depth`, `connect.queue_overflow_total` e
Métricas `connect.queue_expired_total` referenciadas em todo o roteiro.

## Observadores de streaming e cursores de eventos

`ToriiClient.streamEvents()` expõe `/v1/events/sse` como um iterador assíncrono com automação
novas tentativas, para que as CLIs do Node/Bun possam acompanhar a atividade do pipeline da mesma forma que a CLI do Rust faz.
Persista o cursor `Last-Event-ID` ao lado dos artefatos do seu runbook para que os operadores possam
retomar um fluxo sem pular eventos quando um processo for reiniciado.

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

- Alterne `PIPELINE_STATUS` (por exemplo `Pending`, `Applied` ou `Approved`) ou defina
  `STREAM_FILTER_JSON` para reproduzir os mesmos filtros que a CLI aceita.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` mantém o iterador ativo até que um
  o sinal é recebido; passe `STREAM_MAX_EVENTS=25` quando você precisar apenas dos primeiros eventos
  para um teste de fumaça.
- `ToriiClient.streamSumeragiStatus()` espelha a mesma interface para
  `/v1/sumeragi/status/sse` para que a telemetria de consenso possa ser seguida separadamente, e o
  iterador honra `Last-Event-ID` da mesma maneira.
- Consulte `javascript/iroha_js/recipes/streaming.mjs` para obter uma CLI pronta para uso (persistência de cursor,
  substituições de filtro env-var e registro `extractPipelineStatusKind`) usados no JS4
  entrega do roteiro de streaming/WebSocket.

## Portfólios UAID e Diretório Espacial

As APIs do Space Directory revelam o ciclo de vida do Universal Account ID (UAID). O
ajudantes aceitam literais `uaid:<hex>` ou resumos brutos de 64 hexadecimais (LSB = 1) e
canonizá-los antes de enviar solicitações:- `getUaidPortfolio(uaid, { assetId })` agrega saldos por espaço de dados,
  agrupar ativos por IDs de contas canônicas; passe `assetId` para filtrar o
  portfólio até uma única instância de ativo.
- `getUaidBindings(uaid)` enumera todos os espaços de dados ↔ contas
  ligação (`I105` retorna os literais `i105`).
- `getUaidManifests(uaid, { dataspaceId })` retorna cada manifesto de capacidade,
  status do ciclo de vida e contas vinculadas para auditoria.

Para pacotes de evidências de operadores, fluxos de publicação/revogação de manifestos e migração de SDK
orientação, siga o Guia de Conta Universal (`docs/source/universal_accounts_guide.md`)
junto com esses ajudantes do cliente para que o portal e a documentação de origem permaneçam sincronizados.

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

Os operadores também podem alternar manifestos ou executar fluxos emergenciais de negação de ganhos sem
caindo para a CLI. Ambos os auxiliares aceitam um objeto `{ signal }` opcional, então
envios de longa duração podem ser cancelados com `AbortController`; não-objeto
opções ou entradas não `AbortSignal` geram um `TypeError` síncrono antes do
solicitação atinge Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "i105...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "i105...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` aceita JSON de manifesto bruto (correspondendo ao
fixtures sob `fixtures/space_directory/`) ou qualquer objeto que seja serializado para o
mesma estrutura. `privateKey`, `privateKeyHex` ou `privateKeyMultihash` mapeiam para
o campo `ExposedPrivateKey` Torii espera e o padrão é `ed25519`
algoritmo quando nenhum prefixo é fornecido. Ambas as solicitações retornam quando Torii é enfileirado
a instrução (`202 Accepted`), momento em que o razão emitirá o
correspondente `SpaceDirectoryEvent`.

## Governança e ponte ISO

`ToriiClient` expõe as APIs de governança para inspecionar contratos, preparar
propostas, enviando cédulas (simples ou ZK), rotacionando o conselho e convocando
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` sem DTOs escritos à mão. Ajudantes da ISO 20022
siga o mesmo padrão via `buildPacs008Message`/`buildPacs009Message` e o
Trio `submitIso*`/`waitForIsoMessageStatus`.

Consulte a [receita de governança e ponte ISO](./recipes/javascript-governance-iso.md)
para amostras prontas para CLI, além de referências ao guia de campo completo em
`docs/source/sdk/js/governance_iso_examples.md`.

## Amostragem de RBC e evidências de entrega

O roteiro JS também requer amostragem Roadrunner Block Commitment (RBC) para que os operadores possam
provar que o bloco que eles buscaram por meio de Sumeragi corresponde às provas de pedaços que eles verificaram.
Use os auxiliares integrados em vez de criar cargas manualmente:

1. `getSumeragiRbcSessions()` espelha `/v1/sumeragi/rbc/sessions` e
   `findRbcSamplingCandidate()` seleciona automaticamente a primeira sessão entregue com um hash de bloco
   (o conjunto de integração recorre a ele sempre que
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` não está definido).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` normaliza `{blockHash,height,view}`
   além de substituições opcionais `{count,seed,apiToken}` para que números inteiros hexadecimais ou negativos mal formados nunca
   alcance Torii.
3. `sampleRbcChunks()` envia a solicitação para `/v1/sumeragi/rbc/sample`, retornando provas de blocos
   e caminhos Merkle (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) você deve arquivar com
   o resto de suas evidências de adoção.
4. `getSumeragiRbcDelivered(height, view)` captura os metadados de entrega da coorte para que os auditores
   pode reproduzir a prova de ponta a ponta.

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
```Persista ambas as respostas sob a raiz do artefato que você submete à governança. Substituir o
sessão selecionada automaticamente via `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
whenever you need to probe a specific block, and treat failures to fetch RBC snapshots as a
erro de ativação pré-voo em vez de fazer o downgrade silenciosamente para o modo direto.

## Teste e CI

1. Cache de carga e artefatos npm.
2. Execute `npm run build:native`.
3. Execute `npm test` (ou `node --test` para trabalhos de fumaça).

O fluxo de trabalho de referência do GitHub Actions reside em
`docs/source/examples/iroha_js_ci.md`.

##Próximas etapas

- Revise os tipos gerados em `javascript/iroha_js/index.d.ts`.
- Explore as receitas em `javascript/iroha_js/recipes/`.
- Emparelhe `ToriiClient` com o início rápido Norito para inspecionar cargas úteis ao lado
  Chamadas SDK.