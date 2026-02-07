---
lang: pt
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Guia de migração de produção FASTPQ

Este runbook descreve como validar o provador FASTPQ de produção Stage6.
O back-end do espaço reservado determinístico foi removido como parte deste plano de migração.
Ele complementa o plano faseado em `docs/source/fastpq_plan.md` e pressupõe que você já rastreia
status do espaço de trabalho em `status.md`.

## Público e escopo
- Operadores validadores implementando o provador de produção em ambientes de teste ou mainnet.
- Engenheiros de liberação criando binários ou contêineres que serão enviados com o back-end de produção.
- Equipes de SRE/observabilidade conectando novos sinais de telemetria e alertas.

Fora do escopo: criação de contrato Kotodama e alterações de ABI IVM (consulte `docs/source/nexus.md` para obter o
modelo de execução).

## Matriz de recursos
| Caminho | Recursos de carga para ativar | Resultado | Quando usar |
| ---- | ----------------------- | ------ | ----------- |
| Provador de produção (padrão) | _nenhum_ | Backend Stage6 FASTPQ com planejador FFT/LDE e pipeline DEEP-FRI.【crates/fastpq_prover/src/backend.rs:1144】 | Padrão para todos os binários de produção. |
| Aceleração de GPU opcional | `fastpq_prover/fastpq-gpu` | Habilita kernels CUDA/Metal com fallback automático de CPU.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Hosts com aceleradores suportados. |

## Procedimento de construção
1. **Compilação somente CPU**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   O back-end de produção é compilado por padrão; nenhum recurso extra é necessário.

2. **Compilação habilitada para GPU (opcional)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   O suporte de GPU requer um kit de ferramentas SM80+ CUDA com `nvcc` disponível durante a construção.【crates/fastpq_prover/Cargo.toml:11】

3. **Autotestes**
   ```bash
   cargo test -p fastpq_prover
   ```
   Execute isso uma vez por versão para confirmar o caminho do Stage6 antes do empacotamento.

### Preparação do conjunto de ferramentas de metal (macOS)
1. Instale as ferramentas de linha de comando do Metal antes de compilar: `xcode-select --install` (se as ferramentas CLI estiverem faltando) e `xcodebuild -downloadComponent MetalToolchain` para buscar o conjunto de ferramentas da GPU. O script de construção invoca `xcrun metal`/`xcrun metallib` diretamente e falhará rapidamente se os binários estiverem ausentes.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Para validar o pipeline antes do CI, você pode espelhar o script de construção localmente:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Quando isso é bem-sucedido, a compilação emite `FASTPQ_METAL_LIB=<path>`; o tempo de execução lê esse valor para carregar o metallib de forma determinística.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Defina `FASTPQ_SKIP_GPU_BUILD=1` ao fazer compilação cruzada sem o conjunto de ferramentas Metal; a compilação imprime um aviso e o planejador permanece no caminho da CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Os nós retornam à CPU automaticamente se o Metal não estiver disponível (estrutura ausente, GPU sem suporte ou `FASTPQ_METAL_LIB` vazio); o script de construção limpa o env var e o planejador registra o downgrade.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### Lista de verificação de lançamento (Etapa 6)
Mantenha o ticket de liberação do FASTPQ bloqueado até que todos os itens abaixo estejam completos e anexados.

1. **Métricas de prova em menos de um segundo** — Inspecione o `fastpq_metal_bench_*.json` recém-capturado e
   confirme a entrada `benchmarks.operations` onde `operation = "lde"` (e o espelhado
   Amostra `report.operations`) relata `gpu_mean_ms ≤ 950` para a carga de trabalho de 20.000 linhas (32768 preenchimento
   linhas). As capturas fora do teto exigem repetições antes que a lista de verificação possa ser assinada.
