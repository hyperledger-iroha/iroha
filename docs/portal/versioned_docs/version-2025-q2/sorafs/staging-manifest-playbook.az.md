---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-az
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-az
---

:::Qeyd Kanonik Mənbə
Güzgülər `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Hər iki nüsxəni buraxılışlar arasında uyğunlaşdırın.
:::

## Baxış

Bu dərslik istehsala dəyişikliyi təşviq etməzdən əvvəl Parlament tərəfindən ratifikasiya olunmuş chunker profilinin Torii mərhələli yerləşdirməsində işə salınmasından bəhs edir. O güman edir ki, SoraFS idarəetmə nizamnaməsi təsdiq edilib və kanonik qurğular anbarda mövcuddur.

## 1. İlkin şərtlər

1. Kanonik qurğuları və imzaları sinxronlaşdırın:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii işə salındıqda oxuyacağı qəbul zərfinin kataloqunu hazırlayın (nümunə yol): `/var/lib/iroha/admission/sorafs`.
3. Torii konfiqurasiyasının kəşf keşini və qəbulun tətbiqini təmin etdiyinə əmin olun:

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

## 2. Qəbul zərflərini dərc edin

1. Təsdiq edilmiş provayder qəbulu zərflərini `torii.sorafs.discovery.admission.envelopes_dir` tərəfindən istinad edilən kataloqa kopyalayın:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii-i yenidən başladın (və ya yükləyicini tez bir zamanda yenidən yükləmə ilə bağlamısınızsa, SIGHUP göndərin).
3. Qəbul mesajları üçün qeydləri sıralayın:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Kəşflərin yayılmasını təsdiq edin

1. İmzalanmış provayderin reklam yükünü (Norito bayt) yerləşdirin.
   provayder boru kəməri:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Tapıntının son nöqtəsini sorğulayın və reklamın kanonik ləqəblərlə göründüyünü təsdiqləyin:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases`-ə ilk giriş kimi `"sorafs.sf1@1.0.0"` daxil olduğundan əmin olun.

## 4. Məşq Manifestini və Son Nöqtələri Planlayın

1. Manifest metadatasını əldə edin (qəbul olunarsa axın nişanı tələb olunur):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON çıxışını yoxlayın və yoxlayın:
   - `chunk_profile_handle` `sorafs.sf1@1.0.0`-dir.
   - `manifest_digest_hex` determinizm hesabatına uyğun gəlir.
   - `chunk_digests_blake3` bərpa edilmiş qurğularla uyğunlaşdırılır.

## 5. Telemetriya Yoxlamaları

- Prometheus yeni profil ölçülərini ifşa etdiyini təsdiqləyin:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Tablolar gözlənilən ləqəb altında səhnələşdirmə provayderini göstərməli və profil aktiv olduğu müddətdə qaralama sayğaclarını sıfırda saxlamalıdır.

## 6. Yayılma Hazırlığı

1. URL-lər, manifest identifikatoru və telemetriya snapşotu ilə qısa hesabat çəkin.
2. Planlaşdırılan istehsalın aktivləşdirilməsi pəncərəsi ilə yanaşı hesabatı Nexus yayım kanalında paylaşın.
3. Maraqlı tərəflər imzaladıqdan sonra istehsal yoxlama siyahısına (`chunker_registry_rollout_checklist.md`-də Bölmə 4) keçin.

Bu kitabçanın güncəllənməsi, hər bir parça/qəbul buraxılışının səhnələşdirmə və istehsalda eyni deterministik addımları izləməsini təmin edir.
