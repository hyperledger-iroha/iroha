---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: Чеклист rollout реестра chunker SoraFS
sidebar_label: Verifica o chunker de implementação
descrição: Implementação do plano de implementação para a restauração do chunker.
---

:::nota História Canônica
Verifique `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Tire uma cópia da sincronização, mas o Sphinx não está disponível para download.
:::

# Verifique a implementação da restauração SoraFS

Esta é uma ótima configuração, necessária para produzir um novo perfil de chunker
ou admissão de provedor de pacote para renegociar a produção após a transação
carta de governança.

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, envelopes de admissão do provedor ou
> Pacotes de acessórios canônicos (`fixtures/sorafs_chunker/*`).

## 1. Validação de segurança

1. Altere os equipamentos e verifique a determinação:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Verifique quais hashes determinam em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou relevante fora
   perfil) é fornecido com artefactos de regeneração.
3. Verifique, este `sorafs_manifest::chunker_registry` compilado com
   `ensure_charter_compliance()` por exemplo:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите dossier предложения:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Registre os minutos em `docs/source/sorafs/council_minutes_*.md`
   - Escolha a determinação

## 2. Aprovação da governança

1. Leia o Grupo de Trabalho de Ferramentas e resumir as instruções em
   Painel de Infraestrutura do Parlamento Sora.
2. Verifique detalhadamente a qualidade do produto em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Abra o envelope, coloque o documento, coloque os acessórios:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Проверьте, что envelope доступен через helper получения governança:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementação de teste

Passo a passo de Подробный см. в [manual de manifesto de preparação](./staging-manifest-playbook).

1. Altere Torii com a descoberta `torii.sorafs` e a aplicação de segurança
   admissão (`enforce_admission = true`).
2. Organize envelopes de admissão de provedores aprovados no diretório de registro de teste,
   usado em `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique quais provedores de anúncios usam a API de descoberta:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Programar manifesto/plano de endpoints com cabeçalhos de governança:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Verifique quais são os painéis de telemetria (`torii_sorafs_*`) e regras de alerta
   отображают новый профиль без ошибок.

## 4. Lançamento da produção

1. Coloque o teste no produto Torii-узлах.
2. Объявите окно активации (data/vida, período de carência, plano de reversão) nos canais
   operadores e SDK.
3. Смёрджите релизный PR com:
   - Fixação de luminárias e envelope
   - Документационными изменениями (ссылки на charter, отчет о детерминизме)
   - Roteiro / status de atualização
4. Coloque a confiança e envie os artefatos de acordo com a proveniência.

## 5. Auditoria pós-lançamento

1. Снимите финальные метрики (contagens de descoberta, taxa de sucesso de busca, erro
   histogramas) durante 24 horas após o lançamento.
2. Verifique a configuração do `status.md` e verifique a definição correta.
3. Siga as etapas de acompanhamento (por exemplo, orientação adicional para autoria
   perfil) em `roadmap.md`.