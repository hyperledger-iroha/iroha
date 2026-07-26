---
lang: pt
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2026-01-03T18:07:57.206662+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Monitor Iroha

O monitor Iroha refatorado combina uma interface de usuário de terminal leve com animação
festival de arte ASCII e o tradicional tema Etenraku.  Ele se concentra em dois
fluxos de trabalho simples:

- **Modo Spawn-lite** – inicia stubs de status/métricas efêmeros que imitam pares.
- **Modo anexar** – aponte o monitor para endpoints HTTP Torii existentes.

A UI renderiza três regiões em cada atualização:

1. **Torii cabeçalho do horizonte** – portão torii animado, Monte. Fuji, ondas de koi e estrela
   campo que rola em sincronia com a cadência de atualização.
2. **Faixa de resumo** – blocos/transações/gas agregados mais tempo de atualização.
3. **Mesa de pares e sussurros de festival** – fileiras de pares à esquerda, evento rotativo
   log à direita que captura avisos (tempos limite, cargas superdimensionadas, etc.).
4. **Tendência de gás opcional** – habilite `--show-gas-trend` para anexar um minigráfico
   resumindo o uso total de gás em todos os pares.

Novidade neste refatorador:

- Cena ASCII animada em estilo japonês com koi, torii e lanternas.
- Superfície de comando simplificada (`--spawn-lite`, `--attach`, `--interval`).
- Banner de introdução com reprodução de áudio opcional do tema gagaku (MIDI externo
  player ou o soft synth integrado quando a plataforma/pilha de áudio suporta).
- Sinalizadores `--no-theme` / `--no-audio` para CI ou corridas rápidas de fumaça.
- Coluna de “humor” por peer mostrando o último aviso, tempo de confirmação ou tempo de atividade.

## Início rápido

Crie o monitor e execute-o nos pares em stub:

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

Anexe aos endpoints Torii existentes:

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

Invocação compatível com CI (pular animação e áudio de introdução):

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### Sinalizadores CLI

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## Introdução ao tema

Por padrão, a inicialização reproduz uma curta animação ASCII enquanto a pontuação Etenraku
começa.  Ordem de seleção de áudio:

1. Se `--midi-player` for fornecido, gere o MIDI de demonstração (ou use `--midi-file`)
   e gerar o comando.
2. Caso contrário, em macOS/Windows (ou Linux com `--features iroha_monitor/linux-builtin-synth`)
   renderize a partitura com o sintetizador suave gagaku integrado (sem áudio externo
   ativos necessários).
3. Se o áudio estiver desativado ou a inicialização falhar, a introdução ainda imprimirá o
   animação e entra imediatamente na TUI.

O sintetizador com tecnologia CPAL é ativado automaticamente no macOS e no Windows. No Linux é
opt-in para evitar a perda de cabeçalhos ALSA/Pulse durante a construção do espaço de trabalho; ative-o
com `--features iroha_monitor/linux-builtin-synth` se o seu sistema fornecer um
pilha de áudio funcionando.

Use `--no-theme` ou `--no-audio` ao executar em CI ou shells headless.

O sintetizador suave agora segue o arranjo capturado no design do sintetizador *MIDI em
Rust.pdf*: hichiriki e ryūteki compartilham uma melodia heterofônica enquanto o shō
fornece as almofadas aitake descritas no documento.  Os dados da nota cronometrada permanecem
em `etenraku.rs`; ele alimenta o retorno de chamada CPAL e o MIDI de demonstração gerado.
Quando a saída de áudio não está disponível, o monitor pula a reprodução, mas ainda renderiza
a animação ASCII.

## Visão geral da IU- **Arte do cabeçalho** – gerado cada quadro por `AsciiAnimator`; koi, lanternas torii,
  e as ondas derivam para dar movimento contínuo.
- **Faixa de resumo** – mostra peers on-line, contagem de peers relatada, totais de blocos,
  totais de blocos não vazios, aprovações/rejeições de transações, uso de gás e taxa de atualização.
- **Tabela de pares** – colunas para alias/endpoint, blocos, transações, tamanho da fila,
  uso de gás, latência e uma dica de “humor” (avisos, tempo de confirmação, tempo de atividade).
