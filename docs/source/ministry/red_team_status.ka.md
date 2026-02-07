---
lang: ka
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# სამინისტროს წითელი გუნდის სტატუსი

ეს გვერდი ავსებს [Moderation Red-Team Plan] (moderation_red_team_plan.md)
ახლოვადიანი სავარჯიშო კალენდრის, მტკიცებულებების პაკეტებისა და გამოსწორების თვალყურის დევნებით
სტატუსი. განაახლეთ იგი ყოველი გაშვების შემდეგ ქვეშ დაფიქსირებულ არტეფაქტებთან ერთად
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## მომავალი წვრთნები

| თარიღი (UTC) | სცენარი | მფლობელ(ებ)ი | მტკიცებულების მომზადება | შენიშვნები |
|------------|---------|---------|--------------|-------|
| 2026-11-12 | **ოპერაცია Blindfold** — Taikai შერეული რეჟიმის კონტრაბანდის რეპეტიცია კარიბჭის დაქვეითების მცდელობებით | უსაფრთხოების ინჟინერია (Miyu Sato), Ministry Ops (Liam O'Connor) | `scripts/ministry/scaffold_red_team_drill.py` პაკეტი `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + დადგმის დირექტორია `artifacts/ministry/red-team/2026-11/operation-blindfold/` | სავარჯიშოები GAR/Taikai გადახურვა პლუს DNS failover; მოითხოვს უარმყოფელი Merkle-ის სნეპშოტს დაწყებამდე და `export_red_team_evidence.py` გაშვებას დაფების აღების შემდეგ. |

## ბოლო სავარჯიშო სურათი

| თარიღი (UTC) | სცენარი | მტკიცებულებათა ნაკრები | გამოსწორება და შემდგომი დაკვირვებები |
|------------|---------|---------------|-------------------------|
| 2026-08-18 | **ოპერაცია SeaGlass** — კარიბჭეების კონტრაბანდა, მმართველობის განმეორება და გაფრთხილების რეპეტიცია | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana ექსპორტი, Alertmanager ჟურნალები, `seaglass_evidence_manifest.json`) | **ღია:** განმეორებითი ბეჭდის ავტომატიზაცია (`MINFO-RT-17`, მფლობელი: Governance Ops, ვადა 2026-09-05); დაფის დაფის გაყინვა SoraFS-ზე (`MINFO-RT-18`, დაკვირვებადობა, ვადა 2026-08-25). **დახურულია:** ჟურნალის შაბლონი განახლებულია Norito მანიფესტის ჰეშებისთვის. |

## თვალთვალი და ინსტრუმენტები

- გამოიყენეთ `scripts/ministry/moderation_payload_tool.py` საინექციო შესაფუთად
  payloads და Denylist პატჩები თითო სცენარი.
- ჩაწერეთ დაფა/ლოგის გადაღებები `scripts/ministry/export_red_team_evidence.py`-ის საშუალებით
  ყოველი ვარჯიშის შემდეგ დაუყოვნებლივ, ასე რომ მტკიცებულების მანიფესტი შეიცავს ხელმოწერილ ჰეშებს.
- CI მცველი `ci/check_ministry_red_team.sh` ახორციელებს სავარჯიშო ანგარიშებს
  არ შეიცავს ჩანაცვლების ტექსტს და მითითებულ არტეფაქტებს ადრე არსებობდა
  შერწყმა.

იხილეთ `status.md` (§ * სამინისტროს წითელი გუნდის სტატუსი*) პირდაპირი მინიშნებისთვის
ყოველკვირეულ საკოორდინაციო ზარებში.