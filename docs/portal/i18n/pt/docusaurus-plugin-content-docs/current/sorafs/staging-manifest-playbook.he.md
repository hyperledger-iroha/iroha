---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4cee710913f5cfd7d810355b22f66f3b6598f4645a14ad5344faf445de5c3e1
source_last_modified: "2025-11-14T04:43:22.339313+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas as copias sincronizadas.
:::

## Visao geral

Este playbook descreve como habilitar o perfil de chunker ratificado pelo Parlamento em um deployment Torii de staging antes de promover a mudanca para producao. Ele assume que a carta de governanca da SoraFS foi ratificada e que os fixtures canonicos estao disponiveis no repositorio.

## 1. Prerequisitos

1. Sincronize os fixtures canonicos e as assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare o diretorio de admission envelopes que o Torii lera no startup (caminho exemplo): `/var/lib/iroha/admission/sorafs`.
3. Garanta que o config do Torii habilite o discovery cache e o enforcement de admission:

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

## 2. Publicar admission envelopes

1. Copie os provider admission envelopes aprovados para o diretorio referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie o Torii (ou envie um SIGHUP se voce embrulhou o loader com hot reload).
3. Acompanhe os logs para mensagens de admission:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar a propagacao de discovery

1. Publique o payload assinado de provider advert (bytes Norito) produzido pelo pipeline do provedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consulte o endpoint de discovery e confirme que o advert aparece com aliases canonicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Garanta que `profile_aliases` inclua `"sorafs.sf1@1.0.0"` como a primeira entrada.

## 4. Exercitar endpoints de manifest e plan

1. Busque a metadata do manifest (exige stream token se admission estiver enforced):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecione o JSON e verifique:
   - `chunk_profile_handle` e `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` corresponde ao relatorio de determinismo.
   - `chunk_digests_blake3` alinham com os fixtures regenerados.

## 5. Checks de telemetria

- Confirme que o Prometheus expoe as novas metricas do perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Dashboards devem mostrar o provider de staging sob o alias esperado e manter os contadores de brownout em zero enquanto o perfil estiver ativo.

## 6. Rollout readiness

1. Capture um relatorio curto com URLs, manifest ID e telemetry snapshot.
2. Compartilhe o relatorio no canal de rollout do Nexus com a janela planejada de ativacao em producao.
3. Prossiga para o checklist de producao (Section 4 em `chunker_registry_rollout_checklist.md`) quando as partes interessadas aprovarem.

Manter este playbook atualizado garante que cada rollout de chunker/admission siga os mesmos passos deterministas entre staging e producao.
