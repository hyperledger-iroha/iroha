---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : staging-manifest-playbook
titre : دليل مانيفست الـstaging
sidebar_label : دليل مانيفست الـstaging
description : Vous avez besoin d'un chunker pour la mise en scène.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Vous pouvez utiliser Docusaurus et Markdown pour utiliser Sphinx.
:::

## نظرة عامة

Vous pouvez utiliser le chunker pour la mise en scène de la mise en scène Torii إلى الإنتاج. يفترض أن ميثاق حوكمة SoraFS تم التصديق عليه وأن الـ luminaires المعتمدة متاحة داخل المستودع.

## 1. المتطلبات المسبقة

1. زامن الـ luminaires المعتمدة والتواقيع:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. حضّر دليل أظرف القبول الذي يقرأه Torii عند الإقلاع (مسار مثال): `/var/lib/iroha/admission/sorafs`.
3. Utilisez le module Torii pour découvrir la fonction de découverte :

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

1. Mettre en place un système d'assistance à la conduite en ligne avec `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Utilisez le Torii (si vous utilisez SIGHUP si vous avez besoin d'aide).
3. راقب السجلات لرسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من انتشار découverte

1. Ajouter une annonce de fournisseur (بايتات Norito) à votre adresse :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. استعلم عن نقطة Discovery وتأكد من ظهور الإعلان مع الأسماء المستعارة المعتمدة:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Utilisez `profile_aliases` pour `"sorafs.sf1@1.0.0"`.

## 4. اختبار نقاط نهاية manifeste et plan

1. اجلب بيانات manifeste الوصفية (يتطلب stream token إذا كان القبول مفعّلًا) :

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Utiliser JSON et utiliser :
   - Par `chunk_profile_handle` et `sorafs.sf1@1.0.0`.
   - أن `manifest_digest_hex` يطابق تقرير الحتمية.
   - أن `chunk_digests_blake3` تتطابق مع الـ luminaires المعاد توليدها.

## 5. فحوصات التليمترية

- تأكد من أن Prometheus يعرض مقاييس الملف الجديد:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Il s'agit d'une mise en scène, d'une mise en scène et d'une baisse de tension. تفعيل الملف.

## 6. الجاهزية للإطلاق

1. La description de l'URL et du manifeste et du manifeste.
2. Déployez le déploiement Nexus en utilisant la méthode de déploiement.
3. انتقل إلى قائمة التحقق الخاصة بالإنتاج (Section 4 dans `chunker_registry_rollout_checklist.md`) pour موافقة أصحاب المصلحة.

Il s'agit d'une opportunité pour le chunker/admission d'être impliqué dans la mise en scène et la mise en scène.