---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.997191+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Sürətli Başlanğıc

Bu praktiki bələdçi deterministik SF-1 chunker profilini gəzir,
manifest imzalanması və SoraFS-i dəstəkləyən çox provayder gətirmə axını
saxlama boru kəməri. Onu [manifest boru xəttinin dərin dalışı] (manifest-pipeline.md) ilə birləşdirin
dizayn qeydləri və CLI bayrağı istinad materialı üçün.

## İlkin şərtlər

- Rust alətlər silsiləsi (`rustup update`), yerli olaraq klonlaşdırılmış iş sahəsi.
- İsteğe bağlı: [OpenSSL tərəfindən yaradılan Ed25519 klaviatura](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  manifestləri imzalamaq üçün.
- İsteğe bağlı: Docusaurus portalına baxış keçirməyi planlaşdırırsınızsa, Node.js ≥ 18.

Faydalı CLI mesajlarını üzə çıxarmaq üçün təcrübə apararkən `export RUST_LOG=info` təyin edin.

## 1. Deterministik qurğuları yeniləyin

Kanonik SF-1 parçalanma vektorlarını bərpa edin. Əmr də imzalı yayır
`--signing-key` verildikdə manifest zərfləri; `--allow-unsigned` istifadə edin
yalnız yerli inkişaf zamanı.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Çıxışlar:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (imzalanmışdırsa)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Faydalı yükü parçalayın və planı yoxlayın

İstənilən fayl və ya arxivi parçalamaq üçün `sorafs_chunker` istifadə edin:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Əsas sahələr:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` parametrlərini təsdiqləyir.
- `chunks[]` – sifarişli ofsetlər, uzunluqlar və BLAKE3 həzmləri.

Daha böyük qurğular üçün axın və yayımı təmin etmək üçün proptest tərəfindən dəstəklənən reqressiyanı işlədin
toplu parçalanma sinxron olaraq qalır:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Manifest qurun və imzalayın

Parça planını, ləqəbləri və idarəetmə imzalarını istifadə edərək manifestə sarın
`sorafs-manifest-stub`. Aşağıdakı əmr bir fayllı yükü nümayiş etdirir; keçmək
ağacı paketləmək üçün qovluq yolu (CLI onu leksikoqrafik olaraq gəzir).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json`-i nəzərdən keçirin:

- `chunking.chunk_digest_sha3_256` – ofsetlərin/uzunluqların SHA3 həzminə uyğundur
  chunker armaturları.
- `manifest.manifest_blake3` – manifest zərfində imzalanmış BLAKE3 həzm.
- `chunk_fetch_specs[]` – orkestrlər üçün sifarişli gətirmə təlimatları.

Həqiqi imzaları təqdim etməyə hazır olduqda, `--signing-key` və `--signer` əlavə edin
arqumentlər. Komanda hər bir Ed25519 imzasını yazmadan əvvəl yoxlayır
zərf.

## 4. Çox provayder axtarışını simulyasiya edin

Parça planını bir və ya daha çoxuna qarşı təkrar oynamaq üçün tərtibatçının CLI-ni gətirməsindən istifadə edin
provayderlər. Bu, CI tüstü testləri və orkestrator prototipi üçün idealdır.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Təsdiqlər:

- `payload_digest_hex` manifest hesabatına uyğun olmalıdır.
- `provider_reports[]` hər provayder üçün uğur/uğursuzluq sayılarını göstərir.
- Sıfır olmayan `chunk_retry_total` geri təzyiq tənzimləmələrini vurğulayır.
- Qaçış üçün planlaşdırılan provayderlərin sayını məhdudlaşdırmaq üçün `--max-peers=<n>` keçin
  və CI simulyasiyalarını əsas namizədlərə yönəldin.
- `--retry-budget=<n>` defolt hər hissəyə yenidən cəhd sayını (3) ləğv edir, beləliklə siz
  uğursuzluqları inyeksiya edərkən orkestr reqressiyalarını daha sürətli üzə çıxara bilər.

Uğursuzluq üçün `--expect-payload-digest=<hex>` və `--expect-payload-len=<bytes>` əlavə edin
yenidən qurulan faydalı yük manifestdən kənara çıxdıqda sürətli.

## 5. Növbəti addımlar

- **İdarəetmə inteqrasiyası** – manifest həzmini və
  `manifest_signatures.json` şuranın iş prosesinə daxil edin ki, Pin Reyestrinə daxil olun
  mövcudluğu reklam etmək.
- **Reyestr danışıqları** – [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md) ilə məsləhətləşin
  yeni profilləri qeydiyyatdan keçirməzdən əvvəl. Avtomatlaşdırma kanonik tutacaqlara üstünlük verməlidir
  (`namespace.name@semver`) rəqəmsal identifikatorlar üzərində.
- **CI avtomatlaşdırılması** – boru kəmərlərini buraxmaq üçün yuxarıdakı əmrləri əlavə edin ki, sənədlər,
  qurğular və artefaktlar deterministik manifestləri imzalayırlar
  metadata.