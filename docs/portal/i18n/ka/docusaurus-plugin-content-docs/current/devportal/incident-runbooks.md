---
id: incident-runbooks
lang: ka
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## მიზანი

საგზაო რუქის პუნქტი **DOCS-9** ითხოვს ქმედითუნარიან სათამაშო წიგნებს პლუს რეპეტიციების გეგმას.
პორტალის ოპერატორებს შეუძლიათ გამოცნობის გარეშე გამოჯანმრთელდნენ გადაზიდვის წარუმატებლობისგან. ეს შენიშვნა
მოიცავს სამ მაღალი სიგნალის ინციდენტს - წარუმატებელი განლაგება, რეპლიკაცია
დეგრადაცია და ანალიტიკის გათიშვა-და დოკუმენტირებულია კვარტალური წვრთნები, რომ
დაამტკიცეთ მეტსახელის დაბრუნება და სინთეტიკური ვალიდაცია მაინც მუშაობს ბოლომდე.

### დაკავშირებული მასალა

- [`devportal/deploy-guide`](./deploy-guide) - შეფუთვა, ხელმოწერა და მეტსახელი
  სარეკლამო სამუშაო პროცესი.
- [`devportal/observability`](./observability) — გამოშვების ტეგები, ანალიტიკა და
  ზონდები, რომლებიც მითითებულია ქვემოთ.
