---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: Checklist de implementação do registro de chunker da SoraFS
sidebar_label: Lista de verificação de implementação do chunker
description: Plano de implementação passo a passo para atualizações do registro de chunker.
---

:::nota Fonte canônica
Reflete `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas as cópias sincronizadas.
:::

# Checklist de rollout do registro da SoraFS

Este checklist captura os passos necessários para promover um novo perfil de chunker
ou pacote de admissão de provedor da revisão para produção depois que o afretamento
de governança para ratificado.

> **Escopo:** Aplicar-se a todas as versões que modificam
> `sorafs_manifest::chunker_registry`, envelopes de admissão de provedor, ou pacotes
> de luminárias canônicas (`fixtures/sorafs_chunker/*`).

## 1. Validação pré-voo

1. Regenere os fixtures e verifique o determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirme que os hashes de determinismo estão em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou o relato de perfil
   relevante) batem com os artistas regenerados.
3. Garanta que `sorafs_manifest::chunker_registry` compila com
   `ensure_charter_compliance()` executando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Atualizar o dossiê da proposta:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atas do conselho em `docs/source/sorafs/council_minutes_*.md`
   - Relatório de determinismo

## 2. Aprovação de governança

1. Apresente o relator do Tooling Working Group e o resumo da proposta ao
   Painel de Infraestrutura do Parlamento Sora.
2. Cadastre detalhes de aprovação em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publico o envelope aprovado pelo Parlamento juntamente com os calendários:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique se o envelope está acessível através do ajudante de governança buscar:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementação na preparação

Consulte o [playbook de manifesto em staging](./staging-manifest-playbook) para um
passo a passo detalhado.

1. Implante Torii com descoberta `torii.sorafs` habilitado e imposição de admissão
   ligado (`enforce_admission = true`).
2. Enviar os envelopes de admissão de fornecedores aprovados para o diretório de registro
   de staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique se os anúncios do provedor se propagam através de uma API de descoberta:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Exercer endpoints de manifesto/plano com cabeçalhos de governança:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme que dashboards de telemetria (`torii_sorafs_*`) e regras de alerta
   reportam o novo perfil sem erros.

## 4. Rollout em produção

1. Repita os passos de staging nos nós Torii de produção.
2. Anuncie a janela de ativação (dados/hora, período de carência, plano de reversão) nos
   canais de operadores e SDK.
3. Mesclar o PR de lançamento contendo:
   - Luminárias e envelopes atualizados
   - Mudanças na documentação (referências ao charter, relatorio de determinismo)
   - Atualizar roteiro/status
4. Tagueie a liberação e arquivo dos artefatos contratados para procedência.

## 5. Pós-lançamento do Auditório1. Capture métricas finais (contagens de descoberta, taxas de sucesso de busca, histogramas
   de erro) 24h após o lançamento.
2. Atualizar `status.md` com um resumo curto e link para o relato de determinismo.
3. Cadastrar tarefas de acompanhamento (ex., orientação adicional para autoria
   de perfis) em `roadmap.md`.