---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: Playbook de manifesto em preparação
sidebar_label: Manual de manifesto e teste
description: Lista de verificação para habilitar o perfil de chunker ratificado pelo Parlamento nos despliegues Torii de staging.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas as cópias sincronizadas.
:::

## Resumo

Este manual descreve como habilitar o perfil de bloco ratificado pelo Parlamento em uma solução Torii de teste antes de promover a mudança de produção. Suponha que a carta de governo de SoraFS tenha sido ratificada e que os equipamentos canônicos estejam disponíveis no repositório.

## 1. Pré-requisitos

1. Sincronize os fixtures canônicos e as firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare o diretório de sobres de admissão que Torii lerá para iniciar (rota de exemplo): `/var/lib/iroha/admission/sorafs`.
3. Certifique-se de que a configuração do Torii habilite o cache de descoberta e o aplicativo de admissão:

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

## 2. Publicar sobre admissão

1. Copie os sobres de admissão aprovados no diretório referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie Torii (ou envie um SIGHUP se envolver o carregador com recarga quente).
3. Revisa os registros das mensagens de admissão:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar a propagação da descoberta

1. Publica a carga útil firmada pelo anúncio do provedor (bytes Norito) produzida pelo seu pipeline de fornecedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consulte o endpoint de descoberta e confirme que o anúncio aparece com aliases canônicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Certifique-se de que `profile_aliases` inclui `"sorafs.sf1@1.0.0"` como primeira entrada.

## 4. Teste os endpoints do manifesto e do plano

1. Obtenha os metadados do manifesto (requer um token de stream se a admissão estiver ativa):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecione a saída JSON e verifique:
   - `chunk_profile_handle` e `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` coincide com o relatório de determinismo.
   - `chunk_digests_blake3` é alinhado com os acessórios regenerados.

## 5. Comprovações de telemetria

- Confirme que Prometheus expõe as novas informações do perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Os painéis devem mostrar o provedor de teste abaixo do alias esperado e manter os contadores de queda de energia em zero enquanto o perfil estiver ativo.

## 6. Preparação para o lançamento

1. Capture um relatório curto com as URLs, o ID do manifesto e o instantâneo da telemetria.
2. Compare o relatório no canal de implementação do Nexus com a janela planejada de ativação na produção.
3. Continue com a lista de verificação de produção (Seção 4 em `chunker_registry_rollout_checklist.md`) uma vez que as partes interessadas no visto sejam boas.

Mantenha este manual atualizado para garantir que cada implementação de chunker/admissão siga os mesmos passos deterministas entre preparação e produção.