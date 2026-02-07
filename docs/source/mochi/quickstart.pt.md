---
lang: pt
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2026-01-03T18:07:56.999063+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Início rápido do MOCHI

**MOCHI** é o supervisor de desktop para redes locais Hyperledger Iroha. Este guia percorre
instalando os pré-requisitos, construindo o aplicativo, iniciando o shell egui e usando o
ferramentas de tempo de execução (configurações, snapshots, limpezas) para desenvolvimento diário.

## Pré-requisitos

- Conjunto de ferramentas Rust: `rustup default stable` (espaço de trabalho destinado à edição 2024/Rust 1.82+).
- Conjunto de ferramentas da plataforma:
  - macOS: Ferramentas de linha de comando Xcode (`xcode-select --install`).
  - Linux: GCC, pkg-config, cabeçalhos OpenSSL (`sudo apt install build-essential pkg-config libssl-dev`).
- Dependências do espaço de trabalho Iroha:
  - `cargo xtask mochi-bundle` requer `irohad`, `kagami` e `iroha_cli` integrados. Construa-os uma vez via
    `cargo build -p irohad -p kagami -p iroha_cli`.
- Opcional: `direnv` ou `cargo binstall` para gerenciamento de binários de carga locais.

MOCHI desembolsa para os binários CLI. Certifique-se de que eles possam ser descobertos por meio das variáveis de ambiente
abaixo ou disponível no PATH:

| Binário | Substituição do ambiente | Notas |
|----------|----------------------|----------------------------------------|
| `irohad` | `MOCHI_IROHAD` | Supervisiona pares |
| `kagami` | `MOCHI_KAGAMI` | Gera manifestos/instantâneos do genesis |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Opcional para futuros recursos auxiliares |

## Construindo MOCHI

Da raiz do repositório:

```bash
cargo build -p mochi-ui-egui
```

Este comando cria `mochi-core` e o frontend egui. Para produzir um pacote distribuível, execute:

```bash
cargo xtask mochi-bundle
```

A tarefa do pacote agrupa os binários, o manifesto e os stubs de configuração em `target/mochi-bundle`.

## Iniciando o shell egui

Execute a UI diretamente do cargo:

```bash
cargo run -p mochi-ui-egui
```

Por padrão, o MOCHI cria uma predefinição de ponto único em um diretório de dados temporário:

- Raiz de dados: `$TMPDIR/mochi`.
- Porta base Torii: `8080`.
- Porta base P2P: `1337`.

Use sinalizadores CLI para substituir os padrões ao iniciar:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

