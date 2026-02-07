---
lang: pt
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2026-01-03T18:07:57.000591+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guia de solução de problemas MOCHI

Use este runbook quando clusters MOCHI locais se recusarem a iniciar, ficarem presos em um
reinicie o loop ou interrompa o streaming de atualizações de bloco/evento/status. Ele estende o
item do roteiro “Documentação e implementação”, transformando os comportamentos do supervisor em
`mochi-core` em etapas concretas de recuperação.

## 1. Lista de verificação do socorrista

1. Capture a raiz de dados que o MOCHI está usando. O padrão segue
   `$TMPDIR/mochi/<profile-slug>`; caminhos personalizados aparecem na barra de título da IU e
   através de `cargo run -p mochi-ui-egui -- --data-root ...`.
2. Execute `./ci/check_mochi.sh` na raiz da área de trabalho. Isso valida o núcleo,
   UI e caixas de integração antes de começar a modificar as configurações.
3. Anote a predefinição (`single-peer` ou `four-peer-bft`). A topologia gerada
   determina quantas pastas/logs de pares você deve esperar na raiz de dados.

## 2. Colete registros e evidências de telemetria

`NetworkPaths::ensure` (consulte `mochi/mochi-core/src/config.rs`) cria um ambiente estável
disposição:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

Siga estas etapas antes de fazer alterações:

- Use a guia **Logs** ou abra `logs/<alias>.log` diretamente para capturar o último
  200 linhas para cada par. O supervisor segue os canais stdout/stderr/system
  via `PeerLogStream`, portanto, esses arquivos correspondem à saída da UI.
- Exporte um instantâneo via **Manutenção → Exportar instantâneo** (ou ligue
  `Supervisor::export_snapshot`). O snapshot agrupa armazenamento, configurações e
  efetua login em `snapshots/<timestamp>-<label>/`.
- Se o problema envolver widgets de stream, copie o `ManagedBlockStream`,
  Indicadores de integridade `ManagedEventStream` e `ManagedStatusStream` do
  Painel. A IU apresenta a última tentativa de reconexão e o motivo do erro; agarrar
  uma captura de tela para o registro do incidente.

## 3. Resolvendo problemas de inicialização entre pares

A maioria das falhas de lançamento de pares se enquadra em três grupos:

### Binários ausentes ou substituições incorretas

`SupervisorBuilder` desembolsa para `irohad`, `kagami` e (futuro) `iroha_cli`.
Se a IU relatar “falha ao gerar processo” ou “permissão negada”, aponte MOCHI
em binários em bom estado:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

Você pode definir `MOCHI_IROHAD`, `MOCHI_KAGAMI` e `MOCHI_IROHA_CLI` para evitar
digitando os sinalizadores repetidamente. Ao depurar compilações de pacotes configuráveis, compare o
`BundleConfig` em `mochi/mochi-ui-egui/src/config/` em relação aos caminhos em
`target/mochi-bundle`.

### Colisões portuárias

`PortAllocator` testa a interface de loopback antes de gravar configurações. Se você ver
`failed to allocate Torii port` ou `failed to allocate P2P port`, outro
o processo já está escutando no intervalo padrão (8080/1337). Reiniciar MOCHI
com bases explícitas:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

O construtor distribuirá portas sequenciais a partir dessas bases, então reserve um intervalo
dimensionado para sua predefinição (pares `peer_count` → portas `peer_count` por transporte).

### Gênesis e corrupção de armazenamentoSe Kagami sair antes de emitir um manifesto, os peers travarão imediatamente. Verifique
`genesis/*.json`/`.toml` dentro da raiz de dados. Execute novamente com
`--kagami /path/to/kagami` ou aponte a caixa de diálogo **Configurações** para o binário direito.
Para corrupção de armazenamento, use **Wipe & re-genesis** da seção Manutenção
botão (abordado abaixo) em vez de excluir pastas manualmente; ele recria o
diretórios pares e raízes de instantâneos antes de reiniciar processos.

