---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28cc43e407412d66481f25146c19d35f0e102523d22f954be3c106231d95e891
source_last_modified: "2026-01-05T09:28:11.868629+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-index
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

SoraFS құралдар тізбегімен жеткізілетін әрбір тілге арналған көмекшілерді қадағалау үшін осы хабты пайдаланыңыз.
Тотқа тән үзінділер үшін [Rust SDK үзінділеріне](./developer-sdk-rust.md) өтіңіз.

## Тіл көмекшілері

- **Python** — `sorafs_multi_fetch_local` (жергілікті оркестрдің түтін сынақтары) және
  `sorafs_gateway_fetch` (E2E шлюз жаттығулары) енді қосымшаны қабылдайды
  `telemetry_region` плюс `transport_policy` қайта анықтау
  (`"soranet-first"`, `"soranet-strict"` немесе `"direct-only"`), CLI шағылыстыру
  айналдыру тұтқалары. Жергілікті QUIC прокси қосылғанда,
  `sorafs_gateway_fetch` астында шолғыш манифестін қайтарады
  `local_proxy_manifest`, сондықтан сынақтар сенімді топтаманы браузер адаптерлеріне бере алады.
- **JavaScript** — `sorafsMultiFetchLocal` Python көмекшісін көрсетеді, қайтарылады
  `sorafsGatewayFetch` жаттығулары кезінде пайдалы жүк байттары мен түбіртектердің қорытындылары
  Torii шлюздері, жергілікті прокси манифесттерін ағындар және бірдей көрсетеді
  телеметрия/тасымалдау CLI ретінде қайта анықтайды.
- **Rust** — қызметтер жоспарлаушыны тікелей арқылы ендіре алады
  `sorafs_car::multi_fetch`; [Rust SDK үзінділерін] қараңыз (./developer-sdk-rust.md)
  proof-stream көмекшілері мен оркестрлерді біріктіруге арналған сілтеме.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP протоколын қайта пайдаланады
  орындаушы және құрметті `GatewayFetchOptions`. Онымен біріктіріңіз
  `ClientConfig.Builder#setSorafsGatewayUri` және PQ жүктеп салу туралы кеңес
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) жүктеп салулар сақталуы керек кезде
  Тек PQ жолдары.

## Есеп тақтасы және саясат тұтқалары

Python (`sorafs_multi_fetch_local`) және JavaScript екеуі де
(`sorafsMultiFetchLocal`) көмекшілері телеметриядан хабардар жоспарлаушы көрсеткіштер тақтасын көрсетеді
CLI пайдаланатын:

- Өндірістік екілік файлдар таблоны әдепкі бойынша қосады; `use_scoreboard=True` орнатыңыз
  (немесе `telemetry` жазбаларын қамтамасыз ету) көмекші шығару үшін арматураларды қайта ойнату кезінде
  жарнама метадеректерінен және соңғы телеметриялық суреттерден өлшенген провайдер тапсырысы.
- Бөлшекпен қатар есептелген салмақтарды алу үшін `return_scoreboard=True` орнатыңыз
  түбіртектер, сондықтан CI журналдары диагностиканы жаза алады.
- Құрдастарды қабылдамау немесе қосу үшін `deny_providers` немесе `boost_providers` массивтерін пайдаланыңыз.
  Жоспарлаушы провайдерлерді таңдаған кезде `priority_delta`.
- Бағалауды төмендетпейінше, әдепкі `"soranet-first"` қалпын сақтаңыз; қамтамасыз ету
  `"direct-only"` сәйкестік аймағы релелерден аулақ болу керек болғанда ғана немесе қашан
  SNNet-5a қалпына келтіруді қайталау және `"soranet-strict"` тек PQ үшін резервтеу
  басқару мақұлдауымен ұшқыштар.
- Шлюз көмекшілері де `scoreboardOutPath` және `scoreboardNowUnixSecs` көрсетеді.
  Есептелген көрсеткіштер тақтасын сақтау үшін `scoreboardOutPath` параметрін орнатыңыз (CLI айнасын көрсетеді)
  `--scoreboard-out` жалаушасы) сондықтан `cargo xtask sorafs-adoption-check` тексере алады
  SDK артефактілері және арматураларға тұрақтылық қажет болғанда `scoreboardNowUnixSecs` пайдаланыңыз.
  Қайталанатын метадеректер үшін `assume_now` мәні. JavaScript көмекшісінде
  қосымша `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` орнатуға болады;
  жапсырма алынып тасталса, ол `region:<telemetryRegion>` (артқа түседі
  `sdk:js`). Python көмекшісі автоматты түрде `telemetry_source="sdk:python"` шығарады
  ол көрсеткіштер тақтасын сақтаған кезде және жасырын метадеректерді өшірулі күйде сақтайды.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```