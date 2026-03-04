---
lang: ka
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage პაკეტის შაბლონი (SN13-C)

საგზაო რუკის პუნქტი **SN13-C — მანიფესტები და SoraNS წამყვანები** მოითხოვს ყველა მეტსახელს
როტაცია დეტერმინისტული მტკიცებულების ნაკრების გასაგზავნად. დააკოპირეთ ეს შაბლონი თქვენს
არტეფაქტის დირექტორია (მაგალითად
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) და შეცვალეთ
ჩანაცვლების მფლობელები პაკეტის მმართველობისთვის გაგზავნამდე.

## 1. მეტამონაცემები

| ველი | ღირებულება |
|-------|-------|
| ღონისძიების ID | `<taikai.event.launch-2026-07-10>` |
| ნაკადი / გადაცემა | `<main-stage>` |
| მეტსახელის სახელთა სივრცე / სახელი | `<sora / docs>` |
| მტკიცებულებათა დირექტორია | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| ოპერატორის კონტაქტი | `<name + email>` |
| GAR / RPT ბილეთი | `<governance ticket or GAR digest>` |

## პაკეტის დამხმარე (სურვილისამებრ)

დააკოპირეთ კოჭის არტეფაქტები და გამოუშვით JSON (სურვილისამებრ ხელმოწერილი) შეჯამება მანამდე
დარჩენილი სექციების შევსება:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

დამხმარე იზიდავს `taikai-anchor-request-*`, `taikai-trm-state-*`,
`taikai-lineage-*`, კონვერტები და მცველები ტაიკაის კოჭის დირექტორიადან
(`config.da_ingest.manifest_store_dir/taikai`) ასე რომ მტკიცებულების საქაღალდე უკვე
შეიცავს ქვემოთ მითითებულ ზუსტ ფაილებს.

## 2. Lineage ledger & მინიშნება

მიამაგრეთ როგორც დისკზე საგვარეულო წიგნი, ასევე მინიშნება JSON Torii, რომელიც დაწერა ამისათვის
ფანჯარა. ეს პირდაპირ მოდის
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` და
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| საგვარეულო წიგნი | `taikai-trm-state-docs.json` | `<sha256>` | ადასტურებს წინა მანიფესტის დაიჯესტს/ფანჯარას. |
| ხაზის მინიშნება | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | გადაღებულია SoraNS წამყვანში ატვირთვამდე. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. ანკერის დატვირთვის დაჭერა

ჩაწერეთ POST დატვირთვა, რომელიც Torii მიაწოდა წამყვანის სერვისს. ტვირთამწეობა
მოიცავს `envelope_base64`, `ssm_base64`, `trm_base64` და inline
`lineage_hint` ობიექტი; აუდიტი ეყრდნობა ამ დაჭერას იმის დასამტკიცებლად, რომ ეს იყო
გაგზავნილი SoraNS-ში. Torii ახლა წერს ამ JSON ავტომატურად როგორც
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool დირექტორიაში (`config.da_ingest.manifest_store_dir/taikai/`), ასე რომ
ოპერატორებს შეუძლიათ პირდაპირ დააკოპირონ ის HTTP ჟურნალების დაწერის ნაცვლად.

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| წამყვანი POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | დაუმუშავებელი მოთხოვნა დაკოპირებულია `taikai-anchor-request-*.json`-დან (Taikai spool). |

## 4. მანიფესტი დაიჯესტის აღიარება

| ველი | ღირებულება |
|-------|-------|
| ახალი მანიფესტის დაიჯესტი | `<hex digest>` |
| წინა მანიფესტი დაიჯესტი (მინიშნებიდან) | `<hex digest>` |
| ფანჯრის დაწყება/დასრულება | `<start seq> / <end seq>` |
| მიღების დროის შტამპი | `<ISO8601>` |

მიუთითეთ ledger/hint ჰეშები, რომლებიც ჩაწერილია ზემოთ, რათა მიმომხილველებმა შეძლონ ამის გადამოწმება
ფანჯარა, რომელიც ჩანაცვლდა.

## 5. მეტრიკა / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` სნეპშოტი: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` ნაგავსაყრელი (სხვა სახელით): `<file path + hash>`

მიუთითეთ Prometheus/Grafana ექსპორტი ან `curl` გამომავალი, რომელიც აჩვენებს მრიცხველს
increment და `/status` მასივი ამ მეტსახელისთვის.

## 6. მანიფესტი მტკიცებულებების დირექტორია

შექმენით მტკიცებულებათა დირექტორიას დეტერმინისტული მანიფესტი (სპულის ფაილები,
ტვირთის აღება, მეტრიკის სნეპშოტები), რათა მმართველობამ შეძლოს ყოველი ჰეშის გარეშე გადამოწმება
არქივის გახსნა.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| არტეფაქტი | ფაილი | SHA-256 | შენიშვნები |
|----------|------|---------|-------|
| მტკიცებულება მანიფესტი | `manifest.json` | `<sha256>` | მიამაგრეთ ეს მართვის პაკეტს / GAR. |

## 7. ჩამონათვალი

- [ ] Lineage ledger კოპირებულია + ჰეშირებული.
- [ ] Lineage მინიშნება კოპირებულია + ჰეშირებული.
- [ ] Anchor POST payload აღბეჭდილია და ჰეშირებულია.
- [ ] მანიფესტის დაიჯესტის ცხრილი შევსებულია.
- [ ] Metrics Snapshots ექსპორტირებული (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] მანიფესტი გენერირებულია `scripts/repo_evidence_manifest.py`-ით.
- [ ] პაკეტი ატვირთულია მმართველობაში ჰეშებით + საკონტაქტო ინფორმაცია.

ამ შაბლონის შენარჩუნება ყოველგვარი მეტსახელის როტაციისთვის ინარჩუნებს SoraNS მმართველობას
შეკვრა რეპროდუცირებადი და აკავშირებს შთამომავლობას პირდაპირ GAR/RPT მტკიცებულებაზე.