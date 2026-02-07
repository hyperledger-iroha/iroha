---
lang: pt
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2026-01-03T18:08:01.870300+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plano de aceleração Metal e NEON (Swift & Rust)

Este documento captura o plano compartilhado para habilitar hardware determinístico
aceleração (Metal GPU + NEON/Accelerate SIMD + integração StrongBox) em
o espaço de trabalho Rust e o Swift SDK. Ele aborda os itens do roteiro rastreados
em **Fluxo de trabalho de aceleração de hardware (macOS/iOS)** e fornece uma transferência
artefato para a equipe Rust IVM, proprietários de pontes Swift e ferramentas de telemetria.

> Última atualização: 12/01/2026  
> Proprietários: IVM Performance TL, líder do Swift SDK

## Metas

1. Reutilize os kernels da GPU Rust (Poseidon/BN254/CRC64) em hardware Apple via Metal
   computa shaders com paridade determinística em relação aos caminhos da CPU.
2. Exponha os alternadores de aceleração (`AccelerationConfig`) de ponta a ponta para aplicativos Swift
   pode optar por Metal/NEON/StrongBox preservando as garantias de ABI/paridade.
3. Instrumente painéis CI + para exibir dados e sinalização de paridade/benchmark
   regressões entre caminhos de CPU versus GPU/SIMD.
4. Compartilhe lições de StrongBox/enclave seguro entre Android (AND2) e Swift
   (IOS4) para manter os fluxos de assinatura alinhados deterministicamente.

**Atualização (atualização CRC64 + Estágio 1):** Os auxiliares de GPU CRC64 agora estão conectados ao `norito::core::hardware_crc64` com um corte padrão de 192 KB (substituição via `NORITO_GPU_CRC64_MIN_BYTES` ou caminho auxiliar explícito `NORITO_CRC64_GPU_LIB`), mantendo SIMD e substitutos escalares. Os cutovers JSON Stage-1 foram reavaliados (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), mantendo o cutover escalar em 4KiB e alinhando o padrão da GPU Stage-1 a 192KiB (`NORITO_STAGE1_GPU_MIN_BYTES`) para que pequenos documentos permaneçam na CPU e grandes cargas úteis amortizem os custos de inicialização da GPU.

## Entregáveis e Proprietários

| Marco | Entregável | Proprietário(s) | Alvo |
|-----------|-------------|----------|--------|
| Ferrugem WP2-A/B | Interfaces de shader de metal espelhando kernels CUDA | IVM Perf TL | Fevereiro de 2026 |
| Ferrugem WP2-C | Testes de paridade de metal BN254 e pista CI | IVM Perf TL | 2º trimestre de 2026 |
| Rápido IOS6 | Bridge alterna com fio (`connect_norito_set_acceleration_config`) + SDK API + amostras | Proprietários da ponte Swift | Concluído (janeiro de 2026) |
| Rápido IOS5 | Exemplos de aplicativos/documentos demonstrando o uso de configuração | Swift DXTL | 2º trimestre de 2026 |
| Telemetria | Feeds de painel com paridade de aceleração + métricas de benchmark | Programa Swift PM / Telemetria | Dados piloto do 2º trimestre de 2026 |
| CI | Arnês de fumaça XCFramework exercitando CPU vs Metal/NEON no pool de dispositivos | Líder de controle de qualidade Swift | 2º trimestre de 2026 |
| Caixa-forte | Testes de paridade de assinatura apoiados por hardware (vetores compartilhados) | Segurança Android Crypto TL / Swift | 3º trimestre de 2026 |

