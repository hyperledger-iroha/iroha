---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: Playbook de manifesto em staging
sidebar_label: Manual de manifesto em teste
description: Checklist para habilitar o perfil de chunker ratificado pelo Parlamento em implantações Torii de staging.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas as cópias sincronizadas.
:::

## Visão geral

Este playbook descreve como habilitar o perfil de chunker ratificado pelo Parlamento em uma implantação Torii de staging antes de promover a mudança para produção. Ele assume que a carta de governança da SoraFS foi ratificada e que os dispositivos canônicos estão disponíveis no repositório.

## 1. Pré-requisitos

1. Sincronizar os jogos canônicos e as assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare o diretório de admissão envelopes que o Torii lerá no startup (caminho exemplo): `/var/lib/iroha/admission/sorafs`.
3. Garanta que a configuração do Torii habilite o cache de descoberta e a imposição de admissão:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Envelopes de admissão pública

1. Cópia dos envelopes de admissão de prestadores aprovados para o diretório referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie o Torii (ou envie um SIGHUP se você embrulhou o loader com hot reload).
3. Acompanhe os logs para mensagens de admissão:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar a propagação da descoberta

1. Publicar o payload contratado do anúncio do provedor (bytes Norito) produzido pelo pipeline do provedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consulte o endpoint de descoberta e confirme que o anúncio aparece com aliases canonicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Garanta que `profile_aliases` incluindo `"sorafs.sf1@1.0.0"` como primeira entrada.

## 4. Exercitar endpoints de manifesto e plano

1. Busque os metadados do manifesto (o token de fluxo necessário se a admissão for aplicada):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecione o JSON e verifique:
   -`chunk_profile_handle` e `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` corresponde ao relatorio de determinismo.
   - `chunk_digests_blake3` alinham com os fixtures regenerados.

## 5. Verificações de telemetria

- Confirme que o Prometheus expõe as novas métricas do perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Os dashboards devem mostrar o provedor de staging sob o alias desejado e manter os contadores de brownout em zero enquanto o perfil estiver ativo.

## 6. Preparação para implementação

1. Capture um relato curto com URLs, ID de manifesto e instantâneo de telemetria.
2. Compartilhe o relato no canal de rollout do Nexus com a janela programada de ativação em produção.
3. Prossiga para o checklist de produção (Seção 4 em `chunker_registry_rollout_checklist.md`) quando as partes interessadas aprovarem.

Manter este manual atualizado garante que cada implementação de chunker/admissão siga os mesmos passos deterministas entre preparação e produção.