---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: Provisionamento de pista elástica (NX-7)
sidebar_label: Provisão de faixa elástica
description: Fluxo de trabalho de inicialização para criar manifestos da via Nexus, entradas de catálogo e testes de implementação.
---

:::nota Fonte canônica
Esta página representa `docs/source/nexus_elastic_lane.md`. Gardez les duas cópias alinhadas até que a vaga de tradução chegue ao portal.
:::

# Kit de provisionamento de pista elástica (NX-7)

> **Elemento do roteiro:** NX-7 - ferramentas de provisionamento de pista elástica  
> **Estatuto:** ferramentas completas - gerar manifestos, trechos de catálogo, cargas úteis Norito, testes de fumaça,
> e o ajudante do pacote de teste de carga monta mantendo o controle de latência por slot + os manifestos de verificação para que as execuções dos validadores de carga
> pode ser publicado sem scripts sob medida.

Este guia acompanha os operadores por meio do novo ajudante `scripts/nexus_lane_bootstrap.sh` que automatiza a geração de manifestos de pista, trechos de catálogo de pista/espaço de dados e pré-lançamentos. O objetivo é facilitar a criação de novas faixas Nexus (públicas ou privadas) sem editar os principais arquivos adicionais e derivar novamente a geometria do catálogo para a principal.

## 1. Pré-requisito

1. Aprovação de governo para o pseudônimo de pista, o espaço de dados, o conjunto de validadores, a tolerância aos painéis (`f`) e a política de liquidação.
2. Uma lista final de validadores (IDs de conta) e uma lista de namespaces protegidos.
3. Acesse o depósito de configuração das pessoas para poder adicionar os snippets gerados.
4. Caminhos para registro de manifestos de pista (veja `nexus.registry.manifest_directory` e `cache_directory`).
5. Contatos telemétricos / manipuladores PagerDuty para a pista para que os alertas soientem conectados de que a pista está online.

## 2. Gerar artefatos de pista

Lance o ajudante depois da racine du depot:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Sinalizadores cle:

- `--lane-id` corresponde ao índice da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlam a entrada do catálogo do espaço de dados (por padrão, o ID da pista quando omitido).
- `--validator` pode ser repetido ou depois de `--validators-file`.
- `--route-instruction` / `--route-account` emitem as regras de rota pretes a coller.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) captura contatos do runbook para que os painéis mostrem bons proprietários.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` adiciona o hook runtime-upgrade ao manifesto quando a pista requer controles operacionais contínuos.
- `--encode-space-directory` invoca automaticamente `cargo xtask space-directory encode`. Combine-o com `--space-directory-out` quando você quiser que o arquivo `.to` codifique todos os itens que o caminho padrão.

O script produz três artefatos em `--output-dir` (por padrão, o repertório atual), além de uma quarta opção quando a codificação está ativa:

1. `<slug>.manifest.json` - manifesto da pista contendo o quorum dos validadores, os namespaces protegidos e as metadonnees opcionais do hook runtime-upgrade.
2. `<slug>.catalog.toml` - um trecho TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e todas as regras de rota exigidas. Certifique-se de que `fault_tolerance` está definido no espaço de dados principal para dimensionar o comite lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumo da auditoria descritiva da geometria (slug, segmentos, metadonnees) mais as etapas de implementação necessárias e o comando exato `cargo xtask space-directory encode` (sou `space_directory_encode.command`). Acesse este JSON no ticket de integração como antes.
4. `<slug>.manifest.to` - emis quando `--encode-space-directory` está ativo; pret pour le flux `iroha app space-directory manifest publish` de Torii.

Use `--dry-run` para pré-visualizar JSON/snippets sem gravação de arquivos e `--force` para apagar os artefatos existentes.

## 3. Aplicar alterações1. Copie o manifesto JSON na configuração `nexus.registry.manifest_directory` (e no diretório de cache no registro de pacotes distantes). Confirme o arquivo se os manifestos forem versões em seu repositório de configuração.
2. Adicione o trecho do catálogo para `config/config.toml` (ou para `config.d/*.toml` apropriado). Certifique-se de que `nexus.lane_count` esteja em menos de `lane_id + 1` e coloque um novo `nexus.routing_policy.rules` que aponta para a nova pista.
3. Codifique (se você tiver saute `--encode-space-directory`) e publique o manifesto no Space Directory por meio do comando capturado no resumo (`space_directory_encode.command`). Ela produz a carga útil `.manifest.to` atendida por Torii e registra a pré-avaliação para auditorias; soumettez através de `iroha app space-directory manifest publish`.
4. Lance `irohad --sora --config path/to/config.toml --trace-config` e arquive o rastreamento de saída no ticket de lançamento. Isso prova que a nova geometria corresponde aos segmentos Kura du slug genere.
5. Redemarrez les validadores atribui à la lane uma vez que as alterações do manifesto/catálogo são implantadas. Guarde o resumo JSON no ticket para auditorias futuras.

## 4. Construa um pacote de distribuição de registro

Empaque o gene do manifesto e a sobreposição para que os operadores possam distribuir os dados de governança das pistas sem editar as configurações em cada hotel. O ajudante de empacotamento copia os manifestos no layout canônico, produz uma opção de sobreposição de catálogo de governo para `nexus.registry.cache_directory` e pode emitir um tarball para transferências off-line:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Sorteios:

1. `manifests/<slug>.manifest.json` - copie os arquivos na configuração `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - depositado em `nexus.registry.cache_directory`. Cada entrada `--module` possui uma definição de módulo ramificado, permitindo trocas de módulo de governo (NX-2) e atualizando a sobreposição de cache de várias vezes no editor `config.toml`.
3. `summary.json` - inclui hashes, metadados de sobreposição e instruções operacionais.
4. Opcional `registry_bundle.tar.*` - preparado para SCP, S3 ou rastreadores de artefatos.

Sincronize todo o repertório (ou arquivo) com cada validador, extraia dos hotéis air-gapped e copie os manifestos + sobreposição de cache em seus arquivos de registro antes de reiniciar Torii.

## 5. Testes de fumaça de validação

Após a redemarrage de Torii, lance o novo ajudante de fumaça para verificar se a pista corresponde a `manifest_ready=true`, se as métricas expõem o nome do atendimento das pistas, e se o jauge selado é vide. As faixas que exigem manifestos devem expor um `manifest_path` não vide; o ajudante ecoa de quem é o caminho até que cada implantação NX-7 inclua a pré-venda do sinal de manifesto:

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

Adicione `--insecure` ao testar ambientes autoassinados. O script é classificado com um código diferente de zero se a pista estiver fechada, selada ou se as métricas/telemetria derivarem dos valores atendidos. Use os botões `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` para manter a telemetria por via (altura de bloco/finalização/backlog/headroom) em seus envelopes operacionais, e emparelhá-los com `--max-slot-p95` / `--max-slot-p99` (mais `--min-slot-samples`) para impor os objetivos de duração do slot NX-18 sem sair do ajudante.

Para validações air-gapped (ou CI), você pode obter uma resposta Torii capturada em vez de interrogar um endpoint live :

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

Os fixtures registrados sob `fixtures/nexus/lanes/` refletem os artefatos produzidos pelo auxiliar de bootstrap para que os novos se manifestem poderosamente e com linhas sem scripts sob medida. O CI executa o fluxo de meme via `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para provar que o auxiliar de fumaça NX-7 está em conformidade com o formato de carga útil publicado e para garantir que os resumos/sobreposições do pacote sejam reproduzíveis.