## Interfaces e contratos de API### Ferrugem (`ivm::AccelerationConfig`)
- Manter os campos existentes (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, limites).
- Adicione aquecimento explícito do Metal para evitar latência no primeiro uso (Rust #15875).
- Fornece APIs de paridade retornando status/diagnósticos para dashboards:
  - por exemplo `ivm::vector::metal_status()` -> {habilitado, paridade, last_error}.
- Métricas de benchmarking de saída (tempos de árvore Merkle, taxa de transferência CRC) via
  ganchos de telemetria para `ci/xcode-swift-parity`.
- Metal host agora carrega o `fastpq.metallib` compilado, despacha FFT/IFFT/LDE
  e kernels Poseidon, e recorre à implementação da CPU sempre que o
  metallib ou fila de dispositivos não estão disponíveis.

###C FFI (`connect_norito_bridge`)
- Nova estrutura `connect_norito_acceleration_config` (concluída).
- A cobertura do getter agora inclui `connect_norito_get_acceleration_config` (somente configuração) e `connect_norito_get_acceleration_state` (config + paridade) para espelhar o setter.
- Layout da estrutura do documento nos comentários do cabeçalho para consumidores SPM/CocoaPods.

### Rápido (`AccelerationSettings`)
- Padrões: Metal habilitado, CUDA desabilitado, limites nulos (herdar).
- Valores negativos ignorados; `apply()` invocado automaticamente por `IrohaSDK`.
- `AccelerationSettings.runtimeState()` agora apresenta o `connect_norito_get_acceleration_state`
  carga útil (configuração + status de paridade Metal/CUDA) para que os painéis Swift emitam a mesma telemetria
  como ferrugem (`supported/configured/available/parity`). O auxiliar retorna `nil` quando o
  bridge está ausente para manter os testes portáteis.
- `AccelerationBackendStatus.lastError` copia o motivo da desativação/erro de
  `connect_norito_get_acceleration_state` e libera o buffer nativo quando a string é
  materializado para que os painéis de paridade móvel possam anotar por que o Metal/CUDA foi desativado em
  cada hospedeiro.
-`AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  testes sob `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) agora
  resolve manifestos do operador na mesma ordem de prioridade da demonstração Norito: honra
  `NORITO_ACCEL_CONFIG_PATH`, pesquisa agrupada `acceleration.{json,toml}` / `client.{json,toml}`,
  registre a fonte escolhida e volte aos padrões. Os aplicativos não precisam mais de carregadores personalizados para
  espelhe a superfície Rust `iroha_config`.
- Atualize aplicativos de amostra e README para mostrar alternâncias e integração de telemetria.

