---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-05T09:28:11.879581+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Çünki → Manifest Boru Kəməri

Sürətli başlanğıc üçün bu yoldaş, xam halına gələn uçdan-uca boru kəmərini izləyir
SoraFS Pin Reyestrinə uyğun olan Norito təzahürlərinə bayt. Məzmundur
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)-dən uyğunlaşdırılmışdır;
kanonik spesifikasiya və dəyişiklik jurnalı üçün həmin sənədə müraciət edin.

## 1. Qəti şəkildə yığın

SoraFS SF-1 (`sorafs.sf1@1.0.0`) profilindən istifadə edir: FastCDC-dən ilhamlanmış yuvarlanma
64KiB minimum yığın ölçüsü, 256KiB hədəf, maksimum 512KiB və
`0x0000ffff` qırılma maskası. Profil qeydiyyatdan keçib
`sorafs_manifest::chunker_registry`.

### Pas köməkçiləri

- `sorafs_car::CarBuildPlan::single_file` – Parça ofsetlərini, uzunluqlarını və
  BLAKE3 CAR metadatasını hazırlayarkən həzm edir.
- `sorafs_car::ChunkStore` – Faydalı yükləri ötürür, parça metadatasını saxlayır və
  64KiB / 4KiB Retrievability Proof-of-of-Retrievability (PoR) seçmə ağacını əldə edir.
- `sorafs_chunker::chunk_bytes_with_digests` – Hər iki CLI-nin arxasında kitabxana köməkçisi.

### CLI alətləri

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON sifarişli ofsetləri, uzunluqları və yığın həzmlərini ehtiva edir. Davam et
manifestlər və ya orkestr gətirmə spesifikasiyalar qurarkən plan.

### PoR şahidləri

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` və
`--por-sample=<count>` beləliklə, auditorlar deterministik şahid dəstlərini tələb edə bilsinlər. Cütləşdirmək
JSON qeyd etmək üçün `--por-proof-out` və ya `--por-sample-out` olan bayraqlar.

## 2. Manifestə sarın

`ManifestBuilder` yığın metadata ilə idarəetmə qoşmalarını birləşdirir:

- Kök CID (dag-cbor) və CAR öhdəlikləri.
- Alias ​​sübutları və provayder qabiliyyəti iddiaları.
- Şura imzaları və isteğe bağlı metadata (məsələn, ID-lərin qurulması).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Mühüm çıxışlar:

- `payload.manifest` – Norito kodlu manifest baytları.
- `payload.report.json` – İnsan/avtomatlaşdırma oxuna bilən xülasə, o cümlədən
  `chunk_fetch_specs`, `payload_digest_hex`, CAR həzmləri və ləqəb metadata.
- `payload.manifest_signatures.json` – BLAKE3 manifestini ehtiva edən zərf
  həzm, yığın-plan SHA3 həzm və sıralanmış Ed25519 imzaları.

Xarici tərəfindən təmin edilən zərfləri yoxlamaq üçün `--manifest-signatures-in` istifadə edin
imzalayanlar onları geri yazmadan əvvəl və `--chunker-profile-id` və ya
Reyestr seçimini kilidləmək üçün `--chunker-profile=<handle>`.

## 3. Yayımlayın və bərkidin

1. **İdarəetmə təqdimatı** – Manifest həzmini və imzanı təqdim edin
   zərfləri şuraya göndərin ki, pin qəbul olunsun. Kənar auditorlar olmalıdır
   manifest həzmlə yanaşı yığın planlı SHA3 həzmini saxlayın.
2. **Pin yükləri** – İstinad edilən CAR arxivini (və əlavə CAR indeksini) yükləyin
   Pin Reyestrinin manifestində. Manifest və CAR-ın paylaşılmasını təmin edin
   eyni kök CID.
3. **Telemetriyanı qeyd edin** – JSON hesabatını, PoR şahidlərini və istənilən məlumatı davam etdirin
   buraxılış artefaktlarında ölçülər. Bu qeydlər operatorun idarə panellərini və
   böyük yükləri yükləmədən problemlərin təkrar istehsalına kömək edin.

## 4. Çox provayder gətirmə simulyasiyası

`yük axını -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` provayder başına paralelliyi artırır (yuxarıda `#4`).
- `@<weight>` melodiyalar planlaşdırma meylini; default olaraq 1.
- `--max-peers=<n>` nə vaxt işə salınması planlaşdırılan provayderlərin sayını əhatə edir
  kəşf arzu ediləndən daha çox namizəd verir.
- `--expect-payload-digest` və `--expect-payload-len` səssizliyə qarşı qoruyur
  korrupsiya.
- `--provider-advert=name=advert.to` əvvəl provayderin imkanlarını yoxlayır
  simulyasiyada onlardan istifadə.
- `--retry-budget=<n>` hər bir hissənin təkrar sınama sayını üstələyir (defolt: 3) beləliklə CI
  uğursuzluq ssenarilərini sınaqdan keçirərkən reqressiyaları daha sürətli üzə çıxara bilər.

`fetch_report.json` toplanmış metrikləri (`chunk_retry_total`,
`provider_failure_rate` və s.) CI təsdiqləri və müşahidə oluna bilməsi üçün uyğundur.

## 5. Reyestr yeniləmələri və idarəetmə

Yeni chunker profilləri təklif edərkən:

1. `sorafs_manifest::chunker_registry_data`-də deskriptorun müəllifi.
2. `docs/source/sorafs/chunker_registry.md` və əlaqəli nizamnamələri yeniləyin.
3. Qurğuları bərpa edin (`export_vectors`) və imzalanmış manifestləri çəkin.
4. Nizamnaməyə uyğunluq hesabatını idarəetmə imzaları ilə təqdim edin.

Avtomatlaşdırma kanonik tutacaqlara (`namespace.name@semver`) üstünlük verməlidir və düşməlidir