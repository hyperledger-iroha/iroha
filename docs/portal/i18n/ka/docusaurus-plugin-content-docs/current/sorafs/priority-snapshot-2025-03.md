---
id: priority-snapshot-2025-03
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> კანონიკური წყარო: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> სტატუსი: **ბეტა / ელოდება მართვის ACK-ებს** (ქსელი, შენახვა, Docs ლიდერები).

## მიმოხილვა

მარტის სნეპშოტი ინახავს დოკუმენტების/კონტენტის ქსელის ინიციატივებს შესაბამისობაში
SoraFS მიწოდების ტრასები (SF‑3, SF‑6b, SF‑9). მას შემდეგ რაც ყველა წამყვანი აღიარებს ამას
სნეპშოტი Nexus საჭის არხში, ამოიღეთ „ბეტა“ შენიშვნა ზემოთ.

### ძაფების ფოკუსირება

1. **გაავრცელეთ პრიორიტეტული სნეპშოტი** — შეაგროვეთ მადლიერებები და დაარეგისტრირეთ ისინი
   2025-03-05 საბჭოს ოქმი.
2. **Gateway/DNS kickoff close-out** — გაიმეორეთ ახალი ფასილიტაციის ნაკრები (ნაწილი6
   runbook) 2025-03-03 სახელოსნომდე.
3. **ოპერატორის runbook მიგრაცია** — პორტალი `Runbook Index` არის ცოცხალი; გამოავლინეთ ბეტა
   წინასწარ გადახედეთ URL-ს მიმომხილველის ბორტზე შესვლის შემდეგ.
4. **SoraFS მიწოდების ძაფები** — გაასწორეთ დარჩენილი სამუშაოები SF‑3/6b/9 გეგმასთან/საგზაო რუქასთან:
   - `sorafs-node` PoR გადაყლაპვის მუშაკი + სტატუსის საბოლოო წერტილი.
   - CLI/SDK სავალდებულო გაპრიალება Rust/JS/Swift ორკესტრის ინტეგრაციაში.
   - PoR კოორდინატორი Runtime გაყვანილობა და GovernanceLog ღონისძიებები.

იხილეთ წყარო ფაილი სრული ცხრილისთვის, განაწილების სიის და ჟურნალის ჩანაწერებისთვის.