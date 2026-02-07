---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 855dd4bff7bf9581f485ac6ad7fa332f17595efb41127b74795c4b3a5a955406
source_last_modified: "2026-01-05T09:28:11.856260+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-profile-authoring
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Chunker Profilinin Müəlliflik Bələdçisi

Bu bələdçi SoraFS üçün yeni chunker profillərini necə təklif etməyi və dərc etməyi izah edir.
O, arxitektura RFC (SF-1) və reyestr arayışını (SF-2a) tamamlayır.
konkret müəlliflik tələbləri, doğrulama addımları və təklif şablonları ilə.
Kanonik bir nümunə üçün baxın
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
və onu müşayiət edən quru işə daxil olun
`docs/source/sorafs/reports/sf1_determinism.md`.

## Baxış

Reyestrə daxil olan hər bir profil:

- deterministik CDC parametrlərini və eyni olan multihash parametrlərini reklam edin
  memarlıq;
- gəmi təkrar oynatılan qurğular (Rust/Go/TS JSON + fuzz corpora + PoR şahidləri)
  aşağı axın SDK-lar sifarişli alətlər olmadan yoxlaya bilər;
- idarəetməyə hazır metadata (ad sahəsi, ad, semver) və miqrasiya daxildir
- Şuranın nəzərdən keçirilməsindən əvvəl deterministik fərq paketini keçin.

Həmin qaydalara cavab verən təklif hazırlamaq üçün aşağıdakı yoxlama siyahısına əməl edin.

## Registry Charter Snapshot