### Telemetria (Dashboards + Exportadores)
- Feed de paridade (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {habilitado, paridade, perf_delta_pct}.
  - Aceite a comparação entre CPU e GPU de linha de base `perf_delta_pct`.
  -`acceleration.metal.disable_reason` espelhos `AccelerationBackendStatus.lastError`
    então a automação Swift pode sinalizar GPUs desabilitadas com a mesma fidelidade que o Rust
    painéis.
- Feed de CI (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> Duplo.
- Os exportadores devem obter métricas de benchmarks Rust ou execuções de CI (por exemplo, executar
  Microbancada de metal/CPU como parte de `ci/xcode-swift-parity`).### Botões de configuração e padrões (WP6-C)
- Padrões `AccelerationConfig`: `enable_metal = true` em compilações do macOS, `enable_cuda = true` quando o recurso CUDA é compilado, `max_gpus = None` (sem limite). O wrapper Swift `AccelerationSettings` herda os mesmos padrões por meio de `connect_norito_set_acceleration_config`.
- Heurística Merkle Norito (GPU vs CPU): `merkle_min_leaves_gpu = 8192` permite hash de GPU para árvores com ≥8192 folhas; substituições de back-end (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) são padronizadas para o mesmo limite, a menos que sejam explicitamente definidas.
- Heurística de preferência de CPU (SHA2 ISA presente): tanto em AArch64 (ARMv8 SHA2) quanto em x86/x86_64 (SHA-NI) o caminho de CPU permanece preferencial até as folhas `prefer_cpu_sha2_max_leaves_* = 32_768`; acima disso, o limite da GPU se aplica. Esses valores são configuráveis ​​via `AccelerationConfig` e devem ser ajustados somente com evidências de benchmark.

## Estratégia de teste

1. **Testes de paridade de unidade (Rust)**: garanta que os kernels Metal correspondam às saídas da CPU para
   vetores determinísticos; executado em `cargo test -p ivm --features metal`.
   `crates/fastpq_prover/src/metal.rs` agora fornece testes de paridade somente para macOS que
   exercite FFT/IFFT/LDE e Poseidon contra a referência escalar.
2. **Arnês de fumaça rápido**: estenda o executor de teste IOS6 para executar CPU vs Metal
   codificação (Merkle/CRC64) em emuladores e dispositivos StrongBox; comparar
   resultados e status de paridade de log.
3. **CI**: atualize `norito_bridge_ios.yml` (já chama `make swift-ci`) para enviar
   métricas de aceleração para artefatos; certifique-se de que a execução confirme o Buildkite
   Metadados `ci/xcframework-smoke:<lane>:device_tag` antes de publicar alterações de chicote,
   e falhar na pista devido ao desvio de paridade/benchmark.
4. **Painéis**: novos campos agora são renderizados na saída CLI. Garantir que os exportadores produzam
   dados antes de lançar os painéis ao vivo.

## Plano WP2-A Metal Shader (Poseidon Pipelines)

O primeiro marco do WP2 cobre o trabalho de planejamento para os kernels Poseidon Metal
que refletem a implementação do CUDA. O plano divide o esforço em núcleos,
agendamento de host e preparação constante compartilhada para que o trabalho posterior possa se concentrar puramente em
implementação e testes.

### Escopo do Kernel

1. `poseidon_permute`: permuta estados independentes `state_count`. Cada tópico
   possui um `STATE_CHUNK` (4 estados) e executa todas as iterações `TOTAL_ROUNDS` usando
   constantes redondas compartilhadas por threadgroup preparadas no momento do despacho.
2. `poseidon_hash_columns`: lê o catálogo esparso `PoseidonColumnSlice` e
   executa hashing compatível com Merkle de cada coluna (correspondendo ao CPU
   Layout `PoseidonColumnBatch`). Ele usa o mesmo buffer constante de grupo de threads
   como o kernel permutado, mas faz um loop sobre `(states_per_lane * block_count)`
   saídas para que o kernel possa amortizar os envios da fila.
3. `poseidon_trace_fused`: calcula os resumos pai/folha para a tabela de rastreamento
   em uma única passagem. O kernel fundido consome `PoseidonFusedArgs` para que o host
   pode descrever regiões não contíguas e um `leaf_offset`/`parent_offset`, e
   ele compartilha todas as tabelas round/MDS com os outros kernels.

### Agendamento de comandos e contratos de host- Cada despacho do kernel é executado através do `MetalPipelines::command_queue`, que
  impõe o agendador adaptativo (alvo ~2 ms) e os controles de distribuição da fila
  exposto via `FASTPQ_METAL_QUEUE_FANOUT` e
  `FASTPQ_METAL_COLUMN_THRESHOLD`. O caminho de aquecimento em `with_metal_state`
  compila todos os três kernels Poseidon antecipadamente para que o primeiro despacho não
  pagar uma penalidade de criação de pipeline.
- O dimensionamento do grupo de threads reflete os padrões existentes do Metal FFT/LDE: o alvo é
  8.192 tópicos por envio com um limite máximo de 256 tópicos por grupo. O
  host pode reduzir o multiplicador `states_per_lane` para dispositivos de baixo consumo de energia
  discando as substituições de ambiente (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  a ser adicionado no WP2-B) sem modificar a lógica do shader.
- A preparação da coluna segue o mesmo pool de buffer duplo já usado pela FFT
  oleodutos. Os kernels Poseidon aceitam ponteiros brutos para esse buffer de teste
  e nunca toque nas alocações globais de heap, o que mantém o determinismo da memória
  alinhado com o host CUDA.

### Constantes compartilhadas

- O manifesto `PoseidonSnapshot` descrito em
  `docs/source/fastpq/poseidon_metal_shared_constants.md` agora é o canônico
  fonte para as constantes redondas e a matriz MDS. Ambos metálicos (`poseidon2.metal`)
  e os kernels CUDA (`fastpq_cuda.cu`) devem ser regenerados sempre que o manifesto
  mudanças.
- WP2-B adicionará um pequeno carregador de host que lê o manifesto em tempo de execução e
  emite o SHA-256 em telemetria (`acceleration.poseidon_constants_sha`) então
  painéis de paridade podem afirmar que as constantes do shader correspondem ao publicado
  instantâneo.
- Durante o aquecimento copiaremos as constantes `TOTAL_ROUNDS x STATE_WIDTH` em um
  `MTLBuffer` e carregue-o uma vez por dispositivo. Cada kernel então copia os dados
  na memória do grupo de threads antes de processar seu pedaço, garantindo
  ordenação mesmo quando vários buffers de comando são executados em andamento.

### Ganchos de validação

- Os testes unitários (`cargo test -p fastpq_prover --features fastpq-gpu`) crescerão
  afirmação que faz o hash das constantes do shader incorporadas e as compara com
  o SHA do manifesto antes de executar o conjunto de acessórios GPU.
- As estatísticas do kernel existentes alternam (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, telemetria de profundidade de fila) tornam-se evidências obrigatórias
  para saída do WP2: cada execução de teste deve provar que o escalonador nunca viola o
  fan-out configurado e que o kernel de rastreamento fundido mantém a fila abaixo do
  janela adaptativa.
- O arnês de fumaça Swift XCFramework e os corredores de benchmark Rust serão iniciados
  exportando `acceleration.poseidon.permute_p90_ms{cpu,metal}` para que WP2-D possa traçar
  Deltas Metal vs CPU sem reinventar novos feeds de telemetria.

## WP2-B Poseidon Manifest Loader e paridade de autoteste- `fastpq_prover::poseidon_manifest()` agora incorpora e analisa
  `artifacts/offline_poseidon/constants.ron`, calcula seu SHA-256
  (`poseidon_manifest_sha256()`) e valida o instantâneo em relação à CPU
  tabelas poseidon antes de qualquer trabalho de GPU ser executado. `build_metal_context()` registra o
  resumo durante o aquecimento para que os exportadores de telemetria possam publicar
  `acceleration.poseidon_constants_sha`.
- O analisador de manifesto rejeita tuplas de largura/taxa/contagem de rounds incompatíveis e
  garante que a matriz MDS do manifesto seja igual à implementação escalar, evitando
  desvio silencioso quando as tabelas canônicas são regeneradas.
- Adicionado `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`, que
  analisa as tabelas Poseidon incorporadas em `poseidon2.metal` e
  `fastpq_cuda.cu` e afirma que ambos os kernels serializam exatamente da mesma forma
  constantes como o manifesto. CI agora falha se alguém editar o shader/CUDA
  arquivos sem regenerar o manifesto canônico.
- Ganchos de paridade futuros (WP2-C/D) podem reutilizar `poseidon_manifest()` para preparar o
  arredondar constantes em buffers de GPU e expor o resumo via Norito
  alimentações de telemetria.

## WP2-C BN254 Tubulações metálicas e testes de paridade- **Escopo e lacuna:** Despachantes de host, chicotes de paridade e `bn254_status()` estão ativos, e `crates/fastpq_prover/metal/kernels/bn254.metal` agora implementa os primitivos Montgomery mais loops FFT/LDE sincronizados com grupos de threads. Cada despacho executa uma coluna inteira dentro de um único grupo de threads com barreiras por estágio, de modo que os kernels exercitam os manifestos preparados em paralelo. A telemetria agora está conectada e as substituições do agendador são respeitadas para que possamos ativar a implementação padrão com a mesma evidência que usamos para os kernels Cachinhos Dourados.
- **Requisitos do kernel:** ✅ reutilizar os manifestos twiddle/coset preparados, converter entradas/saídas uma vez e executar todos os estágios radix-2 dentro do grupo de threads por coluna para que não precisemos de sincronização de vários despachos. Os auxiliares Montgomery permanecem compartilhados entre FFT/LDE, portanto, apenas a geometria do loop é alterada.
- **Fiação do host:** ✅ `crates/fastpq_prover/src/metal.rs` prepara membros canônicos, preenche com zero o buffer LDE, seleciona um único grupo de threads por coluna e expõe `bn254_status()` para gate. Nenhuma alteração extra de host é necessária para telemetria.
- **Proteções de construção:** o `fastpq.metallib` envia os kernels lado a lado, portanto, o CI ainda falha rapidamente se o sombreador oscilar. Quaisquer otimizações futuras ficam atrás de portas de telemetria/recursos, em vez de opções de tempo de compilação.
- **Aparelhos de paridade:** ✅ Os testes `bn254_parity` continuam a comparar as saídas FFT/LDE da GPU com os acessórios da CPU e agora são executados ao vivo em hardware Metal; tenha em mente os testes de manifesto adulterado se novos caminhos de código do kernel aparecerem.
- **Telemetria e benchmarks:** `fastpq_metal_bench` agora emite:
  - um bloco `bn254_dispatch` que resume larguras de grupos de encadeamentos por despacho, contagens de encadeamentos lógicos e limites de pipeline para lotes de coluna única FFT/LDE; e
  - um bloco `bn254_metrics` que registra `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` para a linha de base da CPU mais qualquer back-end da GPU executado.
  O wrapper de benchmark copia ambos os mapas em cada artefato empacotado para que os painéis WP2-D ingiram latências/geometria rotuladas sem fazer engenharia reversa da matriz de operações brutas. `FASTPQ_METAL_THREADGROUP` agora também se aplica aos despachos BN254 FFT/LDE, tornando o botão utilizável para desempenho varreduras.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## Perguntas abertas (resolvidas em maio de 2027)1. **Limpeza de recursos de metal:** `warm_up_metal()` reutiliza o thread-local
   `OnceCell` e agora possui testes de idempotência/regressão
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state`/
   `warm_up_metal_is_noop_on_non_metal_targets`), portanto, transições do ciclo de vida do aplicativo
   pode chamar com segurança o caminho de aquecimento sem vazamento ou inicialização dupla.
2. **Linhas de referência de referência:** As pistas metálicas devem permanecer dentro de 20% da CPU
   linha de base para FFT/IFFT/LDE e dentro de 15% para auxiliares Poseidon CRC/Merkle;
   o alerta deve ser acionado quando `acceleration.*_perf_delta_pct > 0.20` (ou ausente)
   no feed de paridade móvel. Regressões IFFT observadas no pacote de rastreamento de 20k
   agora são bloqueados pela correção de substituição de fila observada no WP2-D.
3. **Backup do StrongBox:** Swift segue o manual de fallback do Android
   registrar falhas de atestado no runbook de suporte
   (`docs/source/sdk/swift/support_playbook.md`) e mudando automaticamente para
   o caminho documentado do software apoiado pelo HKDF com registro de auditoria; vetores de paridade
   permaneça compartilhado através dos equipamentos OA existentes.
4. **Armazenamento de telemetria:** Capturas de aceleração e provas de pool de dispositivos são
   arquivado em `configs/swift/` (por exemplo,
   `configs/swift/xcframework_device_pool_snapshot.json`) e exportadores
   deve espelhar o mesmo layout (`artifacts/swift/telemetry/acceleration/*.json`
   ou `.prom`) para que as anotações do Buildkite e os painéis do portal possam ingerir o
   alimenta sem raspagem ad-hoc.

## Próximas etapas (fevereiro de 2026)

- [x] Rust: integração de host Land Metal (`crates/fastpq_prover/src/metal.rs`) e
      expor a interface do kernel para Swift; transferência de documentos rastreada junto com o
      Notas de ponte rápidas.
- [x] Swift: expõe as configurações de aceleração no nível do SDK (concluído em janeiro de 2026).
- [x] Telemetria: `scripts/acceleration/export_prometheus.py` agora converte
      Saída `cargo xtask acceleration-state --format json` em um Prometheus
      arquivo de texto (com rótulo `--instance` opcional) para que as execuções de CI possam anexar GPU/CPU
      ativação, limites e motivos de paridade/desativação diretamente no arquivo de texto
      colecionadores sem raspagem personalizada.
- [x] Swift QA: `scripts/acceleration/acceleration_matrix.py` agrega vários
      capturas de estado de aceleração em tabelas JSON ou Markdown codificadas por dispositivo
      rótulo, dando ao chicote de fumaça uma matriz determinística “CPU vs Metal/CUDA”
      para fazer upload junto com o aplicativo de amostra fuma. A saída do Markdown espelha o
      Formato de evidência Buildkite para que os painéis possam ingerir o mesmo artefato.
- [x] Atualize status.md agora que `irohad` envia os exportadores de fila/preenchimento zero e
      os testes de validação env/config cobrem as substituições da fila Metal, então WP2-D
      telemetria + ligações têm evidências ao vivo anexadas.【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Comandos auxiliares de telemetria/exportação:

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## Referência de lançamento do WP2-D e notas vinculativas- **Captura de lançamento de 20 mil linhas:** Registrou um novo benchmark Metal vs CPU no macOS14
  (arm64, parâmetros balanceados de pista, rastreamento preenchido de 32.768 linhas, lotes de duas colunas) e
  verifiquei o pacote JSON em `fastpq_metal_bench_20k_release_macos14_arm64.json`.
  O benchmark exporta tempos por operação mais evidências de microbanco Poseidon para que
  WP2-D tem um artefato de qualidade GA vinculado à nova heurística da fila Metal. Título
  deltas (a tabela completa reside em `docs/source/benchmarks.md`):

  | Operação | Média de CPU (ms) | Média de metais (ms) | Aceleração |
  |-----------|---------------|-----------------|---------|
  | FFT (32.768 entradas) | 12.741 | 10.963 | 1,16× |
  | IFFT (32.768 entradas) | 17.499 | 25.688 | 0,68× *(regressão: distribuição da fila acelerada para manter o determinismo; precisa de ajuste de acompanhamento)* |
  | LDE (262.144 entradas) | 68.389 | 65.701 | 1,04× |
  | Colunas hash Poseidon (524.288 entradas) | 1.728.835 | 1.447.076 | 1,19× |

  Cada captura registra temporizações `zero_fill` (9,651 ms para 33.554.432 bytes) e
  Entradas `poseidon_microbench` (pista padrão 596,229ms vs escalar 656,251ms,
  Aceleração de 1,10×) para que os consumidores do painel possam diferenciar a pressão da fila junto com o
  principais operações.
- **Link cruzado de vinculações/documentos:** `docs/source/benchmarks.md` agora faz referência ao
  liberar JSON e comando do reprodutor, as substituições da fila Metal são validadas
  por meio de testes env/manifest `iroha_config` e `irohad` publica ao vivo
  Medidores `fastpq_metal_queue_*` para que os painéis sinalizem regressões IFFT sem
  raspagem de log ad-hoc. O `AccelerationSettings.runtimeState` do Swift expõe o
  mesma carga útil de telemetria enviada no pacote JSON, fechando o WP2-D
  vinculação/lacuna de documento com uma linha de base de aceitação reproduzível.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **Correção da fila IFT:** Lotes FFT inversos agora ignoram o envio de múltiplas filas sempre que
  a carga de trabalho mal atinge o limite de distribuição (16 colunas na faixa balanceada
  perfil), removendo a regressão Metal-vs-CPU mencionada acima, mantendo
  cargas de trabalho de colunas grandes no caminho de múltiplas filas para FFT/LDE/Poseidon.