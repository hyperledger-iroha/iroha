---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Taikai Anchor Observability Runbook

Այս պորտալի պատճենը արտացոլում է կանոնական մատյանը
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md):
Օգտագործեք այն SN13-C երթուղային մանիֆեստի (TRM) խարիսխները կրկնելիս, որպեսզի SoraFS/SoraNet
օպերատորները կարող են փոխկապակցել կծիկի արտեֆակտները, Prometheus հեռաչափությունը և կառավարումը
ապացույցներ՝ առանց պորտալի նախադիտման կառուցումը թողնելու:

## Շրջանակ և սեփականատերեր

- **Ծրագիր.** SN13-C — Taikai-ի դրսևորումներ և SoraNS խարիսխներ:
- **Սեփականատերեր.** Media Platform WG, DA Program, Networking TL, Docs/DevRel:
- ** Նպատակ. ** Տրամադրել դետերմինիստական խաղագիրք Sev1/Sev2 ահազանգերի, հեռաչափության համար
  վավերացում և ապացույցների հավաքում, մինչ Taikai երթուղիների դրսևորումները առաջ են շարժվում
  փոխանունների միջով:

## Արագ մեկնարկ (Sev1/Sev2)

1. **Սպանել կծիկի արտեֆակտները** — պատճենել վերջինը
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` և
   `taikai-lineage-*.json` ֆայլեր
   `config.da_ingest.manifest_store_dir/taikai/` նախքան աշխատողներին վերագործարկելը:
2. **Թափել `/status` հեռաչափությունը** — գրանցել
   `telemetry.taikai_alias_rotations` զանգված՝ ապացուցելու, թե որն է մանիֆեստի պատուհանը
   ակտիվ:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Ստուգեք վահանակները և ազդանշանները** — բեռնել
   `dashboards/grafana/taikai_viewer.json` (կլաստեր + հոսքի զտիչներ) և նշում
   արդյոք որևէ կանոն կա
   `dashboards/alerts/taikai_viewer_rules.yml` կրակված (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS ապացույց-առողջական իրադարձություններ):
4. **Ստուգեք Prometheus** — գործարկեք հարցումները §«Մետրական հղում»՝ հաստատելու համար
   ingest latency/drift և alias-rotation հաշվիչներն իրենց պահում են այնպես, ինչպես սպասվում էր: Էսկալացիա
   եթե `taikai_trm_alias_rotations_total`-ը փակ է մի քանի պատուհանների համար, կամ եթե
   սխալների հաշվիչներն ավելանում են:

## Մետրային հղում

| Մետրական | Նպատակը |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF-ը ներծծում է հետաձգման հիստոգրամ մեկ կլաստերի/հոսքի համար (նպատակը՝ p95<750ms, p99<900ms): |
| `taikai_ingest_live_edge_drift_ms` | Կոդավորիչի և խարիսխի աշխատողների միջև ուղիղ շեղում (էջեր p99>1,5 վրկ 10 րոպեի համար): |
| `taikai_ingest_segment_errors_total{reason}` | Սխալները հաշվվում են ըստ պատճառի (`decode`, `manifest_mismatch`, `lineage_replay`,…): Ցանկացած աճ առաջացնում է `TaikaiIngestFailure`: |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Աճում է ամեն անգամ, երբ `/v1/da/ingest`-ն ընդունում է նոր TRM կեղծանունը. օգտագործեք `rate()` ռոտացիայի արագությունը հաստատելու համար: |
| `/status → telemetry.taikai_alias_rotations[]` | JSON լուսանկար՝ `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` և ապացույցների փաթեթների ժամանակային դրոշմանիշներով: |
| `taikai_viewer_*` (rebuffer, CEK պտտման տարիք, PQ առողջություն, ահազանգեր) | Դիտողի կողմից KPI-ները՝ ապահովելու համար CEK ռոտացիան + PQ սխեմաները խարիսխների ընթացքում առողջ մնալու համար: |

### PromQL հատվածներ

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Վահանակներ և ահազանգեր

- **Grafana դիտող տախտակ.** `dashboards/grafana/taikai_viewer.json` — p95/p99
  ուշացում, ուղիղ եզրերի շեղում, հատվածի սխալներ, CEK պտտման տարիք, դիտողների ազդանշաններ:
- **Grafana քեշ տախտակ:** `dashboards/grafana/taikai_cache.json` — տաք/տաք/սառը
  առաջխաղացումները և QoS-ի մերժումները, երբ պտտվում են կեղծանունները:
- **Alertmanager կանոններ.** `dashboards/alerts/taikai_viewer_rules.yml` — դրեյֆ
  Էջավորում, խափանման նախազգուշացումներ, CEK ռոտացիայի հետաձգում և SoraFS հաստատված առողջություն
  տույժեր/սառեցումներ. Համոզվեք, որ ընդունիչներ կան յուրաքանչյուր արտադրական կլաստերի համար:

## Ապացույցների փաթեթի ստուգաթերթ

- Կծիկավոր արտեֆակտներ (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`):
- Գործարկեք `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`՝ առկախ/առաքված ծրարների ստորագրված JSON գույքագրումը և պատճենեք հարցումը/SSM/TRM/տոհմային ֆայլերը փորված փաթեթում: Լռելյայն պտույտի ուղին `storage/da_manifests/taikai` է `torii.toml`-ից:
- `/status` լուսանկար, որը ծածկում է `telemetry.taikai_alias_rotations`:
- Prometheus արտահանում է (JSON/CSV) վերը նշված չափումների համար միջադեպի պատուհանի վրա:
- Grafana սքրինշոթներ՝ տեսանելի զտիչներով:
- Alertmanager ID-ները, որոնք վկայակոչում են համապատասխան կանոնների հրդեհները:
- Հղում դեպի `docs/examples/taikai_anchor_lineage_packet.md`, որը նկարագրում է
  կանոնական ապացույցների փաթեթ:

