---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: Implementação do registro do chunker SoraFS
sidebar_label: implementação do Chunker
descrição: atualizações de registro do chunker کے لیے قدم بہ قدم implementação
---

:::nota مستند ماخذ
:::

# Implementação do registro SoraFS چیک لسٹ

یہ چیک لسٹ نئے perfil chunker یا pacote de admissão do provedor کو revisão سے produção
تک promover کرنے کے لیے درکار مراحل کو capturar کرتی ہے جب carta de governança
ratificar ہو چکا ہو۔

> **Escopo:** تمام lançamentos پر لاگو ہے جو
> `sorafs_manifest::chunker_registry`, envelopes de admissão do provedor, یا canônico
> pacotes de acessórios (`fixtures/sorafs_chunker/*`) میں تبدیلی کریں۔

## 1. Validação pré-voo

1. fixtures podem gerar کریں اور determinismo verificar کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ relatório de perfil) میں
   determinismo hashes de artefatos regenerados سے correspondência کریں۔
3. یقینی بنائیں کہ `sorafs_manifest::chunker_registry`،
   `ensure_charter_compliance()` کے ساتھ compilar ہوتا ہے، چلائیں:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. dossiê de proposta اپڈیٹ کریں:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Lançamento da ata do conselho `docs/source/sorafs/council_minutes_*.md`
   - Relatório de determinismo

## 2. Aprovação da governança

1. Relatório do Grupo de Trabalho de Ferramentas e resumo da proposta
   Painel de Infraestrutura do Parlamento Sora میں پیش کریں۔
2. detalhes de aprovação کو
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. Envelope assinado pelo Parlamento کو jogos کے ساتھ publicar کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. auxiliar de busca de governança کے ذریعے envelope acessível ہونے کی تصدیق کریں:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementação de teste

ان etapas کی تفصیلی passo a passo کے لیے [manual de preparação do manifesto](./staging-manifest-playbook) دیکھیں۔

1. Torii e `torii.sorafs` descoberta habilitada e aplicação de admissão em
   (`enforce_admission = true`) کے ساتھ implantar کریں۔
2. envelopes de admissão de provedor aprovados کو diretório de registro de teste میں push کریں
   جسے `torii.sorafs.discovery.admission.envelopes_dir` consulte کرتا ہے۔
3. API de descoberta کے ذریعے anúncios do provedor کی verificação de propagação کریں:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. cabeçalhos de governança کے ساتھ exercício de endpoints de manifesto/plano کریں:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. painéis de telemetria (`torii_sorafs_*`) اور regras de alerta سے نئے perfil کی
   رپورٹنگ بغیر erros کے confirmar کریں۔

## 4. Lançamento da produção

1. etapas de teste e produção de nós Torii e repetição de passos
2. janela de ativação (data/hora, período de carência, plano de reversão) کو operador اور SDK
   canais پر anunciar کریں۔
3. liberar mesclagem de PR کریں جس میں شامل ہو:
   - luminárias atualizadas e envelope
   - alterações na documentação (referências de estatuto, relatório de determinismo)
   - atualização de roteiro/status
4. etiqueta de liberação کریں اور artefatos assinados کو proveniência کے لیے arquivo کریں۔

## 5. Auditoria pós-lançamento

1. implementação em 24 horas antes das métricas finais (contagens de descoberta, taxa de sucesso de busca, erro
   histogramas) capturar کریں۔
2. `status.md` کو مختصر resumo e relatório de determinismo کے link کے ساتھ atualização کریں۔
3. tarefas de acompanhamento (orientação de criação de perfil) کو `roadmap.md` میں درج کریں۔