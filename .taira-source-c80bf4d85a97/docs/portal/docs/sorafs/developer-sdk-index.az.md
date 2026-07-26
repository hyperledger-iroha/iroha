---
lang: az
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

:::Qeyd Kanonik Mənbə
:::

SoraFS alətlər silsiləsi ilə göndərilən hər dil köməkçilərini izləmək üçün bu mərkəzdən istifadə edin.
Pas-xüsusi fraqmentlər üçün [Rust SDK fraqmentləri](./developer-sdk-rust.md) keçidinə keçin.

## Dil köməkçiləri

- **Python** — `sorafs_multi_fetch_local` (yerli orkestr tüstü testləri) və
  `sorafs_gateway_fetch` (Gateway E2E məşqləri) indi isteğe bağlı qəbul edir
  `telemetry_region` plus `transport_policy` ləğvi
  (`"soranet-first"`, `"soranet-strict"` və ya `"direct-only"`), CLI-ni əks etdirir
  yayma düymələri. Yerli QUIC proksi işə salındıqda,
  `sorafs_gateway_fetch` altındakı brauzer manifestini qaytarır
  `local_proxy_manifest` beləliklə, testlər etibar paketini brauzer adapterlərinə ötürə bilsin.
- **JavaScript** — `sorafsMultiFetchLocal` Python köməkçisini əks etdirir, qayıdır
  faydalı yük baytları və qəbz xülasələri, `sorafsGatewayFetch` məşqləri isə
  Torii şlüzləri, yerli proksi manifestləri ötürür və eyni şeyi ifşa edir
  telemetriya/nəqliyyat CLI kimi ləğv edilir.
- **Rust** — xidmətlər planlayıcını birbaşa vasitəsilə yerləşdirə bilər
  `sorafs_car::multi_fetch`; [Rust SDK parçalarına] baxın (./developer-sdk-rust.md)
  sübut axını köməkçiləri və orkestrator inteqrasiyası üçün istinad.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP-dən təkrar istifadə edir
  icraçı və fəxri `GatewayFetchOptions`. ilə birləşdirin
  `ClientConfig.Builder#setSorafsGatewayUri` və PQ yükləmə işarəsi
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) yükləmələr sadiq qaldıqda
  Yalnız PQ yolları.

## Hesab lövhəsi və siyasət düymələri

Həm Python (`sorafs_multi_fetch_local`), həm də JavaScript
(`sorafsMultiFetchLocal`) köməkçiləri telemetriyadan xəbərdar olan planlaşdırıcı tablosunu ifşa edir
CLI tərəfindən istifadə olunur:

- İstehsal ikililəri defolt olaraq hesab tablosunu aktivləşdirir; set `use_scoreboard=True`
  (və ya `telemetry` girişlərini təmin edin) armaturları təkrar oynatarkən köməkçinin əldə etməsi üçün
  reklam metadatasından və son telemetriya görüntülərindən sifariş verən ölçülmüş provayder.
- Hesablanmış çəkiləri yığınla birlikdə qəbul etmək üçün `return_scoreboard=True` seçin
  qəbzlər, beləliklə CI qeydləri diaqnostikanı tuta bilsin.
- Həmyaşıdları rədd etmək və ya əlavə etmək üçün `deny_providers` və ya `boost_providers` massivlərindən istifadə edin.
  Planlayıcı provayderləri seçdikdə `priority_delta`.
- Defolt `"soranet-first"` duruşunu endirmə etmədikcə saxlayın; təchizatı
  `"direct-only"` yalnız uyğunluq bölgəsi relelərdən qaçmalı olduqda və ya zaman
  SNNet-5a geri dönüşünü məşq edin və yalnız PQ üçün `"soranet-strict"`-ni ehtiyatda saxlayın
  idarəetmənin təsdiqi ilə pilotlar.
- Gateway köməkçiləri də `scoreboardOutPath` və `scoreboardNowUnixSecs`-i ifşa edirlər.
  Hesablanmış tabloda davam etmək üçün `scoreboardOutPath` təyin edin (CLI-ni əks etdirir)
  `--scoreboard-out` bayrağı) buna görə də `cargo xtask sorafs-adoption-check` təsdiq edə bilər
  SDK artefaktları və qurğular sabitliyə ehtiyac duyduqda `scoreboardNowUnixSecs` istifadə edin
  Təkrarlana bilən metadata üçün `assume_now` dəyəri. JavaScript köməkçisində siz
  əlavə olaraq `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` təyin edə bilər;
  etiket buraxıldıqda, `region:<telemetryRegion>` (geri düşür
  `sdk:js`). Python köməkçisi avtomatik olaraq `telemetry_source="sdk:python"` yayır
  hesab tablosunu saxladıqda və gizli metadatanı qeyri-aktiv saxladıqda.

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