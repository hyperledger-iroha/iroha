<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + runbook de API

Este runbook abrange implantação e operações orientadas à produção para:

- um site estático Vue3 (`--template site`); e
- um serviço Vue3 SPA + API (`--template webapp`),

usando APIs de plano de controle Soracloud em Iroha 3 com suposições SCR/IVM (sem
Dependência de tempo de execução WASM e nenhuma dependência Docker).

## 1. Gere projetos modelo

Andaime de site estático:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Estrutura SPA + API:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Cada diretório de saída inclui:

-`container_manifest.json`
-`service_manifest.json`
- arquivos de origem do modelo em `site/` ou `webapp/`

## 2. Construir artefatos de aplicativos

Site estático:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

Front-end do SPA + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Empacotar e publicar ativos de front-end

Para hospedagem estática via SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

Para interface de SPA:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Implante no plano de controle Soracloud ativo

Implante o serviço de site estático:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Implantar serviço SPA + API:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Valide a vinculação da rota e o estado de implementação:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Verificações esperadas do plano de controle:

- Conjunto `control_plane.services[].latest_revision.route_host`
- Conjunto `control_plane.services[].latest_revision.route_path_prefix` (`/` ou `/api`)
- `control_plane.services[].active_rollout` presente imediatamente após a atualização

## 5. Atualize com implementação controlada por saúde

1. Insira `service_version` no manifesto de serviço.
2. Execute a atualização:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Promova a implementação após verificações de integridade:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Se a saúde falhar, relate não saudável:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Quando relatórios não íntegros atingem o limite da política, o Soracloud rola automaticamente
volta à revisão da linha de base e registra eventos de auditoria de reversão.

## 6. Reversão manual e resposta a incidentes

Reverter para versão anterior:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Use a saída de status para confirmar:

- `current_version` revertido
- `audit_event_count` incrementado
- `active_rollout` liberado
- `last_rollout.stage` é `RolledBack` para reversões automáticas

## 7. Lista de verificação de operações

- Mantenha os manifestos gerados por modelos sob controle de versão.
- Registre `governance_tx_hash` para cada etapa de implementação para preservar a rastreabilidade.
- Tratar `service_health`, `routing`, `resource_pressure` e
  `failed_admissions` como entradas de porta de implementação.
- Use porcentagens canário e promoção explícita em vez de corte completo direto
  atualizações para serviços voltados ao usuário.
- Validar o comportamento de sessão/autenticação e verificação de assinatura em
  `webapp/api/server.mjs` antes da produção.