As variáveis de ambiente refletem as mesmas substituições quando os sinalizadores CLI são omitidos: defina `MOCHI_DATA_ROOT`,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX` ou `MOCHI_RESTART_BACKOFF_MS` para pré-configurar o construtor supervisor; caminhos binários
continue a respeitar `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`, e `MOCHI_CONFIG` aponta para um
explícito `config/local.toml`.

## Configurações e persistência

Abra a caixa de diálogo **Configurações** na barra de ferramentas do painel para ajustar a configuração do supervisor:

- **Raiz de dados** — diretório base para configurações de pares, armazenamento, logs e instantâneos.
- **Torii / Portas base P2P** — portas iniciais para alocação determinística.
- **Visibilidade do log** — alterna canais stdout/stderr/system no visualizador de log.

Botões avançados, como a política de reinicialização do supervisor, estão disponíveis
`config/local.toml`. Defina `[supervisor.restart] mode = "never"` para desativar
reinicializações automáticas durante a depuração de incidentes ou ajuste
`max_restarts`/`backoff_ms` (por meio do arquivo de configuração ou dos sinalizadores CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) para controlar a nova tentativa
comportamento.A aplicação de alterações reconstrói o supervisor, reinicia quaisquer pares em execução e grava as substituições em
`config/local.toml`. A mesclagem de configuração preserva chaves não relacionadas para que usuários avançados possam manter
ajustes manuais junto com os valores gerenciados pelo MOCHI.

## Instantâneos e limpeza/regênese

A caixa de diálogo **Manutenção** expõe duas operações de segurança:

- **Exportar instantâneo** — copia armazenamento/configuração/logs de pares e o manifesto de gênese atual para
  `snapshots/<label>` na raiz de dados ativa. As etiquetas são higienizadas automaticamente.
- **Restaurar snapshot** — reidrata o armazenamento peer, raízes de snapshot, configurações, logs e a gênese
  manifesto de um pacote existente. `Supervisor::restore_snapshot` aceita um caminho absoluto ou
  o nome da pasta `snapshots/<label>` higienizada; a IU reflete esse fluxo, então Manutenção → Restaurar
  pode reproduzir pacotes de evidências sem tocar nos arquivos manualmente.
- **Wipe & re-genesis** — interrompe a execução de peers, remove diretórios de armazenamento, regenera genesis via
  Kagami e reinicia os peers quando a limpeza for concluída.

Ambos os fluxos são cobertos por testes de regressão (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) para garantir saídas determinísticas.

## Registros e fluxos

O painel expõe dados/métricas rapidamente:

- **Logs** — segue mensagens `irohad` stdout/stderr/ciclo de vida do sistema. Alterne os canais em Configurações.
- **Blocos/Eventos** — fluxos gerenciados se reconectam automaticamente com espera exponencial e quadros de anotação
  com resumos decodificados em Norito.
- **Status** — pesquisa `/status` e renderiza minigráficos para profundidade da fila, taxa de transferência e latência.
- **Prontidão de inicialização** — depois de pressionar **Iniciar** (ponto único ou todos os pares), o MOCHI testa
  `/status` com backoff limitado; o banner informa quando cada par fica pronto (com o observado
  profundidade da fila) ou apresenta o erro Torii se o tempo de prontidão expirar.

As guias do explorador e compositor de estado fornecem acesso rápido a contas, ativos, pares e recursos comuns.
instruções sem sair da UI. A visualização Peers espelha a consulta `FindPeers` para que você possa confirmar
quais chaves públicas estão atualmente registradas no conjunto de validadores antes de executar os testes de integração.

Use o botão **Gerenciar cofre de assinatura** da barra de ferramentas do compositor para importar ou editar autoridades de assinatura. O
caixa de diálogo grava entradas na raiz da rede ativa (`<data_root>/<profile>/signers.json`) e salva
as chaves do cofre ficam imediatamente disponíveis para visualizações e envios de transações. Quando o cofre estiver
vazio, o compositor recorre às chaves de desenvolvimento incluídas para que os fluxos de trabalho locais continuem funcionando.
Os formulários agora cobrem mint/burn/transfer (incluindo recebimento implícito), domínio/conta/definição de ativo
registro, políticas de admissão de conta, propostas multisig, manifestos do Space Directory (AXT/AMX),
Manifestos de pin SoraFS e ações de governança, como concessão ou revogação de funções tão comuns
as tarefas de criação de roteiros podem ser ensaiadas sem cargas úteis Norito escritas à mão.

## Limpeza e solução de problemas- Pare o aplicativo para encerrar pares supervisionados.
- Remova a raiz de dados (`rm -rf <data_root>`) para redefinir todo o estado.
- Se os locais Kagami ou irohad mudarem, atualize as variáveis de ambiente ou execute novamente o MOCHI com o
  sinalizadores CLI apropriados; a caixa de diálogo Configurações persistirá novos caminhos na próxima aplicação.

Para verificação de automação adicional `mochi/mochi-core/tests` (testes de ciclo de vida do supervisor) e
`mochi/mochi-integration` para cenários Torii simulados. Para enviar pacotes ou conectar o
desktop em pipelines de CI, consulte o guia {doc}`mochi/packaging`.

## Portão de teste local

Execute `ci/check_mochi.sh` antes de enviar patches para que o portão CI compartilhado exercite todos os três MOCHI
caixas:

```bash
./ci/check_mochi.sh
```

O auxiliar executa `cargo check`/`cargo test` para `mochi-core`, `mochi-ui-egui` e
`mochi-integration`, que captura desvios de fixtures (capturas de blocos/eventos canônicos) e chicote egui
regressões de uma só vez. Se o script relatar fixtures obsoletos, execute novamente os testes de regeneração ignorados,
por exemplo:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Executar novamente o portão após a regeneração garante que os bytes atualizados permaneçam consistentes antes do push.