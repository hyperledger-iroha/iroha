---
lang: pt
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2026-01-03T18:07:57.716816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Relatório de benchmarking

Instantâneos detalhados por execução e o histórico do FASTPQ WP5-B ao vivo
[`benchmarks/history.md`](benchmarks/history.md); use esse índice ao anexar
artefatos para revisões de roteiros ou auditorias de SRE. Regenere-o com
`python3 scripts/fastpq/update_benchmark_history.py` sempre que uma nova GPU captura
ou Poseidon manifesta terra.

## Pacote de evidências de aceleração

Cada benchmark de GPU ou modo misto deve incluir as configurações de aceleração aplicadas
portanto, o WP6-B/WP6-C pode provar a paridade de configuração junto com os artefatos de temporização.

- Capture o instantâneo do tempo de execução antes/depois de cada execução:
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (use `--format table` para logs legíveis por humanos). Isso registra `enable_{metal,cuda}`,
  Limites de Merkle, limites de polarização de CPU SHA-2, bits de integridade de back-end detectados e quaisquer
  erros de paridade persistentes ou motivos de desativação.
- Armazene o JSON próximo à saída do benchmark empacotado
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, varredura Merkle
  capturas, etc.) para que os revisores possam diferenciar os tempos e a configuração juntos.
- As definições e padrões dos botões residem em `docs/source/config/acceleration.md`; quando
  substituições são aplicadas (por exemplo, `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  anote-os nos metadados de execução para manter as repetições reproduzíveis entre os hosts.

## Referência de estágio 1 Norito (WP5-B/C)

- Comando: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  emite JSON + Markdown sob `benchmarks/norito_stage1/` com tempos por tamanho
  para o construtor de índice estrutural escalar vs acelerado.
- Últimas execuções (macOS aarch64, perfil dev) ao vivo em
  `benchmarks/norito_stage1/latest.{json,md}` e o novo CSV de transição de
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) mostra SIMD
  ganha de ~6–8KiB em diante. GPU/paralelo Stage-1 agora tem como padrão **192KiB**
  corte (`NORITO_STAGE1_GPU_MIN_BYTES=<n>` para substituir) para evitar thrash de lançamento
  em documentos pequenos e, ao mesmo tempo, ativar aceleradores para cargas maiores.

## Enum vs Despacho de Objeto Trait

- Tempo de compilação (compilação de depuração): 16,58s
- Tempo de execução (Critério, quanto menor melhor):
  - `enum`: 386 cv (média)
  - `trait_object`: 1,56 ns (média)

Essas medições vêm de um microbenchmark que compara um despacho baseado em enum com uma implementação de objeto de característica em caixa.

## Lote Poseidon CUDA

O benchmark Poseidon (`crates/ivm/benches/bench_poseidon.rs`) agora inclui cargas de trabalho que exercitam tanto permutações de hash único quanto os novos auxiliares em lote. Execute o pacote com:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

O critério registrará os resultados em `target/criterion/poseidon*_many`. Quando um trabalhador de GPU estiver disponível, exporte os resumos JSON (por exemplo, copie `target/criterion/**/new/benchmark.json` em `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) (por exemplo, copie `target/criterion/**/new/benchmark.json` em `benchmarks/poseidon/`) para que as equipes downstream possam comparar a taxa de transferência de CPU versus CUDA para cada tamanho de lote. Até que a faixa de GPU dedicada entre em operação, o benchmark volta à implementação SIMD/CPU e ainda fornece dados de regressão úteis para desempenho em lote.

