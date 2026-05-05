---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f68e8cc639bd6a780c33fd14ab4e25df1c6e9381595c7a4c44ff577fea02400d
source_last_modified: "2026-01-22T16:26:46.494851+00:00"
translation_last_reviewed: 2026-02-07
id: publishing-monitoring
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
---

საგზაო რუკის პუნქტი **DOCS-3c** მოითხოვს უფრო მეტს, ვიდრე შეფუთვის საკონტროლო სია: ყოველი
SoraFS გამოვაქვეყნოთ ჩვენ მუდმივად უნდა დავამტკიცოთ, რომ დეველოპერის პორტალი, სცადეთ
პროქსი, და კარიბჭის საკინძები ჯანმრთელი რჩება. ეს გვერდი ადასტურებს მონიტორინგს
ზედაპირი, რომელიც ახლავს [განლაგების სახელმძღვანელოს] (./deploy-guide.md) ისე CI და შემდეგ
ზარის ინჟინრებს შეუძლიათ განახორციელონ იგივე შემოწმებები, რომლებსაც Ops იყენებს SLO-ს აღსასრულებლად.

## მილსადენის მიმოხილვა

1. **აშენეთ და მოაწერეთ ხელი** – მიჰყევით [განლაგების სახელმძღვანელოს] (./deploy-guide.md) გასაშვებად
   `npm run build`, `scripts/preview_wave_preflight.sh` და Sigstore +
   მანიფესტი წარდგენის ნაბიჯები. წინასწარი ფრენის სკრიპტი ასხივებს `preflight-summary.json`
   ასე რომ, ყოველი გადახედვა შეიცავს build/link/probe მეტამონაცემებს.
2. **დაამაგრეთ და დაადასტურეთ** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   და DNS cutover გეგმა უზრუნველყოფს დეტერმინისტულ არტეფაქტებს მმართველობისთვის.
3. **არქივის მტკიცებულება** – შეინახეთ CAR რეზიუმე, Sigstore პაკეტი, მეტსახელის მტკიცებულება,
   ზონდის გამომავალი და `docs_portal.json` დაფის სნეპშოტები ქვეშ
   `artifacts/sorafs/<tag>/`.

## არხების მონიტორინგი

### 1. გამომცემელი მონიტორები (`scripts/monitor-publishing.mjs`)

ახალი `npm run monitor:publishing` ბრძანება ახვევს პორტალის ზონდს, სცადეთ
proxy probe და სავალდებულო ვერიფიკატორი ერთ CI მეგობრულ შემოწმებაში. მიაწოდეთ ა
JSON კონფიგურაცია (შემოწმდა CI საიდუმლოებში ან `configs/docs_monitor.json`) და გაუშვით:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

დაამატეთ `--prom-out ../../artifacts/docs_monitor/monitor.prom` (და სურვილისამებრ
`--prom-job docs-preview`) გამოსცეს Prometheus ტექსტური ფორმატის მეტრიკა, რომელიც შესაფერისია
Pushgateway-ის ატვირთვები ან პირდაპირი Prometheus სკრეპები დადგმაში/წარმოებაში. The
მეტრიკა ასახავს JSON-ის შეჯამებას, რათა SLO-ის საინფორმაციო დაფებმა და გაფრთხილების წესებმა თვალი ადევნოს
პორტალი, სცადეთ, სავალდებულო და DNS ჯანმრთელობა მტკიცებულების ნაკრების გაანალიზების გარეშე.

კონფიგურაციის მაგალითი საჭირო ღილაკებით და მრავალჯერადი შეკვრით:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/<i105-account-id>/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

მონიტორი წერს JSON-ის შეჯამებას (S3/SoraFS მეგობრული) და გამოდის ნულის გარეშე, როდესაც
ნებისმიერი ზონდი ვერ ხერხდება, რაც მას შესაფერისს ხდის Cron სამუშაოებისთვის, Buildkite ნაბიჯებისთვის ან
Alertmanager webhooks. `--evidence-dir` გავლა გრძელდება `summary.json`,
`portal.json`, `tryit.json` და `binding.json` `checksums.sha256`-თან ერთად
მანიფესტი ისე, რომ მმართველობის მიმომხილველებს შეუძლიათ განასხვავონ მონიტორის შედეგები ამის გარეშე
ხელახლა გაუშვით ზონდები.

