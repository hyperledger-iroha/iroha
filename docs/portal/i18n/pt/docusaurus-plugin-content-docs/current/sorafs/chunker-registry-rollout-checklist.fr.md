---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: Lista de verificação de implementação do bloco de registro SoraFS
sidebar_label: Bloco de implementação da lista de verificação
descrição: Plano de implementação para as tarefas do dia do registro.
---

:::nota Fonte canônica
Reflete `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Gardez as duas cópias sincronizadas junto com o retorno completo do conjunto Sphinx herité.
:::

# Checklist de implementação do registro SoraFS

Esta lista de verificação detalhada das etapas necessárias para promover um novo perfil
chunker ou un bundle d'admission fournisseur de la revue à la production après
ratificação da carta de governo.

> **Portée :** Aplique todos os lançamentos que modificar
> `sorafs_manifest::chunker_registry`, les envelopes d'admission fournisseurs ou
> os pacotes de luminárias canônicas (`fixtures/sorafs_chunker/*`).

## 1. Validação preliminar

1. Gerencie os jogos e verifique o determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirme os hashes de determinação em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou o relacionamento do perfil
   pertinente) correspondente aux artefacts régénérés.
3. Certifique-se de que `sorafs_manifest::chunker_registry` compile com
   `ensure_charter_compliance()` lançado:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Prepare hoje o dossiê de proposta:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Entrada de minutos do conselho sob `docs/source/sorafs/council_minutes_*.md`
   - Relatório de determinismo

## 2. Governança de validação

1. Apresentamos o relatório do Grupo de Trabalho de Ferramentas e o resumo da proposta
   Painel de Infraestrutura do Parlamento au Sora.
2. Registre os detalhes de aprovação em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publique o envelope assinado pelo Parlamento no local dos jogos:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique se o envelope está acessível através do auxiliar de busca de governo:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Preparação de implementação

Consulte-o [playbook manifest staging](./staging-manifest-playbook) para um
procedimento detalhado.

1. Implemente o Torii com o Discovery `torii.sorafs` ativo e o aplicativo de
   admissão ativa (`enforce_admission = true`).
2. Coloque os envelopes de admissão aprovados no repertório
   de registro staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique se os anúncios do provedor são propagados por meio da descoberta da API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Execute o manifesto/plano de endpoints com cabeçalhos de governo:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme os painéis de telefone (`torii_sorafs_*`) e as regras
   d'alerta reportar o novo perfil sem erros.

## 4. Produção de lançamento

1. Repita as etapas de preparação nos números Torii de produção.
2. Anuncie a janela de ativação (data/hora, período de graça, plano de reversão)
   entre operadores canadianos e SDK.
3. Mescle o PR do conteúdo do lançamento:
   - Luminárias e envelope mises à jour
   - Alterações de documentação (referências à carta, relatório de determinismo)
   - Atualizar roteiro/status
4. Marque a liberação e arquive os artefatos assinados de acordo com a proveniência.

## 5. Auditoria pós-lançamento1. Capture as métricas finais (comptes discovery, taux de succès fetch,
   histogramas de erros) 24 horas após a implementação.
2. Envie o dia `status.md` com um breve currículo e uma garantia sobre o relacionamento de determinismo.
3. Consignez les tâches de suivi (ex. orientação para criação de perfis) em
   `roadmap.md`.