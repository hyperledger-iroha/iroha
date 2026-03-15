---
lang: ka
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# მონაცემთა ხელმისაწვდომობის რეპლიკაციის პოლიტიკა (DA-4)

_ სტატუსი: მიმდინარეობს — მფლობელები: Core Protocol WG / Storage Team / SRE_

DA გადაყლაპვის მილსადენი ახლა ახორციელებს დეტერმინისტულ შეკავების მიზნებს
`roadmap.md`-ში (სამუშაო ნაკადი DA-4) აღწერილი ყველა blob კლასი. Torii უარს ამბობს
აბონენტის მიერ მოწოდებული კონვერტების შენარჩუნება, რომლებიც არ ემთხვევა კონფიგურირებულს
პოლიტიკა, რომელიც გარანტიას იძლევა, რომ ყველა ვალიდიატორი/შენახვის კვანძი ინარჩუნებს საჭიროებას
ეპოქების და რეპლიკების რაოდენობა წარმდგენის განზრახვაზე დაყრდნობის გარეშე.

## ნაგულისხმევი პოლიტიკა

| Blob კლასი | ცხელი შეკავება | სიცივის შეკავება | საჭირო ასლები | შენახვის კლასი | მმართველობის ტეგი |
|------------|--------------|----------------------------------|-----------------------------------------------------
| `taikai_segment` | 24 საათი | 14 დღე | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 საათი | 7 დღე | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 საათი | 180 დღე | 3 | `cold` | `da.governance` |
| _ ნაგულისხმევი (ყველა სხვა კლასი) _ | 6 საათი | 30 დღე | 3 | `warm` | `da.default` |

ეს მნიშვნელობები ჩაშენებულია `torii.da_ingest.replication_policy`-ში და გამოიყენება
ყველა `/v2/da/ingest` წარდგენილი. Torii ხელახლა წერს მანიფესტებს იძულებით
შეკავების პროფილი და ავრცელებს გაფრთხილებას, როდესაც აბონენტები უზრუნველყოფენ შეუსაბამო მნიშვნელობებს
ოპერატორებს შეუძლიათ აღმოაჩინონ მოძველებული SDK-ები.

### ტაიკაის ხელმისაწვდომობის კლასები

Taikai მარშრუტიზაციის მანიფესტები (`taikai.trm` მეტამონაცემები) ახლა მოიცავს
`availability_class` მინიშნება (`Hot`, `Warm`, ან `Cold`). როდესაც იმყოფება, Torii
ირჩევს შესატყვისი შეკავების პროფილს `torii.da_ingest.replication_policy`-დან
დატვირთვის დაქუცმაცებამდე, რაც საშუალებას აძლევს ღონისძიების ოპერატორებს შეამცირონ უმოქმედო
გადმოცემა გლობალური პოლიტიკის ცხრილის რედაქტირების გარეშე. ნაგულისხმევი არის:

| ხელმისაწვდომობის კლასი | ცხელი შეკავება | სიცივის შეკავება | საჭირო ასლები | შენახვის კლასი | მმართველობის ტეგი |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 საათი | 14 დღე | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 საათი | 30 დღე | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 საათი | 180 დღე | 3 | `cold` | `da.taikai.archive` |

თუ მანიფესტმა გამოტოვა `availability_class`, ჩასმის გზა ბრუნდება
`hot` პროფილი, ასე რომ, პირდაპირი სტრიმინგები ინარჩუნებენ სრულ რეპლიკა კომპლექტს. ოპერატორებს შეუძლიათ
ამ მნიშვნელობების უგულებელყოფა ახლის რედაქტირებით
`torii.da_ingest.replication_policy.taikai_availability` ბლოკი კონფიგურაციაში.

## კონფიგურაცია

პოლიტიკა მოქმედებს `torii.da_ingest.replication_policy`-ის ქვეშ და ავლენს ა
*ნაგულისხმევი* შაბლონი პლუს მასივი თითო კლასში. კლასის იდენტიფიკატორები არიან
რეზონანსისადმი მგრძნობიარე და მიიღეთ `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ან `custom:<u16>` მმართველობის მიერ დამტკიცებული გაფართოებებისთვის.
შენახვის კლასები მიიღება `hot`, `warm`, ან `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

დატოვეთ ბლოკი ხელშეუხებლად, რათა იმუშაოს ზემოთ ჩამოთვლილი ნაგულისხმევი პარამეტრებით. გამკაცრდეს ა
კლასი, განაახლეთ შესატყვისი უგულებელყოფა; ახალი კლასების საბაზისო შესაცვლელად,
რედაქტირება `default_retention`.Taikai ხელმისაწვდომობის კონკრეტული კლასების დასარეგულირებლად, დაამატეთ ჩანაწერები ქვემოთ
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## აღსრულების სემანტიკა

- Torii ცვლის მომხმარებლის მიერ მიწოდებულ `RetentionPolicy`-ს იძულებითი პროფილით
  დაქუცმაცებამდე ან მანიფესტაციამდე.
- წინასწარ ჩაშენებული მანიფესტები, რომლებიც აცხადებენ შეუსაბამო შენახვის პროფილს, უარყოფილია
  `400 schema mismatch`-ით, ამიტომ მოძველებულმა კლიენტებმა ვერ შეასუსტონ კონტრაქტი.
- ყოველი უგულებელყოფის მოვლენა აღრიცხულია (`blob_class`, გაგზავნილი მოსალოდნელი პოლიტიკის წინააღმდეგ)
  გამოქვეყნებისას შეუსაბამო აბონენტების გამოსახატავად.

იხილეთ `docs/source/da/ingest_plan.md` (შემოწმების სია) განახლებული კარიბჭისთვის
მოიცავს შეკავების აღსრულებას.

