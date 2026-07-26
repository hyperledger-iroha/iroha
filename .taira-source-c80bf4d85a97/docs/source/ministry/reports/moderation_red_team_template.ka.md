---
lang: ka
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **როგორ გამოვიყენოთ:** დააკოპირეთ ეს შაბლონი `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`-ზე ყოველი სწავლის შემდეგ დაუყოვნებლივ. შეინახეთ ფაილების სახელები მცირე ასოებით, დეფისით და გასწორებული საბურღი ID-სთან, რომელიც შესულია Alertmanager-ში.

# Red-Team Drill Report — `<SCENARIO NAME>`

- **საბურღი ID:** `<YYYYMMDD>-<scenario>`
- **თარიღი და ფანჯარა:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **სცენარის კლასი:** `smuggling | bribery | gateway | ...`
- **ოპერატორები:** `<names / handles>`
- ** დაფები გაყინულია ჩადენისგან:** `<git SHA>`
- **მტკიცებულებების ნაკრები:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (არასავალდებულო):** `<cid>`  
- ** დაკავშირებული საგზაო რუქის ელემენტები:** `MINFO-9`, პლუს ნებისმიერი დაკავშირებული ბილეთი.

## 1. მიზნები და შესვლის პირობები

- **პირველადი მიზნები**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- ** წინაპირობები დადასტურებულია **
  - `emergency_canon_policy.md` ვერსია `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` დაიჯესტი `<sha256>`
  - გამოძახების უფლებამოსილების უგულებელყოფა: `<name>`

## 2. შესრულების ვადები

| დროის ანაბეჭდი (UTC) | მსახიობი | მოქმედება / ბრძანება | შედეგი / შენიშვნები |
|-------------------------|-----------------|---------------|
|  |  |  |  |

> ჩართეთ Torii მოთხოვნის პირადობის მოწმობები, ნაჭრების ჰეშები, დამტკიცებების უგულებელყოფა და Alertmanager ბმულები.

## 3. დაკვირვებები და მეტრიკა

| მეტრული | სამიზნე | დაკვირვებული | გავლა/ჩავარდნა | შენიშვნები |
|--------|--------|----------|----------|------|
| გაფრთხილების პასუხის შეყოვნება | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| ზომიერების გამოვლენის მაჩვენებელი | `>= <value>` |  |  |  |
| კარიბჭის ანომალიის გამოვლენა | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. დასკვნები და გამოსწორება

| სიმძიმე | მოძიება | მფლობელი | სამიზნე თარიღი | სტატუსი / ბმული |
|----------|---------|-------|------------|--------------|
| მაღალი |  |  |  |  |

დაასაბუთეთ, თუ როგორ ვლინდება კალიბრაცია, უნდა შეიცვალოს უარყოფის წესები ან SDK/ინსტრუმენტები. დაუკავშირდით GitHub/Jira-ს საკითხებს და შენიშნეთ დაბლოკილი/განბლოკილი მდგომარეობები.

## 5. მმართველობა და დამტკიცებები

- ** ინციდენტის მეთაურის გაფორმება:** `<name / timestamp>`
- **მმართველობის საბჭოს განხილვის თარიღი:** `<meeting id>`
- **შემდეგი საკონტროლო სია:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. დანართები

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

მონიშნეთ თითოეული დანართი `[x]` ერთხელ ატვირთული მტკიცებულებების პაკეტში და SoraFS სნეპშოტით.

---

_ბოლო განახლება: {{ თარიღი | ნაგულისხმევი ("2026-02-20") }}_