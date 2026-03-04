---
lang: az
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS Manifest CLI Başdan-Uca Nümunə

Bu misal istifadə edərək SoraFS-ə sənədləşmənin dərc edilməsini göstərir.
`sorafs_manifest_stub` CLI deterministik parçalanma qurğuları ilə birlikdə
SoraFS Arxitektura RFC-də təsvir edilmişdir. Axın açıq-aşkar nəsli əhatə edir,
gözləntilərin yoxlanılması, planın təsdiqlənməsi və axtarışın sübutu üçün sınaq
komandalar eyni addımları CI-də yerləşdirə bilər.

## İlkin şərtlər

- İş sahəsi klonlanmış və alətlər silsiləsi hazırdır (`cargo`, `rustc`).
- `fixtures/sorafs_chunker`-dən armaturlar mövcuddur, beləliklə gözlənilən dəyərlər ola bilər
  əldə edilmişdir (istehsal əməliyyatları üçün miqrasiya kitabçası girişindən dəyərləri çəkin
  artefaktla əlaqələndirilir).
- Dərc etmək üçün nümunə yükü kataloqu (bu nümunə `docs/book` istifadə edir).

## Addım 1 — Manifest, CAR, imzalar yaradın və plan gətirin

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

Əmr:

- `ChunkProfile::DEFAULT` vasitəsilə faydalı yükü ötürür.
- CARv2 arxivi üstəgəl yığın gətirmə planı verir.
- `ManifestV1` qeydini qurur, açıq imzaları yoxlayır (əgər təmin edilirsə) və
  zərfi yazır.
- Gözləmə bayraqlarını tətbiq edir ki, baytların sürüşməsi halında qaçış uğursuz olsun.

## Addım 2 — Çıxışları yığın mağazası + PoR məşqi ilə yoxlayın

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Bu, deterministik yığın mağazası vasitəsilə CAR-ı təkrarlayır, əldə edir
Proof-of-Retrievability nümunə ağacı və uyğun açıq hesabat yayır
idarəetmənin nəzərdən keçirilməsi.

## Addım 3 — Çox provayder axtarışını simulyasiya edin

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI mühitləri üçün hər provayder üçün ayrı yük yolları təmin edin (məsələn, quraşdırılmış
qurğular) diapazonun planlaşdırılması və nasazlıqların aradan qaldırılması üçün məşq etmək.

## Addım 4 — Mühasibat dəftərinə girişi qeyd edin

Nəşri `docs/source/sorafs/migration_ledger.md`-də qeyd edin, qeyd edin:

- Manifest CID, CAR həzm və şura imza hash.
- Vəziyyət (`Draft`, `Staging`, `Pinned`).
- CI yarışlarına və ya idarəetmə biletlərinə keçidlər.

## Addım 5 — İdarəetmə alətləri vasitəsilə pin edin (reyestr canlı olduqda)

Pin Qeydiyyatı yerləşdirildikdən sonra (miqrasiya yol xəritəsində Milestone M2),
CLI vasitəsilə manifest təqdim edin:

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

Təklif identifikatoru və sonrakı təsdiq əməliyyatı hashləri olmalıdır
yoxlanıla bilməsi üçün miqrasiya kitabçasına daxil edilmişdir.

## Təmizləmə

`target/sorafs/` altında olan artefaktlar arxivləşdirilə və ya quruluş qovşaqlarına yüklənə bilər.
Manifest, imzalar, CAR və gətirmə planını bir yerdə saxlayın
operatorlar və SDK komandaları yerləşdirməni qəti şəkildə təsdiq edə bilər.