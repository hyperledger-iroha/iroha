---
lang: pt
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

# Toolkit de provisionamento de lane elastica (NX-7)

> **Item do roadmap:** NX-7 - tooling de provisionamento de lane elastica  
> **Status:** Tooling completo - gera manifests, catalog snippets, payloads Norito, smoke tests, e o
> helper de bundle de load-test agora une o gating de latencia de slots + manifests de evidencia
> para que as corridas de carga de validadores sejam publicadas sem scripting sob medida.

Este guia orienta os operadores no helper `scripts/nexus_lane_bootstrap.sh` que automatiza a geracao
 de manifests de lane, snippets de catalogo lane/dataspace e evidencia de rollout. O objetivo e
facilitar a criacao de novas lanes Nexus (publicas ou privadas) sem editar multiplos arquivos ou
re-derivar a geometria do catalogo na mao.

## 1. Prerequisitos

1. Aprovacao de governanca para o alias da lane, dataspace, set de validadores, tolerancia a falhas
   (`f`) e politica de settlement.
2. Uma lista final de validadores (account IDs) e uma lista de namespaces protegidos.
3. Acesso ao repositorio de configuracao de nos para anexar os snippets gerados.
4. Caminhos para o registry de manifests de lane (veja `nexus.registry.manifest_directory` e
   `cache_directory`).
5. Contatos de telemetria/PagerDuty para a lane para que alertas sejam conectados ao ativar.

## 2. Gerar artefatos de lane

Execute o helper a partir da raiz do repositorio:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Flags chave:

- `--lane-id` deve corresponder ao indice da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlam a entrada do catalogo de dataspace
  (padrao usa o lane id se omitido).
- `--validator` pode ser repetido ou vindo de `--validators-file`.
- `--route-instruction` / `--route-account` emitem regras de roteamento prontas para colar.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capturam contatos de runbook
  para que dashboards exibam os owners corretos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` adicionam o hook de runtime-upgrade ao manifest
  quando a lane requer controles estendidos do operador.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` automaticamente. Use
  `--space-directory-out` quando quiser o arquivo `.to` em outro caminho.

O script produz tres artefatos dentro de `--output-dir` (padrao o diretorio atual), mais um quarto
 opcional quando o encoding esta ativo:

1. `<slug>.manifest.json` - manifest de lane contendo quorum de validadores, namespaces protegidos e
   metadata opcional do hook de runtime-upgrade.
2. `<slug>.catalog.toml` - snippet TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e
   regras de roteamento solicitadas. Garanta que `fault_tolerance` esteja definido na entrada de
   dataspace para dimensionar o comite lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumo de auditoria descrevendo a geometria (slug, segmentos, metadata)
   mais os passos de rollout e o comando exato `cargo xtask space-directory encode`
   (em `space_directory_encode.command`). Anexe este JSON ao ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - emitido quando `--encode-space-directory` esta ativo; pronto para o fluxo
   `iroha app space-directory manifest publish` do Torii.

Use `--dry-run` para previsualizar os JSON/snippets sem escrever arquivos, e `--force` para
sobrescrever artefatos existentes.

## 3. Aplicar as mudancas

1. Copie o manifest JSON para o `nexus.registry.manifest_directory` configurado (e para o diretorio
   cache se o registry espelhar bundles remotos). Comite o arquivo se os manifests forem
   versionados no seu repo de configuracao.
2. Anexe o snippet de catalogo a `config/config.toml` (ou `config.d/*.toml`). Garanta que
   `nexus.lane_count` seja pelo menos `lane_id + 1`, e atualize quaisquer
   `nexus.routing_policy.rules` que devam apontar para a nova lane.
3. Codifique (se voce pulou `--encode-space-directory`) e publique o manifest no Space Directory
   usando o comando capturado no summary (`space_directory_encode.command`). Isso produz o payload
   `.manifest.to` esperado pelo Torii e registra evidencia para auditores; envie via
   `iroha app space-directory manifest publish`.
4. Execute `irohad --sora --config path/to/config.toml --trace-config` e arquive a saida de trace no
   ticket de rollout. Isso prova que a nova geometria corresponde ao slug/segmentos gerados.
5. Reinicie os validadores atribuidores a lane quando as mudancas de manifest/catalogo forem
   implantadas. Mantenha o summary JSON no ticket para auditorias futuras.

## 4. Construir um bundle de distribuicao do registry

Quando o manifest, o snippet do catalogo e o summary estiverem prontos, empacote-os para
 distribuicao aos validadores. O novo bundler copia manifests para o layout esperado por
