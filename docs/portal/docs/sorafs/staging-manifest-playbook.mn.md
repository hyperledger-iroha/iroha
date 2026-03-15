---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 768bcb70ff95d1445e6bd02a3f255ff2272a7796cc32d94f52abf99971b8dc7a
source_last_modified: "2026-01-05T09:28:11.910212+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
:::

## Тойм

Энэхүү тоглоомын ном нь үйлдвэрлэлд өөрчлөлт оруулахаас өмнө УИХ-аас соёрхон баталсан chunker профайлыг Torii үе шаттайгаар байршуулах боломжийг олгодог. Энэ нь SoraFS засаглалын дүрмийг соёрхон баталсан бөгөөд каноник бэхэлгээг хадгалах газарт байгаа гэж үзэж байна.

## 1. Урьдчилсан нөхцөл

1. Каноник бэхэлгээ болон гарын үсгийг синк хийнэ үү:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii эхлүүлэх үед унших элсэлтийн дугтуйны лавлахыг бэлтгэ (жишээ нь): `/var/lib/iroha/admission/sorafs`.
3. Torii тохиргоо нь илрүүлэлтийн кэш болон зөвшөөрлийн хэрэгжилтийг идэвхжүүлж байгаа эсэхийг шалгаарай:

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

## 2. Элсэлтийн дугтуйг нийтлэх

1. Зөвшөөрөгдсөн үйлчилгээ үзүүлэгчийн элсэлтийн дугтуйг `torii.sorafs.discovery.admission.envelopes_dir` лавлах лавлах руу хуулна уу:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii-г дахин эхлүүлнэ үү (эсвэл хэрэв та ачаалагчийг шууд ачааллаар ороосон бол SIGHUP илгээнэ үү).
3. Элсэлтийн мессежийн бүртгэлийг дарааллаар нь бичнэ үү:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Нээлтийн тархалтыг баталгаажуулах

1. Гарын үсэг зурсан үйлчилгээ үзүүлэгчийн зар сурталчилгааны ачааллыг (Norito байт) байршуулна уу.
   нийлүүлэгч дамжуулах хоолой:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Нээлтийн төгсгөлийн цэгийг асууж, зар сурталчилгааг каноник нэрээр харуулахыг баталгаажуулна уу:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases` эхний оруулгад `"sorafs.sf1@1.0.0"` орсон эсэхийг шалгаарай.

## 4. Дасгалын Манифест ба Төгсгөлийн цэгүүдийг төлөвлө

1. Манифест мета өгөгдлийг дуудах (хэрэв элсэлтийн зөвшөөрлийг баталгаажуулсан бол дамжуулалтын тэмдэг шаардлагатай):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON гаралтыг шалгаж, баталгаажуулна уу:
   - `chunk_profile_handle` бол `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` нь детерминизмын тайлантай таарч байна.
   - `chunk_digests_blake3` нь шинэчлэгдсэн бэхэлгээтэй нийцдэг.

## 5. Телеметрийн шалгалт

- Prometheus шинэ профайлын хэмжигдэхүүнийг харуулж байгааг баталгаажуулна уу:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Хяналтын самбарууд нь хүлээгдэж буй нэрийн дор тайзны үйлчилгээ үзүүлэгчийг харуулах ёстой бөгөөд профайл идэвхтэй байх үед тоологчийг тэг дээр байлгах ёстой.

## 6. Дамжуулахад бэлэн байдал

1. URL-ууд, манифест ID болон телеметрийн агшин зуурын зураг бүхий богино тайланг аваарай.
2. Төлөвлөсөн үйлдвэрлэлийг идэвхжүүлэх цонхны хажуугаар Nexus нэвтрүүлэх сувагт тайланг хуваалцаарай.
3. Оролцогч талууд гарын үсэг зурсны дараа үйлдвэрлэлийн хяналтын хуудсыг (`chunker_registry_rollout_checklist.md`-ийн 4-р хэсэг) үргэлжлүүлнэ үү.

Энэхүү тоглоомын дэвтрийг шинэчилж байх нь хэсэгчилсэн хэсэг/элсэлтийн танилцуулга бүр үе шат, үйлдвэрлэлд ижил тодорхой алхамуудыг дагаж мөрддөг.