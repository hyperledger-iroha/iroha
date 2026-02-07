---
lang: pt
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Captura de desempenho SM e plano de linha de base

Status: Elaborado — 18/05/2025  
Proprietários: Performance WG (líder), Infra Ops (programação de laboratório), QA Guild (CI gating)  
Tarefas de roteiro relacionadas: SM-4c.1a/b, SM-5a.3b, captura entre dispositivos FASTPQ Stage 7

### 1. Objetivos
1. Registre as medianas do Neoverse em `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. As linhas de base atuais são exportadas da captura `neoverse-proxy-macos` sob `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (rótulo de CPU `neoverse-proxy-macos`) com a tolerância de comparação SM3 ampliada para 0,70 para macOS/Linux aarch64. Quando o tempo bare-metal for aberto, execute novamente `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` no host Neoverse e promova as medianas agregadas nas linhas de base.  
2. Reúna medianas x86_64 correspondentes para que `ci/check_sm_perf.sh` possa proteger ambas as classes de host.  
3. Publicar um procedimento de captura repetível (comandos, layout do artefato, revisores) para que futuros desempenhos não dependam do conhecimento tribal.

### 2. Disponibilidade de hardware
Somente hosts Apple Silicon (macOS arm64) podem ser acessados no espaço de trabalho atual. A captura `neoverse-proxy-macos` é exportada como a linha de base provisória do Linux, mas a captura de medianas bare-metal Neoverse ou x86_64 ainda requer que o hardware de laboratório compartilhado rastreado em `INFRA-2751` seja executado pelo Performance WG assim que a janela do laboratório for aberta. As janelas de captura restantes agora são reservadas e rastreadas na árvore de artefatos:

- Neoverse N2 bare-metal (rack B de Tóquio) reservado para 12/03/2026. Os operadores reutilizarão os comandos da Seção 3 e armazenarão artefatos em `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- x86_64 Xeon (rack D de Zurique) reservado para 19/03/2026 com SMT desativado para reduzir ruído; os artefatos pousarão sob `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- Depois que ambas as execuções chegarem, promova medianas nos JSONs de linha de base e habilite a porta CI em `ci/check_sm_perf.sh` (data de mudança prevista: 25/03/2026).

Até essas datas, apenas as linhas de base do macOS arm64 podem ser atualizadas localmente.### 3. Procedimento de Captura
1. **Sincronizar conjuntos de ferramentas**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Gerar matriz de captura** (por host)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   O auxiliar agora escreve `capture_commands.sh` e `capture_plan.json` no diretório de destino. O script configura caminhos de captura `raw/*.json` por modo para que os técnicos de laboratório possam agrupar as execuções de forma determinística.
3. **Execute capturas**  
   Execute cada comando de `capture_commands.sh` (ou execute o equivalente manualmente), garantindo que cada modo emita um blob JSON estruturado por meio de `--capture-json`. Sempre forneça um rótulo de host via `--cpu-label "<model/bin>"` (ou `SM_PERF_CPU_LABEL=<label>`) para que os metadados de captura e as linhas de base subsequentes registrem o hardware exato que produziu as medianas. O auxiliar já fornece o caminho apropriado; para execuções manuais, o padrão é:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Validar resultados**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Certifique-se de que a variação permaneça dentro de ±3% entre as execuções. Caso contrário, execute novamente o modo afetado e anote a nova tentativa no log.
5. **Promova medianas**  
   Use `scripts/sm_perf_aggregate.py` para calcular medianas e copiá-las nos arquivos JSON de linha de base:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   Os grupos auxiliares capturados por `metadata.mode`, validam que cada conjunto compartilha o
   mesmo `{target_arch, target_os}` triplo e emite um resumo JSON com uma entrada
   por modo. As medianas que deveriam aparecer nos arquivos de linha de base estão sob
   `modes.<mode>.benchmarks`, enquanto o bloco `statistics` que acompanha registra
   a lista completa da amostra, mín/máx, média e desvio padrão da população para revisores e CI.
   Depois que o arquivo agregado existir, você poderá gravar automaticamente os JSONs de linha de base (com
   o mapa de tolerância padrão) via:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   Substitua `--mode` para restringir a um subconjunto ou `--cpu-label` para fixar o
   nome da CPU gravado quando a fonte agregada o omite.
   Assim que ambos os hosts por arquitetura terminarem, atualize:
   -`sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   -`sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (novo)

   Os arquivos `aarch64_unknown_linux_gnu_*` agora refletem o `m3-pro-native`
   capturar (etiqueta da CPU e notas de metadados preservadas) para que `scripts/sm_perf.sh` possa
   detecta automaticamente hosts aarch64-unknown-linux-gnu sem sinalizadores manuais. Quando o
   A execução do laboratório bare-metal é concluída, execute novamente `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   com as novas capturas para sobrescrever as medianas provisórias e carimbar o real
   rótulo de host.

   > Referência: a captura Apple Silicon de julho de 2025 (rótulo de CPU `m3-pro-local`) é
   > arquivado em `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > Espelhe esse layout ao publicar os artefatos Neoverse/x86 para que os revisores
   > pode diferenciar os resultados brutos/agregados de forma consistente.

### 4. Layout e aprovação do artefato
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` registra o hash do comando, revisão do git, operador e quaisquer anomalias.
- Arquivos JSON agregados são alimentados diretamente nas atualizações de linha de base e são anexados à análise de desempenho em `docs/source/crypto/sm_perf_baseline_comparison.md`.
- O QA Guild analisa os artefatos antes da mudança das linhas de base e assina `status.md` na seção Desempenho.### 5. Cronograma de controle de CI
| Data | Marco | Ação |
|------|-----------|--------|
| 12/07/2025 | Capturas de Neoverse concluídas | Atualize os arquivos JSON `sm_perf_baseline_aarch64_*`, execute `ci/check_sm_perf.sh` localmente, abra PR com artefatos anexados. |
| 24/07/2025 | Capturas x86_64 concluídas | Adicione novos arquivos de linha de base + gate em `ci/check_sm_perf.sh`; garantir que as faixas CI de arco cruzado os consumam. |
| 27/07/2025 | Aplicação de CI | Ative o fluxo de trabalho `sm-perf-gate` para execução em ambas as classes de host; as mesclagens falharão se as regressões excederem as tolerâncias configuradas. |

### 6. Dependências e comunicação
- Coordenar alterações de acesso ao laboratório via `infra-ops@iroha.tech`.  
- O GT de desempenho publica atualizações diárias no canal `#perf-lab` enquanto as capturas são executadas.  
- O QA Guild prepara o diferencial de comparação (`scripts/sm_perf_compare.py`) para que os revisores possam visualizar os deltas.  
- Depois que as linhas de base forem mescladas, atualize `roadmap.md` (SM-4c.1a/b, SM-5a.3b) e `status.md` com notas de conclusão de captura.

Com esse plano, o trabalho de aceleração do SM ganha medianas reproduzíveis, controle de CI e uma trilha de evidências rastreáveis, satisfazendo o item de ação “reservar janelas de laboratório e capturar medianas”.

### 7. Portão CI e fumaça local

- `ci/check_sm_perf.sh` é o ponto de entrada canônico do CI. Ele paga `scripts/sm_perf.sh` para cada modo em `SM_PERF_MODES` (o padrão é `scalar auto neon-force`) e define `CARGO_NET_OFFLINE=true` para que os bancos sejam executados de forma determinística nas imagens de CI.  
- `.github/workflows/sm-neon-check.yml` agora chama o portão no executor arm64 do macOS para que cada solicitação pull exercite o trio escalar/automático/neon-force por meio do mesmo auxiliar usado localmente; a via complementar Linux/Neoverse será conectada assim que as capturas x86_64 pousarem e as linhas de base do proxy Neoverse forem atualizadas com a execução bare-metal.  
- Os operadores podem substituir a lista de modos localmente: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` reduz a execução para uma única passagem para um teste rápido de fumaça, enquanto argumentos adicionais (por exemplo, `--tolerance 0.20`) são encaminhados diretamente para `scripts/sm_perf.sh`.  
- `make check-sm-perf` agora envolve o portão para conveniência do desenvolvedor; Os trabalhos de CI podem invocar o script diretamente enquanto os desenvolvedores do macOS aproveitam o destino make.  
- Assim que as linhas de base Neoverse/x86_64 chegarem, o mesmo script selecionará o JSON apropriado por meio da lógica de detecção automática de host já presente em `scripts/sm_perf.sh`, portanto, nenhuma fiação extra será necessária nos fluxos de trabalho além de definir a lista de modos desejada por pool de hosts.

### 8. Auxiliar de atualização trimestral- Execute `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` para criar um diretório com carimbo de um quarto, como `artifacts/sm_perf/2026-Q1/<label>/`. O auxiliar envolve `scripts/sm_perf_capture_helper.sh --matrix` e emite `capture_commands.sh`, `capture_plan.json` e `quarterly_plan.json` (proprietário + metadados trimestrais) para que os operadores do laboratório possam agendar execuções sem planos escritos à mão.
- Execute o `capture_commands.sh` gerado no host de destino, agregue as saídas brutas com `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` e promova as medianas nos JSONs de linha de base por meio de `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. Execute novamente `ci/check_sm_perf.sh` para confirmar se as tolerâncias permanecem verdes.
- Quando o hardware ou os conjuntos de ferramentas mudarem, atualize as tolerâncias/notas de comparação em `docs/source/crypto/sm_perf_baseline_comparison.md`, aumente as tolerâncias `ci/check_sm_perf.sh` se as novas medianas se estabilizarem e alinhe quaisquer limites de painel/alerta com as novas linhas de base para que os alarmes operacionais permaneçam significativos.
- Confirmar `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh` e o JSON agregado junto com as atualizações de linha de base; anexe os mesmos artefatos às atualizações de status/roteiro para rastreabilidade.