---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> Հարմարեցված է [`docs/source/sorafs/provider_advert_rollout.md`]-ից (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md):

# SoraFS Մատակարարի Գովազդի տարածման պլան

Այս ծրագիրը համակարգում է թույլատրելի մատակարարների գովազդներից կտրումը
լիովին կառավարվող `ProviderAdvertV1` մակերեսը, որն անհրաժեշտ է բազմաղբյուր հատվածի համար
առբերում. Այն կենտրոնանում է երեք առաքման վրա.

- **Օպերատորի ուղեցույց:** Քայլ առ քայլ գործողությունները պահեստավորման մատակարարները պետք է կատարեն
  յուրաքանչյուր դարպասի շրջվելուց առաջ:
- **Հեռաչափության ծածկույթ:** Վահանակներ և ծանուցումներ, որոնք օգտագործում են Observability-ը և Ops-ը
  հաստատելու համար, որ ցանցն ընդունում է միայն համապատասխան գովազդներ:
Տարածումը համընկնում է SF-2b/2c կարևոր իրադարձությունների հետ [SoraFS միգրացիայի մեջ
ճանապարհային քարտեզ] (./migration-roadmap) և ստանձնում է ընդունելության քաղաքականությունը
[մատակարարի ընդունելության քաղաքականություն] (./provider-admission-policy) արդեն կա
ազդեցություն.

## Ընթացիկ պահանջներ

SoraFS-ն ընդունում է միայն կառավարման ծրարով `ProviderAdvertV1` օգտակար բեռներ: Այն
Ընդունելության ժամանակ կիրառվում են հետևյալ պահանջները.

- `profile_id=sorafs.sf1@1.0.0` կանոնական `profile_aliases` ներկայությամբ:
- `chunk_range_fetch` կարողությունների օգտակար բեռները պետք է ներառվեն բազմաղբյուրի համար
  առբերում.
- `signature_strict=true`՝ գովազդին կից ավագանու ստորագրություններով
  ծրար.
- `allow_unknown_capabilities`-ը թույլատրվում է միայն հստակ GREASE փորվածքների ժամանակ
  և պետք է գրանցվի:

## Օպերատորի ստուգաթերթ

1. **Գույքագրման գովազդներ.** Ցուցակեք յուրաքանչյուր հրապարակված գովազդը և գրանցեք.
   - Կառավարող ծրարի ճանապարհ (`defaults/nexus/sorafs_admission/...` կամ արտադրական համարժեք):
   - Գովազդ `profile_id` և `profile_aliases`:
   - Հնարավորությունների ցանկ (ակնկալեք առնվազն `torii_gateway` և `chunk_range_fetch`):
   - `allow_unknown_capabilities` դրոշակ (պահանջվում է, երբ առկա են վաճառողի կողմից վերապահված TLV-ներ):
2. **Վերականգնել մատակարարի գործիքակազմով։**
   - Վերակառուցեք օգտակար բեռը ձեր մատակարարի գովազդի հրատարակչի հետ՝ ապահովելով.
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` սահմանված `max_span`-ով
     - `allow_unknown_capabilities=<true|false>`, երբ առկա են GREASE TLVs
   - Վավերացնել `/v2/sorafs/providers` և `sorafs_fetch` միջոցով; նախազգուշացումներ անհայտի մասին
     կարողությունները պետք է փորձարկվեն.
3. **Վավերացրեք բազմաղբյուրի պատրաստակամությունը:**
   - Կատարել `sorafs_fetch` `--provider-advert=<path>`-ով; CLI-ն այժմ ձախողվում է
     երբ `chunk_range_fetch`-ը բացակայում է և տպում է նախազգուշացումներ անտեսված անհայտի համար
     կարողությունները։ Գրեք JSON զեկույցը և արխիվացրեք այն գործառնությունների մատյաններով:
4. **Բեմական թարմացումներ.**
   - Ներկայացրեք `ProviderAdmissionRenewalV1` ծրարները առնվազն 30 օր առաջ
     ժամկետի ավարտը. Թարմացումները պետք է պահպանեն կանոնական բռնակը և կարողությունների հավաքածուն.
     միայն ցցերը, վերջնակետերը կամ մետատվյալները պետք է փոխվեն:
5. **Շփվեք կախյալ թիմերի հետ։**
   - SDK-ի սեփականատերերը պետք է թողարկեն տարբերակներ, որոնք օպերատորներին զգուշացնում են, երբ
     գովազդները մերժվում են.
   - DevRel-ը հայտարարում է յուրաքանչյուր փուլային անցում; ներառել վահանակի հղումները և
     շեմի տրամաբանությունը ստորև.
6. **Տեղադրեք վահանակներ և ծանուցումներ։**
   - Ներմուծեք Grafana արտահանումը և տեղադրեք այն **SoraFS / Մատակարարի տակ
     Տարածում** վահանակի UID `sorafs-provider-admission`-ով:
   - Համոզվեք, որ զգուշացման կանոնները ուղղված են ընդհանուր `sorafs-advert-rollout`-ին
     ծանուցման ալիք բեմադրության և արտադրության մեջ:

## Հեռաչափություն և վահանակներ

Հետևյալ ցուցանիշներն արդեն բացահայտված են `iroha_telemetry`-ի միջոցով.

- `torii_sorafs_admission_total{result,reason}` - հաշվում է ընդունված, մերժված,
  և նախազգուշական արդյունքներ: Պատճառները ներառում են `missing_envelope`, `unknown_capability`,
  `stale` և `policy_violation`:

Grafana արտահանում` [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json):
Ներմուծեք ֆայլը համօգտագործվող վահանակների պահոց (`observability/dashboards`)
և հրապարակելուց առաջ թարմացրեք միայն տվյալների աղբյուրի UID-ը:

Տախտակը հրապարակվում է Grafana թղթապանակում **SoraFS / Provider Rollout** հետ
կայուն UID `sorafs-provider-admission`: Զգուշացման կանոններ
`sorafs-admission-warn` (նախազգուշացում) և `sorafs-admission-reject` (կրիտիկական) են
նախապես կազմաձևված՝ օգտագործելու `sorafs-advert-rollout` ծանուցման քաղաքականությունը. հարմարեցնել
այդ կոնտակտային կետը, եթե նպատակակետի ցանկը փոխվի, այլ ոչ թե խմբագրվի
վահանակ JSON:

Առաջարկվող Grafana վահանակներ.

| Վահանակ | Հարցում | Ծանոթագրություններ |
|-------|-------|-------|
| **Ընդունելության արդյունքը** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Կցեք գծապատկերը՝ պատկերացնելով ընդունելն ընդդեմ նախազգուշացման ընդդեմ մերժման: Զգուշացում, երբ զգուշացնում է > 0,05 * ընդհանուր (նախազգուշացում) կամ մերժում > 0 (կրիտիկական): |
| **Նախազգուշացման հարաբերակցությունը** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Մեկ տողանի ժամանակաշարեր, որոնք սնուցում են փեյջերի շեմը (5% նախազգուշացման արագությունը շարժվում է 15 րոպե): |
| **Մերժման պատճառները** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Վարում է runbook triage; կցել հղումներ մեղմացման քայլերին: |
| **Թարմացնել պարտքը** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Ցույց է տալիս մատակարարներին, որոնք բաց են թողել թարմացման վերջնաժամկետը. խաչաձև հղում հայտնաբերման քեշի մատյանների հետ: |

CLI արտեֆակտներ ձեռքով վահանակների համար.

- `sorafs_fetch --provider-metrics-out`-ը գրում է `failures`, `successes` և
  `disabled` հաշվիչներ մեկ մատակարարի համար: Մոնիտորինգի համար ներմուծեք ժամանակավոր վահանակներ
  նվագախումբը չորանում է, նախքան արտադրության մատակարարներին անցնելը:
- JSON զեկույցի `chunk_retry_rate` և `provider_failure_rate` դաշտերը
  ընդգծել շնչափող կամ հնացած ծանրաբեռնվածության ախտանիշները, որոնք հաճախ նախորդում են ընդունելությանը
  մերժումները.

### Grafana վահանակի դասավորությունը

Observability-ը հրապարակում է հատուկ տախտակ — **SoraFS Մատակարարի ընդունելություն
Տարածում** (`sorafs-provider-admission`) — **SoraFS-ի ներքո / Մատակարարի թողարկում**
հետևյալ կանոնական վահանակի ID-ներով.

