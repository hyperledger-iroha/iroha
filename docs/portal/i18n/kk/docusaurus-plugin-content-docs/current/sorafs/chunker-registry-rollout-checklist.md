---
id: chunker-registry-rollout-checklist
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ескерту Канондық дереккөз
:::

# SoraFS тізілімді шығаруды тексеру тізімі

Бұл бақылау тізімі жаңа chunker профилін жылжыту үшін қажетті қадамдарды қамтиды немесе
басқарудан кейін қараудан өндіруге дейін жеткізушінің рұқсат беру жинағы
жарғысы бекітілді.

> **Қолдану аймағы:** Өзгертетін барлық шығарылымдарға қолданылады
> `sorafs_manifest::chunker_registry`, провайдердің қабылдау конверттері немесе
> канондық арматура байламдары (`fixtures/sorafs_chunker/*`).

## 1. Ұшу алдындағы валидация

1. Арматураларды қалпына келтіріңіз және детерминизмді тексеріңіз:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Детерминизм хэштерін растаңыз
   `docs/source/sorafs/reports/sf1_determinism.md` (немесе тиісті профиль
   есеп) қалпына келтірілген артефактілерді сәйкестендіріңіз.
3. `sorafs_manifest::chunker_registry` компиляциясын қамтамасыз етіңіз
   `ensure_charter_compliance()` іске қосу арқылы:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Ұсыныс құжаттамасын жаңартыңыз:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` бойынша кеңес хаттамалары
   - Детерминизм туралы есеп

## 2. Басқарудан шығу

1. Құралмен жұмыс тобының есебін және ұсыныс дайджестін Сораға ұсыныңыз
   Парламенттің инфрақұрылымдық панелі.
2. Мақұлдау мәліметтерін келесіге жазыңыз
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Парламент қол қойған конвертті бекітпелермен бірге жариялаңыз:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Конвертке басқаруды алу көмекшісі арқылы қол жеткізуге болатынын тексеріңіз:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Шығарылымды шығару

[Staging manifest playbook](./staging-manifest-playbook) бөлімін қараңыз.
осы қадамдардың егжей-тегжейлі өтуі.

1. `torii.sorafs` табу қосылған және рұқсатпен Torii қолданбасын орналастырыңыз
   орындау қосылды (`enforce_admission = true`).
2. Бекітілген провайдер қабылдау конверттерін кезең тізіліміне итеріңіз
   каталогқа сілтеме `torii.sorafs.discovery.admission.envelopes_dir`.
3. Провайдер жарнамаларының API API арқылы таралатынын тексеріңіз:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Басқару тақырыптары бар манифест/жоспардың соңғы нүктелерін орындаңыз:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Телеметрия бақылау тақталарын (`torii_sorafs_*`) және ескерту ережелерін растаңыз.
   қатесіз жаңа профиль.

## 4. Өндірістің шығуы

1. Torii түйіндеріне қарсы кезеңдік қадамдарды қайталаңыз.
2. Белсендіру терезесін (күн/уақыт, жеңілдік кезеңі, кері қайтару жоспары) хабарлаңыз.
   оператор және SDK арналары.
3. Мыналарды қамтитын шығарылым PR біріктіріңіз:
   - Жаңартылған арматура мен конверт
   - Құжаттамадағы өзгерістер (жарғылық анықтамалар, детерминизм туралы есеп)
   - Жол картасы/күйді жаңарту
4. Шығарылымды белгілеңіз және қол қойылған артефактілерді шығу тегі үшін мұрағаттаңыз.

## 5. Шығарудан кейінгі аудит

1. Қорытынды көрсеткіштерді түсіру (табылғандар саны, алу сәттілігі, қате
   гистограммалар) шығарылғаннан кейін 24 сағаттан кейін.
2. `status.md` қысқаша қорытындымен және детерминизм есебіне сілтемемен жаңартыңыз.
3. Кез келген кейінгі тапсырмаларды (мысалы, профильді жасау бойынша қосымша нұсқаулық) файлында
   `roadmap.md`.