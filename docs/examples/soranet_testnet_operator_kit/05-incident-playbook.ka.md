---
lang: ka
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Downgrade Response Playbook

1. **გამოცნობა**
   - გაფრთხილება `soranet_privacy_circuit_events_total{kind="downgrade"}` ხანძარი ან
     brownout webhook იწვევს მმართველობიდან.
   - დაადასტურეთ `kubectl logs soranet-relay` ან სისტემური ჟურნალის საშუალებით 5 წუთის განმავლობაში.

2. **სტაბილიზაცია**
   - გაყინვის დამცავი როტაცია (`relay guard-rotation disable --ttl 30m`).
   - ჩართეთ მხოლოდ პირდაპირი უგულებელყოფა დაზარალებული კლიენტებისთვის
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - გადაიღეთ მიმდინარე შესაბამისობის კონფიგურაციის ჰეში (`sha256sum compliance.toml`).

3. **დიაგნოსტიკა**
   - შეაგროვეთ დირექტორია უახლესი სნეპშოტი და სარელეო მეტრიკის ნაკრები:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - გაითვალისწინეთ PoW რიგის სიღრმე, დროსელის მრიცხველები და GAR კატეგორიის მწვერვალები.
   - დაადგინეთ, გამოიწვია თუ არა PQ დეფიციტი, შესაბამისობის უკმარისობა ან რელეს უკმარისობა.

4. **ესკალაცია **
   - შეატყობინეთ მართვის ხიდს (`#soranet-incident`) შეჯამებით და შეფუთვის ჰეშით.
   - გახსენით ინციდენტის ბილეთი, რომელიც დაკავშირებულია გაფრთხილებასთან, მათ შორის დროის ანაბეჭდები და შემარბილებელი ნაბიჯები.

5. **აღდგენა **
   - ძირეული მიზეზის მოგვარების შემდეგ, ხელახლა ჩართეთ როტაცია
     (`relay guard-rotation enable`) და დააბრუნეთ მხოლოდ პირდაპირი გადაფარვები.
   - KPI-ების მონიტორინგი 30 წუთის განმავლობაში; დარწმუნდით, რომ არ გამოჩნდეს ახალი ნაოჭები.

6. **პოსტმორტმი**
   - წარადგინეთ ინციდენტის ანგარიში 48 საათის განმავლობაში მმართველობის შაბლონის გამოყენებით.
   - განაახლეთ runbooks, თუ აღმოჩენილია წარუმატებლობის ახალი რეჟიმი.