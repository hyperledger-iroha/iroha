---
id: dispute-revocation-runbook
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

## Ниәт

Был runbook I18NT0000000000X ҡәҙерле бәхәстәр аша идара итеү операторҙарын етәкләй, ревокацияларҙы координациялау һәм мәғлүмәттәрҙе эвакуациялауҙы тәьмин итеү детерминистик рәүештә тамамлай.

## 1. Инцидентты баһалау

- **Триггер шарттары:** SLA боҙоуҙы асыҡлау (өҫкә ваҡыт/PoR етешһеҙлеге), репликация етешмәүе, йәки биллинг ризаһыҙлыҡ.
- **Телеметрияны раҫлау:** провайдер өсөн `/v1/sorafs/capacity/state` һәм I18NI0000000010X снимоктарын төшөрөү.
- **Ҡатнашыусыларға хәбәр итеү:** Һаҡлау командаһы (провайдер операциялары), Идара итеү советы (ҡарар органы), Күҙәтеүсәнлек (приборҙар таҡтаһы яңыртыуҙары).

## 2. Дәлилдәр өйөмөн әҙерләү

1. Сеймал артефакттарын йыйыу (телеметрия JSON, CLI журналдары, аудитор билдәләй).
2. Детерминистик архивты нормалаштырыу (мәҫәлән, татарбол); яҙымта:
   - BLAKE3-256 һеңдереүҙең (`evidence_digest`)
   - тип медиа (`application/zip`, `application/jsonl`, һ.б.)
   - Хостинг URI (объект һаҡлау, I18NT0000000001X булавка, йәки I18NT000000003X-ҡулланыусы ос нөктәһе)
3. Һаҡлау өйөм идара итеү дәлилдәре йыйыу биҙрә менән яҙыу-бер тапҡыр инеү.

## 3.

1. I18NI000000014X өсөн JSON спецификацияһын булдырыу:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI-ны эшләгеҙ:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` тикшерергә (раҫлау төрө, дәлилдәр үҙләштереү, ваҡыт маркалары).
. `dispute_id_hex` яуап ҡиммәтен төшөрөү; ул якорь эҙмә-эҙлекле ғәмәлдән сығарыу ғәмәлдәре һәм аудит отчеттары.

## 4. Эвакуация & Ҡабул итеү

1. **Рәхмәт тәҙрәһе:** яҡынлашып килгән ҡайтарыу менән тәьмин итеүсегә хәбәр итә; рөхсәт итеү өсөн эвакуация пинированный мәғлүмәттәрҙе ҡасан сәйәсәт рөхсәт итә.
2. **Беренсе I18NI000000018X:**
   - Ҡулланыу I18NI000000019X раҫланған сәбәп менән.
   - Ҡултамғаларҙы һәм ҡабул итеүҙе тикшерергә.
3. **Башҡа сығарыу:**
   - I18NT000000005X-ҡа ҡайтарыу запросын тапшырырға.
   - Тәьмин итеү провайдеры реклама блокировкаланған (көтөү I18NI0000000020X күтәрелергә).
4. **Яңыртыу приборҙар таҡталары:** флаг провайдер, нисек тартып алынған, бәхәс бәхәс идентификаторы, һәм дәлилдәр өйөмөн бәйләй.

## 5. Пост-Мортема & Һуңынан-Ук

- Ваҡыт һыҙығын, тамыр сәбәбен һәм идара итеү инцидент трекерында төҙәтеү ғәмәлдәрен яҙығыҙ.
- Реституцияны билдәләү (перчатка ҡырҡыу, түләү clowbacks, клиенттар ҡайтарыу).
- Документ өйрәнеүҙәре; яңыртыу SLA сиктәре йәки мониторинг иҫкәртмәләр, әгәр кәрәк.

## 6. Һылтанма материалдары

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (бәхәс бүлеге)
- `docs/source/sorafs/provider_admission_policy.md` (ҡайтарыу эш ағымы)
- Күҙәтеүсәнлек панелендә: `SoraFS / Capacity Providers`

## Тикшереү исемлеге

- [ ] Дәлилдәр өйөмө тотолған һәм хешировать.
- [ ] Бәхәс файҙалы йөк локаль раҫланған.
- [ ] Torii бәхәс операцияһы ҡабул ителә.
- [ ] Ҡабул итеү башҡарыла (әгәр раҫланһа).
- [ ] Приборҙар таҡталары/йүгереү китаптары яңыртылған.
- [ ] Идара итеү советына бирелгәндән һуң.