## Վահանակի հայելիավորում և փորված արագություն

SN13-C ճանապարհային քարտեզի պահանջը բավարարելը նշանակում է ապացուցել, որ Taikai-ն
հեռուստադիտողի/քեշի վահանակները արտացոլվում են պորտալի ներսում **և** որ խարիսխը
ապացույցների վարժանքն ընթանում է կանխատեսելի արագությամբ:

1. **Պորտալի հայելավորում։** Ամեն անգամ, երբ `dashboards/grafana/taikai_viewer.json` կամ
   `dashboards/grafana/taikai_cache.json` փոփոխություններ, ամփոփեք դելտաները
   `sorafs/taikai-monitoring-dashboards` (այս պորտալը) և նշեք JSON-ը
   չեկային գումարներ պորտալի PR նկարագրության մեջ: Այսպես ընդգծեք նոր վահանակները/շեմերը
   վերանայողները կարող են փոխկապակցվել կառավարվող Grafana թղթապանակի հետ:
2. **Ամսական փորված.**
   - Վարժությունը կատարեք յուրաքանչյուր ամսվա առաջին երեքշաբթի օրը, ժամը 15:00 UTC-ին, որպեսզի ապացուցեք
     հողերը նախքան SN13 կառավարման համաժամացումը:
   - Ներսում նկարեք կծիկի արտեֆակտներ, `/status` հեռաչափություն և Grafana սքրինշոթներ
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Մուտքագրեք կատարումը
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Վերանայեք և հրապարակեք:** 48 ժամվա ընթացքում ստուգեք ծանուցումները/կեղծ պոզիտիվները
   DA Program + NetOps, գրանցեք հետագա տարրերը փորված մատյանում և կապեք
   կառավարման դույլի վերբեռնում `docs/source/sorafs/runbooks-index.md`-ից:

Եթե ​​վահանակները կամ վարժանքները հետ են մնում, SN13-C-ն չի կարող դուրս գալ 🈺; պահիր սա
բաժինը թարմացվում է ամեն անգամ, երբ փոխվում են կադենսը կամ ապացույցների ակնկալիքները:

## Օգտակար հրամաններ

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Պահպանեք այս պորտալի պատճենը կանոնական գրքի հետ համաժամեցված, երբ Taikai
խարսխված հեռաչափության, վահանակների կամ կառավարման ապացույցների պահանջները փոխվում են: