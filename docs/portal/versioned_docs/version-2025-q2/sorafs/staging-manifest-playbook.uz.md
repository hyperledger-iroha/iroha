---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-uz
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-uz
---

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Ikkala nusxani ham nashrlar bo'ylab tekislang.
:::

## Umumiy ko'rinish

Ushbu qoʻllanma ishlab chiqarishga oʻzgartirish kiritishdan oldin parlament tomonidan tasdiqlangan chunker profilini Torii bosqichma-bosqich joylashtirishda faollashtirish orqali oʻtadi. Bu SoraFS boshqaruv xartiyasi ratifikatsiya qilingan va kanonik moslamalar omborda mavjud deb taxmin qilinadi.

## 1. Old shartlar

1. Kanonik moslamalar va imzolarni sinxronlashtiring:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii ishga tushirilganda o'qiy oladigan qabul konverti katalogini tayyorlang (misol yo'li): `/var/lib/iroha/admission/sorafs`.
3. Torii konfiguratsiyasi kashfiyot keshini va qabul qilish majburiyatini yoqishiga ishonch hosil qiling:

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

## 2. Qabul konvertlarini nashr qilish

1. Tasdiqlangan provayder qabul konvertlarini `torii.sorafs.discovery.admission.envelopes_dir` tomonidan havola qilingan katalogga nusxalang:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii ni qayta ishga tushiring (yoki yuklagichni tezda qayta yuklash bilan o'ralgan bo'lsangiz, SIGHUP yuboring).
3. Qabul qilish xabarlari jurnallarini yozing:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Discovery Propagation-ni tasdiqlash

1. Imzolangan provayderingiz tomonidan ishlab chiqarilgan reklama yukini (Norito bayt) joylashtiring.
   provayder quvuri:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Kashfiyot so'nggi nuqtasini so'rang va reklamaning kanonik taxalluslar bilan paydo bo'lishini tasdiqlang:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases` birinchi yozuv sifatida `"sorafs.sf1@1.0.0"` kiritilganligiga ishonch hosil qiling.

## 4. Mashq manifesti va yakuniy nuqtalarni rejalashtirish

1. Manifest metamaʼlumotlarini oling (qabul qilish majburiy boʻlsa, oqim tokenini talab qiladi):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON chiqishini tekshiring va tekshiring:
   - `chunk_profile_handle` - `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` determinizm hisobotiga mos keladi.
   - `chunk_digests_blake3` qayta tiklangan moslamalar bilan tekislang.

## 5. Telemetriya tekshiruvlari

- Prometheus yangi profil ko'rsatkichlarini ochishini tasdiqlang:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Boshqaruv paneli kutilgan taxallus ostida staging provayderini ko'rsatishi va profil faol bo'lgan vaqtda hisoblagichlarni nolda ushlab turishi kerak.

## 6. Chiqarishga tayyorlik

1. URL manzillari, manifest identifikatori va telemetriya surati bilan qisqa hisobotni oling.
2. Rejalashtirilgan ishlab chiqarishni faollashtirish oynasi bilan birga Nexus tarqatish kanalida hisobotni baham ko'ring.
3. Manfaatdor tomonlar imzo chekkandan so'ng, ishlab chiqarishni tekshirish ro'yxatiga o'ting (`chunker_registry_rollout_checklist.md` da 4-bo'lim).

Ushbu o'yin kitobini yangilab turish har bir chunker/qabul qilish jarayoni sahnalashtirish va ishlab chiqarish bo'yicha bir xil deterministik qadamlarni bajarishini ta'minlaydi.