- Վահանակ 1 — *Ընդունելության արդյունքի դրույքաչափ* (դասավոր տարածք, միավոր «ops/min»):
- Վահանակ 2 — *Զգուշացման հարաբերակցություն* (մեկ սերիա), արտանետող արտահայտությունը
  «sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`:
- Վահանակ 3 — *Մերժման պատճառները* (ժամանակային շարքերը խմբավորված ըստ `reason`), տեսակավորված ըստ
  `rate(...[5m])`.
- Վահանակ 4 — *Թարմացնել պարտքը* (վիճակագրություն), արտացոլելով վերը նշված աղյուսակի հարցումը և
  ծանոթագրված է միգրացիոն մատյանից հանված գովազդի թարմացման վերջնաժամկետներով:

Պատճենեք (կամ ստեղծեք) JSON կմախքը ենթակառուցվածքի վահանակների ռեպո-ում
`observability/dashboards/sorafs_provider_admission.json`, ապա թարմացրեք միայն
տվյալների աղբյուր UID; վահանակի ID-ները և զգուշացման կանոնները հղում են կատարում runbook-ում
ստորև, այնպես որ խուսափեք դրանք վերահամարակալելուց՝ առանց այս փաստաթղթերը վերանայելու:

Հարմարության համար պահոցն այժմ ուղարկում է հղման վահանակի սահմանումը
`docs/source/grafana_sorafs_admission.json`; պատճենեք այն ձեր Grafana թղթապանակում, եթե
Ձեզ անհրաժեշտ է ելակետ տեղական թեստավորման համար:

### Prometheus ահազանգման կանոններ

Ավելացրեք հետևյալ կանոնների խումբը `observability/prometheus/sorafs_admission.rules.yml`-ին
(ստեղծեք ֆայլը, եթե սա SoraFS կանոնների առաջին խումբն է) և ներառեք այն
ձեր Prometheus կոնֆիգուրացիան: Փոխարինեք `<pagerduty>`-ը իրական երթուղիչով
պիտակ ձեր հերթապահ ռոտացիայի համար:

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Գործարկեք `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
նախքան փոփոխությունները մղելը, որպեսզի շարահյուսությունը անցնի `promtool check rules`:

## Ընդունելության արդյունքներ

- Բացակայում է `chunk_range_fetch` հնարավորությունը → մերժել `reason="missing_capability"`-ով:
- Անհայտ ունակությամբ TLVs առանց `allow_unknown_capabilities=true` → մերժել հետ
  `reason="unknown_capability"`.
- `signature_strict=false` → մերժել (վերապահված է մեկուսացված ախտորոշման համար):
- Ժամկետանց ժամկետանց `refresh_deadline` → մերժել:

## Հաղորդակցություն և միջադեպերի կառավարում

- **Շաբաթական կարգավիճակի նամակագրություն:** DevRel-ը տարածում է ընդունելության համառոտ ամփոփագիրը
  չափումներ, չմարված նախազգուշացումներ և առաջիկա վերջնաժամկետներ:
- **Միջադեպի արձագանքը։** Եթե `reject`-ն ահազանգում է կրակի մասին, հերթապահ ինժեներները.
  1. Ստացեք վիրավորող գովազդը Torii հայտնաբերման միջոցով (`/v2/sorafs/providers`):
  2. Կրկին գործարկեք գովազդի վավերացումը մատակարարի խողովակաշարում և համեմատեք դրա հետ
     `/v2/sorafs/providers` սխալը վերարտադրելու համար:
  3. Համակարգեք մատակարարի հետ՝ գովազդը պտտելու համար մինչև հաջորդ թարմացումը
     վերջնաժամկետը.
- **Փոփոխությունը սառչում է:** Հնարավորությունների ոչ մի սխեման չի փոխում հողը R1/R2-ի ընթացքում, եթե
  գործարկման հանձնաժողովը ստորագրում է. GREASE-ի փորձարկումները պետք է նշանակվեն այս ընթացքում
  շաբաթական սպասարկման պատուհան և մուտքագրվել է միգրացիայի մատյան:

## Հղումներ

- [SoraFS հանգույց/հաճախորդի արձանագրություն] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Մատուցողի ընդունելության քաղաքականություն] (./provider-admission-policy)
- [Միգրացիոն ճանապարհային քարտեզ] (./migration-roadmap)
- [Provader Advert Multi-Source Extensions] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)