---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/javascript.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Démarrage rapide du SDK JavaScript
description : Créez des transactions, diffusez des événements et pilotez des aperçus Connect avec `@iroha/iroha-js`.
slug : /sdks/javascript
---

`@iroha/iroha-js` est le package canonique Node.js pour interagir avec Torii. Il
regroupe les constructeurs Norito, les assistants Ed25519, les utilitaires de pagination et un outil résilient
Client HTTP/WebSocket afin que vous puissiez refléter les flux CLI à partir de TypeScript.

##Installation

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

L’étape de génération encapsule `cargo build -p iroha_js_host`. Assurez-vous que la chaîne d'outils de
`rust-toolchain.toml` est disponible localement avant d’exécuter `npm run build:native`.

## Gestion des clés

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

## Créer des transactions

Les constructeurs d'instructions Norito normalisent les identifiants, les métadonnées et les quantités afin
les transactions codées correspondent aux charges utiles Rust/CLI.

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

## ToriiConfiguration client

`ToriiClient` accepte les boutons de nouvelle tentative/délai d'attente qui reflètent `iroha_config`. Utiliser
`resolveToriiClientConfig` pour fusionner un objet de configuration camelCase (normaliser
`iroha_config` en premier), les remplacements d'environnement et les options en ligne.

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

Variables d'environnement pour le développement local :| Variables | Objectif |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Délai d'expiration de la demande (millisecondes). |
| `IROHA_TORII_MAX_RETRIES` | Nombre maximal de tentatives. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Interruption de la nouvelle tentative initiale. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Multiplicateur d'intervalle exponentiel. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Délai maximum de nouvelle tentative. |
| `IROHA_TORII_RETRY_STATUSES` | Codes d'état HTTP séparés par des virgules à réessayer. |
| `IROHA_TORII_RETRY_METHODS` | Méthodes HTTP séparées par des virgules pour réessayer. |
| `IROHA_TORII_API_TOKEN` | Ajoute `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | Ajoute l'en-tête `Authorization: Bearer …`. |

Les profils de nouvelle tentative reflètent les paramètres par défaut d'Android et sont exportés pour les contrôles de parité :
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. Voir `docs/source/sdk/js/torii_retry_policy.md`
pour le mappage point de terminaison-profil et les audits de gouvernance des paramètres au cours
JS4/JS7.

## Listes itérables et pagination

Les aides à la pagination reflètent l'ergonomie du SDK Python pour `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFT, soldes, détenteurs d'actifs et le
historique des transactions du compte.

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
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Allocations hors ligne et métadonnées de verdictLes réponses aux allocations hors ligne exposent dès le départ les métadonnées enrichies du grand livre :
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` et `remaining_amount` sont renvoyés avec le brut
enregistrez afin que les tableaux de bord n'aient pas à décoder les charges utiles Norito intégrées. Le nouveau
aides au compte à rebours (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) mettre en évidence la prochaine échéance expirante (actualisation → politique
→ certificat) afin que les badges d'assurance-chômage puissent avertir les opérateurs dès qu'une allocation est dépassée.
<24h restantes. Le SDK
reflète les filtres REST exposés par `/v1/offline/allowances` :
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` et le
Booléens `requireVerdict` / `onlyMissingVerdict`. Combinaisons invalides (pour
exemple `onlyMissingVerdict` + `verdictIdHex`) sont rejetés localement avant Torii
est appelé.

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

## Recharges hors ligne (problème + inscription)

Utilisez les assistants de recharge lorsque vous souhaitez émettre un certificat et immédiatement
enregistrez-le dans le grand livre. Le SDK vérifie le certificat émis et enregistré
Les identifiants correspondent avant le retour et la réponse inclut les deux charges utiles. Il y a
pas de point final de recharge dédié ; l'assistant enchaîne le problème + enregistre les appels. Si
vous disposez déjà d'un certificat signé, appelez le `registerOfflineAllowance` (ou
`renewOfflineAllowance`) directement.

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

## Torii requêtes et streaming (WebSockets)Les assistants de requête exposent l'état, les métriques Prometheus, les instantanés de télémétrie et les événements
flux utilisant la grammaire de filtre Norito. Le streaming est automatiquement mis à niveau vers
WebSockets et reprend lorsque le budget de nouvelle tentative le permet.

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

Utilisez `streamBlocks`, `streamTransactions` ou `streamTelemetry` pour l'autre
Points de terminaison WebSocket. Tous les assistants de streaming font apparaître de nouvelles tentatives, alors connectez le
Rappel `onReconnect` pour nourrir les tableaux de bord et les alertes.

## Instantanés de l'Explorateur et charges utiles QR

La télémétrie Explorer fournit des aides typées pour le `/v1/explorer/metrics` et
Points de terminaison `/v1/explorer/accounts/{account_id}/qr` pour que les tableaux de bord puissent rejouer les
mêmes instantanés qui alimentent le portail. `getExplorerMetrics()` normalise le
charge utile et renvoie `null` lorsque la route est désactivée. Associez-le à
`getExplorerAccountQr()` chaque fois que vous avez besoin des littéraux I105 (préféré)/sora (deuxième meilleur) plus en ligne
SVG pour les boutons de partage.

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

Passer `I105` reflète la compression par défaut de l'Explorateur
sélecteurs; omettez le remplacement de la sortie I105 préférée ou demandez `i105_qr`
lorsque vous avez besoin de la variante QR-safe. Le littéral compressé est le deuxième meilleur
Option Sora uniquement pour l'UX. L'assistant renvoie toujours l'identifiant canonique,
le littéral sélectionné et les métadonnées (préfixe réseau, version/modules QR, erreur
niveau de correction et SVG en ligne), afin que CI/CD puisse publier les mêmes charges utiles que
l'Explorer fait surface sans faire appel à des convertisseurs sur mesure.## Connecter les sessions et la file d'attente

Les assistants Connect reflètent `docs/source/connect_architecture_strawman.md`. Le
Le chemin le plus rapide vers une session prête pour la prévisualisation est `bootstrapConnectPreviewSession`,
qui assemble la génération déterministe SID/URI et le Torii
appel d'inscription.

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

- Passez `register: false` lorsque vous n'avez besoin que d'URI déterministes pour QR/deeplink
  aperçus.
- `generateConnectSid` reste disponible lorsque vous devez dériver des identifiants de session
  sans créer d'URI.
- Les clés directionnelles et les enveloppes de texte chiffré proviennent du pont natif ; quand
  indisponible, le SDK revient au codec JSON et lance
  `ConnectQueueError.bridgeUnavailable`.
- Les tampons hors ligne sont stockés sous forme de blobs Norito `.to` dans IndexedDB. Surveiller la file d'attente
  état via le `ConnectQueueError.overflow(limit)` / émis
  Erreurs `.expired(ttlMs)` et alimentation de la télémétrie `connect.queue_depth` comme indiqué
  dans la feuille de route.

### Connecter les instantanés du registre et des politiques

Les opérateurs de plateforme peuvent introspecter et mettre à jour le registre Connect sans
quitter Node.js. `iterateConnectApps()` pages via le registre, tandis que
`getConnectStatus()` et `getConnectAppPolicy()` exposent les compteurs d'exécution et
l’enveloppe politique actuelle. `updateConnectAppPolicy()` accepte les champs camelCase,
afin que vous puissiez organiser la même charge utile JSON que celle attendue par Torii.

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

Capturez toujours le dernier instantané `getConnectStatus()` avant d'appliquer
mutations : la liste de contrôle de la gouvernance exige des preuves que les mises à jour des politiques commencent
des limites actuelles de la flotte.### Connecter la numérotation WebSocket

`ToriiClient.openConnectWebSocket()` assemble le canonique
URL `/v1/connect/ws` (y compris `sid`, `role` et paramètres de jeton), mises à niveau
`http→ws` / `https→wss`, et transmet l'URL finale à n'importe quel WebSocket
mise en œuvre que vous fournissez. Les navigateurs réutilisent automatiquement le global
`WebSocket`. Les appelants Node.js doivent transmettre un constructeur tel que `ws` :

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

