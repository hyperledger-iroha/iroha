---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-pt
slug: /sorafs/staging-manifest-playbook-pt
---

:::nota Fonte Canônica
Espelhos `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas as cópias alinhadas entre os lançamentos.
:::

## Visão geral

Este manual orienta a habilitação do perfil chunker ratificado pelo Parlamento em uma implantação Torii de teste antes de promover a mudança para produção. Ele pressupõe que a carta de governança SoraFS foi ratificada e os acessórios canônicos estão disponíveis no repositório.

## 1. Pré-requisitos

1. Sincronize os fixtures canônicos e assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare o diretório do envelope de admissão que Torii irá ler na inicialização (exemplo de caminho): `/var/lib/iroha/admission/sorafs`.
3. Certifique-se de que a configuração Torii habilite o cache de descoberta e a imposição de admissão:

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

## 2. Publicar envelopes de admissão

1. Copie os envelopes de admissão do fornecedor aprovado no diretório referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie Torii (ou envie um SIGHUP se você envolveu o carregador com recarga instantânea).
3. Acompanhe os logs para mensagens de admissão:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar a propagação da descoberta

1. Publique a carga útil do anúncio do provedor assinado (bytes Norito) produzida por seu
   pipeline do provedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Consulte o endpoint de descoberta e confirme se o anúncio aparece com aliases canônicos:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Certifique-se de que `profile_aliases` inclua `"sorafs.sf1@1.0.0"` como a primeira entrada.

## 4. Manifesto de Exercício e Pontos Finais do Plano

1. Obtenha os metadados do manifesto (requer um token de fluxo se a admissão for imposta):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecione a saída JSON e verifique:
   - `chunk_profile_handle` é `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` corresponde ao relatório de determinismo.
   - `chunk_digests_blake3` alinhe com os equipamentos regenerados.

## 5. Verificações de telemetria

- Confirme que Prometheus expõe as novas métricas de perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Os painéis devem mostrar o provedor de teste sob o alias esperado e manter os contadores de indisponibilidade em zero enquanto o perfil estiver ativo.

## 6. Preparação para implantação

1. Capture um breve relatório com URLs, ID do manifesto e instantâneo de telemetria.
2. Compartilhe o relatório no canal de implementação Nexus junto com a janela de ativação de produção planejada.
3. Prossiga para a lista de verificação de produção (Seção 4 em `chunker_registry_rollout_checklist.md`) assim que as partes interessadas assinarem.

Manter este manual atualizado garante que cada implementação de chunker/admissão siga as mesmas etapas determinísticas na preparação e na produção.
