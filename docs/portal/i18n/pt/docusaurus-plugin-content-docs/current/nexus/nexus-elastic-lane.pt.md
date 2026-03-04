---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: Provisionamento de pista elástica (NX-7)
sidebar_label: Provisionamento de pista elástica
description: Fluxo de bootstrap para criar manifestos de lane Nexus, entradas de catálogo e evidências de rollout.
---

:::nota Fonte canônica
Esta página espelha `docs/source/nexus_elastic_lane.md`. Mantenha ambas as cópias homologadas até que a varredura de localização chegue ao portal.
:::

# Kit de provisionamento de pista elástica (NX-7)

> **Item do roadmap:** NX-7 - ferramental de provisionamento de pista elástica  
> **Status:** ferramentas completas - gera manifestos, snippets de catálogo, payloads Norito, testes de fumaça,
> e o helper de bundle de load-test agora costura gating de latência por slot + manifestos de evidência para que as rodadas de carga de validadores
> podem ser publicadas sem script sob medida.

Este guia leva operadores pelo novo helper `scripts/nexus_lane_bootstrap.sh` que automatiza a geração de manifestos de lane, snippets de catálogo de lane/dataspace e evidências de rollout. O objetivo é facilitar a criação de novas faixas Nexus (publicas ou privadas) sem editar manualmente vários arquivos nem derivar manualmente a geometria do catálogo.

## 1. Pré-requisitos

1. Aprovação de governança para o alias de lane, dataspace, conjunto de validadores, tolerância a falhas (`f`) e política de liquidação.
2. Uma lista final de validadores (IDs de conta) e uma lista de namespaces protegidos.
3. Acesso ao repositório de configuração do node para poder anexar os snippets gerados.
4. Caminhos para o registro de manifestos de pista (veja `nexus.registry.manifest_directory` e `cache_directory`).
5. Contatos de telemetria/atendimentos do PagerDuty para a pista, para que os alertas sejam conectados assim que a pista estiver online.

## 2. Gere artistas de lane

Execute o helper a partir da raiz do repositório:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Chave das bandeiras:

- `--lane-id` deve representar o índice da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlam a entrada do catálogo do dataspace (por padrão usa o id do lane quando omitido).
- `--validator` pode ser repetido ou lido de `--validators-file`.
- `--route-instruction` / `--route-account` emitem regras de roteamento prontas para colar.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) captura contatos de runbook para que os dashboards mostrem os proprietários corretos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` adicionam o gancho de atualização de tempo de execução ao manifesto quando os controles de pista exigem controles estendidos do operador.
- `--encode-space-directory` chama `cargo xtask space-directory encode` automaticamente. Combine com `--space-directory-out` quando quiser que o arquivo `.to` codificado va para outro lugar além do padrão.

O script produz três artefatos dentro do `--output-dir` (por padrão o diretor atual), mais um quarto opcional quando a codificação está habilitada:

1. `<slug>.manifest.json` - manifesto de pista contendo o quorum de validadores, namespaces protegidos e metadados enviados do gancho de runtime-upgrade.
2. `<slug>.catalog.toml` - um snippet TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e quaisquer regras de roteamento solicitadas. Garanta que `fault_tolerance` esteja definido na entrada do espaço de dados para dimensionar o comitê de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumo de auditoria descrevendo a geometria (slug, segmentos, metadados) mais os passos de rollout necessários e o comando exato de `cargo xtask space-directory encode` (em `space_directory_encode.command`). Anexe esse JSON ao ticket de onboarding como evidência.
4. `<slug>.manifest.to` - quando emitido `--encode-space-directory` está habilitado; pronto para o fluxo `iroha app space-directory manifest publish` do Torii.

Use `--dry-run` para visualizar os JSON/snippets sem gravar arquivos e `--force` para sobrescrever artefatos existentes.

## 3. Aplique as mudancas1. Copie o manifesto JSON para o `nexus.registry.manifest_directory` configurado (e para o diretório de cache se o registro espelhar pacotes remotos). Comite o arquivo se manifests são versionados no seu repositório de configuração.
2. Anexo o trecho de catálogo em `config/config.toml` (ou não `config.d/*.toml` de proteção). Garanta que `nexus.lane_count` seja pelo menos `lane_id + 1` e atualize qualquer `nexus.routing_policy.rules` que devam apontar para a nova pista.
3. Codifique (se você pressionar `--encode-space-directory`) e publique o manifesto no Space Directory usando o comando capturado no resumo (`space_directory_encode.command`). Isso produz o payload `.manifest.to` que o Torii espera e registra evidências para auditores; envie via `iroha app space-directory manifest publish`.
4. Execute `irohad --sora --config path/to/config.toml --trace-config` e arquive a saida do trace no ticket de rollout. Isso prova que uma nova geometria corresponde aos slug/segmentos de Kura gerados.
5. Reinicie os validadores atribuídos ao caminho quando as mudanças de manifesto/catálogo forem implantadas. Mantenha o resumo JSON no ticket para auditorias futuras.

## 4. Monte um pacote de distribuição do registro

Empacote o manifesto gerado e o overlay para que os operadores possam distribuir dados de governança de pistas sem editar configurações em cada host. O helper de bundling copia manifests para o layout canonico, produz uma sobreposição opcional do catálogo de governança para `nexus.registry.cache_directory` e pode emitir um tarball para transferências offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Disse:

1. `manifests/<slug>.manifest.json` - copie estes arquivos para o `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - coloque em `nexus.registry.cache_directory`. Cada entrada `--module` vira uma definição de módulo plugável, permitindo swap-outs do módulo de governança (NX-2) ao atualizar ou sobrepor o cache em vez de editar `config.toml`.
3. `summary.json` - inclui hashes, metadados de sobreposição e instruções para operadoras.
4. Opcional `registry_bundle.tar.*` - pronto para SCP, S3 ou rastreadores de artistas.

Sincronize o diretório inteiro (ou o arquivo) para cada validador, extraia em hosts air-gapped e copie os manifests + overlay de cache para seus caminhos de registro antes de reiniciar o Torii.

## 5. Testes de fumaça de validadores

Depois que o Torii for reiniciado, execute o novo helper de fumaça para verificar se o relatório de pista `manifest_ready=true`, se as métricas expoem a contagem esperada de pistas e se o medidor de selado estiver limpo. As faixas que desativam manifestos devem expor um `manifest_path` não vazio; o helper agora falha imediatamente quando falta o caminho para que cada implantação do NX-7 inclua a evidência do manifesto contratado:

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

Adicione `--insecure` ao testar ambientes autoassinados. O script sai com código não zero se a pista estiver ausente, selada ou se as métricas/telemetria divergirem dos valores esperados. Utilize os botões `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` para manter a telemetria por pista (altura de bloco/finalidade/backlog/headroom) dentro dos seus limites operacionais e combinar com `--max-slot-p95` / `--max-slot-p99` (mais `--min-slot-samples`) para importar as metas de duração do slot NX-18 sem sair do helper.

Para validações air-gapped (ou CI) você pode reproduzir uma resposta Torii capturada ao acessar um endpoint live:

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

Os fixtures gravados em `fixtures/nexus/lanes/` refletem os artefatos produzidos pelo helper de bootstrap para que novos manifestos possam ser lintados sem scripts sob medida. Um CI executa o mesmo fluxo via `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para provar que o helper de smoke NX-7 continua compatível com o formato de payload publicado e garantir que os digests/overlays do bundle sejam reproduzíveis.