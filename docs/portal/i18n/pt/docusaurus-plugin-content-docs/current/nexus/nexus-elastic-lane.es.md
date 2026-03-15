---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: Provisionamento de pista elástica (NX-7)
sidebar_label: Provisão de pista elástica
description: Fluxo de bootstrap para criar manifestos da pista Nexus, entradas de catálogo e evidências de implementação.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_elastic_lane.md`. Mantenha ambas as cópias alinhadas até que o barrido de localização chegue ao portal.
:::

# Kit de provisionamento de pista elástica (NX-7)

> **Elemento do roteiro:** NX-7 - ferramenta de provisionamento de pista elástica  
> **Estado:** ferramentas completas - manifestos de gênero, trechos de catálogo, cargas úteis Norito, testes de humor,
> e o ajudante de pacote de teste de carga agora combina gating de latência por slot + manifestos de evidência para que as corridas de carga de validadores
> se publica sem scripts à medida.

Este guia levará os operadores pelo novo auxiliar `scripts/nexus_lane_bootstrap.sh` que automatiza a geração de manifesto de pista, trechos de catálogo de pista/espaço de dados e evidências de implementação. O objetivo é facilitar a alta de novas faixas Nexus (públicas ou privadas) sem editar a mão múltiplos arquivos e redefinir a geometria do catálogo à mão.

## 1. Pré-requisitos

1. Aprovação de governança para o pseudônimo de lane, dataspace, conjunto de validadores, tolerância a falhas (`f`) e política de liquidação.
2. Uma lista final de validadores (IDs de conta) e uma lista de namespaces protegidos.
3. Acesse o repositório de configuração do nó para poder anexar os snippets gerados.
4. Rotas para o registro de manifestos de pista (versão `nexus.registry.manifest_directory` e `cache_directory`).
5. Contatos de telemetria/cabos de PagerDuty para a pista, para que os alertas sejam conectados quando a pista estiver online.

## 2. Gêneros de artefatos de pista

Execute o helper da raiz do repositório:

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

Clave de bandeiras:

- `--lane-id` deve coincidir com o índice da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlam a entrada do catálogo do espaço de dados (por defeito, usa o ID da pista quando é omitido).
- `--validator` pode ser repetido ou lido a partir de `--validators-file`.
- `--route-instruction` / `--route-account` emite regras de enrutamiento listas para pegar.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) captura contatos do runbook para que os painéis mostrem os proprietários corretos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` adiciona o gancho de atualização de tempo de execução ao manifesto quando a pista requer controles estendidos de operadores.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` automaticamente. Combine-o com `--space-directory-out` quando quiser que o arquivo `.to` codificado vá para um lugar diferente do padrão.

O script produz três artefatos dentro de `--output-dir` (por defeito do diretório atual), mas um quarto opcional quando a codificação é habilitada:

1. `<slug>.manifest.json` - manifesto de pista que contém o quorum de validadores, namespaces protegidos e metadados opcionais do gancho de atualização de tempo de execução.
2. `<slug>.catalog.toml` - um snippet TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e qualquer regulamento de inscrição solicitado. Certifique-se de que `fault_tolerance` esteja configurado na entrada do espaço de dados para dimensionar o comitê de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumo de auditorias que descrevem a geometria (slug, segmentos, metadados) mas as etapas de implementação necessárias e o comando exato de `cargo xtask space-directory encode` (abaixo `space_directory_encode.command`). Adjunta este JSON ao ticket de onboarding como evidência.
4. `<slug>.manifest.to` - emitido quando `--encode-space-directory` está ativado; lista para o fluxo `iroha app space-directory manifest publish` de Torii.

Use `--dry-run` para pré-visualizar JSON/snippets sem escrever arquivos, e `--force` para sobrescrever artefatos existentes.

## 3. Aplicar as mudanças1. Copie o manifesto JSON no `nexus.registry.manifest_directory` configurado (e no cache do diretório se o registro refletir pacotes remotos). Envie o arquivo se os manifestos forem atualizados em seu repositório de configuração.
2. Anexe o trecho de catálogo para `config/config.toml` (ou para `config.d/*.toml` correspondente). Certifique-se de que `nexus.lane_count` seja pelo menos `lane_id + 1` e atualize qualquer `nexus.routing_policy.rules` que deba apuntar na nova pista.
3. Codifique (omita `--encode-space-directory`) e publique o manifesto no Space Directory usando o comando capturado no resumo (`space_directory_encode.command`). Isso produz a carga útil `.manifest.to` que Torii espera e registra a evidência para auditores; envie-me `iroha app space-directory manifest publish`.
4. Execute `irohad --sora --config path/to/config.toml --trace-config` e arquive a saída de rastreamento no ticket de implementação. Isso prova que a nova geometria coincide com os slug/segmentos de Kura gerados.
5. Reinicie os validadores atribuídos à pista uma vez que as alterações do manifesto/catálogo estejam desplegadas. Guarde o resumo JSON no ticket para auditorias futuras.

## 4. Construa um pacote de distribuição de registro

Empaque o manifesto gerado e a sobreposição para que os operadores possam distribuir dados de governança de pistas sem editar configurações em cada host. O ajudante de empacotamento copia manifestado no layout canônico, produz uma sobreposição opcional do catálogo de governo para `nexus.registry.cache_directory`, e pode emitir um tarball para transferências offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Saídas:

1. `manifests/<slug>.manifest.json` - copie esses arquivos no `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - deixe isso em `nexus.registry.cache_directory`. Cada entrada `--module` é convertida em uma definição de módulo enchufável, habilitando trocas do módulo de governança (NX-2) para atualizar a sobreposição de cache em vez de editar `config.toml`.
3. `summary.json` - inclui hashes, metadados de sobreposição e instruções para operadores.
4. Opcional `registry_bundle.tar.*` - lista para SCP, S3 ou rastreadores de artefatos.

Sincroniza todo o diretório (ou arquivo) para cada validador, extrai os hosts air-gapped e copia os manifestos + sobreposição de cache nas rotas do registro antes de reiniciar o Torii.

## 5. Testes de humor para validadores

Após reiniciar Torii, execute o novo assistente de fumaça para verificar se a pista reporta `manifest_ready=true`, que as métricas expõem o conteúdo esperado das pistas e que o medidor de vedação está limpo. As pistas que requerem manifestos devem expor um `manifest_path` no vazio; o ajudante agora falha imediatamente quando falta a rota para que cada despliegue NX-7 inclua a evidência do manifesto firmado:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
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

Agrega `--insecure` quando você testa ambientes com certificados autoassinados. O script termina com código sem zero se a pista estiver faltando, esta selada ou as métricas/telemetria são desalineadas dos valores esperados. Use os botões `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` para manter a telemetria por pista (altura de bloqueio/finalidade/backlog/headroom) dentro de seus limites operacionais, e combine-os com `--max-slot-p95` / `--max-slot-p99` (mas `--min-slot-samples`) para impor os objetivos de duração do slot NX-18 sem sair do helper.

Para validações air-gapped (ou CI) você pode reproduzir uma resposta Torii capturada em vez de golpear um endpoint vivo:

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

Os fixtures capturados abaixo de `fixtures/nexus/lanes/` refletem os artefatos produzidos pelo auxiliar de bootstrap para que os novos manifestos possam ser lint-ear sem scripts na medida. CI executa o mesmo fluxo via `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para demonstrar que o auxiliar de fumaça NX-7 segue alinhado com o formato de carga útil publicado e para garantir que os resumos/sobreposições do pacote sejam reproduzidos.