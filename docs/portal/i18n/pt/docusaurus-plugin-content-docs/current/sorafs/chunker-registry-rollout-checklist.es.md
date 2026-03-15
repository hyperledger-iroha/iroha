---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: Checklist de implementação do registro de chunker de SoraFS
sidebar_label: Lista de verificação de implementação do chunker
description: Plano de implementação passo a passo para atualizações do registro do chunker.
---

:::nota Fonte canônica
Reflexo `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja retirado.
:::

# Checklist de implementação do registro de SoraFS

Esta lista de verificação captura os passos necessários para promover um novo perfil de chunker
ou um pacote de admissão de provedores desde revisão e produção após que
carta de governo foi ratificada.

> **Alcance:** Aplicar a todos os lançamentos que foram modificados
> `sorafs_manifest::chunker_registry`, as sobres de admissão de provadores ou os
> pacotes de luminárias canônicas (`fixtures/sorafs_chunker/*`).

## 1. Validação prévia

1. Regenera fixtures e verifica o determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirme os hashes de determinismo em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou o relatório de perfil relevante)
   coincide com os artefatos regenerados.
3. Certifique-se de que `sorafs_manifest::chunker_registry` compila com
   `ensure_charter_compliance()` executando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Atualize o dossiê da proposta:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atos do conselho em `docs/source/sorafs/council_minutes_*.md`
   - Relatório de determinismo

## 2. Aprovação de governo

1. Apresenta o relatório do Grupo de Trabalho de Ferramentas e o resumo da proposta
   Painel de Infraestrutura do Parlamento Sora.
2. Registre os detalhes de aprovação em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publica a mensagem firmada pelo Parlamento junto com os jogos:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique se o mar está acessível por meio do ajudante de busca de governo:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementação na preparação

Consulte o [manual de manifesto em teste](./staging-manifest-playbook) para um
recorrido detalhado de esses passos.

1. Desplie Torii com descoberta `torii.sorafs` habilitado e o aplicativo de
   admissão ativada (`enforce_admission = true`).
2. Sube as sobres de admissão de provedores aprovados no diretório de registro
   de staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique se os anúncios do provedor são propagados por meio da API de descoberta:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Execute os endpoints do manifesto/plano com cabeçalhos de governo:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme os painéis de telemetria (`torii_sorafs_*`) e as regras de
   alerta reportar o novo perfil sem erros.

## 4. Implementação na produção

1. Repita os passos de preparação contra os nós de produção Torii.
2. Anuncie a janela de ativação (fecha/hora, período de graça, plano de reversão)
   nos canais de operadores e SDK.
3. Combine o PR de lançamento que contém:
   - Luminárias e sobre atualizados
   - Mudanças de documentação (referências à carta, relatório de determinismo)
   - Atualizar roteiro/status
4. Etiqueta a liberação e arquiva os artefatos firmados para a procedência.

## 5. Auditoria pós-lançamento1. Captura de métricas finais (conteúdos de descoberta, taxa de sucesso de busca,
   histogramas de erro) 24 horas após o lançamento.
2. Atualize `status.md` com um resumo curto e um link para o relatório de determinismo.
3. Registre qualquer tarefa de acompanhamento (p. ej., mais guia de autoria de perfis)
   em `roadmap.md`.