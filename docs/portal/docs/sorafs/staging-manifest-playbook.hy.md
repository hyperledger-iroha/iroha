---
lang: hy
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

:::note Կանոնական աղբյուր
:::

## Տեսություն

Այս գրքույկը հնարավորություն է տալիս խորհրդարանի կողմից վավերացված chunker պրոֆիլը բեմադրող Torii տեղակայման միջոցով՝ նախքան արտադրության փոփոխությունը խթանելը: Այն ենթադրում է, որ SoraFS կառավարման կանոնադրությունը վավերացվել է, և կանոնական հարմարանքները հասանելի են պահոցում:

## 1. Նախադրյալներ

1. Համաժամացրեք կանոնական սարքերը և ստորագրությունները.

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Պատրաստեք ընդունելության ծրարի գրացուցակը, որը Torii-ը կկարդա գործարկման ժամանակ (օրինակ ուղի՝ `/var/lib/iroha/admission/sorafs`):
3. Համոզվեք, որ Torii կազմաձևը հնարավորություն է տալիս հայտնաբերման քեշը և մուտքի կիրառումը.

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

## 2. Հրապարակեք ընդունելության ծրարները

1. Պատճենեք հաստատված մատակարարի ընդունելության ծրարները `torii.sorafs.discovery.admission.envelopes_dir`-ի կողմից նշված գրացուցակում՝

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Վերագործարկեք Torii-ը (կամ ուղարկեք SIGHUP, եթե բեռնիչը փաթաթել եք անմիջապես վերաբեռնմամբ):
3. Պահեք ընդունելության հաղորդագրությունների տեղեկամատյանները.

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Վավերացնել հայտնաբերման տարածումը

1. Տեղադրեք ստորագրված մատակարարի գովազդի օգտակար բեռը (Norito բայթ), որը արտադրվել է ձեր կողմից
   մատակարարի խողովակաշար.

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Հարցրեք հայտնաբերման վերջնակետին և հաստատեք, որ գովազդը հայտնվում է կանոնական կեղծանուններով.

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Համոզվեք, որ `profile_aliases` ներառում է `"sorafs.sf1@1.0.0"` որպես առաջին մուտք:

## 4. Զորավարժությունների մանիֆեստ և պլանավորեք վերջնակետերը

1. Վերցրեք մանիֆեստի մետատվյալները (պահանջվում է հոսքի նշան, եթե ընդունումը պարտադիր է):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Ստուգեք JSON ելքը և ստուգեք.
   - `chunk_profile_handle`-ը `sorafs.sf1@1.0.0` է:
   - `manifest_digest_hex`-ը համապատասխանում է դետերմինիզմի զեկույցին:
   - `chunk_digests_blake3` համահունչ է վերականգնված հարմարանքների հետ:

## 5. Հեռաչափության ստուգումներ

- Հաստատեք, որ Prometheus-ը բացահայտում է պրոֆիլի նոր չափումները.

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Վահանակները պետք է ցուցադրեն բեմադրող մատակարարին ակնկալվող կեղծանունների ներքո և պահեն զրոյական հաշվիչները՝ մինչ պրոֆիլը ակտիվ է:

## 6. Տեղադրման պատրաստակամություն

1. Կատարեք կարճ զեկույց URL-ներով, մանիֆեստի ID-ով և հեռաչափության պատկերով:
2. Կիսվեք զեկույցով Nexus թողարկման ալիքում՝ պլանավորված արտադրության ակտիվացման պատուհանի հետ մեկտեղ:
3. Անցեք արտադրության ստուգաթերթին (`chunker_registry_rollout_checklist.md`-ի 4-րդ բաժին), երբ շահագրգիռ կողմերը ստորագրեն:

Այս գրքույկը թարմացված պահելը երաշխավորում է, որ յուրաքանչյուր բլոկ/ընդունելության թողարկում հետևում է նույն դետերմինիստական ​​քայլերին բեմադրության և արտադրության մեջ: