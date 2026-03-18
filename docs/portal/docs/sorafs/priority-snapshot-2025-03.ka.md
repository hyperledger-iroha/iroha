---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
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