`nexus.registry.manifest_directory` / `cache_directory`, emite um overlay de catalogo de governanca
para que modulos possam ser trocados sem editar o config principal, e opcionalmente arquiva o
bundle:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Resultados:

1. `manifests/<slug>.manifest.json` - copie para `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - coloque em `nexus.registry.cache_directory` para sobrescrever ou
   trocar modulos de governanca (`--module ...` sobrescreve o catalogo cacheado). Este e o caminho
   de modulo plugavel para NX-2: substitua uma definicao de modulo, rode o bundler novamente e
   distribua o overlay de cache sem tocar em `config.toml`.
3. `summary.json` - inclui digests SHA-256 / Blake2b para cada manifest mais metadata do overlay.
4. `registry_bundle.tar.*` opcional - pronto para Secure Copy / armazenamento de artefatos.

Se o seu deploy espelha bundles para hosts air-gapped, sincronize todo o diretorio de saida (ou o
 tarball gerado). Nos online podem montar o diretorio de manifests diretamente, enquanto nos offline
consomem o tarball, extraem e copiam manifests + overlay de cache para seus caminhos configurados.

## 5. Smoke tests de validadores

Apos reiniciar o Torii, rode o helper de smoke para verificar que a lane reporta `manifest_ready=true`,
que as metricas expoem a contagem esperada de lanes, e que o gauge sealed esta limpo. Lanes que
exigem manifest agora devem expor `manifest_path` nao vazio - o helper falha rapido quando essa
 evidencia falta para que os controles de mudanca NX-7 incluam referencias do bundle assinado.

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Adicione `--insecure` quando testar ambientes self-signed. O script sai com codigo nao zero se a
lane estiver ausente, estiver sealed, ou se metricas/telemetria divergirem dos valores esperados.
Use os novos knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e
`--max-headroom-events` para manter telemetria de altura/finalidade/backlog/headroom dentro das
suas envelopes operacionais. Combine com `--max-slot-p95/--max-slot-p99` (mais `--min-slot-samples`)
para aplicar o SLO de duracao de slots NX-18 diretamente no helper, e passe
`--allow-missing-lane-metrics` apenas quando clusters de staging ainda nao expuserem esses gauges
(em producao, mantenha os defaults).

O mesmo helper agora enforce a telemetria de load-test do scheduler. Use `--min-teu-capacity` para
provar que cada lane reporta `nexus_scheduler_lane_teu_capacity`, limite a utilizacao de slots com
`--max-teu-slot-commit-ratio` (compara `nexus_scheduler_lane_teu_slot_committed` contra a capacidade),
e mantenha os contadores de deferral/truncation em zero via `--max-teu-deferrals` e
`--max-must-serve-truncations`. Esses knobs transformam o requisito NX-7 de "load tests mais
profundos" em um check CLI repetivel: o helper falha quando uma lane adia trabalho PQ/TEU ou quando
o TEU comprometido por slot passa do headroom configurado, e o CLI imprime o resumo por lane para
que pacotes de evidencia capturem os mesmos numeros validados pela CI.

Para validacoes air-gapped (ou CI) voce pode reproduzir uma resposta capturada do Torii em vez de
consultar um node vivo:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

As fixtures em `fixtures/nexus/lanes/` espelham os artefatos produzidos pelo helper de bootstrap
para que novos manifests possam ser lint sem scripting sob medida. A CI exercita o mesmo fluxo via
`ci/check_nexus_lane_smoke.sh` e tambem roda `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`) para provar que o helper smoke NX-7 permanece compativel com o
formato de payload publicado e garantir que digests/overlays de bundle continuam reproduziveis.

Quando uma lane e renomeada, capture eventos de telemetria `nexus.lane.topology` (por exemplo com
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) e alimente-os no helper de
smoke. O novo flag `--telemetry-file/--from-telemetry` aceita o log newline-delimited e
`--require-alias-migration old:new` assegura que um evento `alias_migrated` registrou o rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

O fixture `telemetry_alias_migrated.ndjson` agrupa o exemplo canonico de rename para que a CI
verifique o path de parsing de telemetria sem contactar um node vivo.

## 6. Testes de carga de validadores (evidencia NX-7)

O roadmap **NX-7** exige que operadores de lane capturem um run de carga reproduzivel antes de uma
lane ser marcada como pronta para producao. O objetivo e estressar a lane o suficiente para exercitar
slot duration, backlog de settlement, quorum DA, oraculos, scheduler headroom e metricas TEU, e
entao arquivar o resultado de forma que auditores possam reproduzir sem tooling ad hoc. O novo helper
`scripts/nexus_lane_load_test.py` une os checks de smoke, o gating de slot-duration e o slot bundle
manifest em um conjunto de artefatos para publicar runs de carga diretamente nos tickets de
 governanca.

### 6.1 Preparacao de workload

1. Crie um diretorio de run e capture fixtures canonicas para a lane sob teste:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   As fixtures espelham `fixtures/nexus/lane_commitments/*.json` e fornecem ao gerador de carga uma
   seed deterministica (registre a seed em `artifacts/.../README.md`).
2. Baseline da lane antes da corrida:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   Mantenha stdout/stderr no diretorio de run para que os thresholds de smoke sejam auditaveis.
3. Capture o log de telemetria que vai alimentar `--telemetry-file` (evidencia de alias migration) e
   `validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Inicie o workload da lane (perfil k6, replay harness, ou testes de ingestao federados) e mantenha
   a seed de workload + range de slots a mao; a metadata e consumida pelo validador do telemetry
   manifest na secao 6.3.

5. Empacote a evidencia do run com o novo helper. Forneca os payloads capturados de status/metrics/
   telemetry, os aliases de lane, e qualquer alias migration que deva aparecer em telemetria. O
   helper escreve `smoke.log`, `slot_summary.json`, um slot bundle manifest e `load_test_manifest.json`
   unindo tudo para revisao de governanca:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   O comando enforce os mesmos gates de quorum DA, oracle, settlement buffer, TEU e slot-duration
   usados neste guia e produz um manifest pronto para anexar sem scripting sob medida.

### 6.2 Run instrumentado

Enquanto o workload satura a lane:

1. Snapshot de status + metrics do Torii:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Calcule quantis de slot-duration e arquive o summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Exporte o snapshot de lane-governance como JSON + Parquet para auditorias de longo prazo:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   O snapshot JSON/Parquet agora registra utilizacao TEU, niveis de trigger do scheduler, contadores
   RBC chunk/bytes e estatisticas do grafo de transacoes por lane para que a evidencia de rollout
   mostre backlog e pressao de execucao.

4. Rode o helper de smoke novamente no pico de carga para avaliar thresholds sob estresse (grave a
   saida em `smoke_during.log`) e rode outra vez quando o workload terminar (`smoke_after.log`).

### 6.3 Telemetry pack e manifest de governanca

O diretorio do run deve incluir um telemetry pack (`prometheus.tgz`, stream OTLP, logs estruturados
 e qualquer output do harness). Valide-o e grave a metadata esperada pela governanca:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

Por fim, anexe o log de telemetria capturado e exija evidencia de alias migration quando uma lane
for renomeada durante o teste:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Arquive os seguintes artefatos para o ticket de governanca:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + conteudo do pack (`prometheus.tgz`, `otlp.ndjson`, etc.)
- `nexus.lane.topology.ndjson` (ou o slice de telemetria relevante)

O run agora pode ser referenciado dentro de Space Directory manifests e trackers de governanca como
 o load test NX-7 canonico para a lane.

## 7. Telemetria e follow-ups de governanca

- Atualize os dashboards de lane (`dashboards/grafana/nexus_lanes.json` e overlays relacionados)
  com o novo lane id e metadata. As keys geradas (`contact`, `channel`, `runbook`, etc.) facilitam
  pre-preencher labels.
- Conecte regras de PagerDuty/Alertmanager para a nova lane antes de habilitar admission. O
  `summary.json` espelha a checklist em `docs/source/nexus_operations.md`.
- Registre o manifest bundle no Space Directory quando o set de validadores estiver live. Use o
  mesmo manifest JSON gerado pelo helper, assinado conforme o runbook de governanca.
- Siga `docs/source/sora_nexus_operator_onboarding.md` para smoke tests (FindNetworkStatus,
  reachability do Torii) e capture a evidencia com o conjunto de artefatos produzido acima.

## 8. Exemplo de dry-run

Para previsualizar os artefatos sem escrever arquivos:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

O comando imprime o JSON summary e o snippet TOML em stdout, permitindo iteracao rapida durante o
planejamento.

---

Para mais contexto veja:

- `docs/source/nexus_operations.md` - checklist operacional e requisitos de telemetria.
- `docs/source/sora_nexus_operator_onboarding.md` - fluxo de onboarding detalhado que referencia o
  novo helper.
- `docs/source/nexus_lanes.md` - geometria de lanes, slugs e layout de storage usado pela
  ferramenta.
