---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: Примеры управления и моста ISO
описание: Управляйте расширенными рабочими процессами Torii с помощью `@iroha/iroha-js`.
пул: /sdks/javascript/governance-iso-examples
---

Это практическое руководство расширяет краткое руководство, демонстрируя управление и
Мост ISO 20022 работает с `@iroha/iroha-js`. Фрагменты повторно используют одно и то же
помощники времени выполнения, которые поставляются с `ToriiClient`, поэтому вы можете скопировать их непосредственно в
Инструменты CLI, средства CI или долгоработающие сервисы.

Дополнительные ресурсы:

- `javascript/iroha_js/recipes/governance.mjs` — работоспособный сквозной скрипт для
  предложения, голосования и ротация советов.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — помощник CLI для отправки
  Полезные данные pacs.008/pacs.009 и детерминированный статус опроса.
- `docs/source/finance/settlement_iso_mapping.md` — каноническое сопоставление полей ISO.

## Запуск встроенных рецептов

Эти примеры зависят от сценариев в `javascript/iroha_js/recipes/`. Беги
`npm install && npm run build:native` заранее, чтобы сгенерированные привязки были
доступен.

### Пошаговое руководство помощника по управлению

Настройте следующие переменные среды перед вызовом
`recipes/governance.mjs`:

- `TORII_URL` — конечная точка Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — учетная запись и ключ подписывающего лица (шестнадцатеричный). Храните ключи в
  безопасный секретный склад.
- `CHAIN_ID` — необязательный идентификатор сети.
— `GOV_SUBMIT=1` — отправить сгенерированные транзакции на Torii.
- `GOV_FETCH=1` — получать предложения/блокировки после отправки.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — используются дополнительные запросы поиска.
  когда `GOV_FETCH=1`.

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

Хэши регистрируются для каждого шага, а ответы Torii отображаются, когда
`GOV_SUBMIT=1`, чтобы задания CI могли быстро завершаться сбоем из-за ошибок отправки.

### Помощник по мосту ISO

`recipes/iso_bridge.mjs` отправляет сообщение pacs.008 или pacs.009 и проводит опросы.
мост ISO, пока статус не установится. Настройте его с помощью:

- `TORII_URL` — конечная точка Torii, предоставляющая API-интерфейсы моста ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (по умолчанию) или `pacs.009`. Помощник использует
  соответствующий конструктор образцов (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  когда вы не предоставляете свой собственный XML.
- `ISO_MESSAGE_SUFFIX` — необязательный суффикс, добавляемый к образцам идентификаторов полезной нагрузки для
  сохранять повторяющиеся репетиции уникальными (по умолчанию — секунды текущей эпохи в шестнадцатеричном формате).
- `ISO_CONTENT_TYPE` — переопределить заголовок `Content-Type` для отправки.
  (например `application/pacs009+xml`); игнорируется, когда вы только опрашиваете
  существующий идентификатор сообщения.
- `ISO_MESSAGE_ID` — вообще пропустить отправку и опрашивать только предоставленные
  идентификатор через `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — настроить стратегию ожидания для
  шумное или медленное развертывание моста.
- `ISO_RESOLVE_ON_ACCEPTED=1` — выйти, как только Torii вернет `Accepted`,
  даже если хеш транзакции все еще находится в ожидании (удобно во время обслуживания моста).
  когда фиксация реестра задерживается).

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

Оба сценария завершаются с кодом состояния `1`, если Torii никогда не сообщает о терминале.
переход, что делает их подходящими для работы на шлюзах CI.

### Помощник по псевдонимам ISO`recipes/iso_alias.mjs` нацелен на конечные точки псевдонимов ISO, чтобы репетиции могли охватывать
хеширование скрытых элементов и поиск псевдонимов без написания специальных инструментов. Это
вызывает `ToriiClient.evaluateAliasVoprf` плюс `resolveAlias` / `resolveAliasByIndex`
и печатает серверную часть, дайджест, привязку учетной записи, источник и детерминированный индекс.
возвращено Torii.

Переменные среды:

- `TORII_URL` — конечная точка Torii, предоставляющая помощники псевдонимов.
- `ISO_VOPRF_INPUT` — закрытый элемент в шестнадцатеричной кодировке (по умолчанию `deadbeef`).
- `ISO_SKIP_VOPRF=1` — пропустить вызов VOPRF только при тестировании поиска.
- `ISO_ALIAS_LABEL` — буквальный псевдоним для разрешения (например, строки в стиле IBAN).
- `ISO_ALIAS_INDEX` — десятичный индекс или индекс с префиксом `0x`, передаваемый в `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — дополнительные заголовки для защищенных развертываний Torii.

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

Помощник отражает поведение Torii: он выдает ошибку 404, когда псевдонимы отсутствуют.
и обрабатывает ошибки, отключенные во время выполнения, как мягкие пропуски, поэтому потоки CI могут допускать мост
окна обслуживания.

## Рабочие процессы управления

### Проверка экземпляров контрактов и предложений

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

### Подача предложений и бюллетеней

Используйте `AbortController`, когда вам нужно отменить или ограничить отправку данных по управлению — SDK
принимает дополнительный объект `{ signal }` для каждого помощника POST, показанного ниже.

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

### Совет ВРФ и постановление

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

## Рецепты мостов ISO&nbsp;20022

### Сборка полезных нагрузок pacs.008/pacs.009

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

Все идентификаторы (BIC, LEI, IBAN, сумма ISO) проверяются перед отправкой XML.
генерируется. Замените `buildPacs008Message` на `buildPacs009Message` для создания PvP.
финансирование полезной нагрузки.

### Отправка и опрос ISO-сообщений

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

Оба `resolveOnAccepted` и `resolveOnAcceptedWithoutTransaction` действительны; используйте любой флаг
обрабатывать статусы `Accepted` (без хэша транзакции) как терминальные при организации опросов.

Помощники выдают `IsoMessageTimeoutError`, если мост никогда не сообщает о
терминальное состояние. Используйте нижний уровень `submitIsoPacs008`/`submitIsoPacs009`.
звонки, когда вам нужно организовать собственную логику опроса; `getIsoMessageStatus`
предоставляет однократный поиск.

### Сопутствующие поверхности

- `torii.getSorafsPorWeeklyReport("2026-W05")` извлекает пакет PoR за неделю ISO.
  упомянуты в плане действий и могут повторно использовать помощники ожидания для оповещений.
- `resolveAlias` / `resolveAliasByIndex` предоставляют привязки псевдонимов моста ISO, поэтому
  инструменты сверки могут подтвердить право собственности на учетную запись до совершения платежа.