Təklifi tərtib etməzdən əvvəl onun qüvvədə olan reyestr nizamnaməsinə uyğun olduğunu təsdiqləyin
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` tərəfindən:

- Profil identifikatorları boşluqlar olmadan monoton şəkildə artan müsbət tam ədədlərdir.
- Kanonik tutacaq (`namespace.name@semver`) ləqəb siyahısında görünməlidir
  və ** ilk giriş olmalıdır.
- Heç bir ləqəb başqa kanonik tutacaqla toqquşmaya və ya bir dəfədən çox görünə bilməz.
- Ləqəblər boş olmamalıdır və boşluqdan kəsilməlidir.

Əlverişli CLI köməkçiləri:

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Bu əmrlər təklifləri reyestr nizamnaməsinə uyğun saxlayır və təmin edir
idarəetmə müzakirələrində lazım olan kanonik metadata.

## Tələb olunan Metadata

| Sahə | Təsvir | Nümunə (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Əlaqədar profillər üçün məntiqi qruplaşdırma. | `sorafs` |
| `name` | İnsan tərəfindən oxuna bilən etiket. | `sf1` |
| `semver` | Parametr dəsti üçün semantik versiya sətri. | `1.0.0` |
| `profile_id` | Profil eniş etdikdən sonra təyin edilən monoton rəqəmli identifikator. Növbəti id-i rezerv edin, lakin mövcud nömrələri təkrar istifadə etməyin. | `1` |
| `profile_aliases` | Danışıqlar zamanı müştərilərə məruz qalan əlavə tutacaqlar. Həmişə kanonik sapı ilk giriş kimi daxil edin. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Minimum parça uzunluğu baytla. | `65536` |
| `profile.target_size` | Hədəf yığın uzunluğu baytlarla. | `262144` |
| `profile.max_size` | Baytlarda maksimum parça uzunluğu. | `524288` |
| `profile.break_mask` | Yuvarlanan hash (hex) tərəfindən istifadə edilən uyğunlaşdırıcı maska. | `0x0000ffff` |
| `profile.polynomial` | Dişli polinom sabiti (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Toxum 64KiB dişli cədvəlini əldə etmək üçün istifadə olunur. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Parça başına həzmlər üçün multihash kodu. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Kanonik qurğular dəstinin həzmi. | `13fa...c482` |
| `fixtures_root` | Yenilənmiş qurğuları ehtiva edən nisbi kataloq. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Deterministik PoR nümunəsi üçün toxum (`splitmix64`). | `0xfeedbeefcafebabe` (nümunə) |

Metadata həm təklif sənədində, həm də yaradılan sənədin daxilində görünməlidir
qurğular, beləliklə, reyestr, CLI alətləri və idarəetmə avtomatlaşdırılması təsdiq edə bilər
əllə çarpaz istinad olmadan dəyərlər. Şübhə olduqda, yığın mağazasını işə salın və
hesablanmış metaməlumatları nəzərdən keçirmək üçün `--json-out=-` ilə manifest CLIs
qeydlər.

### CLI & Registry Touch Points

- `sorafs_manifest_chunk_store --profile=<handle>` – parça metadatasını yenidən işə salın,
  manifest həzm, PoR təklif olunan parametrlərlə yoxlayır.
- `sorafs_manifest_chunk_store --json-out=-` – yığın-mağaza hesabatını yayımlayın
  Avtomatlaşdırılmış müqayisələr üçün stdout.
- `sorafs_manifest_stub --chunker-profile=<handle>` – manifestləri və CAR-ı təsdiqləyin
  planlar kanonik sapı və ləqəbləri yerləşdirir.
- `sorafs_manifest_stub --plan=-` – əvvəlki `chunk_fetch_specs`-i geri qaytarın
  Dəyişiklikdən sonra ofsetləri/həzmləri yoxlamaq üçün.

Təklifdə komanda çıxışını qeyd edin (həzmlər, PoR kökləri, manifest hashları)
beləliklə, rəyçilər onları sözlə təkrar edə bilərlər.

## Determinizm və Validasiya Yoxlama Siyahısı

1. **Aparatları bərpa edin**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Paritet dəstini işə salın** – `cargo test -p sorafs_chunker` və
   dillər arası fərq qoşqu (`crates/sorafs_chunker/tests/vectors.rs`) olmalıdır
   yerində yeni qurğular ilə yaşıl.
3. **Fuzz/back-pressure corpora-nı təkrar oxuyun** – `cargo fuzz list` və
   regenerasiya edilmiş aktivlərə qarşı axın qoşqu (`fuzz/sorafs_chunker`).
4. **Retrievability sübutunun şahidlərini yoxlayın** – qaçın
   Təklif olunan profildən istifadə edərək `sorafs_manifest_chunk_store --por-sample=<n>` və
   köklərin fikstür manifestinə uyğun olduğunu təsdiqləyin.
5. **CI quru qaçış** – yerli olaraq `ci/check_sorafs_fixtures.sh` çağırın; skript
   yeni qurğular və mövcud `manifest_signatures.json` ilə uğur qazanmalıdır.
6. **Çapraz iş vaxtı təsdiqi** – Go/TS bağlamalarının bərpa olunanları istehlak etməsinə əmin olun
   JSON və eyni yığın sərhədləri və həzmlər buraxın.

Təklifdə əmrləri və nəticədə həzmləri sənədləşdirin ki, Tooling WG
onları təxmin etmədən yenidən işlədə bilər.

### Manifest / PoR Təsdiqi

Armaturları bərpa etdikdən sonra CAR-ı təmin etmək üçün tam manifest boru kəmərini işə salın
metadata və PoR sübutları ardıcıl olaraq qalır:

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Daxiletmə faylını qurğularınız tərəfindən istifadə olunan hər hansı təmsilçi korpusla əvəz edin
(məsələn, 1GiB deterministik axın) və əldə edilən həzmləri əlavə edin
təklif.

## Təklif Şablonu

Təkliflər `ChunkerProfileProposalV1` Norito qeydləri yoxlanılaraq təqdim olunur.
`docs/source/sorafs/proposals/`. Aşağıdakı JSON şablonu gözlənilənləri göstərir
forma (lazım olduqda dəyərləri əvəz edin):


uyğun olan Markdown hesabatını (`determinism_report`) təqdim edin.
komanda çıxışı, yığın həzmləri və yoxlama zamanı rast gəlinən hər hansı sapma.

## İdarəetmə İş Akışı

1. **Təklif + qurğularla PR təqdim edin.** Yaradılan aktivləri,
   Norito təklifi və `chunker_registry_data.rs` yeniləmələri.
2. **Alətlər Qrupunun nəzərdən keçirilməsi.** Nəzərdən keçirənlər təsdiqləmə yoxlama siyahısını yenidən işlədir və təsdiqləyirlər
   təklif reyestr qaydalarına uyğun gəlir (id təkrar istifadə edilmir, determinizm təmin edilir).
3. **Şura zərfi.** Təsdiq edildikdən sonra şura üzvləri təkliflər toplusunu imzalayır
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) və onları əlavə edin
   armaturların yanında saxlanılan profil zərfinə imzalar.
4. **Reyestr nəşri.** Birləşdirmə reyestr, sənədlər və qurğuları sıxışdırır. The
   Defolt CLI, idarəetmə elan edənə qədər əvvəlki profildə qalır
   miqrasiya hazırdır.
5. **Qeydiyyatın izlənməsi.** Miqrasiya pəncərəsindən sonra reyestri yeniləyin
   dəftər.

## Müəlliflik Məsləhətləri

- Kənar-case parçalanma davranışını minimuma endirmək üçün hətta iki gücün hüdudlarına üstünlük verin.
- Manifest və şlüz koordinasiya etmədən multihash kodunu dəyişməkdən çəkinin
- Ötürücü masa toxumlarını insanlar tərəfindən oxuna bilən, lakin auditi sadələşdirmək üçün qlobal miqyasda unikal saxlayın
  cığırlar.
- İstənilən müqayisə artefaktlarını (məsələn, ötürmə qabiliyyətinin müqayisəsi) altında saxlayın
  Gələcək istinad üçün `docs/source/sorafs/reports/`.

Yayım zamanı əməliyyat gözləntiləri üçün miqrasiya kitabçasına baxın
(`docs/source/sorafs/migration_ledger.md`). İş vaxtı uyğunluq qaydaları üçün baxın
`docs/source/sorafs/chunker_conformance.md`.