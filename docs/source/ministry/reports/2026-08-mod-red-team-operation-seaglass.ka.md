---
lang: ka
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — ოპერაცია SeaGlass

- **საბურღი ID:** `20260818-operation-seaglass`
- **თარიღი და ფანჯარა:** `2026-08-18 09:00Z – 11:00Z`
- **სცენარის კლასი:** `smuggling`
- **ოპერატორები:** `Miyu Sato, Liam O'Connor`
- ** დაფები გაყინული ჩადენისგან:** `364f9573b`
- **მტკიცებულებების ნაკრები:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (არასავალდებულო):** `not pinned (local bundle only)`
- ** დაკავშირებული საგზაო რუქის ელემენტები:** `MINFO-9`, პლუს დაკავშირებული შემდგომი ღონისძიებები `MINFO-RT-17` / `MINFO-RT-18`.

## 1. მიზნები და შესვლის პირობები

- **პირველადი მიზნები**
  - დაადასტურეთ უარმყოფელი TTL-ის აღსრულება და კარიბჭის კარანტინი კონტრაბანდის მცდელობის დროს, ტვირთის შემცირების სიგნალიზაციის დროს.
  - დაადასტურეთ მმართველობის ხელახალი გამოვლენა და გაფრთხილება ბრაუნაუტის მართვაზე მოდერაციის წიგნში.
- ** წინაპირობები დადასტურებულია **
  - `emergency_canon_policy.md` ვერსია `v2026-08-seaglass`.
  - `dashboards/grafana/ministry_moderation_overview.json` დაიჯესტი `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - გამოძახების უფლებამოსილების უგულებელყოფა: `Kenji Ito (GovOps pager)`.

## 2. შესრულების ვადები

| დროის ანაბეჭდი (UTC) | მსახიობი | მოქმედება / ბრძანება | შედეგი / შენიშვნები |
|-------------------------|-----------------|---------------|
| 09:00:12 | Miyu Sato | გაყინული დაფები/გაფრთხილებები `364f9573b`-ზე `scripts/ministry/export_red_team_evidence.py --freeze-only`-ის მეშვეობით | საწყისი ხაზი აღებული და შენახული ქვეშ `dashboards/` |
| 09:07:44 | ლიამ ო'კონორი | გამოქვეყნებულია უარმყოფელი სნეპშოტი + GAR გადაფარვა დადგმაზე `sorafs_cli ... gateway update-denylist --policy-tier emergency`-ით | Snapshot მიღებულია; გადაფარვის ფანჯარა ჩაწერილია Alertmanager |
| 09:17:03 | Miyu Sato | ინექციური კონტრაბანდის ტვირთამწეობა + მართვის გამეორება `moderation_payload_tool.py --scenario seaglass` გამოყენებით | გაფრთხილება გაშვებულია 3მ12 წამის შემდეგ; მმართველობის გამეორება მონიშნულია |
| 09:31:47 | ლიამ ო'კონორი | გაუშვა მტკიცებულება ექსპორტი და დალუქული მანიფესტი `seaglass_evidence_manifest.json` | მტკიცებულებათა ნაკრები პლუს ჰეშები ინახება ქვეშ `manifests/` |

## 3. დაკვირვებები და მეტრიკა

| მეტრული | სამიზნე | დაკვირვებული | გავლა/ჩავარდნა | შენიშვნები |
|--------|--------|----------|----------|------|
| გაფრთხილების პასუხის შეყოვნება | = 0.98 | 0.992 | ✅ | აღმოჩენილია როგორც კონტრაბანდა, ასევე ხელახალი დატვირთვა |
| კარიბჭის ანომალიის გამოვლენა | გაფრთხილება გაშვებულია | გაფრთხილება გააქტიურებულია + ავტომატური კარანტინი | ✅ | კარანტინი გამოყენებული იქნა ხელახალი ცდის ბიუჯეტის ამოწურვამდე |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. დასკვნები და გამოსწორება

| სიმძიმე | მოძიება | მფლობელი | სამიზნე თარიღი | სტატუსი / ბმული |
|----------|---------|-------|------------|--------------|
| მაღალი | მმართველობის განმეორებითი გაფრთხილება გაშვებულია, მაგრამ SoraFS ბეჭედი გადაიდო 2 მეტრით, როდესაც ლოდინის სიაში ჩავარდნის გამოწვევა მოხდა | მმართველობის ოპერაციები (ლიამ ო'კონორი) | 2026-09-05 | `MINFO-RT-17` გახსნა — დაამატე განმეორებითი დალუქვის ავტომატიზაცია შეცდომის გზაზე |
| საშუალო | დაფის გაყინვა არ არის მიმაგრებული SoraFS-ზე; ოპერატორები ეყრდნობოდნენ ადგილობრივ პაკეტს | დაკვირვებადობა (Miyu Sato) | 2026-08-25 | `MINFO-RT-18` ღია — პინი `dashboards/*` to SoraFS ხელმოწერილი CID-ით შემდეგი სწავლის წინ |
| დაბალი | CLI logbook გამოტოვებულია Norito მანიფესტის ჰეში პირველი გავლისას | სამინისტროს ოპერაციები (კენჯი იტო) | 2026-08-22 | ფიქსირდება საბურღი დროს; თარგი განახლებულია ჩანაწერთა წიგნში |დაასაბუთეთ, თუ როგორ ვლინდება კალიბრაცია, უნდა შეიცვალოს უარყოფის წესები ან SDK/ინსტრუმენტები. დაუკავშირდით GitHub/Jira-ს საკითხებს და შენიშნეთ დაბლოკილი/განბლოკილი მდგომარეობები.

## 5. მმართველობა და დამტკიცებები

- ** ინციდენტის მეთაურის გაფორმება:** `Miyu Sato @ 2026-08-18T11:22Z`
- **მმართველობის საბჭოს განხილვის თარიღი:** `GovOps-2026-08-22`
- **შემდეგი საკონტროლო სია:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. დანართები

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

მონიშნეთ თითოეული დანართი `[x]` მტკიცებულების პაკეტში ატვირთული ერთხელ და SoraFS სნეპშოტით.

---

_ბოლო განახლება: 2026-08-18_