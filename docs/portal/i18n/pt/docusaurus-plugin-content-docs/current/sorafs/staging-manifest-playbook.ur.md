---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: Manual do manifesto de teste SoraFS
sidebar_label: Manual do manifesto de teste SoraFS
description: Torii کی teste de implantações پر perfil chunker ratificado pelo Parlamento فعال کرنے کے لیے checklist۔
---

:::nota مستند ماخذ
:::

## Visão geral

یہ playbook staging implantação Torii پر perfil chunker ratificado pelo Parlamento فعال کرنے کے مراحل بیان کرتا ہے تاکہ تبدیلی کو produção میں promover کرنے سے پہلے تصدیق ہو سکے۔ یہ فرض کرتا ہے کہ Carta de governança SoraFS ratificada ہو چکا ہے اور repositório de luminárias canônicas میں موجود ہیں۔

## 1. Pré-requisitos

1. Dispositivos canônicos e sincronização de assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. envelopes de admissão کی وہ diretório تیار کریں جسے inicialização Torii پر پڑھے گا (caminho de exemplo): `/var/lib/iroha/admission/sorafs`.
3. یقینی بنائیں کہ Torii cache de descoberta de configuração e aplicação de admissão کو ativar کرتی ہے:

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

## 2. Envelopes de admissão publicados کریں

1. envelopes de admissão de provedor aprovados کو `torii.sorafs.discovery.admission.envelopes_dir` کے diretório میں cópia کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii reiniciar کریں (یا اگر loader hot reload کے ساتھ wrap ہے تو SIGHUP بھیجیں).
3. mensagens de admissão کے لیے logs tail کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validação de propagação de descoberta کریں

1. pipeline do provedor سے بنے ہوئے carga útil do anúncio do provedor assinado (Norito bytes) e post کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. consulta de endpoint de descoberta کریں اور confirmar کریں کہ aliases canônicos de anúncio کے ساتھ نظر آتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   یقینی بنائیں کہ `profile_aliases` میں `"sorafs.sf1@1.0.0"` پہلی entrada کے طور پر شامل ہو۔

## 4. Manifesto e exercício de endpoints do plano کریں

1. busca de metadados do manifesto (اگر admissão impor ہے تو token de fluxo درکار ہوگا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Saída JSON inspecionar کریں e verificar کریں:
   - `chunk_profile_handle`, `sorafs.sf1@1.0.0` ہو۔
   - Relatório de determinismo `manifest_digest_hex` سے correspondência کرے۔
   - `chunk_digests_blake3` luminárias regeneradas سے alinhar ہوں۔

## 5. Verificações de telemetria

- تصدیق کریں کہ Prometheus نئی métricas de perfil expõem کر رہا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- painéis کو alias esperado do provedor de teste کے تحت دکھانا چاہیے اور perfil فعال ہونے پر contadores de queda de energia صفر رہنے چاہییں۔

## 6. Preparação para implementação

1. URLs, ID do manifesto, e instantâneo de telemetria
2. رپورٹ کو Nexus canal de implementação میں janela de ativação de produção planejada کے ساتھ شیئر کریں۔
3. as partes interessadas کے assinam کے بعد lista de verificação de produção پر جائیں (Seção 4 em `chunker_registry_rollout_checklist.md`).

اس playbook کو اپ ڈیٹ رکھنا یقینی بناتا ہے کہ ہر chunker/preparação de implementação de admissão اور produção میں ایک جیسے etapas determinísticas پر چلتا ہے۔