Para capturas repetíveis (e para manter evidências de paridade com dados de tempo), execute

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```que semeia lotes determinísticos de Poseidon2/6, registra motivos de saúde/desativação CUDA, verifica
paridade em relação ao caminho escalar e emite resumos de operações/seg + aceleração junto com o Metal
status de tempo de execução (sinalizador de recurso, disponibilidade, último erro). Hosts somente CPU ainda escrevem o escalar
faça referência e anote o acelerador ausente, para que o CI possa publicar artefatos mesmo sem uma GPU
corredor.

## Referência FASTPQ Metal (Apple Silicon)

A pista da GPU capturou uma execução ponta a ponta atualizada de `fastpq_metal_bench` no macOS 14 (arm64) com o conjunto de parâmetros balanceados de pista, 20.000 linhas lógicas (preenchidas para 32.768) e 16 grupos de colunas. O artefato embrulhado reside em `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`, com o traço de Metal armazenado junto com as capturas anteriores em `traces/fastpq_metal_trace_*_rows20000_iter5.trace`. Os tempos médios (de `benchmarks.operations[*]`) agora são:

| Operação | Média de CPU (ms) | Média de metais (ms) | Aceleração (x) |
|-----------|---------------|------|------------|
| FFT (32.768 entradas) | 83,29 | 79,95 | 1.04 |
| IFFT (32.768 entradas) | 93,90 | 78,61 | 1,20 |
| LDE (262.144 entradas) | 669,54 | 657,67 | 1.02 |
| Colunas hash Poseidon (524.288 entradas) | 29.087,53 | 30.004,90 | 0,97 |

Observações:

- FFT/IFFT se beneficiam dos kernels BN254 atualizados (IFFT limpa a regressão anterior em aproximadamente 20%).
- LDE permanece próximo da paridade; o preenchimento zero agora registra 33.554.432 bytes preenchidos com uma média de 18,66 ms para que o pacote JSON capture o impacto da fila.
- O hash do Poseidon ainda está vinculado à CPU neste hardware; continue comparando com os manifestos do microbench Poseidon até que o caminho Metal adote os controles de fila mais recentes.
- Cada captura agora registra `AccelerationSettings.runtimeState().metal.lastError`, permitindo
  os engenheiros anotam os substitutos da CPU com o motivo específico da desativação (alternância de política,
  falha de paridade, nenhum dispositivo) diretamente no artefato de benchmark.

Para reproduzir a execução, construa os kernels Metal e execute:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

Confirme o JSON resultante em `artifacts/fastpq_benchmarks/` junto com o rastreamento Metal para que a evidência de determinismo permaneça reproduzível.

## Automação FASTPQ CUDA

Os hosts CUDA podem executar e agrupar o benchmark SM80 em uma única etapa com:

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

O auxiliar invoca `fastpq_cuda_bench`, encadeia rótulos/dispositivo/notas, homenageia
`--require-gpu` e (por padrão) encapsula/assina via `scripts/fastpq/wrap_benchmark.py`.
As saídas incluem o JSON bruto, o pacote empacotado em `artifacts/fastpq_benchmarks/`,
e um `<name>_plan.json` próximo à saída que registra os comandos/env exatos para
As capturas do estágio 7 permanecem reproduzíveis em executores de GPU. Adicione `--sign-output` e
`--gpg-key <id>` quando assinaturas são necessárias; use `--dry-run` para emitir apenas o
planejar/caminhos sem executar a bancada.

### Captura de versão GA (macOS 14 arm64, pista balanceada)

Para satisfazer o WP2-D, também gravamos uma compilação de lançamento no mesmo host com pronto para GA
heurística de fila e publicou-a como
`fastpq_metal_bench_20k_release_macos14_arm64.json`. O artefato captura dois
lotes de colunas (balanceadas por faixa, preenchidas para 32.768 linhas) e inclui Poseidon
amostras de microbancada para consumo do painel.| Operação | Média de CPU (ms) | Média de metais (ms) | Aceleração | Notas |
|-----------|---------------|------|---------|-------|
| FFT (32.768 entradas) | 12.741 | 10.963 | 1,16× | Os kernels da GPU rastreiam os limites da fila atualizados. |
| IFFT (32.768 entradas) | 17.499 | 25.688 | 0,68× | Regressão atribuída ao fan-out conservador da fila; continue ajustando a heurística. |
| LDE (262.144 entradas) | 68.389 | 65.701 | 1,04× | O preenchimento zero registra 33.554.432 bytes em 9,651 ms para ambos os lotes. |
| Colunas hash Poseidon (524.288 entradas) | 1.728.835 | 1.447.076 | 1,19× | A GPU finalmente supera a CPU após os ajustes na fila do Poseidon. |

Os valores do microbench Poseidon incorporados no JSON mostram uma aceleração de 1,10× (pista padrão
596,229 ms vs escalar 656,251 ms em cinco iterações), então os painéis agora podem traçar
melhorias por pista ao longo da bancada principal. Reproduza a execução com:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Mantenha os rastreamentos JSON e `FASTPQ_METAL_TRACE_CHILD=1` empacotados verificados em
`artifacts/fastpq_benchmarks/` para que análises subsequentes do WP2-D/WP2-E possam diferenciar o GA
captura em relação a execuções de atualização anteriores sem executar novamente a carga de trabalho.

Cada nova captura `fastpq_metal_bench` agora também grava um bloco `bn254_metrics`,
que expõe entradas `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` para a CPU
linha de base e qualquer back-end de GPU (Metal/CUDA) que estava ativo, **e** um
Bloco `bn254_dispatch` que registra as larguras dos grupos de threads observados, thread lógico
contagens e limites de pipeline para despachos BN254 FFT/LDE de coluna única. O
O wrapper de benchmark copia ambos os mapas em `benchmarks.bn254_*`, para que os painéis e
Os exportadores Prometheus podem extrair latências e geometria rotuladas sem nova análise
a matriz de operações brutas. A substituição `FASTPQ_METAL_THREADGROUP` agora se aplica a
Kernels BN254 também, tornando as varreduras de grupos de threads reproduzíveis a partir de um botão.

Para manter os painéis downstream simples, execute `python3 scripts/benchmarks/export_csv.py`
depois de capturar um pacote. O auxiliar nivela `poseidon_microbench_*.json` em
combinando arquivos `.csv` para que os trabalhos de automação possam diferenciar pistas padrão e escalares sem
analisadores personalizados.

## Microbancada Poseidon (Metal)

`fastpq_metal_bench` agora se reexecuta em `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` e promove os tempos em `benchmarks.poseidon_microbench`. Exportamos as capturas de Metal mais recentes com `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` e as agregamos via `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`. Os resumos abaixo estão sob `benchmarks/poseidon/`:

| Resumo | Pacote embrulhado | Média padrão (ms) | Média escalar (ms) | Aceleração vs escalar | Colunas x estados | Iterações |
|---------|----------------|-------------------|-----------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1.990,49 | 1.994,53 | 1.002 | 64 x 262.144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2.167,66 | 2.152,18 | 0,993 | 64 x 262.144 | 5 |Ambas as capturas geraram hash de 262.144 estados por execução (trace log2 = 12) com uma única iteração de aquecimento. A pista "padrão" corresponde ao kernel multiestado ajustado, enquanto "escalar" bloqueia o kernel em um estado por pista para comparação.

## Varreduras de limite Merkle

O exemplo `merkle_threshold` (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) enfatiza os caminhos de hash Merkle Metal vs CPU. A captura mais recente do AppleSilicon (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) reside em `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` com uma exportação CSV correspondente. As linhas de base do macOS 14 somente CPU permanecem em `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` para hosts sem Metal.

| Folhas | Melhor CPU (ms) | Metal melhor (ms) | Aceleração |
|----|---------------|-----------------|---------|
| 1.024 | 23.01 | 19,69 | 1,17× |
| 4.096 | 50,87 | 62,12 | 0,82× |
| 8.192 | 95,77 | 96,57 | 0,99× |
| 16.384 | 64,48 | 58,98 | 1,09× |
| 32.768 | 109,49 | 87,68 | 1,25× |
| 65.536 | 177,72 | 137,93 | 1,29× |

Contagens maiores de folhas se beneficiam do Metal (1,09–1,29×); buckets menores ainda rodam mais rápido na CPU, então o CSV mantém ambas as colunas para análise. O auxiliar CSV preserva o sinalizador `metal_available` ao lado de cada perfil para manter os painéis de regressão GPU versus CPU alinhados.

Etapas de reprodução:

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Defina `FASTPQ_METAL_LIB`/`FASTPQ_GPU` se o host exigir ativação explícita do Metal e mantenha ambas as capturas de CPU + GPU marcadas para que o WP1-F possa traçar os limites da política.

Ao executar a partir de um shell headless, defina `IVM_DEBUG_METAL_ENUM=1` para registrar a enumeração do dispositivo e `IVM_FORCE_METAL_ENUM=1` para ignorar `MTLCreateSystemDefaultDevice()`. A CLI aquece a sessão CoreGraphics **antes** de solicitar o dispositivo Metal padrão e volta para `MTLCreateSystemDefaultDevice()` quando `MTLCopyAllDevices()` retorna zero; se o host ainda não relatar nenhum dispositivo, a captura manterá `metal_available=false` (linhas de base de CPU úteis em `macos14_arm64_*`), enquanto os hosts de GPU devem manter `FASTPQ_GPU=metal` habilitado para que o pacote registre o back-end escolhido.

`fastpq_metal_bench` expõe um botão semelhante por meio de `FASTPQ_DEBUG_METAL_ENUM=1`, que imprime os resultados `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` antes que o back-end decida se deseja permanecer no caminho da GPU. Ative-o sempre que `FASTPQ_GPU=gpu` ainda relatar `backend="none"` no JSON empacotado para que o pacote de captura registre exatamente como o host enumerou o hardware Metal; o chicote é interrompido imediatamente quando `FASTPQ_GPU=gpu` é definido, mas nenhum acelerador é detectado, apontando para o botão de depuração para que o pacote de lançamento nunca esconda um substituto da CPU atrás de uma execução forçada da GPU.

O auxiliar CSV emite tabelas por perfil (por exemplo, `macos14_arm64_*.csv` e `takemiyacStudio.lan_25.0.0_arm64.csv`), preservando o sinalizador `metal_available` para que os painéis de regressão possam ingerir as medições de CPU e GPU sem analisadores personalizados.