> **TLS დამცავი მოაჯირი:** `monitorPortal` უარყოფს `http://` საბაზისო URL-ებს, თუ არ დააყენეთ
> `allowInsecureHttp: true` კონფიგურაციაში. განაგრძეთ წარმოების/დადგმის ზონდები
> HTTPS; არჩევა არსებობს მხოლოდ ადგილობრივი გადახედვისთვის.

თითოეული სავალდებულო ჩანაწერი გადის `cargo xtask soradns-verify-binding` დაჭერილის წინააღმდეგ
`portal.gateway.binding.json` პაკეტი (და სურვილისამებრ `manifestJson`) ასე რომ, მეტსახელი,
მტკიცებულების სტატუსი და შინაარსი CID შეესაბამება გამოქვეყნებულ მტკიცებულებებს. The
არასავალდებულო `hostname` მცველი ადასტურებს, რომ მეტსახელიდან მიღებული კანონიკური მასპინძელი ემთხვევა
კარიბჭის მასპინძელი, რომლის პოპულარიზაციასაც აპირებთ, თავიდან აიცილებთ DNS-ის ამოკვეთებს, რომლებიც გადაადგილდებიან
ჩაწერილი სავალდებულო.

არჩევითი `dns` ბლოკის მავთულები DOCS-7-ის SoraDNS ავრცელებს იმავე მონიტორს.
თითოეული ჩანაწერი წყვეტს ჰოსტის სახელს/ჩანაწერის ტიპის წყვილს (მაგალითად
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) და
ადასტურებს პასუხების შესაბამისობას `expectedRecords` ან `expectedIncludes`. მეორე
ჩანაწერი სნიპეტში ზემოთ მყარ კოდებს აწარმოებს კანონიკური ჰეშირებული ჰოსტის სახელს
`cargo xtask soradns-hosts --name docs-preview.sora.link`; მონიტორი ახლა ამტკიცებს
როგორც ადამიანისთვის შესაფერისი მეტსახელი, ასევე კანონიკური ჰეში (`igjssx53…gw.sora.id`)
გადაწყვეტს მიმაგრებულ ლამაზ მასპინძელს. ეს ხდის DNS-ის ხელშეწყობის მტკიცებულებებს ავტომატურს:
მონიტორი მარცხდება, თუ რომელიმე ჰოსტი გადაინაცვლებს, მაშინაც კი, როცა HTTP აკავშირებს
დაამაგრეთ სწორი მანიფესტი.

### 2. OpenAPI ვერსიის მანიფესტი მცველი

DOCS-2b-ის „ხელმოწერილი OpenAPI მანიფესტის“ მოთხოვნა ახლა აგზავნის ავტომატურ მცველს:
`ci/check_openapi_spec.sh` ურეკავს `npm run check:openapi-versions`-ს, რომელიც იწვევს
`scripts/verify-openapi-versions.mjs` გადასამოწმებლად
`docs/portal/static/openapi/versions.json` რეალური Torii სპეციფიკაციებით და
ვლინდება. მცველი ამოწმებს, რომ:

- `versions.json`-ში ჩამოთვლილ ყველა ვერსიას აქვს შესაბამისი დირექტორია ქვემოთ
  `static/openapi/versions/`.
- თითოეული ჩანაწერის `bytes` და `sha256` ველები ემთხვევა დისკის სპეციფიკურ ფაილს.
- `latest` მეტსახელი ასახავს `current` ჩანაწერს (დაჯესტი/ზომა/ხელმოწერის მეტამონაცემები)
  ასე რომ, ნაგულისხმევი ჩამოტვირთვა შეუძლებელია.
- ხელმოწერილი ჩანაწერები მიუთითებს მანიფესტზე, რომლის `artifact.path` მიუთითებს უკან
  იგივე სპეციფიკაცია და რომლის ხელმოწერის/საჯარო გასაღების თექვსმეტობითი მნიშვნელობები ემთხვევა მანიფესტს.

