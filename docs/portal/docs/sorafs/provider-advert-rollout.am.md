---
lang: am
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

> ከ[`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) የተወሰደ።

# SoraFS አቅራቢ የማስታወቂያ ልቀት እቅድ

ይህ እቅድ ከፈቃድ ሰጪ አቅራቢዎች ማስታዎቂያዎች እስከ መቋረጥን ያስተባብራል።
ሙሉ በሙሉ የሚተዳደረው I18NI0000030X ወለል ለብዙ-ምንጭ ቁራጭ ያስፈልጋል
መልሶ ማግኘት. በሦስት ማቅረቢያዎች ላይ ያተኩራል-

- ** የኦፕሬተር መመሪያ።** የደረጃ በደረጃ እርምጃዎች ማከማቻ አቅራቢዎች ማጠናቀቅ አለባቸው
  እያንዳንዱ በር ከመገለባበጥ በፊት.
- **የቴሌሜትሪ ሽፋን።** ታዛቢነት እና ኦፕስ የሚጠቀሙባቸው ዳሽቦርዶች እና ማንቂያዎች
  ለማረጋገጥ አውታረ መረቡ የሚያከብሩ ማስታወቂያዎችን ብቻ ይቀበላል።
ልቀቱ በ[SoraFS ፍልሰት ውስጥ ከSF-2b/2c ችካሎች ጋር ይስማማል።
የመንገድ ካርታ](./migration-roadmap) እና የመግቢያ ፖሊሲውን በ
[የአቅራቢ መግቢያ ፖሊሲ](./provider-admission-policy) አስቀድሞ ገብቷል።
ተፅዕኖ.

## ወቅታዊ መስፈርቶች

SoraFS የሚቀበለው በአስተዳደር የታሸጉ `ProviderAdvertV1` ክፍያዎችን ብቻ ነው። የ
በመግቢያው ላይ የሚከተሉት መስፈርቶች ተፈጻሚ ይሆናሉ

- `profile_id=sorafs.sf1@1.0.0` ከቀኖናዊ I18NI0000033X ጋር።
- `chunk_range_fetch` የችሎታ ጭነት ለብዙ ምንጭ መካተት አለበት።
  መልሶ ማግኘት.
- `signature_strict=true` የምክር ቤት ፊርማዎች ከማስታወቂያው ጋር ተያይዘዋል።
  ኤንቨሎፕ.
- `allow_unknown_capabilities` የሚፈቀደው በግልፅ የ GREASE ልምምዶች ጊዜ ብቻ ነው።
  እና መመዝገብ አለበት.

## የኦፕሬተር ማረጋገጫ ዝርዝር

1. **የእቃ ዝርዝር ማስታወቂያዎች** እያንዳንዱን የታተመ ማስታወቂያ እና መዝገብ ይዘርዝሩ፡-
   - የአስተዳደር ፖስታ መንገድ (`defaults/nexus/sorafs_admission/...` ወይም የምርት አቻ)።
   - `profile_id` እና I18NI0000039X ያስተዋውቁ።
   - የችሎታ ዝርዝር (ቢያንስ `torii_gateway` እና `chunk_range_fetch` ይጠብቁ)።
   - `allow_unknown_capabilities` ባንዲራ (በሻጭ የተያዙ TLVዎች በሚኖሩበት ጊዜ ያስፈልጋል)።
2. ** በአገልግሎት አቅራቢ መሳሪያዎች እንደገና ማመንጨት።**
   - ክፍያውን በአቅራቢዎ የማስታወቂያ አታሚ እንደገና ይገንቡ፣
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` ከተገለጸው I18NI0000045X ጋር
     - GREASE TLVs ሲገኙ `allow_unknown_capabilities=<true|false>`
   - በI18NI0000047X እና I18NI0000048X በኩል ያረጋግጡ; ስለማይታወቅ ማስጠንቀቂያዎች
     ችሎታዎች መከፋፈል አለባቸው.
3. **የባለብዙ ምንጭ ዝግጁነትን ያረጋግጡ።**
   - `sorafs_fetch` በ I18NI0000050X ያስፈጽሙ; CLI አሁን አልተሳካም።
     `chunk_range_fetch` ሲጎድል እና ችላ ተብሎ ለማይታወቅ ማስጠንቀቂያዎችን ሲያትም።
     ችሎታዎች. የJSON ሪፖርቱን ይቅረጹ እና በኦፕሬሽን ምዝግብ ማስታወሻዎች ያስቀምጡት።
4. ** የመድረክ እድሳት።**
   - ቢያንስ ከ 30 ቀናት በፊት `ProviderAdmissionRenewalV1` ፖስታዎችን ያስገቡ
     የማለቂያ ጊዜ. እድሳት ቀኖናዊውን እጀታ እና የችሎታ ስብስብ ማቆየት አለበት;
     ድርሻ፣ የመጨረሻ ነጥብ ወይም ሜታዳታ ብቻ መቀየር አለበት።
5. **ከጥገኛ ቡድኖች ጋር ተገናኝ።**
   - የኤስዲኬ ባለቤቶች መቼ ለኦፕሬተሮች ማስጠንቀቂያ የሚሰጡ ስሪቶችን መልቀቅ አለባቸው
     ማስታወቂያዎች ውድቅ ናቸው.
   - DevRel እያንዳንዱን ደረጃ ሽግግር ያስታውቃል; ዳሽቦርድ አገናኞችን እና የ
     የመነሻ አመክንዮ ከዚህ በታች።
6. ** ዳሽቦርዶችን እና ማንቂያዎችን ጫን።**
   - Grafana ወደ ውጭ መላክ እና በ **SoraFS / አቅራቢ ስር ያድርጉት
     ልቀቅ *** ከዳሽቦርድ UID `sorafs-provider-admission`።
   - የማንቂያ ደንቦቹ ወደ የተጋራው `sorafs-advert-rollout` እንደሚጠቁሙ ያረጋግጡ
     በማዘጋጀት እና በምርት ውስጥ የማሳወቂያ ቻናል ።

## ቴሌሜትሪ እና ዳሽቦርዶች

የሚከተሉት መለኪያዎች ቀድሞውኑ በ`iroha_telemetry` በኩል ተጋልጠዋል።

- `torii_sorafs_admission_total{result,reason}` - ቆጠራዎች ተቀባይነት አግኝተዋል ፣ ውድቅ ተደርጓል ፣
  እና የማስጠንቀቂያ ውጤቶች. ምክንያቶቹ `missing_envelope`፣ `unknown_capability`፣
  `stale`፣ እና `policy_violation`።

Grafana ወደ ውጭ መላክ፡ [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)።
ፋይሉን ወደ የተጋራው ዳሽቦርድ ማከማቻ አስመጣ (`observability/dashboards`)
እና ከማተምዎ በፊት የውሂብ ምንጭ UID ብቻ ያዘምኑ።

ቦርዱ በI18NT0000004X አቃፊ **SoraFS/የአቅራቢ ልቀት** ያትማል።
የተረጋጋው UID I18NI0000063X. የማንቂያ ደንቦች
`sorafs-admission-warn` (ማስጠንቀቂያ) እና I18NI0000065X (ወሳኝ) ናቸው
የ `sorafs-advert-rollout` የማሳወቂያ ፖሊሲን ለመጠቀም አስቀድሞ የተዋቀረ; ማስተካከል
የመድረሻ ዝርዝሩን ከማርትዕ ይልቅ ከተቀየረ ያንን የመገናኛ ነጥብ
ዳሽቦርድ JSON

የሚመከሩ Grafana ፓነሎች፡

| ፓነል | ጥያቄ | ማስታወሻ |
|-------|-------|------|
| ** የመግቢያ ውጤት መጠን** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | በምስላዊ ለማየት ገበታ ቁልል ተቀበል እና አስጠንቅቅን አልቀበልም። አስጠንቅቅ> 0.05 * ጠቅላላ (ማስጠንቀቂያ) ወይም ውድቅ > 0 (ወሳኝ) |
| **የማስጠንቀቂያ ጥምርታ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | የፔጀር ጣራን የሚመግቡ ነጠላ-መስመር ተከታታይ ጊዜዎች (5% የማስጠንቀቂያ መጠን 15 ደቂቃ ማንከባለል)። |
| ** ውድቅ የተደረገባቸው ምክንያቶች** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Runbook triage ያንቀሳቅሳል; አገናኞችን ወደ ቅነሳ እርምጃዎች ያያይዙ። |
| **ዕዳ ያድሱ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | የማደሻ ቀነ-ገደቡን የሚያጡ አቅራቢዎችን ያሳያል። ማመሳከሪያ ከግኝት መሸጎጫ ምዝግብ ማስታወሻዎች ጋር። |

በእጅ ዳሽቦርድ የ CLI ቅርሶች፡-

- `sorafs_fetch --provider-metrics-out` `failures`፣ `successes` ይጽፋል፣ እና
  `disabled` ቆጣሪዎች በአንድ አቅራቢ። ለመከታተል ወደ ad-hoc ዳሽቦርዶች ያስመጡ
  ኦርኬስትራ የማምረቻ አቅራቢዎችን ከመቀየርዎ በፊት ይደርቃል።
- የJSON ዘገባ I18NI0000075X እና `provider_failure_rate` መስኮች
  ብዙውን ጊዜ ከመግባት በፊት የሚመጡ የመጎሳቆል ወይም የቆዩ የክፍያ ምልክቶችን ያሳዩ
  አለመቀበል።

### Grafana ዳሽቦርድ አቀማመጥ

ታዛቢነት ራሱን የቻለ ቦርድ ያትማል - ** SoraFS የአቅራቢ መግቢያ
ልቀት** (I18NI0000077X) — በ**SoraFS/የአቅራቢ ልቀት** ስር
ከሚከተሉት ቀኖናዊ ፓነል መታወቂያዎች ጋር፡-

- ፓነል 1 - * የመግቢያ ውጤት መጠን * (የተቆለለ ቦታ ፣ አሃድ “ops / ደቂቃ”)።
- ፓነል 2 - * የማስጠንቀቂያ ጥምርታ * (ነጠላ ተከታታይ) ፣ አገላለጹን ያወጣል።
  `ድምር(ተመን(የቶሪ_ሶራፍ_ጠቅላላ_መቀበያ_ጠቅላላ{ውጤት=«ማስጠንቀቂያ»[5ሚ]))) /
   ድምር(ተመን(torii_sorafs_ጠቅላላ[5m]))`
- ፓነል 3 - * ውድቅ የተደረገባቸው ምክንያቶች* (በ`reason` የተከፋፈሉ ተከታታይ) ፣ የተደረደሩ
  `rate(...[5m])`.
- ፓነል 4 - * ዕዳን አድስ * (ስታቲስቲክስ)፣ ከላይ ባለው ሠንጠረዥ ውስጥ ያለውን መጠይቅ በማንጸባረቅ እና
  ከስደት ደብተር በተወጣው የማስታወቂያ ማደሻ ቀነ-ገደቦች ተብራርቷል።

በመሠረተ ልማት ዳሽቦርዶች ውስጥ የJSON አጽሙን ይቅዱ (ወይም ይፍጠሩ) በ
`observability/dashboards/sorafs_provider_admission.json`፣ ከዚያ ማዘመን ብቻ
የውሂብ ምንጭ UID; የፓነል መታወቂያዎች እና የማንቂያ ደንቦች በ runbooks ተጠቅሰዋል
ከታች፣ ስለዚህ ይህን ሰነድ ሳይከልሱ እነሱን እንደገና ከመቁጠር ይቆጠቡ።

ለመመቻቸት ማከማቻው አሁን የማመሳከሪያ ዳሽቦርድ ትርጉም በ ላይ ይልካል።
`docs/source/grafana_sorafs_admission.json`; ከሆነ ወደ የእርስዎ Grafana አቃፊ ይቅዱት።
ለአካባቢያዊ ሙከራዎች መነሻ ያስፈልግዎታል.

### Prometheus ማንቂያ ደንቦች

የሚከተለውን ደንብ ቡድን ወደ I18NI0000082X ያክሉ
(ይህ የመጀመሪያው SoraFS ደንብ ቡድን ከሆነ ፋይሉን ይፍጠሩ) እና ያካትቱት
የእርስዎ Prometheus ውቅር። `<pagerduty>` በእውነተኛው ማዞሪያ ተካ
በጥሪ ላይ ማሽከርከርዎ ላይ ምልክት ያድርጉ።

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

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` አሂድ
አገባብ `promtool check rules` ማለፉን ለማረጋገጥ ለውጦችን ከመግፋቱ በፊት።

## የመግቢያ ውጤቶች

- የጠፋ I18NI0000086X አቅም → ከ`reason="missing_capability"` ጋር ውድቅ አድርግ።
- ያልታወቀ ችሎታ TLVs ያለ `allow_unknown_capabilities=true` → ውድቅ
  `reason="unknown_capability"`.
- `signature_strict=false` → ውድቅ (ለገለልተኛ ምርመራ የተቀመጠ)።
- ጊዜው ያለፈበት I18NI0000091X → ውድቅ

## ኮሙኒኬሽን እና የክስተት አያያዝ

- ** ሳምንታዊ ሁኔታ ፖስታ።** DevRel የመግቢያ አጭር ማጠቃለያ ያሰራጫል።
  መለኪያዎች፣ አስደናቂ ማስጠንቀቂያዎች እና መጪ የግዜ ገደቦች።
- **የአደጋ ምላሽ።** `reject` ማስጠንቀቂያ ከተቃጠለ፣ የጥሪ መሐንዲሶች፡-
  1. የሚያስከፋውን ማስታወቂያ በI18NT0000019X ግኝት (`/v1/sorafs/providers`) ያውጡ።
  2. በአቅራቢው ቧንቧ መስመር ውስጥ የማስታወቂያ ማረጋገጫን እንደገና ያሂዱ እና ከ ጋር ያወዳድሩ
     `/v1/sorafs/providers` ስህተቱን ለማባዛት.
  3. ከሚቀጥለው እድሳት በፊት ማስታወቂያውን ለማዞር ከአቅራቢው ጋር ይተባበሩ
     ቀነ ገደብ.
- ** ይቀዘቅዛል።** በR1/R2 ጊዜ ምንም የችሎታ ንድፍ ካልሆነ በስተቀር መሬት አይቀየርም።
  የታቀደው ኮሚቴ ይፈርማል; የGREASE ሙከራዎች በሂደቱ ጊዜ መርሐግብር ሊሰጣቸው ይገባል።
  ሳምንታዊ የጥገና መስኮት እና ወደ ፍልሰት ደብተር ገብቷል።

## ዋቢዎች

- [SoraFS መስቀለኛ መንገድ/የደንበኛ ፕሮቶኮል](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [የአቅራቢ መግቢያ መመሪያ](./provider-admission-policy)
- [የስደት ፍኖተ ካርታ](./migration-roadmap)
- [የአቅራቢ ባለብዙ ምንጭ ቅጥያዎች](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)