### Ajustando reinicializações automáticas

`[supervisor.restart]` em `config/local.toml` (ou os sinalizadores CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) controlam a frequência com que o
o supervisor tenta novamente os pares com falha. Defina `mode = "never"` quando precisar que a UI
revele a primeira falha imediatamente ou encurte `max_restarts`/`backoff_ms`
para diminuir a janela de novas tentativas para trabalhos de CI que devem falhar rapidamente.

## 4. Redefinindo pares com segurança

1. Pare os peers afetados no Dashboard ou saia da UI. O supervisor
   recusa-se a limpar o armazenamento enquanto um par está em execução (`PeerHandle::wipe_storage`
   retorna `PeerStillRunning`).
2. Navegue até **Manutenção → Limpeza e regeneração**. MOCHI irá:
   - excluir `peers/<alias>/storage`;
   - execute novamente Kagami para reconstruir configurações/genesis em `genesis/`; e
   - reinicie os peers com as substituições de CLI/ambiente preservadas.
3. Se você precisar fazer isso manualmente:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Depois reinicie o MOCHI para que `NetworkPaths::ensure` recrie a árvore.

Sempre arquive a pasta `snapshots/<timestamp>` antes de limpar, mesmo em local
desenvolvimento - esses pacotes capturam os logs e configurações `irohad` precisos necessários
para reproduzir bugs.

### 4.1 Restaurando a partir de snapshots

Quando um experimento corrompe o armazenamento ou você precisa reproduzir um estado em bom estado, use o Manutenção
botão **Restaurar instantâneo** da caixa de diálogo (ou chame `Supervisor::restore_snapshot`) em vez de copiar
diretórios manualmente. Forneça um caminho absoluto para o pacote ou o nome da pasta higienizada
sob `snapshots/`. O supervisor irá:

1. interrompa qualquer peer em execução;
2. verifique se o `metadata.json` do instantâneo corresponde ao `chain_id` atual e à contagem de pares;
3. copie `peers/<alias>/{storage,snapshot,config.toml,latest.log}` de volta para o perfil ativo; e
4. restaure `genesis/genesis.json` antes de reiniciar os peers, caso eles estivessem em execução anteriormente.

Se o instantâneo foi criado para uma predefinição ou identificador de cadeia diferente, a chamada de restauração retornará um
`SupervisorError::Config` para que você possa pegar um pacote correspondente em vez de misturar artefatos silenciosamente.
Mantenha pelo menos um instantâneo atualizado por predefinição para acelerar os exercícios de recuperação.

## 5. Reparando fluxos de bloco/evento/status- **Stream travado, mas pares íntegros.** Verifique os painéis **Eventos**/**Bloqueios**
  para barras de status vermelhas. Clique em “Parar” e depois em “Iniciar” para forçar o fluxo gerenciado a
  assinar novamente; o supervisor registra cada tentativa de reconexão (com alias de peer e
  erro) para que você possa confirmar os estágios de espera.
- **Sobreposição de status desatualizada.** `ManagedStatusStream` pesquisa `/status` a cada
  dois segundos e marca os dados como obsoletos após `STATUS_POLL_INTERVAL *
  STATUS_STALE_MULTIPLIER` (padrão seis segundos). Se o selo permanecer vermelho, verifique
  `torii_status_url` na configuração do peer e certifique-se de que o gateway ou VPN não esteja
  bloqueando conexões de loopback.
- **Falhas de decodificação de eventos.** A IU imprime o estágio de decodificação (bytes brutos,
  decodificação `BlockSummary` ou Norito) e o hash da transação ofensiva. Exportar
  o evento através do botão da área de transferência para que você possa reproduzir a decodificação nos testes
  (`mochi-core` expõe construtores auxiliares em
  `mochi/mochi-core/src/torii.rs`).

Quando os fluxos falharem repetidamente, atualize o problema com o alias exato do par e
string de erro (`ToriiErrorKind`) para que os marcos de telemetria do roteiro permaneçam vinculados
a provas concretas.