---
lang: ka
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI ხელმოწერა
---------------

- Torii OpenAPI სპეციფიკაცია (`torii.json`) უნდა იყოს ხელმოწერილი და მანიფესტი დამოწმებულია `cargo xtask openapi-verify`-ით.
- ხელმომწერის დაშვებული გასაღებები ცხოვრობს `allowed_signers.json`-ში; გადაატრიალეთ ეს ფაილი, როდესაც ხელმოწერის გასაღები იცვლება. შეინახეთ `version` ველი `1`-ზე.
- CI (`ci/check_openapi_spec.sh`) უკვე ახორციელებს დაშვებულ სიას როგორც უახლესი, ასევე მიმდინარე სპეციფიკაციებისთვის. თუ სხვა პორტალი ან მილსადენი მოიხმარს ხელმოწერილ სპეციფიკას, მიუთითეთ მისი გადამოწმების ნაბიჯი იმავე დაშვების სიის ფაილზე, რათა თავიდან აიცილოთ დრიფტი.
- ხელახლა ხელმოწერა გასაღების როტაციის შემდეგ:
  1. განაახლეთ `allowed_signers.json` ახალი საჯარო გასაღებით.
  2. რეგენერაცია/ხელმოწერა სპეციფიკაცია: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. ხელახლა გაუშვით `ci/check_openapi_spec.sh` (ან `cargo xtask openapi-verify` ხელით), რათა დაადასტუროთ, რომ მანიფესტი შეესაბამება დაშვებულ სიას.