2. **Manifesto assinado** — Executar
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   então o ticket de liberação traz tanto o manifesto quanto sua assinatura separada
   (`artifacts/fastpq_bench_manifest.sig`). Os revisores verificam o par resumo/assinatura antes
   promovendo um lançamento.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 O manifesto da matriz (criado
   via `scripts/fastpq/capture_matrix.sh`) já codifica o piso de 20 mil linhas e
   depurando uma regressão.
3. **Anexos de evidências** — Faça upload do JSON do benchmark Metal, log stdout (ou rastreamento de instrumentos),
   Saídas do manifesto CUDA/Metal e a assinatura separada do ticket de lançamento. A entrada da lista de verificação
   deve vincular a todos os artefatos, além da impressão digital da chave pública usada para assinatura, para auditorias posteriores
   pode repetir a etapa de verificação.【artifacts/fastpq_benchmarks/README.md:65】### Fluxo de trabalho de validação de metal
1. Após uma compilação habilitada para GPU, confirme os pontos `FASTPQ_METAL_LIB` em um `.metallib` (`echo $FASTPQ_METAL_LIB`) para que o tempo de execução possa carregá-lo deterministicamente.【crates/fastpq_prover/build.rs:188】
2. Execute o conjunto de paridade com pistas de GPU ativadas:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. O back-end exercitará os kernels Metal e registrará um substituto determinístico da CPU se a detecção falhar.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Capture uma amostra de benchmark para painéis:\
   localize a biblioteca Metal compilada (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   exporte-o via `FASTPQ_METAL_LIB` e execute\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  O perfil canônico `fastpq-lane-balanced` agora preenche cada captura para 32.768 linhas (2¹⁵), então o JSON carrega `rows` e `padded_rows` junto com a latência Metal LDE; execute novamente a captura se `zero_fill` ou as configurações de fila empurrarem o LDE da GPU além da meta de 950 ms (<1s) em hosts da série AppleM. Arquive o JSON/log resultante junto com outras evidências de lançamento; o fluxo de trabalho noturno do macOS executa a mesma execução e carrega seus artefatos para comparação.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Quando você precisar de telemetria somente Poseidon (por exemplo, para registrar um rastreamento de Instrumentos), adicione `--operation poseidon_hash_columns` ao comando acima; o banco ainda respeitará `FASTPQ_GPU=gpu`, emitirá `metal_dispatch_queue.poseidon` e incluirá o novo bloco `poseidon_profiles` para que o pacote de lançamento documente explicitamente o gargalo do Poseidon.
  A evidência agora inclui `zero_fill.{bytes,ms,queue_delta}` mais `kernel_profiles` (por kernel
  ocupação, GB/s estimados e estatísticas de duração) para que a eficiência da GPU possa ser representada graficamente sem
  reprocessando rastreamentos brutos e um bloco `twiddle_cache` (acertos/erros + `before_ms`/`after_ms`) que
  prova que os uploads twiddle armazenados em cache estão em vigor. `--trace-dir` relança o arnês em
  `xcrun xctrace record` e
  armazena um arquivo `.trace` com carimbo de data e hora junto com o JSON; você ainda pode fornecer um personalizado
  `--trace-output` (com `--trace-template` / `--trace-seconds` opcional) ao capturar para um
  local/modelo personalizado. O JSON registra `metal_trace_{template,seconds,output}` para auditoria.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Após cada captura, execute `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` para que a publicação carregue metadados de host (agora incluindo `metadata.metal_trace`) para o pacote de placa/alerta Grafana (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). O relatório agora carrega um objeto `speedup` por operação (`speedup.ratio`, `speedup.delta_ms`), o wrapper iça `zero_fill_hotspots` (bytes, latência, GB/s derivados e os contadores delta da fila Metal), nivela `kernel_profiles` em `benchmarks.kernel_summary`, mantém o bloco `twiddle_cache` intacto, copia o novo bloco/resumo `post_tile_dispatches` para que os revisores possam provar que o kernel multipassagem foi executado durante a captura e agora resume a evidência do microbench Poseidon em `benchmarks.poseidon_microbench` para que os painéis possam citar a latência escalar versus padrão sem analisar novamente o relatório bruto. A porta de manifesto lê o mesmo bloco e rejeita pacotes de evidências da GPU que o omitem, forçando os operadores a atualizar as capturas sempre que o caminho pós-ladrilho for ignorado ou configurado incorretamente.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  O kernel Poseidon2 Metal compartilha os mesmos botões: `FASTPQ_METAL_POSEIDON_LANES` (32–256, potências de dois) e `FASTPQ_METAL_POSEIDON_BATCH` (1–32 estados por pista) permitem fixar a largura de lançamento e o trabalho por pista sem reconstruir; o host encadeia esses valores por meio de `PoseidonArgs` antes de cada envio. Por padrão, o tempo de execução inspeciona `MTLDevice::{is_low_power,is_headless,location}` para direcionar GPUs discretas para lançamentos em camadas VRAM (`256×24` quando ≥48GiB é relatado, `256×20` em 32GiB, `256×16` caso contrário) enquanto SoCs de baixo consumo permanecem ligados `256×8` (e peças mais antigas de 128/64 pistas aderem a 8/6 estados por pista), então a maioria dos operadores nunca precisa definir os env vars manualmente. `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` e emite um bloco `poseidon_microbench` que registra ambos os perfis de lançamento mais a aceleração medida versus a faixa escalar para que os pacotes de lançamento possam provar que o novo kernel realmente reduz `poseidon_hash_columns`, e inclui o bloco `poseidon_pipeline` para que a evidência do Stage7 capture os botões de profundidade/sobreposição do pedaço ao lado dos novos níveis de ocupação. Deixe o ambiente não definido para execuções normais; o chicote gerencia a reexecução automaticamente, registra falhas se a captura secundária não puder ser executada e sai imediatamente quando `FASTPQ_GPU=gpu` é definido, mas nenhum back-end de GPU está disponível para que substitutos silenciosos de CPU nunca entrem no desempenho artefatos.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】O wrapper rejeita capturas Poseidon que não possuem o delta `metal_dispatch_queue.poseidon`, os contadores `column_staging` compartilhados ou os blocos de evidências `poseidon_profiles`/`poseidon_microbench`, portanto, os operadores devem atualizar qualquer captura que não consiga provar a sobreposição de teste ou o escalar vs padrão speedup.【scripts/fastpq/wrap_benchmark.py:732】 Quando você precisar de um JSON independente para painéis ou deltas de CI, execute `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; o auxiliar aceita artefatos agrupados e capturas `fastpq_metal_bench*.json` brutas, emitindo `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` com os tempos padrão/escalares, ajustando metadados e aceleração gravada.【scripts/fastpq/export_poseidon_microbench.py:1】
  Conclua a execução executando `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` para que a lista de verificação de lançamento do Stage6 aplique o limite LDE `<1 s` e emita um pacote de manifesto/resumo assinado que acompanha o tíquete de lançamento.
4. Verifique a telemetria antes da implementação: enrole o endpoint Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) e inspecione os logs `telemetry::fastpq.execution_mode` em busca de `resolved="cpu"` inesperado entradas.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Documente o caminho de fallback da CPU forçando-o intencionalmente (`FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"`) para que os playbooks SRE permaneçam alinhados com o comportamento determinístico.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Ajuste opcional: por padrão, o host seleciona 16 pistas para rastreamentos curtos, 32 para rastreamentos médios e 64/128 uma vez `log_len ≥ 10/14`, chegando a 256 quando `log_len ≥ 18`, e agora mantém o bloco de memória compartilhada em cinco estágios para rastreamentos pequenos, quatro uma vez `log_len ≥ 12` e 14/12/16 estágios para `log_len ≥ 18/20/22` antes de iniciar o trabalho no kernel pós-tile. Exporte `FASTPQ_METAL_FFT_LANES` (potência de dois entre 8 e 256) e/ou `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) antes de executar as etapas acima para substituir essas heurísticas. Os tamanhos de lote das colunas FFT/IFFT e LDE derivam da largura do grupo de threads resolvido (≈2.048 threads lógicos por despacho, limitado a 32 colunas, e agora diminuindo até 32→16→8→4→2→1 à medida que o domínio cresce) enquanto o caminho LDE ainda impõe seus limites de domínio; defina `FASTPQ_METAL_FFT_COLUMNS` (1–32) para fixar um tamanho de lote FFT determinístico e `FASTPQ_METAL_LDE_COLUMNS` (1–32) para aplicar a mesma substituição ao despachante LDE quando precisar de comparações bit por bit entre hosts. A profundidade do bloco LDE também reflete a heurística FFT - os rastreamentos com `log₂ ≥ 18/20/22` executam apenas estágios de memória compartilhada 12/10/8 antes de entregar as borboletas largas ao kernel pós-ladrilho - e você pode substituir esse limite por meio de `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). O tempo de execução encadeia todos os valores por meio dos argumentos do kernel Metal, fixa substituições não suportadas e registra os valores resolvidos para que os experimentos permaneçam reproduzíveis sem reconstruir o metallib; o JSON de referência apresenta o ajuste resolvido e o orçamento de preenchimento zero do host (`zero_fill.{bytes,ms,queue_delta}`) capturado por meio de estatísticas LDE para que os deltas da fila sejam vinculados diretamente a cada captura e agora adiciona um bloco `column_staging` (lotes achatados, flatten_ms, wait_ms, wait_ratio) para que os revisores possam verificar a sobreposição de host/dispositivo introduzida pelo pipeline com buffer duplo. Quando a GPU se recusa a reportar telemetria de preenchimento zero, o chicote agora sintetiza um tempo determinístico a partir de limpezas de buffer do lado do host e o injeta no bloco `zero_fill` para que a evidência de liberação nunca seja enviada sem o campo.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. O envio de várias filas é automático em Macs discretos: quando `Device::is_low_power()` retorna falso ou o dispositivo Metal relata um local de slot/externo, o host instancia dois `MTLCommandQueue`s, apenas se espalha quando a carga de trabalho carrega ≥16 colunas (escalada pelo fan-out) e faz round-robins dos lotes de colunas nas filas, de forma que traços longos mantenham ambas as pistas da GPU ocupadas sem comprometendo o determinismo. Substitua a política por `FASTPQ_METAL_QUEUE_FANOUT` (1 a 4 filas) e `FASTPQ_METAL_COLUMN_THRESHOLD` (total mínimo de colunas antes da distribuição) sempre que precisar de capturas reproduzíveis em máquinas; os testes de paridade forçam essas substituições para que Macs multi-GPU permaneçam cobertos e o fan-out/limite resolvido seja registrado próximo à telemetria de profundidade da fila.### Evidências para arquivar
| Artefato | Capturar | Notas |
|----------|------------|-------|
| Pacote `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` e `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` seguidos por `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` e `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Prova que o Metal CLI/toolchain foi instalado e produziu uma biblioteca determinística para este commit.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Instantâneo do ambiente | `echo $FASTPQ_METAL_LIB` após a construção; mantenha o caminho absoluto com seu ticket de liberação. | Saída vazia significa que o Metal foi desativado; registrar o valor documenta que as pistas da GPU permanecem disponíveis no artefato de remessa.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| Registro de paridade GPU | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` e arquive o snippet que contém `backend="metal"` ou o aviso de downgrade. | Demonstra que os kernels são executados (ou retrocedem deterministicamente) antes de você promover a compilação.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Resultado de referência | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; embrulhe e assine via `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | Os registros JSON agrupados `speedup.ratio`, `speedup.delta_ms`, ajuste FFT, linhas preenchidas (32.768), `zero_fill`/`kernel_profiles` enriquecido, o `kernel_summary` nivelado, o verificado Blocos `metal_dispatch_queue.poseidon`/`poseidon_profiles` (quando `--operation poseidon_hash_columns` é usado) e os metadados de rastreamento para que a média LDE da GPU permaneça ≤950ms e Poseidon permaneça <1s; mantenha o pacote e a assinatura `.json.asc` gerada com o ticket de lançamento para que os painéis e auditores possam verificar o artefato sem executar novamente as cargas de trabalho. |
| Manifesto de bancada | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Valida ambos os artefatos de GPU, falha se a média LDE ultrapassar o teto `<1 s`, registra resumos BLAKE3/SHA-256 e emite um manifesto assinado para que a lista de verificação de lançamento não possa avançar sem métricas verificáveis.
| Pacote CUDA | Execute `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` no host de laboratório SM80, envolva/assine o JSON em `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (use `--label device_class=xeon-rtx-sm80` para que os painéis escolham a classe correta), adicione o caminho para `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` e mantenha o par `.json`/`.asc` com o Metal artefato antes de regenerar o manifesto. O `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` verificado ilustra o formato exato do pacote que os auditores esperam.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| Prova de telemetria | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` mais o log `telemetry::fastpq.execution_mode` emitido na inicialização. | Confirma que Prometheus/OTEL expõe `device_class="<matrix>", backend="metal"` (ou um log de downgrade) antes de ativar o tráfego.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 || Exercício forçado de CPU | Execute um lote curto com `FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"` e capture o log de downgrade. | Mantém os runbooks SRE alinhados com o caminho de fallback determinístico caso uma reversão seja necessária no meio do lançamento.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Captura de rastreamento (opcional) | Repita um teste de paridade com `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` e salve o rastreamento de despacho emitido. | Preserva evidências de ocupação/grupo de threads para análises posteriores de perfil sem reexecutar benchmarks.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Os arquivos multilíngues `fastpq_plan.*` fazem referência a esta lista de verificação para que os operadores de preparação e produção sigam a mesma trilha de evidências.【docs/source/fastpq_plan.md:1】

## Construções reproduzíveis
Use o fluxo de trabalho do contêiner fixado para produzir artefatos reproduzíveis do Stage6:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

O script auxiliar cria a imagem do conjunto de ferramentas `rust:1.88.0-slim-bookworm` (e `nvidia/cuda:12.2.2-devel-ubuntu22.04` para GPU), executa a construção dentro do contêiner e grava `manifest.json`, `sha256s.txt` e os binários compilados na saída de destino diretório.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Substituições de ambiente:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – fixa uma base/tag Rust explícita.
- `FASTPQ_CUDA_IMAGE` – troque a base CUDA ao produzir artefatos de GPU.
- `FASTPQ_CONTAINER_RUNTIME` – força um tempo de execução específico; padrão `auto` tenta `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – ordem de preferência separada por vírgula para detecção automática de tempo de execução (o padrão é `docker,podman,nerdctl`).

## Atualizações de configuração
1. Defina o modo de execução do tempo de execução em seu TOML:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   O valor é analisado por meio de `FastpqExecutionMode` e encadeado no backend na inicialização.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Substitua na inicialização, se necessário:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   As substituições CLI alteram a configuração resolvida antes da inicialização do nó.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Os desenvolvedores podem coagir temporariamente a detecção sem alterar as configurações, exportando
   `FASTPQ_GPU={auto,cpu,gpu}` antes de lançar o binário; a substituição é registrada e o pipeline
   ainda aparece o modo resolvido.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Lista de verificação de verificação
1. **Registros de inicialização**
   - Espere `FASTPQ execution mode resolved` do alvo `telemetry::fastpq.execution_mode` com
     Etiquetas `requested`, `resolved` e `backend`.【crates/fastpq_prover/src/backend.rs:208】
   - Na detecção automática de GPU, um log secundário de `fastpq::planner` informa a pista final.
   - Superfície de hosts de metal `backend="metal"` quando o metallib é carregado com sucesso; se a compilação ou o carregamento falharem, o script de construção emitirá um aviso, limpará `FASTPQ_METAL_LIB` e o planejador registrará `GPU acceleration unavailable` antes de permanecer em CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Métricas Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   O contador é incrementado via `record_fastpq_execution_mode` (agora rotulado por
   `{device_class,chip_family,gpu_kind}`) sempre que um nó resolve sua execução
   modo.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Para cobertura metálica confirme
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     incrementos junto com seus painéis de implantação.【crates/iroha_telemetry/src/metrics.rs:5397】
   - Nós macOS compilados com `irohad --features fastpq-gpu` expõem adicionalmente
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     e
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` para que os painéis do Stage7
     pode rastrear o ciclo de trabalho e o headroom da fila a partir de arranhões Prometheus ao vivo.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Exportação de telemetria**
   - As construções OTEL emitem `fastpq.execution_mode_resolutions_total` com os mesmos rótulos; garanta o seu
     painéis ou alertas observam `resolved="cpu"` inesperado quando as GPUs deveriam estar ativas.

4. **Provar/verificar sanidade**
   - Execute um pequeno lote por meio de `iroha_cli` ou um chicote de integração e confirme as provas em um
     peer compilado com os mesmos parâmetros.

## Solução de problemas
- **O modo resolvido permanece CPU em hosts GPU** — verifique se o binário foi compilado com
  `fastpq_prover/fastpq-gpu`, as bibliotecas CUDA estão no caminho do carregador e `FASTPQ_GPU` não está forçando
  `cpu`.
- **Metal indisponível no Apple Silicon** — verifique se as ferramentas CLI estão instaladas (`xcode-select --install`), execute novamente `xcodebuild -downloadComponent MetalToolchain` e certifique-se de que a compilação produziu um caminho `FASTPQ_METAL_LIB` não vazio; um valor vazio ou ausente desativa o back-end por design.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **Erros `Unknown parameter`** — certifique-se de que tanto o provador quanto o verificador usem o mesmo catálogo canônico
  emitido por `fastpq_isi`; superfície incompatível como `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Backup inesperado da CPU** — inspecione `cargo tree -p fastpq_prover --features` e
  confirme que `fastpq_prover/fastpq-gpu` está presente em compilações de GPU; verifique se as bibliotecas `nvcc`/CUDA estão no caminho de pesquisa.
- **Contador de telemetria ausente** — verifique se o nó foi iniciado com `--features telemetry` (padrão)
  e que a exportação OTEL (se habilitada) inclui o pipeline de métricas.【crates/iroha_telemetry/src/metrics.rs:8887】

## Procedimento alternativo
O back-end do espaço reservado determinístico foi removido. Se uma regressão exigir reversão,
reimplantar os artefatos de lançamento anteriormente conhecidos e investigar antes de reeditar o Stage6
binários. Documente a decisão de gerenciamento de mudanças e garanta que o avanço seja concluído somente após a
a regressão é compreendida.

3. Monitore a telemetria para garantir que `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` reflita o esperado
   execução de espaço reservado.

## Linha de base de hardware
| Perfil | CPU | GPU | Notas |
| ------- | --- | --- | ----- |
| Referência (Etapa 6) | AMD EPYC7B12 (32 núcleos), 256GiB de RAM | NVIDIA A10040GB (CUDA12.2) | Lotes sintéticos de 20.000 linhas devem ser concluídos ≤1000ms.【docs/source/fastpq_plan.md:131】 |
| Somente CPU | ≥32 núcleos físicos, AVX2 | – | Espere aproximadamente 0,9–1,2s para 20.000 linhas; mantenha `execution_mode = "cpu"` para determinismo. |## Testes de regressão
-`cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (em hosts GPU)
- Verificação opcional do acessório dourado:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Documente quaisquer desvios desta lista de verificação em seu runbook de operações e atualize `status.md` após o
a janela de migração é concluída.