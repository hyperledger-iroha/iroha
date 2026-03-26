---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.he.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Exemples de gouvernance et de passerelle ISO
description : Pilotez des flux de travail Torii avancés avec `@iroha/iroha-js`.
slug : /sdks/javascript/governance-iso-examples
---

Ce guide de terrain développe le démarrage rapide en démontrant la gouvernance et
Les flux de pont ISO 20022 avec `@iroha/iroha-js`. Les extraits réutilisent la même chose
les assistants d'exécution fournis avec `ToriiClient`, afin que vous puissiez les copier directement dans
Outils CLI, harnais CI ou services de longue durée.

Ressources supplémentaires :

- `javascript/iroha_js/recipes/governance.mjs` — script de bout en bout exécutable pour
  propositions, scrutins et rotations des conseils.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — Assistant CLI pour la soumission
  Charges utiles pacs.008/pacs.009 et statut déterministe de l'interrogation.
- `docs/source/finance/settlement_iso_mapping.md` — mappage canonique des champs ISO.

## Exécuter les recettes groupées

Ces exemples dépendent des scripts dans `javascript/iroha_js/recipes/`. Courir
`npm install && npm run build:native` au préalable afin que les liaisons générées soient
disponible.

### Procédure pas à pas de l'aide à la gouvernance

Configurez les variables d'environnement suivantes avant d'appeler
`recipes/governance.mjs` :- `TORII_URL` — Point de terminaison Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — compte signataire et clé (hex). Conservez les clés dans un
  magasin secret sécurisé.
- `CHAIN_ID` — identifiant de réseau facultatif.
- `GOV_SUBMIT=1` — poussez les transactions générées vers Torii.
- `GOV_FETCH=1` — récupérer les propositions/verrous après la soumission.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — recherches facultatives utilisées
  quand `GOV_FETCH=1`.

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

Les hachages sont enregistrés pour chaque étape et les réponses Torii apparaissent lorsque
`GOV_SUBMIT=1` afin que les tâches CI puissent échouer rapidement en cas d'erreurs de soumission.

### Assistant de pont ISO

`recipes/iso_bridge.mjs` soumet un message pacs.008 ou pacs.009 et interroge
le pont ISO jusqu'à ce que le statut soit réglé. Configurez-le avec :- `TORII_URL` — Point de terminaison Torii exposant les API du pont ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (par défaut) ou `pacs.009`. L'assistant utilise le
  générateur d'échantillons correspondants (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  lorsque vous ne fournissez pas votre propre XML.
- `ISO_MESSAGE_SUFFIX` — suffixe facultatif ajouté aux exemples d'ID de charge utile pour
  garder les répétitions répétées uniques (par défaut les secondes de l'époque actuelle en hexadécimal).
- `ISO_CONTENT_TYPE` — remplace l'en-tête `Content-Type` pour les soumissions
  (par exemple `application/pacs009+xml`) ; ignoré lorsque vous interrogez uniquement un
  identifiant de message existant.
- `ISO_MESSAGE_ID` — ignorez complètement la soumission et interrogez uniquement les éléments fournis
  identifiant via `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — ajustez la stratégie d'attente pour
  déploiements de ponts bruyants ou lents.
- `ISO_RESOLVE_ON_ACCEPTED=1` — quittez dès que Torii renvoie `Accepted`,
  même si le hachage de la transaction est toujours en attente (pratique lors de la maintenance du pont
  lorsque la validation du grand livre est retardée).

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

Les deux scripts se terminent avec le code d'état `1` si Torii ne signale jamais de terminal.
transition, ce qui les rend adaptés aux travaux de porte CI.

### Assistant d'alias ISO`recipes/iso_alias.mjs` cible les points de terminaison de l'alias ISO afin que les répétitions puissent couvrir
hachage d'éléments en aveugle et recherches d'alias sans écrire d'outils sur mesure. Il
appelle `ToriiClient.evaluateAliasVoprf` plus `resolveAlias` / `resolveAliasByIndex`
et imprime le backend, le résumé, la liaison de compte, la source et l'index déterministe
renvoyé par Torii.

Variables d'environnement :

- `TORII_URL` — Point de terminaison Torii exposant les assistants d'alias.
- `ISO_VOPRF_INPUT` — élément masqué codé en hexadécimal (par défaut : `deadbeef`).
- `ISO_SKIP_VOPRF=1` — ignore l'appel VOPRF lorsque vous testez uniquement les recherches.
- `ISO_ALIAS_LABEL` — alias littéral à résoudre (par exemple, chaînes de style IBAN).
- `ISO_ALIAS_INDEX` — index décimal ou avec préfixe `0x` transmis à `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — en-têtes facultatifs pour les déploiements Torii sécurisés.

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

L'assistant reflète le comportement de Torii : il fait apparaître les 404 lorsque les alias sont manquants.
et traite les erreurs d'exécution désactivées comme des sauts logiciels afin que les flux CI puissent tolérer le pont
fenêtres de maintenance.

## Workflows de gouvernance

### Inspecter les instances de contrat et les propositions

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

### Soumettre des propositions et des bulletins de vote

Utilisez un `AbortController` lorsque vous devez annuler ou soumettre des soumissions de gouvernance limitées dans le temps : le SDK
accepte un objet `{ signal }` facultatif pour chaque assistant POST indiqué ci-dessous.

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

### Conseil VRF et promulgation

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

## Recettes de pont ISO&nbsp;20022### Construire les charges utiles pacs.008 / pacs.009

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

Tous les identifiants (BIC, LEI, IBAN, montant ISO) sont validés avant que XML ne soit
généré. Échangez `buildPacs008Message` contre `buildPacs009Message` pour émettre du PvP
financer des charges utiles.

### Soumettre et interroger les messages ISO

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

`resolveOnAccepted` et `resolveOnAcceptedWithoutTransaction` sont tous deux valides ; utiliser l'un ou l'autre des drapeaux
pour traiter les statuts `Accepted` (sans hachage de transaction) comme terminal lors de l'orchestration des sondages.

Les assistants lancent `IsoMessageTimeoutError` si le pont ne signale jamais de
état terminal. Utilisez le niveau inférieur `submitIsoPacs008` / `submitIsoPacs009`
appelle lorsque vous devez orchestrer une logique d’interrogation personnalisée ; `getIsoMessageStatus`
expose une recherche en un seul coup.

### Surfaces associées

- `torii.getSorafsPorWeeklyReport("2026-W05")` récupère le bundle PoR d'une semaine ISO
  référencé dans la feuille de route et peut réutiliser les assistants d'attente pour les alertes.
- `resolveAlias` / `resolveAliasByIndex` exposent les liaisons d'alias de pont ISO afin
  les outils de rapprochement peuvent prouver la propriété du compte avant d’émettre un paiement.