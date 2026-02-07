---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: دليل مانيفست الـstaging
sidebar_label: دليل مانيفست الـstaging
description: قائمة تحقق لتمكين ملف chunker المصادق عليه برلمانياً على نشرات Torii الخاصة بـ staging.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Use o Docusaurus e Markdown para usar o Sphinx Então.
:::

## نظرة عامة

Você pode usar o chunker para fazer o teste no Torii do staging. ترقية التغيير إلى الإنتاج. Verifique se o SoraFS está equipado com um dispositivo de fixação que pode ser usado para configurar os dispositivos elétricos.

## 1. المتطلبات المسبقة

1. زامن الـ fixtures المعتمدة والتواقيع:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Digite o nome do arquivo Torii na versão (referência): `/var/lib/iroha/admission/sorafs`.
3. Use o Torii para descobrir a descoberta e fazer o seguinte:

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

## 2. نشر أظرف القبول

1. Verifique o código de barras do dispositivo no `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Verifique Torii (ou use SIGHUP para obter mais informações).
3. راقب السجلات لرسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Descoberta do livro

1. Anúncio do fornecedor do anúncio do fornecedor (بايتات Norito) do seguinte modo:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. استعلم عن نقطة Discovery وتأكد من ظهور الإعلان مع الأسماء المستعارة المعتمدة:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   O `profile_aliases` deve ser substituído por `"sorafs.sf1@1.0.0"`.

## 4. اختبار نقاط نهاية manifesto e plano

1. Definindo o manifesto do token (token de fluxo e o token de fluxo):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Defina o JSON como exemplo:
   - Em `chunk_profile_handle` ou `sorafs.sf1@1.0.0`.
   - Em `manifest_digest_hex` você pode usar o software.
   - أن `chunk_digests_blake3` تتطابق مع الـ fixtures المعاد توليدها.

## 5. فحوصات التليمترية

- Verifique o valor do Prometheus:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المتابعة مزوّد staging تحت الاسم المستعار المتوقع وأن تبقى عدادات brownout عند Você pode fazer isso.

## 6. الجاهزية للإطلاق

1. Verifique o URL e o manifesto e o manifesto.
2. Execute o rollout Nexus no final do programa para obter mais informações.
3. Consulte o manual de instruções (Seção 4 em `chunker_registry_rollout_checklist.md`) para obter mais informações.

الحفاظ على هذا الدليل محدثًا يضمن أن كل إطلاق لـ chunker/admission يتبع نفس الخطوات الحتمية بين staging Então.