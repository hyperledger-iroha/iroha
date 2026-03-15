---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19518008c228037d6630fc4d085b0f5f01975008ca79858ca20623cb048314ca
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: Provisionamento de lane elastico (NX-7)
sidebar_label: Provisionamento de lane elastico
description: Fluxo de bootstrap para criar manifests de lane Nexus, entradas de catalogo e evidencia de rollout.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/nexus_elastic_lane.md`. Mantenha ambas as copias alinhadas ate que o sweep de localizacao chegue ao portal.
:::

# Kit de provisionamento de lane elastico (NX-7)

> **Item do roadmap:** NX-7 - tooling de provisionamento de lane elastico  
> **Status:** tooling completo - gera manifests, snippets de catalogo, payloads Norito, smoke tests,
> e o helper de bundle de load-test agora costura gating de latencia por slot + manifests de evidencia para que as rodadas de carga de validadores
> possam ser publicadas sem scripting sob medida.

Este guia leva operadores pelo novo helper `scripts/nexus_lane_bootstrap.sh` que automatiza a geracao de manifests de lane, snippets de catalogo de lane/dataspace e evidencia de rollout. O objetivo e facilitar a criacao de novas lanes Nexus (publicas ou privadas) sem editar manualmente varios arquivos nem re-derivar a geometria do catalogo manualmente.

## 1. Prerequisitos

1. Aprovacao de governanca para o alias de lane, dataspace, conjunto de validadores, tolerancia a falhas (`f`) e politica de settlement.
2. Uma lista final de validadores (IDs de conta) e uma lista de namespaces protegidos.
3. Acesso ao repositorio de configuracao do node para poder anexar os snippets gerados.
4. Caminhos para o registro de manifests de lane (veja `nexus.registry.manifest_directory` e `cache_directory`).
5. Contatos de telemetria/handles do PagerDuty para o lane, para que os alertas sejam conectados assim que o lane estiver online.

## 2. Gere artefatos de lane

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

- `--lane-id` deve corresponder ao index da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlam a entrada de catalogo do dataspace (por padrao usa o id do lane quando omitido).
- `--validator` pode ser repetido ou lido de `--validators-file`.
- `--route-instruction` / `--route-account` emitem regras de roteamento prontas para colar.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) captura contatos de runbook para que os dashboards mostrem os owners corretos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` adicionam o hook de runtime-upgrade ao manifest quando o lane requer controles estendidos de operador.
- `--encode-space-directory` chama `cargo xtask space-directory encode` automaticamente. Combine com `--space-directory-out` quando quiser que o arquivo `.to` codificado va para outro lugar alem do default.

O script produz tres artefatos dentro do `--output-dir` (por padrao o diretorio atual), mais um quarto opcional quando o encoding esta habilitado:

1. `<slug>.manifest.json` - manifest de lane contendo o quorum de validadores, namespaces protegidos e metadados opcionais do hook de runtime-upgrade.
2. `<slug>.catalog.toml` - um snippet TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e quaisquer regras de roteamento solicitadas. Garanta que `fault_tolerance` esteja definido na entrada de dataspace para dimensionar o comite de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumo de auditoria descrevendo a geometria (slug, segmentos, metadados) mais os passos de rollout requeridos e o comando exato de `cargo xtask space-directory encode` (em `space_directory_encode.command`). Anexe esse JSON ao ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - emitido quando `--encode-space-directory` esta habilitado; pronto para o fluxo `iroha app space-directory manifest publish` do Torii.

Use `--dry-run` para visualizar os JSON/snippets sem gravar arquivos e `--force` para sobrescrever artefatos existentes.

## 3. Aplique as mudancas

1. Copie o manifest JSON para o `nexus.registry.manifest_directory` configurado (e para o cache directory se o registry espelha bundles remotos). Comite o arquivo se manifests sao versionados no seu repositorio de configuracao.
2. Anexe o snippet de catalogo em `config/config.toml` (ou no `config.d/*.toml` apropriado). Garanta que `nexus.lane_count` seja pelo menos `lane_id + 1` e atualize quaisquer `nexus.routing_policy.rules` que devam apontar para o novo lane.
3. Encode (se voce pulou `--encode-space-directory`) e publique o manifest no Space Directory usando o comando capturado no summary (`space_directory_encode.command`). Isso produz o payload `.manifest.to` que o Torii espera e registra evidencia para auditores; envie via `iroha app space-directory manifest publish`.
4. Execute `irohad --sora --config path/to/config.toml --trace-config` e arquive a saida do trace no ticket de rollout. Isso prova que a nova geometria corresponde ao slug/segmentos de Kura gerados.
5. Reinicie os validadores atribuidos ao lane quando as mudancas de manifest/catalogo estiverem implantadas. Mantenha o summary JSON no ticket para auditorias futuras.

## 4. Monte um bundle de distribuicao do registry

Empacote o manifest gerado e o overlay para que operadores possam distribuir dados de governanca de lanes sem editar configs em cada host. O helper de bundling copia manifests para o layout canonico, produz um overlay opcional do catalogo de governanca para `nexus.registry.cache_directory` e pode emitir um tarball para transferencias offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Saidas:

1. `manifests/<slug>.manifest.json` - copie estes arquivos para o `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - coloque em `nexus.registry.cache_directory`. Cada entrada `--module` vira uma definicao de modulo plugavel, permitindo swap-outs do modulo de governanca (NX-2) ao atualizar o overlay de cache em vez de editar `config.toml`.
3. `summary.json` - inclui hashes, metadados do overlay e instrucoes para operadores.
4. Opcional `registry_bundle.tar.*` - pronto para SCP, S3 ou trackers de artefatos.

Sincronize o diretorio inteiro (ou o arquivo) para cada validador, extraia em hosts air-gapped e copie os manifests + overlay de cache para seus caminhos de registry antes de reiniciar o Torii.

## 5. Smoke tests de validadores

Depois que o Torii reiniciar, execute o novo helper de smoke para verificar se o lane reporta `manifest_ready=true`, se as metricas expoem a contagem esperada de lanes e se o gauge de sealed esta limpo. Lanes que exigem manifests devem expor um `manifest_path` nao vazio; o helper agora falha imediatamente quando o caminho falta para que cada deploy NX-7 inclua a evidencia do manifest assinado:

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
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Adicione `--insecure` ao testar ambientes self-signed. O script sai com codigo nao zero se o lane estiver ausente, sealed ou se metricas/telemetria divergirem dos valores esperados. Use os knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` para manter telemetria por lane (altura de bloco/finalidade/backlog/headroom) dentro dos seus limites operacionais e combine com `--max-slot-p95` / `--max-slot-p99` (mais `--min-slot-samples`) para impor as metas de duracao de slot NX-18 sem sair do helper.

Para validacoes air-gapped (ou CI) voce pode reproduzir uma resposta Torii capturada em vez de acessar um endpoint live:

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
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Os fixtures gravados em `fixtures/nexus/lanes/` refletem os artefatos produzidos pelo helper de bootstrap para que novos manifests possam ser lintados sem scripting sob medida. A CI executa o mesmo fluxo via `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para provar que o helper de smoke NX-7 continua compativel com o formato de payload publicado e garantir que os digests/overlays do bundle fiquem reproduziveis.
