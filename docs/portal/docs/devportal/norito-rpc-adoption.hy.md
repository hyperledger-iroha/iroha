---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-12-29T18:16:35.105044+00:00"
translation_last_reviewed: 2026-02-07
id: norito-rpc-adoption
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
---

> Կանոնական պլանավորման նշումներն ապրում են `docs/source/torii/norito_rpc_adoption_schedule.md`-ում:  
> Այս պորտալի պատճենը թորում է SDK-ի հեղինակների, օպերատորների և վերանայողների թողարկման ակնկալիքները:

## Նպատակներ

- Հավասարեցրեք յուրաքանչյուր SDK-ն (Rust CLI, Python, JavaScript, Swift, Android) երկուական Norito-RPC փոխադրամիջոցի վրա՝ AND4 արտադրության փոխարկիչից առաջ:
- Պահպանեք փուլային դարպասները, ապացույցների փաթեթները և հեռաչափության կեռիկները որոշիչ, որպեսզի կառավարումը կարողանա աուդիտի ենթարկել ներդրումը:
- Դարձրեք աննշան միջոցների և դեղձանիկների ապացույցների հավաքումը ընդհանուր օգնականների հետ, որոնք կանչում է NRPC-4 ճանապարհային քարտեզը:

## Փուլային ժամանակացույց

| Փուլ | Պատուհան | Շրջանակ | Ելքի չափանիշներ |
|-------|--------|-------|--------------|
| **P0 – Լաբորատոր հավասարություն ** | Q22025 | Rust CLI + Python ծխի հավաքակազմերը աշխատում են `/v1/norito-rpc` CI-ում, JS helper-ը անցնում է միավորի թեստեր, Android-ի կեղծ ամրագոտիները կատարում են կրկնակի փոխադրումներ: | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` և `javascript/iroha_js/test/noritoRpcClient.test.js` կանաչ CI-ում; Android զրահը միացված է `./gradlew test`-ին: |
| **P1 – SDK նախադիտում** | Q32025 | Համօգտագործվող սարքերի փաթեթը գրանցված է, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` գրանցում է տեղեկամատյանները + JSON `artifacts/norito_rpc/`-ում, կամընտիր Norito տրանսպորտային դրոշները ցուցադրվում են SDK նմուշներում: | Ֆիլմերի մանիֆեստը ստորագրված է, README-ի թարմացումները ցույց են տալիս միանալու օգտագործումը, Swift նախադիտման API-ն հասանելի է IOS2 դրոշի հետևում: |
| **P2 – Բեմականացում / AND4 նախադիտում** | Q12026 | Բեմականացման Torii լողավազանները նախընտրում են Norito, Android AND4 նախադիտման հաճախորդները և Swift IOS2 հավասարաչափ փաթեթները լռելյայն երկուական տրանսպորտի, հեռաչափության վահանակի `dashboards/grafana/torii_norito_rpc_observability.json` բնակեցված: | `docs/source/torii/norito_rpc_stage_reports.md`-ը գրավում է դեղձանիկը, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` փոխանցումը, Android-ի կեղծ ամրագոտիների կրկնությունը գրանցում է հաջողության/սխալների դեպքերը: |
| **P3 – Արտադրության GA** | Q42026 | Norito-ը դառնում է լռելյայն փոխադրամիջոց բոլոր SDK-ների համար. JSON-ը շարունակում է մնալ անհետացման հետևանք: Թողարկեք աշխատատեղերի արխիվային հավասարության արտեֆակտները յուրաքանչյուր պիտակի հետ: | Թողարկեք ստուգաթերթի փաթեթները Norito ծխի ելք Rust/JS/Python/Swift/Android-ի համար; Norito ընդդեմ JSON սխալի SLO-ների զգոնության շեմերը ուժի մեջ են. `status.md`-ը և թողարկման նշումները վկայակոչում են GA-ի ապացույցները: |

## SDK-ի առաքում և CI կեռիկներ

- **Rust CLI և ինտեգրացիոն ամրագոտի** – ընդլայնել `iroha_cli pipeline` ծխի թեստերը՝ Norito-ին հարկադրելու փոխադրումը, երբ `cargo xtask norito-rpc-verify` վայրէջք կատարվի: Պահակ՝ `cargo test -p integration_tests -- norito_streaming` (լաբորատորիա) և `cargo xtask norito-rpc-verify` (բեմականացում/GA) հետ՝ արտեֆակտները պահելով `artifacts/norito_rpc/` տակ:
- **Python SDK** – լռելյայն թողարկված ծուխը (`python/iroha_python/scripts/release_smoke.sh`) մինչև Norito RPC, պահեք `run_norito_rpc_smoke.sh`-ը որպես CI մուտքի կետ և փաստաթղթերի հավասարության մշակումը `python/iroha_python/README.md`-ում: CI թիրախ՝ `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`:
- **JavaScript SDK** – կայունացրեք `NoritoRpcClient`, թույլ տվեք կառավարման/հարցման օգնականներին լռելյայն դնել Norito, երբ `toriiClientConfig.transport.preferred === "norito_rpc"`, և վերջից մինչև վերջ նմուշներ նկարեք `javascript/iroha_js/recipes/`-ում: CI-ն պետք է գործարկի `npm test`-ը, գումարած dockerized `npm run test:norito-rpc` աշխատանքը, նախքան հրապարակելը. ծագման վերբեռնումներ Norito ծխի տեղեկամատյաններ `javascript/iroha_js/artifacts/` տակ:
- **Swift SDK** – միացրեք Norito կամուրջի փոխադրումը IOS2 դրոշի հետևում, արտացոլեք սարքի արագությունը և համոզվեք, որ Connect/Norito հավասարաչափ փաթեթն աշխատում է `docs/source/sdk/swift/index.md`-ում նշված Buildkite գծերի ներսում:
- **Android SDK** – AND4 նախադիտման հաճախորդները և կեղծ Torii ամրագոտիները ընդունում են Norito՝ `docs/source/sdk/android/networking.md`-ում փաստաթղթավորված կրկնակի/հետադարձ հեռաչափությամբ: Զարդարակը կիսում է հարմարանքները այլ SDK-ների հետ `scripts/run_norito_rpc_fixtures.sh --sdk android`-ի միջոցով:

## Ապացույցներ և ավտոմատացում

- `scripts/run_norito_rpc_fixtures.sh`-ը փաթաթում է `cargo xtask norito-rpc-verify`-ը, ֆիքսում է stdout/stderr-ը և արտանետում `fixtures.<sdk>.summary.json`, այնպես որ SDK-ի սեփականատերերն ունեն `status.md`-ին կցելու որոշիչ արտեֆակտ: Օգտագործեք `--sdk <label>` և `--out artifacts/norito_rpc/<stamp>/`՝ CI փաթեթները կոկիկ պահելու համար:
- `cargo xtask norito-rpc-verify`-ը պարտադրում է սխեմայի հեշ հավասարությունը (`fixtures/norito_rpc/schema_hashes.json`) և ձախողվում է, եթե Torii-ը վերադարձնում է `X-Iroha-Error-Code: schema_mismatch`: Յուրաքանչյուր ձախողում զուգակցեք JSON-ի հետընտրական նկարահանման հետ վրիպազերծման համար:
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh`-ը և `dashboards/grafana/torii_norito_rpc_observability.json`-ը սահմանում են NRPC-2-ի ազդանշանային պայմանագրերը: Գործարկեք սկրիպտը վահանակի յուրաքանչյուր խմբագրումից հետո և պահեք `promtool` ելքը դեղձանիկի փաթեթում:
- `docs/source/runbooks/torii_norito_rpc_canary.md`-ը նկարագրում է բեմականացման և արտադրության զորավարժությունները. թարմացրեք այն, երբ հարմարանքների հեշերը կամ ազդանշանային դարպասները փոխվում են:

