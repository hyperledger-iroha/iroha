---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Exemplos de governança e ponte ISO
description: Impulsione fluxos de trabalho avançados Torii com `@iroha/iroha-js`.
slug: /sdks/javascript/governance-iso-examples
---

Este guia de campo amplia o início rápido, demonstrando governança e
Fluxos de ponte ISO20022 com `@iroha/iroha-js`. Os trechos reutilizam o mesmo
auxiliares de tempo de execução fornecidos com `ToriiClient`, para que você possa copiá-los diretamente para
Ferramentas CLI, chicotes de CI ou serviços de longa duração.

Recursos adicionais:

- `javascript/iroha_js/recipes/governance.mjs` — script executável de ponta a ponta para
  propostas, votações e rotações do conselho.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — auxiliar CLI para envio
  cargas úteis pacs.008/pacs.009 e status determinístico de pesquisa.
- `docs/source/finance/settlement_iso_mapping.md` — mapeamento de campo ISO canônico.

## Executando as receitas incluídas

Esses exemplos dependem dos scripts em `javascript/iroha_js/recipes/`. Corre
`npm install && npm run build:native` antecipadamente para que as ligações geradas sejam
disponível.

### Passo a passo do auxiliar de governança

Configure as seguintes variáveis de ambiente antes de invocar
`recipes/governance.mjs`:

- `TORII_URL` — Ponto de extremidade Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — conta e chave do assinante (hex). Mantenha as chaves em um
  armazenamento secreto seguro.
- `CHAIN_ID` — identificador de rede opcional.
- `GOV_SUBMIT=1` — envia as transações geradas para Torii.
- `GOV_FETCH=1` — busca propostas/bloqueios após o envio.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — pesquisas opcionais usadas
  quando `GOV_FETCH=1`.

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=i105... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

Hashes são registrados para cada etapa e as respostas Torii são exibidas quando
`GOV_SUBMIT=1` para que os trabalhos de CI possam falhar rapidamente em caso de erros de envio.

### Auxiliar de ponte ISO

`recipes/iso_bridge.mjs` envia uma mensagem pacs.008 ou pacs.009 e enquetes
a ponte ISO até que o status seja resolvido. Configure-o com:

- `TORII_URL` — Endpoint Torii expondo as APIs da ponte ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (padrão) ou `pacs.009`. O ajudante usa o
  construtor de amostra correspondente (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  quando você não fornece seu próprio XML.
- `ISO_MESSAGE_SUFFIX` — sufixo opcional anexado aos IDs de carga útil de amostra para
  manter os ensaios repetidos únicos (o padrão é os segundos da época atual em hexadecimal).
- `ISO_CONTENT_TYPE` — substitui o cabeçalho `Content-Type` para envios
  (por exemplo `application/pacs009+xml`); ignorado quando você apenas pesquisa um
  ID da mensagem existente.
- `ISO_MESSAGE_ID` — ignore completamente o envio e pesquise apenas o fornecido
  identificador via `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — ajuste a estratégia de espera para
  implantações de ponte barulhentas ou lentas.
- `ISO_RESOLVE_ON_ACCEPTED=1` — sai assim que Torii retorna `Accepted`,
  mesmo se o hash da transação ainda estiver pendente (útil durante a manutenção da ponte
  quando a confirmação do razão está atrasada).

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

Ambos os scripts saem com o código de status `1` se Torii nunca reportar um terminal
transição, tornando-os adequados para trabalhos de portão CI.

### Auxiliar de alias ISO`recipes/iso_alias.mjs` tem como alvo os endpoints de alias ISO para que os ensaios possam cobrir
hashing de elemento cego e pesquisas de alias sem escrever ferramentas personalizadas. Isso
chama `ToriiClient.evaluateAliasVoprf` mais `resolveAlias` / `resolveAliasByIndex`
e imprime o back-end, resumo, vinculação de conta, origem e índice determinístico
retornado por Torii.

Variáveis de ambiente:

- `TORII_URL` — Endpoint Torii expondo os auxiliares de alias.
- `ISO_VOPRF_INPUT` — elemento cego com codificação hexadecimal (o padrão é `deadbeef`).
- `ISO_SKIP_VOPRF=1` — ignore a chamada VOPRF ao testar apenas pesquisas.
- `ISO_ALIAS_LABEL` — alias literal a ser resolvido (por exemplo, strings no estilo IBAN).
- `ISO_ALIAS_INDEX` — índice decimal ou com prefixo `0x` passado para `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN`/`TORII_API_TOKEN` — cabeçalhos opcionais para implantações seguras de Torii.

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

O auxiliar reflete o comportamento do Torii: ele apresenta 404s quando os aliases estão faltando
e trata erros desabilitados em tempo de execução como pulos suaves para que os fluxos de CI possam tolerar ponte
janelas de manutenção.

## Fluxos de trabalho de governança

### Inspecione instâncias e propostas de contratos

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
  console.log(`${entry.contract_id} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### Envie propostas e votações

Use um `AbortController` quando precisar cancelar ou enviar envios de governança com prazo determinado — o SDK
aceita um objeto `{ signal }` opcional para cada auxiliar POST mostrado abaixo.

```ts
const authority = "i105...";
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

const zkOwner = "i105..."; // canonical i105 account id for ZK public inputs
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

### Conselho VRF e promulgação

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "i105...",
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

## Receitas de ponte ISO20022

### Construir cargas úteis pacs.008 / pacs.009

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
  creditorAccount: { otherId: "i105..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "i105...", leg: "delivery" },
});
```

Todos os identificadores (BIC, LEI, IBAN, valor ISO) são validados antes do XML ser
gerado. Troque `buildPacs008Message` por `buildPacs009Message` para emitir PvP
cargas úteis de financiamento.

### Enviar e pesquisar mensagens ISO

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

Tanto `resolveOnAccepted` quanto `resolveOnAcceptedWithoutTransaction` são válidos; use qualquer sinalizador
para tratar os status `Accepted` (sem hash de transação) como terminal ao orquestrar pesquisas.

Os ajudantes lançam `IsoMessageTimeoutError` se a ponte nunca reportar um
estado terminal. Use o `submitIsoPacs008` / `submitIsoPacs009` de nível inferior
chamadas quando você precisa orquestrar uma lógica de pesquisa personalizada; `getIsoMessageStatus`
expõe uma pesquisa única.

### Superfícies relacionadas

- `torii.getSorafsPorWeeklyReport("2026-W05")` busca o pacote PoR da semana ISO
  referenciado no roteiro e pode reutilizar os auxiliares de espera para alertas.
- `resolveAlias` / `resolveAliasByIndex` expõe ligações de alias de ponte ISO para
  ferramentas de reconciliação podem provar a propriedade da conta antes de emitir um pagamento.