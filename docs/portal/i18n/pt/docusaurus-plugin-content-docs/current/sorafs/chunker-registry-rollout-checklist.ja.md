---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dbf29828756604abff6e5416bbd4f839cd74f9503904ac1cc0293f9185a08f0f
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry-rollout-checklist
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Reflete `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas as copias sincronizadas.
:::

# Checklist de rollout do registro da SoraFS

Este checklist captura os passos necessarios para promover um novo perfil de chunker
ou bundle de admission de provedor da revisao para producao depois que o charter
de governanca for ratificado.

> **Escopo:** Aplica-se a todas as releases que modificam
> `sorafs_manifest::chunker_registry`, provider admission envelopes, ou bundles
> de fixtures canonicos (`fixtures/sorafs_chunker/*`).

## 1. Validacao pre-flight

1. Regenere fixtures e verifique determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirme que os hashes de determinismo em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou o relatorio de perfil
   relevante) batem com os artefatos regenerados.
3. Garanta que `sorafs_manifest::chunker_registry` compila com
   `ensure_charter_compliance()` executando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Atualize o dossier da proposta:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atas do conselho em `docs/source/sorafs/council_minutes_*.md`
   - Relatorio de determinismo

## 2. Sign-off de governanca

1. Apresente o relatorio do Tooling Working Group e o digest da proposta ao
   Sora Parliament Infrastructure Panel.
2. Registre detalhes de aprovacao em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publique o envelope assinado pelo Parlamento junto com os fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique se o envelope esta acessivel via o helper de governance fetch:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Rollout em staging

Consulte o [playbook de manifest em staging](./staging-manifest-playbook) para um
passo a passo detalhado.

1. Implante Torii com discovery `torii.sorafs` habilitado e admission enforcement
   ligado (`enforce_admission = true`).
2. Envie os provider admission envelopes aprovados para o diretorio de registry
   de staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique que provider adverts propagam via a API de discovery:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Exercite endpoints de manifest/plan com headers de governanca:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme que dashboards de telemetria (`torii_sorafs_*`) e regras de alerta
   reportam o novo perfil sem erros.

## 4. Rollout em producao

1. Repita os passos de staging nos nodes Torii de producao.
2. Anuncie a janela de ativacao (data/hora, grace period, plano de rollback) nos
   canais de operadores e SDK.
3. Merge o PR de release contendo:
   - Fixtures e envelope atualizados
   - Mudancas na documentacao (referencias ao charter, relatorio de determinismo)
   - Refresh de roadmap/status
4. Tagueie a release e arquive os artefatos assinados para provenance.

## 5. Auditoria pos-rollout

1. Capture metricas finais (discovery counts, taxa de sucesso de fetch, histograms
   de erro) 24h apos o rollout.
2. Atualize `status.md` com um resumo curto e link para o relatorio de determinismo.
3. Registre tarefas de acompanhamento (ex., orientacao adicional para authoring
   de perfis) em `roadmap.md`.