- `docs/source/sorafs_node_client_protocol.md`
  და [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — რეესტრის ტელემეტრია და ესკალაციის ზღურბლები.
- `docs/portal/scripts/sorafs-pin-release.sh` და `npm run probe:*` დამხმარეები
  მითითებულია მთელ საკონტროლო სიებში.

### საერთო ტელემეტრია და ხელსაწყოები

| სიგნალი / ხელსაწყო | დანიშნულება |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (შეხვედრა/გამოტოვებული/მოლოდინშია) | აღმოაჩენს რეპლიკაციის შეჩერებას და SLA დარღვევებს. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | რაოდენობრივად განსაზღვრავს ტრიაჟის ჩამორჩენის სიღრმეს და დასრულების შეყოვნებას. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | აჩვენებს კარიბჭის მარცხს, რომლებიც ხშირად მოჰყვება ცუდ განლაგებას. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | სინთეზური ზონდები, რომლებიც ათავისუფლებს კარიბჭეს და ამოწმებს გაშვებას. |
| `npm run check:links` | გატეხილი ბმული კარიბჭე; გამოიყენება ყოველი შერბილების შემდეგ. |
| `sorafs_cli manifest submit … --alias-*` (შეფუთული `scripts/sorafs-pin-release.sh`) | Alias-ის ხელშეწყობა/დაბრუნების მექანიზმი. |
| `Docs Portal Publishing` Grafana დაფა (`dashboards/grafana/docs_portal.json`) | აგრეგატების უარყოფა / მეტსახელი / TLS / რეპლიკაციის ტელემეტრია. PagerDuty სიგნალიზაცია მიუთითებს ამ პანელებზე მტკიცებულებისთვის. |

## Runbook — წარუმატებელი განლაგება ან ცუდი არტეფაქტი

### გამომწვევი პირობები

- წინასწარი გადახედვის/წარმოების ზონდები ვერ ხერხდება (`npm run probe:portal -- --expect-release=…`).
- Grafana გაფრთხილებები `torii_sorafs_gateway_refusals_total`-ზე ან
  `torii_sorafs_manifest_submit_total{status="error"}` გაშვების შემდეგ.
- სახელმძღვანელო QA შენიშნავს გატეხილ მარშრუტებს ან Try-It proxy-ის წარუმატებლობას მაშინვე
  მეტსახელის პრომოუშენი.

### დაუყოვნებელი შეკავება

1. **განლაგების გაყინვა:** მონიშნეთ CI მილსადენი `DEPLOY_FREEZE=1`-ით (GitHub
   სამუშაო პროცესის შეყვანა) ან შეაჩერეთ ჯენკინსის სამუშაო ისე, რომ დამატებითი არტეფაქტები არ გამოვიდეს.
2. **არტეფაქტების აღება:** ჩამოტვირთეთ წარუმატებელი კონსტრუქციის `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}` და ზონდის გამომავალი, რათა უკან დაბრუნება შეძლოს
   მინიშნება ზუსტი დაიჯესტები.
3. **აცნობეთ დაინტერესებულ მხარეებს:** შენახვის SRE, Docs/DevRel ლიდერი და მმართველობა
   მორიგე ინფორმირებულობისთვის (განსაკუთრებით მაშინ, როდესაც `docs.sora` არის ზემოქმედება).

### დაბრუნების პროცედურა

1. დაადგინეთ ბოლო ცნობილი-კარგი (LKG) მანიფესტი. წარმოების სამუშაო ნაკადი ინახავს
   მათ `artifacts/devportal/<release>/sorafs/portal.manifest.to` ქვეშ.
2. ხელახლა დააბრუნეთ ალიასი ამ მანიფესტზე გადაზიდვის დამხმარესთან ერთად:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. ჩაწერეთ უკან დაბრუნების შეჯამება ინციდენტის ბილეთში LKG-სთან ერთად და
   წარუმატებელი მანიფესტი დაიჯესტები.

### დადასტურება

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` და `sorafs_cli proof verify …`
   (იხილეთ განლაგების სახელმძღვანელო), რათა დაადასტუროთ ხელახლა დაწინაურებული მანიფესტი მაინც ემთხვევა
   დაარქივებული მანქანა.
4. `npm run probe:tryit-proxy` იმის უზრუნველსაყოფად, რომ Try-It დადგმის პროქსი დაბრუნდა.

### ინციდენტის შემდგომი

1. ხელახლა ჩართეთ განლაგების მილსადენი მხოლოდ მას შემდეგ, რაც გაიგო ძირითადი მიზეზი.
2. შევსება [`devportal/deploy-guide`](./deploy-guide) „ნასწავლი გაკვეთილები“
   ჩანაწერები ახალი გოთებით, ასეთის არსებობის შემთხვევაში.
3. ფაილის დეფექტები წარუმატებელი ტესტის კომპლექტისთვის (ზონდი, ბმულის შემოწმება და ა.შ.).

## Runbook — რეპლიკაციის დეგრადაცია

### გამომწვევი პირობები

- გაფრთხილება: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"შეხვედრა|გამოტოვებული"}), 1) <
  0.95` 10 წუთის განმავლობაში.
- `torii_sorafs_replication_backlog_total > 10` 10 წუთის განმავლობაში (იხ
  `pin-registry-ops.md`).
- მმართველობა იტყობინება ზედმეტსახელების ნელი ხელმისაწვდომობის შესახებ გამოშვების შემდეგ.

### ტრიაჟი

1. შეამოწმეთ [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) დაფები დასადასტურებლად
   არის თუ არა ნარჩენები ლოკალიზებული შენახვის კლასში ან პროვაიდერის ფლოტზე.
2. გადაამოწმეთ Torii ჟურნალები `sorafs_registry::submit_manifest` გაფრთხილებებისთვის
   დაადგინეთ, წარდგენილი თუ არა თავად.
3. ასლის ჯანმრთელობის ნიმუში `sorafs_cli manifest status --manifest …`-ის საშუალებით (სიები
   თითო პროვაიდერზე რეპლიკაციის შედეგები).

### შერბილება

1. ხელახლა გამოუშვით manifest უფრო მაღალი ასლების რაოდენობით (`--pin-min-replicas 7`) გამოყენებით
   `scripts/sorafs-pin-release.sh` ისე, რომ დამგეგმავი ავრცელებს დატვირთვას უფრო დიდზე
   პროვაიდერის ნაკრები. ჩაწერეთ ახალი მანიფესტის დაიჯესტი ინციდენტების ჟურნალში.
2. თუ ნარჩენები მიბმულია ერთ პროვაიდერთან, დროებით გამორთეთ ის
   რეპლიკაციის გრაფიკი (დოკუმენტირებულია `pin-registry-ops.md`-ში) და გაგზავნეთ ახალი
   მანიფესტი აიძულებს სხვა პროვაიდერებს განაახლონ მეტსახელი.
3. როდესაც მეტსახელის სიახლე უფრო კრიტიკულია, ვიდრე რეპლიკაციის პარიტეტი, ხელახლა დააკავშირეთ
   უკვე დადგმული თბილი მანიფესტის მეტსახელი (`docs-preview`), შემდეგ გამოაქვეყნეთ
   შემდგომი მანიფესტი მას შემდეგ, რაც SRE ასუფთავებს ნარჩენებს.

### აღდგენა და დახურვა

1. მონიტორი `torii_sorafs_replication_sla_total{outcome="missed"}` უზრუნველსაყოფად
   გრაფის პლატოები.
2. აიღეთ `sorafs_cli manifest status` გამომავალი, როგორც მტკიცებულება იმისა, რომ ყოველი რეპლიკა არის
   უკან შესაბამისობაში.
3. შეიტანეთ ან განაახლეთ რეპლიკაციის ჩანაწერი სიკვდილის შემდგომი ნაბიჯებით
   (პროვაიდერის სკალირება, ჩუნკერის ტუნინგი და ა.შ.).

## Runbook — ანალიტიკა ან ტელემეტრიის გათიშვა

### გამომწვევი პირობები

- `npm run probe:portal` წარმატებას მიაღწევს, მაგრამ დაფები წყვეტს მიღებას
  `AnalyticsTracker` მოვლენები >15 წუთის განმავლობაში.
- კონფიდენციალურობის მიმოხილვა მიუთითებს გამოტოვებული მოვლენების მოულოდნელ ზრდას.
- `npm run probe:tryit-proxy` ვერ ხერხდება `/probe/analytics` ბილიკებზე.

### პასუხი

1. გადაამოწმეთ შეყვანის დრო: `DOCS_ANALYTICS_ENDPOINT` და
   `DOCS_ANALYTICS_SAMPLE_RATE` წარუმატებელი გამოშვების არტეფაქტში (`build/release.json`).
2. ხელახლა გაუშვით `npm run probe:portal` `DOCS_ANALYTICS_ENDPOINT`-ით მიმართული
   ინსცენირების კოლექციონერი იმის დასადასტურებლად, რომ ტრეკერი კვლავ ასხივებს დატვირთვას.
3. თუ კოლექტორები გათიშულია, დააყენეთ `DOCS_ANALYTICS_ENDPOINT=""` და აღადგინეთ ისე
   ტრეკერის მოკლე ჩართვა; ჩაწერეთ გათიშვის ფანჯარა ინციდენტის ვადებში.
4. დაადასტურეთ `scripts/check-links.mjs` ჯერ კიდევ თითის ანაბეჭდები `checksums.sha256`
   (ანალიტიკის გათიშვა *არ* უნდა დაბლოკოს საიტის რუქის ვალიდაცია).
5. როგორც კი კოლექციონერი გამოჯანმრთელდება, გაუშვით `npm run test:widgets` სავარჯიშოდ
   ანალიტიკის დამხმარე ერთეულის ტესტები ხელახლა გამოქვეყნებამდე.

### ინციდენტის შემდგომი

1. განაახლეთ [`devportal/observability`](./observability) ნებისმიერი ახალი კოლექციონერით
   შეზღუდვები ან შერჩევის მოთხოვნები.
2. ფაილის მართვის შეტყობინება, თუ რაიმე ანალიტიკური მონაცემი ჩამოიშალა ან დადასტურდა გარეთ
   პოლიტიკა.

## კვარტალური გამძლეობის წვრთნები

ჩაატარეთ ორივე სავარჯიშო ** ყოველი კვარტლის პირველ სამშაბათს ** (იანვარი/აპრილი/ივლისი/ოქტომბერი)
ან რაიმე მნიშვნელოვანი ინფრასტრუქტურის ცვლილების შემდეგ დაუყოვნებლივ. შეინახეთ არტეფაქტები ქვეშ
`artifacts/devportal/drills/<YYYYMMDD>/`.

| საბურღი | ნაბიჯები | მტკიცებულება |
| ----- | ----- | -------- |
| Alias ​​rollback რეპეტიცია | 1. ხელახლა გაიმეორეთ „ვერ განლაგების“ უკან დაბრუნება უახლესი წარმოების მანიფესტის გამოყენებით.<br/>2. ხელახლა დაკავშირება წარმოებასთან, როგორც კი ზონდები გაივლის.<br/>3. ჩაწერეთ `portal.manifest.submit.summary.json` და გამოიკვლიეთ ჟურნალები საბურღი საქაღალდეში. | `rollback.submit.json`, ზონდის გამომავალი და რეპეტიციის გამოშვების ტეგი. |
| სინთეტიკური ვალიდაციის აუდიტი | 1. გაუშვით `npm run probe:portal` და `npm run probe:tryit-proxy` წარმოებისა და დადგმის წინააღმდეგ.<br/>2. გაუშვით `npm run check:links` და დაარქივეთ `build/link-report.json`.<br/>3. მიამაგრეთ Grafana პანელების ეკრანის ანაბეჭდები/ექსპორტი, რომლებიც ადასტურებენ ზონდის წარმატებას. | ზონდის ჟურნალი + `link-report.json`, რომელიც მიუთითებს მანიფესტის თითის ანაბეჭდზე. |

გადაიტანეთ გამოტოვებული წვრთნები Docs/DevRel მენეჯერისა და SRE მმართველობის მიმოხილვაში,
ვინაიდან საგზაო რუკა მოითხოვს განმსაზღვრელ, კვარტალურ მტკიცებულებას, რომ ორივე მეტსახელი
უკან დაბრუნება და პორტალური ზონდები ჯანმრთელი რჩება.

## PagerDuty და გამოძახების კოორდინაცია

- PagerDuty სერვისი **Docs Portal Publishing** ფლობს სიგნალებს, რომლებიც გენერირებულია
  `dashboards/grafana/docs_portal.json`. წესები `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` და `DocsPortal/TLSExpiry` გვერდი Docs/DevRel
  პირველადი შენახვის SRE-ით მეორად.
- გვერდის მოთავსებისას ჩართეთ `DOCS_RELEASE_TAG`, დაურთოთ დაზარალებულის ეკრანის ანაბეჭდები
  Grafana პანელები და ბმული ზონდის/ბმულის შემოწმების გამომავალი ინციდენტის ჩანაწერებში ადრე
  შერბილება იწყება.
- შერბილების შემდეგ (დაბრუნება ან გადაყენება), ხელახლა გაუშვით `npm run probe:portal`,
  `npm run check:links` და გადაიღეთ ახალი Grafana კადრები, რომლებიც აჩვენებს მეტრიკას
  უკან ზღურბლების ფარგლებში. მიამაგრეთ ყველა მტკიცებულება PagerDuty ინციდენტამდე
  მისი მოგვარება.
- თუ ორი გაფრთხილება ერთდროულად გააქტიურდება (მაგალითად, TLS ვადის გასვლა პლუს ჩანაწერი), ტრიაჟი
  ჯერ უარს ამბობს (გამოქვეყნების შეწყვეტა), შეასრულეთ უკან დაბრუნება, შემდეგ გაასუფთავეთ
  TLS/ჩამორჩენილი ნივთები Storage SRE-ით ხიდზე.