Lorsque vous n'avez besoin que de l'URL, appelez le `torii.buildConnectWebSocketUrl(params)` ou le
assistant `buildConnectWebSocketUrl(baseUrl, params)` de niveau supérieur et réutiliser le
chaîne résultante dans un transport/file d’attente personnalisé.

Vous recherchez un exemple complet orienté CLI ? Le
[Recette d'aperçu Connect](./recipes/javascript-connect-preview.md) comprend un
script exécutable plus des conseils de télémétrie qui reflètent la feuille de route livrable pour
documenter la file d'attente Connect + le flux WebSocket.

### Télémétrie et alertes de file d'attente

Transférez les métriques de file d’attente directement dans les surfaces d’assistance afin que les tableaux de bord puissent se refléter
les KPI de la feuille de route.

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

`ConnectQueueError#toConnectError()` convertit les échecs de file d'attente en génériques
Taxonomie `ConnectError` afin que les intercepteurs HTTP/WebSocket partagés puissent émettre le
norme `connect.queue_depth`, `connect.queue_overflow_total`, et
Métriques `connect.queue_expired_total` référencées tout au long de la feuille de route.

## Observateurs de streaming et curseurs d'événements`ToriiClient.streamEvents()` expose `/v1/events/sse` en tant qu'itérateur asynchrone avec
nouvelles tentatives, afin que les CLI Node/Bun puissent suivre l'activité du pipeline de la même manière que le fait la CLI Rust.
Conservez le curseur `Last-Event-ID` à côté des artefacts de votre runbook afin que les opérateurs puissent
reprendre un flux sans ignorer les événements lorsqu'un processus redémarre.

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

- Switch `PIPELINE_STATUS` (par exemple `Pending`, `Applied` ou `Approved`) ou définir
  `STREAM_FILTER_JSON` pour relire les mêmes filtres acceptés par la CLI.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` maintient l'itérateur en vie jusqu'à ce qu'un
  le signal est reçu ; passez `STREAM_MAX_EVENTS=25` lorsque vous n'avez besoin que des premiers événements
  pour un test de fumée.
- `ToriiClient.streamSumeragiStatus()` reflète la même interface pour
  `/v1/sumeragi/status/sse` afin que la télémétrie consensuelle puisse être suivie séparément, et le
  l'itérateur honore `Last-Event-ID` de la même manière.
- Voir `javascript/iroha_js/recipes/streaming.mjs` pour une CLI clé en main (persistance du curseur,
  remplacements de filtre env-var et journalisation `extractPipelineStatusKind`) utilisés dans le JS4
  Feuille de route streaming/WebSocket livrable.

## Portefeuilles UAID et répertoire spatial

Les API Space Directory font apparaître le cycle de vie de l'ID de compte universel (UAID). Le
les assistants acceptent les littéraux `uaid:<hex>` ou les résumés bruts de 64 hexadécimaux (LSB=1) et
canonisez-les avant de soumettre des demandes :- `getUaidPortfolio(uaid, { assetId })` agrège les soldes par espace de données,
  regrouper les avoirs par identifiants de compte canoniques ; passer `assetId` pour filtrer le
  portefeuille jusqu’à une seule instance d’actif.
- `getUaidBindings(uaid)` énumère chaque espace de données ↔ compte
  liaison (`I105` renvoie les littéraux `i105`).
- `getUaidManifests(uaid, { dataspaceId })` renvoie chaque manifeste de capacité,
  l'état du cycle de vie et les comptes liés pour l'audit.

Pour les packs de preuves des opérateurs, les flux de publication/révocation de manifeste et la migration du SDK
Pour obtenir des conseils, suivez le Guide de compte universel (`docs/source/universal_accounts_guide.md`).
aux côtés de ces assistants clients afin que le portail et la documentation source restent synchronisés.

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

Les opérateurs peuvent également alterner les manifestes ou exécuter des flux de refus-gagnant d'urgence sans
passer à la CLI. Les deux assistants acceptent un objet `{ signal }` facultatif, donc
les soumissions de longue durée peuvent être annulées avec `AbortController` ; non-objet
Les options ou les entrées non-`AbortSignal` déclenchent un `TypeError` synchrone avant le
la requête atteint Torii :

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
````publishSpaceDirectoryManifest()` accepte soit le JSON du manifeste brut (correspondant au
luminaires sous `fixtures/space_directory/`) ou tout objet sérialisé au
même structure. `privateKey`, `privateKeyHex` ou `privateKeyMultihash` mappé à
le champ `ExposedPrivateKey` Torii attend et par défaut le `ed25519`
algorithme lorsqu’aucun préfixe n’est fourni. Les deux requêtes sont renvoyées une fois que Torii est mis en file d'attente
l'instruction (`202 Accepted`), auquel cas le grand livre émettra le
correspondant à `SpaceDirectoryEvent`.

## Gouvernance & passerelle ISO

`ToriiClient` expose les API de gouvernance pour l'inspection des contrats, la mise en scène
propositions, soumission de bulletins de vote (simples ou ZK), rotation du conseil et convocation
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` sans DTO manuscrits. Assistants ISO 20022
suivez le même schéma via `buildPacs008Message`/`buildPacs009Message` et le
Trio `submitIso*`/`waitForIsoMessageStatus`.

Voir la [recette gouvernance & pont ISO](./recipes/javascript-governance-iso.md)
pour les échantillons prêts pour CLI ainsi que des pointeurs vers le guide de terrain complet dans
`docs/source/sdk/js/governance_iso_examples.md`.

## Preuve d'échantillonnage et d'accouchement de globules rouges

La feuille de route JS nécessite également un échantillonnage Roadrunner Block Commitment (RBC) afin que les opérateurs puissent
prouver que le bloc qu'ils ont récupéré via Sumeragi correspond aux preuves de blocs qu'ils vérifient.
Utilisez les assistants intégrés au lieu de créer des charges utiles à la main :1. `getSumeragiRbcSessions()` reflète `/v1/sumeragi/rbc/sessions`, et
   `findRbcSamplingCandidate()` sélectionne automatiquement la première session livrée avec un hachage de bloc
   (la suite d'intégration y revient à chaque fois
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` n’est pas défini).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` normalise `{blockHash,height,view}`
   plus `{count,seed,apiToken}` en option remplace les hexadécimaux mal formés ou les entiers négatifs jamais
   atteindre Torii.
3. `sampleRbcChunks()` POST la requête sur `/v1/sumeragi/rbc/sample`, renvoyant des preuves de fragments
   et chemins Merkle (`samples[].chunkHex`, `chunkRoot`, `payloadHash`), vous devez archiver avec
   le reste de votre preuve d’adoption.
4. `getSumeragiRbcDelivered(height, view)` capture les métadonnées de livraison de la cohorte afin que les auditeurs
   peut rejouer la preuve de bout en bout.

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

Conservez les deux réponses sous la racine de l'artefact que vous soumettez à la gouvernance. Remplacer le
session sélectionnée automatiquement via `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
chaque fois que vous avez besoin de sonder un bloc spécifique et de traiter les échecs de récupération des instantanés RBC comme un
erreur de contrôle avant le vol plutôt que de passer silencieusement en mode direct.

## Tests et CI

1. Cachez les artefacts cargo et npm.
2. Exécutez `npm run build:native`.
3. Exécutez `npm test` (ou `node --test` pour les tâches de fumée).

Le workflow de référence GitHub Actions se trouve dans
`docs/source/examples/iroha_js_ci.md`.

## Étapes suivantes

- Vérifiez les types générés dans `javascript/iroha_js/index.d.ts`.
- Explorez les recettes sous `javascript/iroha_js/recipes/`.
- Associez le `ToriiClient` au démarrage rapide Norito pour inspecter les charges utiles parallèlement
  Appels SDK.