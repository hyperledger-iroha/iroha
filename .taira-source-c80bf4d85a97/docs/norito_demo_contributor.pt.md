---
lang: pt
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# Guia de Contribuição para a Demo SwiftUI do Norito

Este documento descreve os passos de configuração manual necessários para executar a demo
SwiftUI contra um nó Torii local e um ledger simulado. Ele complementa
`docs/norito_bridge_release.md`, focando em tarefas de desenvolvimento do dia a dia. Para
um walkthrough mais profundo de como integrar o bridge Norito/stack Connect em projetos
Xcode, consulte `docs/connect_swift_integration.md`.

## Configuração de ambiente

1. Instale o toolchain Rust definido em `rust-toolchain.toml`.
2. Instale Swift 5.7+ e as Xcode Command Line Tools no macOS.
3. (Opcional) Instale [SwiftLint](https://github.com/realm/SwiftLint) para linting.
4. Execute `cargo build -p irohad` para garantir que o nó compile na sua máquina.
5. Copie `examples/ios/NoritoDemoXcode/Configs/demo.env.example` para `.env` e ajuste os
   valores de acordo com o seu ambiente. O app lê as seguintes variáveis na inicialização:
   - `TORII_NODE_URL` — URL base REST (as URLs WebSocket são derivadas a partir dela).
   - `CONNECT_SESSION_ID` — identificador de sessão de 32 bytes (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens retornados por
     `/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — identificador de chain anunciado durante o controle do
     handshake.
   - `CONNECT_ROLE` — role padrão pré‑selecionado na UI (`app` ou `wallet`).
   - Helpers opcionais para testes manuais: `CONNECT_PEER_PUB_B64`,
     `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

## Inicializando Torii + ledger simulado

O repositório inclui scripts de ajuda que sobem um nó Torii com um ledger em memória
pré‑carregado com contas de demonstração:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

O script gera:

- Logs do nó Torii em `artifacts/torii.log`.
- Métricas do ledger (formato Prometheus) em `artifacts/metrics.prom`.
- Tokens de acesso de clientes em `artifacts/torii.jwt`.

O `start.sh` mantém o peer de demo rodando até que você aperte `Ctrl+C`. Ele escreve um
snapshot de estado pronto em `artifacts/ios_demo_state.json` (fonte de verdade para os
outros artefatos), copia o log stdout ativo de Torii, faz polling em `/metrics` até que
um scrape Prometheus esteja disponível e renderiza as contas configuradas em `torii.jwt`
(incluindo chaves privadas quando presentes no config). O script aceita `--artifacts`
para sobrescrever o diretório de saída, `--telemetry-profile` para combinar configurações
personalizadas de Torii e `--exit-after-ready` para jobs de CI não interativos.

Cada entrada em `SampleAccounts.json` suporta os seguintes campos:

- `name` (string, opcional) — armazenado como metadata `alias` da conta.
- `public_key` (string multihash, obrigatório) — usado como signatário da conta.
- `private_key` (opcional) — incluído em `torii.jwt` para geração de credenciais de
  cliente.
- `domain` (opcional) — por padrão é o domínio do asset, caso omitido.
- `asset_id` (string, obrigatório) — definição de asset cunhada para a conta.
- `initial_balance` (string, obrigatório) — valor numérico cunhado na conta.

## Executando a demo SwiftUI

1. Construa o XCFramework conforme descrito em `docs/norito_bridge_release.md` e inclua‑o
   no projeto da demo (as referências esperam `NoritoBridge.xcframework` na raiz do
   projeto).
2. Abra o projeto `NoritoDemoXcode` no Xcode.
3. Selecione o esquema `NoritoDemo` e aponte para um simulador ou dispositivo iOS.
4. Certifique‑se de que o `.env` é referenciado através das variáveis de ambiente do
   esquema. Preencha os valores `CONNECT_*` retornados por `/v1/connect/session` para que
   a UI já venha pré‑preenchida ao iniciar o app.
5. Verifique os toggles de aceleração de hardware: `App.swift` chama
   `DemoAccelerationConfig.load().apply()` para que a demo respeite tanto o override de
   ambiente `NORITO_ACCEL_CONFIG_PATH` quanto arquivos empacotados
   `acceleration.{json,toml}`/`client.{json,toml}`. Remova/ajuste essas entradas se você
   quiser forçar fallback em CPU antes de rodar.
6. Compile e execute o aplicativo. A tela inicial solicitará a URL/token do Torii caso
   não tenham sido fornecidos via `.env`.
7. Inicie uma sessão “Connect” para assinar atualizações de conta ou aprovar requisições.
8. Envie uma transferência IRH e acompanhe o log na tela juntamente com os logs do Torii.

### Toggles de aceleração de hardware (Metal / NEON)

`DemoAccelerationConfig` espelha a configuração do node Rust, permitindo que
desenvolvedores exercitem os caminhos Metal/NEON sem hard‑code de thresholds. O loader
procura as configurações, na inicialização, nesta ordem:

1. `NORITO_ACCEL_CONFIG_PATH` (definida em `.env`/argumentos do esquema) — caminho
   absoluto ou `~` expandido para um arquivo `iroha_config` em JSON/TOML.
2. Arquivos de configuração empacotados chamados `acceleration.{json,toml}` ou
   `client.{json,toml}`.
3. Se nenhuma fonte estiver disponível, os defaults (`AccelerationSettings()`) são
   mantidos.

Exemplo de trecho `acceleration.toml`:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

Campos deixados em `nil` herdam os defaults do workspace. Números negativos são ignorados
e a ausência da seção `[accel]` faz com que o comportamento padronizado e determinístico
em CPU seja mantido. Ao rodar em um simulador sem suporte Metal, o bridge mantém o caminho
escalar mesmo que a configuração solicite Metal.

## Testes de integração

- Os testes de integração ficarão em `Tests/NoritoDemoTests` (a adicionar assim que o CI
  macOS estiver disponível).
- Os testes sobem o Torii usando os scripts acima e exercitam assinaturas WebSocket,
  saldos de tokens e fluxos de transferência via pacote Swift.
- Logs das execuções de teste são armazenados em `artifacts/tests/<timestamp>/` juntamente
  com métricas e dumps de ledger de exemplo.

## Checks de paridade em CI

- Execute `make swift-ci` antes de abrir um PR que altere a demo ou fixtures
  compartilhados. Esse target executa checks de paridade de fixtures, valida os feeds dos
  dashboards e gera resumos localmente. No CI, o mesmo workflow depende do metadata do
  Buildkite (`ci/xcframework-smoke:<lane>:device_tag`) para atribuir os resultados ao
  simulador ou lane StrongBox corretos — valide se o metadata continua presente ao alterar
  pipelines ou tags de agentes.
- Quando `make swift-ci` falhar, siga `docs/source/swift_parity_triage.md` e inspecione a
  saída gerada em `mobile_ci` para identificar qual lane exige regeneração ou tratamento
  adicional de incidente.

## Solução de problemas

- Se a demo não conseguir se conectar ao Torii, verifique a URL do node e as configurações
  de TLS.
- Certifique‑se de que o token JWT (se exigido) é válido e não expirou.
- Confira `artifacts/torii.log` em busca de erros do lado do servidor.
- Para problemas de WebSocket, inspecione a janela de log do cliente ou a saída do
  console do Xcode.

