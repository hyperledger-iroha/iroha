---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Início rápido do SDK JavaScript

`@iroha2/torii-client` fornece um navegador e um wrapper amigável para Node.js em torno de Torii.
Este guia de início rápido reflete os principais fluxos das receitas do SDK para que você possa obter uma
cliente rodando em poucos minutos. Para exemplos mais completos, consulte
`javascript/iroha_js/recipes/` no repositório.

## 1. Instalar

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Se você planeja assinar transações localmente, instale também os crypto helpers:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Crie um cliente Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

A configuração reflete o construtor usado nas receitas. Se o seu nó
usa autenticação básica, passe `{username, password}` por meio da opção `basicAuth`.

## 3. Buscar status do nó

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Todas as operações de leitura retornam objetos JSON suportados por Norito. Veja os tipos gerados em
`index.d.ts` para detalhes do campo.

## 4. Envie uma transação

Os signatários podem criar transações com a API auxiliar:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

O auxiliar envolve automaticamente a transação no envelope Norito esperado
por Torii. Para um exemplo mais rico (incluindo esperas por finalidade), consulte
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Use ajudantes de alto nível

O SDK agrupa fluxos especializados que espelham a CLI:

- **Ajudantes de governança** – `recipes/governance.mjs` demonstra a preparação
  propostas e votações com os construtores de instruções `governance`.
- **Ponte ISO** – `recipes/iso_bridge.mjs` mostra como enviar `pacs.008` e
  status de transferência de poll usando os terminais `/v1/iso20022`.
- **SoraFS e gatilhos** – Auxiliares de paginação sob exposição `src/toriiClient.js`
  iteradores digitados para contratos, ativos, gatilhos e provedores SoraFS.

Importe as funções do construtor relevantes de `@iroha2/torii-client` para reutilizar esses fluxos.

## 6. Tratamento de erros

Todas as chamadas do SDK geram instâncias `ToriiClientError` ricas com metadados de transporte
e a carga de erro Norito. Encerre chamadas em `try/catch` ou use `.catch()` para
contexto de superfície para os usuários:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

##Próximas etapas

- Explore as receitas em `javascript/iroha_js/recipes/` para fluxos ponta a ponta.
- Leia os tipos gerados em `javascript/iroha_js/index.d.ts` para detalhes
  assinaturas de método.
- Combine este SDK com o início rápido Norito para inspecionar e depurar as cargas úteis
  você envia para Torii.