- **Sussurros de festival** – registro contínuo de avisos (erros de conexão, carga útil
  limitar violações, pontos finais lentos).  As mensagens são invertidas (as mais recentes no topo).

Atalhos de teclado:

- `n` / Direita / Baixo – move o foco para o próximo par.
- `p` / Esquerda / Cima – move o foco para o par anterior.
- `q` / Esc / Ctrl-C – saia e restaure o terminal.

O monitor usa crossterm + ratatui com um buffer de tela alternativo; ao sair
restaura o cursor e limpa a tela.

## Testes de fumaça

A caixa envia testes de integração que exercitam ambos os modos e os limites HTTP:

-`spawn_lite_smoke_renders_frames`
-`attach_mode_with_stubs_runs_cleanly`
-`invalid_endpoint_surfaces_warning`
-`status_limit_warning_is_rendered`
-`attach_mode_with_slow_peer_renders_multiple_frames`

Execute apenas os testes do monitor:

```bash
cargo test -p iroha_monitor -- --nocapture
```

O espaço de trabalho possui testes de integração mais pesados (`cargo test --workspace`). Correndo
os testes do monitor separadamente ainda são úteis para validação rápida quando você faz
não precisa do conjunto completo.

## Atualizando capturas de tela

A demonstração do docs agora se concentra no horizonte do torii e na tabela de pares.  Para atualizar o
ativos, execute:

```bash
make monitor-screenshots
```

Isso envolve `scripts/iroha_monitor_demo.sh` (modo spawn-lite, seed/viewport fixo,
sem introdução/áudio, paleta de amanhecer, velocidade de arte 1, limite sem cabeça 24) e escreve o
Quadros SVG/ANSI mais `manifest.json` e `checksums.json` em
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
envolve ambos os protetores CI (`ci/check_iroha_monitor_assets.sh` e
`ci/check_iroha_monitor_screenshots.sh`) para gerar hashes, campos de manifesto,
e as somas de verificação permanecem sincronizadas; a verificação da captura de tela também é enviada como
`python3 scripts/check_iroha_monitor_screenshots.py`. Passe `--no-fallback` para
o script de demonstração se desejar que a captura falhe em vez de voltar ao
quadros cozidos quando a saída do monitor está vazia; quando o fallback é usado o bruto
Os arquivos `.ans` são reescritos com os quadros preparados para que o manifesto/somas de verificação permaneçam
determinístico.

## Capturas de tela determinísticas

Os snapshots enviados residem em `docs/source/images/iroha_monitor_demo/`:

![visão geral do monitor](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![monitorar pipeline](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

Reproduza-os com uma viewport/semente fixa:

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

O auxiliar de captura corrige `LANG`/`LC_ALL`/`TERM`, encaminha
`IROHA_MONITOR_DEMO_SEED`, silencia o áudio e fixa o tema/velocidade da arte para que o
os quadros são renderizados de forma idêntica em todas as plataformas. Escreve `manifest.json` (gerador
hashes + tamanhos) e `checksums.json` (resumos SHA-256) em
`docs/source/images/iroha_monitor_demo/`; CI é executado
`ci/check_iroha_monitor_assets.sh` e `ci/check_iroha_monitor_screenshots.sh`
falhar quando os ativos se desviarem dos manifestos registrados.

## Solução de problemas- **Sem saída de áudio** – o monitor volta à reprodução sem som e continua.
- **O fallback sem cabeça sai mais cedo** – o monitor limita as corridas sem cabeça para alguns
  dúzia de quadros (cerca de 12 segundos no intervalo padrão) quando não é possível alternar
  o terminal em modo bruto; passe `--headless-max-frames 0` para mantê-lo funcionando
  indefinidamente.
- **Cargas úteis de status superdimensionadas** – a coluna de humor do colega e o registro do festival
  mostre `body exceeds …` com o limite configurado (`128 KiB`).
- **Peers lentos** – o log de eventos registra avisos de tempo limite; concentre esse ponto em
  destaque a linha.

Aproveite o horizonte do festival!  Contribuições para motivos ASCII adicionais ou
painéis de métricas são bem-vindos – mantenha-os determinísticos para que os clusters renderizem o mesmo
quadro a quadro, independentemente do terminal.