## Գրախոսների ստուգաթերթ

Նախքան NRPC-4 նշաձողը նշելը, հաստատեք.

1. Վերջին հարմարանքների փաթեթի հեշերը համապատասխանում են `fixtures/norito_rpc/schema_hashes.json`-ին և համապատասխան CI արտեֆակտին, որը գրանցված է `artifacts/norito_rpc/<stamp>/`-ում:
2. SDK README / պորտալի փաստաթղթերը նկարագրում են, թե ինչպես հարկադրել JSON-ի հետադարձ կապը և մեջբերել Norito տրանսպորտային կանխադրվածը:
3. Հեռուստաչափության վահանակները ցույց են տալիս կրկնակի կույտի սխալի մակարդակի վահանակներ՝ ազդանշանային հղումներով, և Alertmanager չոր գործարկումը (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) կցված է որոնիչին:
4. Այստեղ ընդունման ժամանակացույցը համընկնում է հետագծող մուտքի (`docs/source/torii/norito_rpc_tracker.md`) և ճանապարհային քարտեզի (NRPC-4) նույն ապացույցների փաթեթին:

Ժամանակացույցում կարգապահ մնալը թույլ է տալիս կանխատեսելի պահել SDK-ի վարքագիծը և թույլ է տալիս կառավարման աուդիտի ենթարկել Norito-RPC-ն առանց պատվիրված հարցումների: