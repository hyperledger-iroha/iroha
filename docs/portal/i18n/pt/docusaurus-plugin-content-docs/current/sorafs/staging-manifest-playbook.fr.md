---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: Playbook de manifesto em preparação
sidebar_label: Manual de manifesto e teste
description: Lista de verificação para ativar o bloco de perfil ratificado pelo Parlamento nas implantações Torii de teste.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Guarde a cópia Docusaurus e o Markdown herdado alinhado junto com o retrato completo do conjunto Sphinx.
:::

## Vista do conjunto

Este manual descreve a ativação do bloco de perfil ratificado pelo Parlamento em uma implantação Torii de preparação antes de promover a alteração na produção. Suponho que a carta de governo SoraFS seja ratificada e que os dispositivos canônicos estejam disponíveis no depósito.

## 1. Pré-requisitos

1. Sincronize os fixtures canônicos e as assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare o repertório dos envelopes de admissão que Torii lira au démarrage (caminho do exemplo): `/var/lib/iroha/admission/sorafs`.
3. Certifique-se de que a configuração Torii ative a descoberta de cache e o aplicativo de admissão:

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

## 2. Publicar os envelopes de admissão

1. Copie os envelopes de admissão aprovados no repertório indicado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Redemarrez Torii (ou envie um SIGHUP se você tiver encapsulado o carregador com uma recarga com calor).
3. Acompanhe os registros das mensagens de admissão:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar a descoberta de propagação

1. Poste a carga útil assinada pelo fornecedor de anúncio (octetos Norito) produzida por seu fornecedor de pipeline:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Interrogue a descoberta do endpoint e confirme se o anúncio aparece com o alias canônico:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Certifique-se de que `profile_aliases` inclui `"sorafs.sf1@1.0.0"` na estreia.

## 4. Execute o manifesto e o plano de endpoints

1. Recupere os metadonnées do manifesto (é necessário um token de fluxo se a admissão for aplicada):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecione a classificação JSON e verifique:
   - `chunk_profile_handle` é `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` corresponde ao relatório de determinismo.
   - `chunk_digests_blake3` é alinhado com os equipamentos regenerados.

## 5. Verificações de télémétrie

- Confirme que Prometheus expõe as novas métricas de perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Os painéis devem monitorar o provedor de teste sob o apelido de atendente e cuidar dos computadores de quedas de energia a zero enquanto o perfil estiver ativo.

## 6. Preparação do lançamento

1. Capture um relatório judicial com URLs, o ID do manifesto e o instantâneo do telefone.
2. Compartilhe o relacionamento no canal de implementação Nexus com a janela de ativação de produção planejada.
3. Passe para a lista de verificação de produção (Seção 4 em `chunker_registry_rollout_checklist.md`) uma vez que as partes interessadas não concordam.

Manter este manual no dia garante que cada rollout chunker/admission se adapte às etapas definidas entre a preparação e a produção.