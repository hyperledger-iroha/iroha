---
lang: kk
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

:::ескерту Канондық дереккөз
:::

## Шолу

Бұл оқу кітапшасы өндіріске өзгертуді алға жылжытпас бұрын, Torii кезеңдік орналастыруында парламент ратификациялаған chunker профилін қосу арқылы жүреді. Ол SoraFS басқару жарғысы ратификацияланды және канондық құрылғылар репозиторийде қолжетімді деп болжайды.

## 1. Пререквизиттер

1. Канондық құрылғылар мен қолтаңбаларды синхрондаңыз:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii іске қосу кезінде оқитын қабылдау конверті каталогын дайындаңыз (мысал жолы): `/var/lib/iroha/admission/sorafs`.
3. Torii конфигурациясының табу кэшіне және рұқсатты орындауға мүмкіндік беретініне көз жеткізіңіз:

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

## 2. Қабылдау конверттерін жариялау

1. `torii.sorafs.discovery.admission.envelopes_dir` сілтемесі бар анықтамалыққа провайдердің рұқсат беру хатқалталарын көшіріңіз:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii қайта іске қосыңыз (немесе жүктегішті жедел қайта жүктеу арқылы ораған болсаңыз, SIGHUP жіберіңіз).
3. Қабылдау хабарлары үшін журналдарды орналастырыңыз:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Ашылу таралуын тексеру

1. Қол қойылған провайдер жарнамасының пайдалы жүктемесін (Norito байт) жіберіңіз.
   провайдер құбыры:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Табудың соңғы нүктесін сұраңыз және жарнаманың канондық бүркеншік аттармен көрсетілетінін растаңыз:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases` бірінші жазба ретінде `"sorafs.sf1@1.0.0"` қосылғанына көз жеткізіңіз.

## 4. Жаттығу манифесті және соңғы нүктелерді жоспарлаңыз

1. Манифест метадеректерін алыңыз (егер рұқсат күшіне енсе, ағын таңбалауышы қажет):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON шығысын тексеріңіз және мынаны тексеріңіз:
   - `chunk_profile_handle` — `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` детерминизм есебіне сәйкес келеді.
   - `chunk_digests_blake3` қалпына келтірілген құрылғылармен туралаңыз.

## 5. Телеметриялық тексерулер

- Prometheus жаңа профиль көрсеткіштерін көрсететінін растаңыз:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Бақылау тақталары күтілетін бүркеншік аттың астындағы кезеңдік провайдерді көрсетуі және профиль белсенді болған кезде өшіру есептегіштерін нөлде ұстауы керек.

## 6. Шығаруға дайындық

1. URL мекенжайлары, манифест идентификаторы және телеметрия суреті бар қысқа есепті түсіріңіз.
2. Жоспарланған өндірісті белсендіру терезесімен қатар Nexus шығару арнасында есепті бөлісіңіз.
3. Мүдделі тараптар қол қойғаннан кейін өндірісті тексеру тізіміне өтіңіз (`chunker_registry_rollout_checklist.md` ішіндегі 4-бөлім).

Осы ойын кітабын жаңартылған күйде ұстау әрбір chunker/қабылдау релизінің кезең мен өндірісте бірдей детерминирленген қадамдарды орындайтынын қамтамасыз етеді.