## ხელახალი გამეორების სამუშაო პროცესი (DA-4 შემდგომი დაკვირვება)

შეკავების აღსრულება მხოლოდ პირველი ნაბიჯია. ოპერატორებმაც უნდა დაამტკიცონ ეს
ცოცხალი მანიფესტები და რეპლიკაციის ბრძანებები შეესაბამება კონფიგურირებულ პოლიტიკას
რომ SoraFS-ს შეუძლია ავტომატურად გაიმეოროს შეუსაბამო ბლოგები.

1. **უყურეთ დრიფტს.** Torii გამოსცემს
   `overriding DA retention policy to match configured network baseline` ნებისმიერ დროს
   აბონენტი წარუდგენს ძველ შეკავების მნიშვნელობებს. დააწყვილეთ ეს ჟურნალი
   `torii_sorafs_replication_*` ტელემეტრია ასლის ნაკლოვანებების ან დაგვიანების გამოსავლენად
   გადანაწილებები.
2. **განსხვავებული განზრახვა ცოცხალი რეპლიკების წინააღმდეგ.** გამოიყენეთ ახალი აუდიტის დამხმარე:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   ბრძანება იტვირთება `torii.da_ingest.replication_policy` მოწოდებულიდან
   კონფიგურაცია, დეკოდირებს თითოეულ მანიფესტს (JSON ან Norito) და სურვილისამებრ ემთხვევა ნებისმიერს
   `ReplicationOrderV1` დატვირთვა მანიფესტის დაიჯესტით. შემაჯამებელი დროშები ორია
   პირობები:

   - `policy_mismatch` - მანიფესტის შეკავების პროფილი განსხვავდება იძულებით
     პოლიტიკა (ეს არასოდეს უნდა მოხდეს, თუ Torii არასწორად არის კონფიგურირებული).
   - `replica_shortfall` - ცოცხალი რეპლიკაციის შეკვეთა მოითხოვს ნაკლებ რეპლიკას, ვიდრე
     `RetentionPolicy.required_replicas` ან უზრუნველყოფს მისზე ნაკლებ დავალებას
     სამიზნე.

   არანულოვანი გასვლის სტატუსი მიუთითებს აქტიურ დეფიციტზე, ამიტომ CI/გამოძახების ავტომატიზაცია
   შეუძლია დაუყოვნებლივ გვერდი. მიამაგრეთ JSON ანგარიში
   `docs/examples/da_manifest_review_template.md` პაკეტი პარლამენტის ხმებისთვის.
3. **გაააქტიურეთ ხელახალი რეპლიკაცია.** როცა აუდიტი აფიქსირებს ხარვეზს, გაუშვით ახალი
   `ReplicationOrderV1` მართვის ინსტრუმენტების მეშვეობით აღწერილია
   `docs/source/sorafs/storage_capacity_marketplace.md` და ხელახლა ჩაატარეთ აუდიტი
   სანამ რეპლიკა ნაკრები არ გადაიყრება. გადაუდებელი უგულებელყოფისთვის, დააწყვილეთ CLI გამომავალი
   `iroha app da prove-availability`-ით, რათა SRE-ებმა შეძლონ იგივე დაიჯესტის მითითება
   და PDP მტკიცებულება.

რეგრესიის დაფარვა ცხოვრობს `integration_tests/tests/da/replication_policy.rs`-ში;
კომპლექტი წარუდგენს შეუსაბამო შენახვის პოლიტიკას `/v2/da/ingest`-ზე და ამოწმებს
რომ მოტანილი მანიფესტი ავლენს იძულებით პროფილს აბონენტის ნაცვლად
განზრახვა.

## ჯანმრთელობის დამადასტურებელი ტელემეტრია და დაფები (DA-5 ხიდი)

საგზაო რუკის პუნქტი **DA-5** მოითხოვს, რომ PDP/PoTR აღსრულების შედეგების აუდიტი იყოს
რეალურ დროში. `SorafsProofHealthAlert` ღონისძიებები ახლა ატარებს სპეციალურ კომპლექტს
Prometheus მეტრიკა:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Health** Grafana დაფა
(`dashboards/grafana/sorafs_pdp_potr_health.json`) ახლა ამჟღავნებს ამ სიგნალებს:- *Proof Health Alerts by Trigger* ჩარტებში ასახავს გაფრთხილების განაკვეთებს გამომწვევი/საჯარიმო დროშის მიხედვით.
  Taikai/CDN ოპერატორებს შეუძლიათ დაამტკიცონ, არის თუ არა მხოლოდ PDP, მხოლოდ PoTR ან ორმაგი დარტყმები
  სროლა.
- *პროვაიდერები Cooldown-ში* იუწყებიან პროვაიდერების ცოცხალ თანხას, რომელიც ამჟამად ა
  SorafsProofHealthAlert გაგრილება.
- *Proof Health Window Snapshot* აერთიანებს PDP/PoTR მრიცხველებს, ჯარიმის ოდენობას,
  გაგრილების დროშა და გაფიცვის ფანჯრის ბოლო ეპოქა თითო პროვაიდერზე, ასე რომ მმართველობის მიმომხილველები
  შეუძლია ცხრილის მიმაგრება ინციდენტის პაკეტებზე.

Runbooks უნდა დაუკავშირდეს ამ პანელებს DA აღსრულების მტკიცებულებების წარდგენისას; მათ
დაუკავშირეთ CLI proof-stream-ის წარუმატებლობა პირდაპირ ჯაჭვზე ჯაჭვის მეტამონაცემებს და
უზრუნველყოს საგზაო რუკაში გამოძახებული დაკვირვებადობის კაკალი.