გაუშვით მცველი ადგილობრივად, როდესაც ასახავთ ახალ სპეციფიკას:

```bash
cd docs/portal
npm run check:openapi-versions
```

წარუმატებლობის შეტყობინებები შეიცავს მინიშნებას ძველი ფაილის შესახებ (`npm run sync-openapi -- --latest`)
ასე რომ, პორტალის ავტორებმა იციან როგორ განაახლონ სნეპშოტები. მცველის შენახვა
CI ხელს უშლის პორტალის გამოშვებას, სადაც ხელმოწერილი მანიფესტი და გამოქვეყნებული დაიჯესტი
სინქრონიდან ამოვარდნა.

### 2. დაფები და გაფრთხილებები

- **`dashboards/grafana/docs_portal.json`** – ძირითადი დაფა DOCS-3c-სთვის. პანელები
  სიმღერა `torii_sorafs_gateway_refusals_total`, რეპლიკაცია SLA გამოტოვებს, სცადეთ
  პროქსის შეცდომები და გამოძიების შეყოვნება (`docs.preview.integrity` გადაფარვა). ექსპორტი
  ბორტზე ყოველი გამოშვების შემდეგ და მიამაგრეთ იგი საოპერაციო ბილეთზე.
- **სცადეთ პროქსის გაფრთხილებები** – Alertmanager წესი `TryItProxyErrors` ჩართულია
  მდგრადი `probe_success{job="tryit-proxy"}` წვეთები ან
  `tryit_proxy_requests_total{status="error"}` მწვერვალები.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` უზრუნველყოფს ალიასის დაკავშირების გაგრძელებას
  დამაგრებული მანიფესტს დაიჯესტის რეკლამირება; ესკალაციები ბმული
  `cargo xtask soradns-verify-binding` CLI ტრანსკრიპტი გადაღებული გამოქვეყნებისას.

### 3. მტკიცებულების ბილიკი

ყოველი მონიტორინგის გაშვება უნდა დაერთოს:

- `monitor-publishing` მტკიცებულების ნაკრები (`summary.json`, თითო განყოფილების ფაილები და
  `checksums.sha256`).
- Grafana ეკრანის ანაბეჭდები `docs_portal` დაფისთვის გამოშვების ფანჯარაში.
- სცადეთ პროქსის შეცვლა/დაბრუნების ტრანსკრიპტები (`npm run manage:tryit-proxy` ჟურნალები).
- მეტსახელის გადამოწმების გამომავალი `cargo xtask soradns-verify-binding`-დან.

შეინახეთ ისინი `artifacts/sorafs/<tag>/monitoring/`-ში და დააკავშირეთ ისინი
გამოშვების საკითხი, რათა აუდიტის ბილიკი გადარჩეს CI ჟურნალების ვადის გასვლის შემდეგ.

## ოპერატიული ჩამონათვალი

1. გაუშვით განლაგების სახელმძღვანელო ნაბიჯი 7-მდე.
2. შეასრულეთ `npm run monitor:publishing` საწარმოო კონფიგურაციით; არქივი
   JSON გამომავალი.
3. გადაიღეთ Grafana პანელები (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) და მიამაგრეთ გაშვების ბილეთს.
4. დაგეგმეთ განმეორებადი მონიტორები (რეკომენდირებულია: ყოველ 15 წუთში)
   წარმოების URL-ები იგივე კონფიგურაციით DOCS-3c SLO კარიბჭის დასაკმაყოფილებლად.
5. ინციდენტების დროს ხელახლა გაუშვით მონიტორის ბრძანება `--json-out`-ით ჩასაწერად
   მტკიცებულებამდე/შემდეგ და დაურთოს სიკვდილის შემდგომ.

ამ მარყუჟის შემდეგ იხურება DOCS-3c: პორტალის აშენების ნაკადი, გამოქვეყნების მილსადენი,
და მონიტორინგის დასტა ახლა ცხოვრობს ერთ სათამაშო წიგნში რეპროდუცირებადი ბრძანებებით,
ნიმუშების კონფიგურაციები და ტელემეტრიის კაკვები.