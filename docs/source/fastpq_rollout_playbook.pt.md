---
lang: pt
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Manual de implementação do FASTPQ (Etapa 7-3)

Este manual implementa o requisito do roteiro Stage7-3: cada atualização da frota
que permite a execução da GPU FASTPQ deve anexar um manifesto de benchmark reproduzível,
evidência Grafana emparelhada e um exercício de reversão documentado. Complementa
`docs/source/fastpq_plan.md` (destinos/arquitetura) e
`docs/source/fastpq_migration_guide.md` (etapas de atualização em nível de nó), concentrando-se
na lista de verificação de implementação voltada para o operador.

## Escopo e funções

- **Engenharia de Liberação/SRE:** capturas de benchmark próprias, assinatura de manifesto e
  exportações do painel antes da aprovação da implementação.
- **Ops Guild:** executa lançamentos graduais, grava ensaios de reversão e armazena
  o pacote de artefatos em `artifacts/fastpq_rollouts/<timestamp>/`.
- **Governança/Conformidade:** verifica se as evidências acompanham cada mudança
  solicitação antes que o padrão FASTPQ seja alternado para uma frota.

## Requisitos do pacote de evidências

Cada envio de implementação deve conter os seguintes artefatos. Anexe todos os arquivos
ao ticket de lançamento/atualização e mantenha o pacote em
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefato | Finalidade | Como produzir |
|----------|------------|----------------|
| `fastpq_bench_manifest.json` | Prova que a carga de trabalho canônica de 20.000 linhas permanece abaixo do teto LDE `<1 s` e registra hashes para cada benchmark empacotado.| Capture execuções de Metal/CUDA, envolva-as e execute:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
| Benchmarks agrupados (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Capture metadados de host, evidências de uso de linha, pontos de acesso de preenchimento zero, resumos de microbancos Poseidon e estatísticas de kernel usadas por painéis/alertas.| Execute `fastpq_metal_bench` / `fastpq_cuda_bench` e, em seguida, envolva o JSON bruto:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`Repita para capturas CUDA (ponto `--row-usage` e `--poseidon-metrics` nos arquivos de testemunha/raspagem relevantes). O auxiliar incorpora as amostras filtradas `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` para que a evidência WP2-E.6 seja idêntica em Metal e CUDA. Use `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` quando precisar de um resumo de microbanco Poseidon independente (entradas empacotadas ou brutas suportadas). |
|  |  | **Requisito de rótulo do estágio 7:** `wrap_benchmark.py` agora falha, a menos que a seção `metadata.labels` resultante contenha `device_class` e `gpu_kind`. Quando a detecção automática não puder inferi-los (por exemplo, ao encapsular um nó de IC desanexado), passe substituições explícitas, como `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **Telemetria de aceleração:** o wrapper também captura `cargo xtask acceleration-state --format json` por padrão, escrevendo `<bundle>.accel.json` e `<bundle>.accel.prom` próximo ao benchmark empacotado (substituir por sinalizadores `--accel-*` ou `--skip-acceleration-state`). A matriz de captura usa esses arquivos para construir `acceleration_matrix.{json,md}` para painéis de frota. |
| Exportação Grafana | Comprova telemetria de adoção e anotações de alerta para a janela de implementação.| Exporte o painel `fastpq-acceleration`:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Anote o quadro com os horários de início/parada da implementação antes de exportar. O pipeline de lançamento pode fazer isso automaticamente por meio de `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (token fornecido por meio de `GRAFANA_TOKEN`). |
| Instantâneo de alerta | Captura as regras de alerta que protegeram a implementação.| Copie `dashboards/alerts/fastpq_acceleration_rules.yml` (e o acessório `tests/`) no pacote para que os revisores possam executar novamente `promtool test rules …`. |
| Registro de exercício de reversão | Demonstra que os operadores ensaiaram o substituto forçado da CPU e os reconhecimentos de telemetria. Use o procedimento em [Rollback Drills](#rollback-drills) e armazene os logs do console (`rollback_drill.log`) mais o scrape Prometheus resultante (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | Registra a alocação de linha ExecWitness FASTPQ que o TF-5 rastreia em CI e painéis.| Baixe uma nova testemunha de Torii, decodifique-a via `iroha_cli audit witness --decode exec.witness` (opcionalmente, adicione `--fastpq-parameter fastpq-lane-balanced` para afirmar o conjunto de parâmetros esperado; lotes FASTPQ são emitidos por padrão) e copie o `row_usage` JSON em `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. Mantenha os nomes dos arquivos com carimbo de data e hora para que os revisores possam correlacioná-los com o ticket de implementação e execute `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (ou `make check-fastpq-rollout`) para que a porta Stage7-3 verifique se cada lote anuncia as contagens do seletor e a invariante `transfer_ratio = transfer_rows / total_rows` antes de anexar a evidência. |

> **Dica:** `artifacts/fastpq_rollouts/README.md` documenta a nomenclatura preferida
> esquema (`<stamp>/<fleet>/<lane>`) e os arquivos de evidências necessários. O
> A pasta `<stamp>` deve codificar `YYYYMMDDThhmmZ` para que os artefatos permaneçam classificáveis
> sem consultar tickets.

## Lista de verificação de geração de evidências1. **Capturar benchmarks de GPU.**
   - Execute a carga de trabalho canônica (20.000 linhas lógicas, 32.768 linhas preenchidas) via
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - Enrole o resultado com `scripts/fastpq/wrap_benchmark.py` usando `--row-usage <decoded witness>` para que o pacote carregue a evidência do gadget junto com a telemetria da GPU. Passe `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` para que o wrapper falhe rapidamente se o acelerador exceder o destino ou se a telemetria da fila/perfil do Poseidon estiver ausente e para gerar a assinatura desanexada.
   - Repita no host CUDA para que o manifesto contenha ambas as famílias de GPU.
   - **Não** retire o `benchmarks.metal_dispatch_queue` ou
     Blocos `benchmarks.zero_fill_hotspots` do JSON empacotado. A porta CI
     (`ci/check_fastpq_rollout.sh`) agora lê esses campos e falha quando enfileira
     o headroom cai abaixo de um slot ou quando qualquer ponto de acesso LDE reporta `mean_ms >
     0,40ms`, aplicando a proteção de telemetria Stage7 automaticamente.
2. **Gere o manifesto.** Use `cargo xtask fastpq-bench-manifest …` como
   mostrado na tabela. Armazene `fastpq_bench_manifest.json` no pacote de implementação.
3. **Exportar Grafana.**
   - Anote a placa `FASTPQ Acceleration Overview` com a janela de implementação,
     vinculando aos IDs de painel Grafana relevantes.
   - Exporte o JSON do dashboard através da API Grafana (comando acima) e inclua
     a seção `annotations` para que os revisores possam combinar as curvas de adoção com o
     lançamento gradual.
4. **Alertas instantâneos.** Copie as regras de alerta exatas (`dashboards/alerts/…`) usadas
   pela implementação no pacote. Se as regras Prometheus foram substituídas, inclua
   a diferença de substituição.
5. ** Raspar Prometheus/OTEL. ** Capture `fastpq_execution_mode_total{device_class="<matrix>"}` de cada
   anfitrião (antes e depois da etapa) mais o contador OTEL
   `fastpq.execution_mode_resolutions_total` e o emparelhado
   Entradas de log `telemetry::fastpq.execution_mode`. Esses artefatos provam que
   A adoção da GPU é estável e os substitutos forçados da CPU ainda emitem telemetria.
6. **Telemetria de uso de linha de arquivo.** Depois de decodificar a execução do ExecWitness para o
   implementação, coloque o JSON resultante em `row_usage/` no pacote. O CI
   auxiliar (`ci/check_fastpq_row_usage.sh`) compara esses instantâneos com o
   linhas de base canônicas e `ci/check_fastpq_rollout.sh` agora requer todos
   pacote para enviar pelo menos um arquivo `row_usage` para manter as evidências do TF-5 anexadas
   para o ticket de lançamento.

## Fluxo de implementação em etapas

Use três fases determinísticas para cada frota. Avance somente após a saída
os critérios em cada fase são satisfeitos e documentados no conjunto de evidências.| Fase | Escopo | Critérios de saída | Anexos |
|-------|-------|---------------|------------|
| Piloto (P1) | 1 nó de plano de controle + 1 nó de plano de dados por região | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% por 48h, zero incidentes do Alertmanager e um exercício de reversão aprovado. | Pacote de ambos os hosts (JSONs de bancada, exportação Grafana com anotação piloto, logs de reversão). |
| Rampa (P2) | ≥50% dos validadores mais pelo menos uma via de arquivo por cluster | Execução da GPU sustentada por 5 dias, não mais que 1 pico de downgrade >10min, e contadores Prometheus comprovam alerta de fallbacks em 60s. | Exportação Grafana atualizada mostrando a anotação de rampa, diferenças de raspagem Prometheus, captura de tela/log do Alertmanager. |
| Padrão (P3) | Nós restantes; FASTPQ marcou padrão em `iroha_config` | Manifesto de banco assinado + exportação Grafana referenciando a curva de adoção final e exercício de reversão documentado demonstrando a alternância de configuração. | Manifesto final, Grafana JSON, log de reversão, referência de ticket para revisão de alteração de configuração. |

Documente cada etapa da promoção no tíquete de lançamento e vincule diretamente ao
Anotações `grafana_fastpq_acceleration.json` para que os revisores possam correlacionar o
cronograma com as evidências.

## Exercícios de reversão

Cada estágio de implementação deve incluir um ensaio de reversão:

1. Escolha um nó por cluster e registre as métricas atuais:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Force o modo CPU por 10 minutos usando o botão de configuração
   (`zk.fastpq.execution_mode = "cpu"`) ou a substituição do ambiente:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Confirme o log de downgrade
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) e raspe
   o terminal Prometheus novamente para mostrar os incrementos do contador.
4. Restaure o modo GPU, verifique se `telemetry::fastpq.execution_mode` agora reporta
   `resolved="metal"` (ou `resolved="cuda"/"opencl"` para pistas não metálicas),
   confirme se o scrape Prometheus contém as amostras de CPU e GPU em
   `fastpq_execution_mode_total{backend=…}` e registre o tempo decorrido em
   detecção/limpeza.
5. Armazene transcrições de shell, métricas e reconhecimentos de operadores como
   `rollback_drill.log` e `metrics_rollback.prom` no pacote de implementação. Estes
   os arquivos devem ilustrar o ciclo completo de downgrade + restauração porque
   `ci/check_fastpq_rollout.sh` agora falha sempre que o log não possui a GPU
   linha de recuperação ou o instantâneo de métricas omite os contadores de CPU ou GPU.

Esses logs provam que cada cluster pode ser degradado normalmente e que as equipes de SRE
saiba como retroceder deterministicamente se os drivers ou kernels da GPU regredirem.

## Evidência de fallback de modo misto (WP2-E.6)

Sempre que um host precisar de GPU FFT/LDE, mas de hashing CPU Poseidon (de acordo com o Stage7 <900ms
requisito), agrupe os seguintes artefatos junto com os logs de reversão padrão:1. **Config diff.** Faça check-in (ou anexe) a substituição host-local que define
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) ao sair
   `zk.fastpq.execution_mode` intocado. Nomeie o patch
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Contra-arranhões de Poseidon.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   A captura deve mostrar `path="cpu_forced"` incrementando em lock-step com o
   Contador GPU FFT/LDE para essa classe de dispositivo. Faça uma segunda raspagem depois de reverter
   de volta ao modo GPU para que os revisores possam ver o currículo da linha `path="gpu"`.

   Passe o arquivo resultante para `wrap_benchmark.py --poseidon-metrics …` para que o benchmark empacotado registre os mesmos contadores dentro de sua seção `poseidon_metrics`; isso mantém as implementações de Metal e CUDA no fluxo de trabalho idêntico e torna a evidência de fallback auditável sem abrir arquivos de raspagem separados.
3. **Trecho do log.** Copie as entradas `telemetry::fastpq.poseidon` que comprovam o
   resolvedor invertido para CPU (`cpu_forced`) em
   `poseidon_fallback.log`, mantendo carimbos de data e hora para que os cronogramas do Alertmanager possam ser
   correlacionado com a mudança de configuração.

O CI impõe verificações de fila/preenchimento zero hoje; assim que o portão de modo misto pousar,
`ci/check_fastpq_rollout.sh` também insistirá que qualquer pacote contendo
`poseidon_fallback.patch` envia o instantâneo `metrics_poseidon.prom` correspondente.
Seguir este fluxo de trabalho mantém a política de fallback WP2-E.6 auditável e vinculada a
os mesmos coletores de evidências usados durante a implementação padrão.

## Relatórios e automação

- Anexe todo o diretório `artifacts/fastpq_rollouts/<stamp>/` ao
  libere o tíquete e faça referência a ele em `status.md` assim que a implementação for concluída.
- Execute `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (via
  `promtool`) dentro do CI para garantir que os pacotes de alertas incluídos na implementação ainda
  compilar.
- Valide o pacote com `ci/check_fastpq_rollout.sh` (ou
  `make check-fastpq-rollout`) e passe `FASTPQ_ROLLOUT_BUNDLE=<path>` quando você
  deseja direcionar um único lançamento. CI invoca o mesmo script via
  `.github/workflows/fastpq-rollout.yml`, portanto, artefatos ausentes falham rapidamente antes de um
  o ticket de liberação pode fechar. O pipeline de lançamento pode arquivar pacotes validados
  juntamente com os manifestos assinados, passando
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` para
  `scripts/run_release_pipeline.py`; o ajudante repete
  `ci/check_fastpq_rollout.sh` (a menos que `--skip-fastpq-rollout-check` esteja definido) e
  copia a árvore de diretórios em `artifacts/releases/<version>/fastpq_rollouts/…`.
  Como parte deste portão, o script impõe a profundidade da fila Stage7 e o preenchimento zero
  orçamentos lendo `benchmarks.metal_dispatch_queue` e
  `benchmarks.zero_fill_hotspots` de cada banco `metal` JSON.

Seguindo este manual, podemos demonstrar a adoção determinística, fornecer uma
pacote único de evidências por implementação e manter os exercícios de reversão auditados juntamente com
os manifestos de benchmark assinados.