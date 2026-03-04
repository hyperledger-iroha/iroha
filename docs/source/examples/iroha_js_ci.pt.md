---
lang: pt
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2026-01-03T18:08:00.440949+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Referência Iroha JS CI

O pacote `@iroha/iroha-js` agrupa ligações nativas por meio de `iroha_js_host`. Qualquer
O pipeline de CI que executa testes ou compilações deve fornecer um tempo de execução Node.js
e o conjunto de ferramentas Rust para que o pacote nativo possa ser compilado antes dos testes
correr.

## Etapas recomendadas

1. Use uma versão Node LTS (18 ou 20) via `actions/setup-node` ou seu CI
   equivalente.
2. Instale o conjunto de ferramentas Rust listado em `rust-toolchain.toml`. Recomendamos
   `dtolnay/rust-toolchain@v1` em ações do GitHub.
3. Armazene em cache os índices de registro de carga/git e o diretório `target/` para evitar
   reconstruindo o complemento nativo em cada trabalho.
4. Execute `npm install` e, em seguida, `npm run lint:test`. O script combinado impõe
   ESLint sem nenhum aviso, cria o complemento nativo e executa o teste do Node
   suíte para que o CI corresponda ao fluxo de trabalho de controle de liberação.
5. Opcionalmente, execute `node --test` como uma etapa de fumaça rápida uma vez `npm run build:native`
   produziu o complemento (por exemplo, pré-enviar pistas de verificação rápida que reutilizam
   artefatos armazenados em cache).
6. Coloque em camadas quaisquer verificações adicionais de linting ou formatação do seu consumidor
   projeto além do `npm run lint:test` quando políticas mais rígidas são necessárias.
7. Ao compartilhar a configuração entre serviços, carregue `iroha_config` e passe o
   documento analisado para `resolveToriiClientConfig({ config })` para clientes Node
   reutilizar a mesma política de tempo limite/nova tentativa/token que o restante da implantação (consulte
   `docs/source/sdk/js/quickstart.md` para um exemplo completo).

## Modelo de ações do GitHub

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run lint:test
```

## Trabalho de fumaça rápida (opcional)

Para solicitações pull que envolvem apenas documentação ou definições TypeScript, um
trabalho mínimo pode reutilizar artefatos armazenados em cache, reconstruir o módulo nativo e executar o
Executor de teste de nó diretamente:

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

Este trabalho é concluído rapidamente enquanto ainda verifica se o complemento nativo é compilado
e que o conjunto de testes do Node seja aprovado.

> **Implementação de referência:** o repositório inclui
> `.github/workflows/javascript-sdk.yml`, que conecta as etapas acima em um
> Matriz Node 18/20 com cache de carga.