---
lang: az
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2025-12-29T18:16:35.986892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI Bundle Alətləri

MOCHI yüngül qablaşdırma iş axını ilə göndərilir ki, tərtibatçılar a
sifarişli CI skriptlərini bağlamadan portativ masa üstü paketi. `xtask`
alt komanda kompilyasiya, tərtibat, hashing və (istəyə görə) arxivi idarə edir
bir atışda yaradılış.

## Paketin yaradılması

```bash
cargo xtask mochi-bundle
```

Varsayılan olaraq, komanda buraxılış binalarını qurur, paketi altında toplayır
`target/mochi-bundle/` və `mochi-<os>-<arch>-release.tar.gz` arxivini yayır
deterministik `manifest.json` ilə yanaşı. Manifest hər faylı siyahıya alır
onun ölçüsü və SHA-256 hashı beləliklə, CI boru kəmərləri yoxlamanı yenidən həyata keçirə və ya dərc edə bilsin
attestasiyalar. Köməkçi həm `mochi` masa üstü qabığını, həm də
iş sahəsi `kagami` binar mövcuddur, buna görə də genezis nəsli
qutu.

### Bayraqlar

| Bayraq | Təsvir |
|--------------------|---------------------------------------------------------------------------------------|
| `--out <dir>` | Çıxış kataloqunu ləğv edin (defolt olaraq `target/mochi-bundle`).         |
| `--profile <name>` | Xüsusi Yük profili ilə qurun (məsələn, testlər üçün `debug`).              |
| `--no-archive` | Yalnız hazırlanmış qovluğu tərk edərək, `.tar.gz` arxivini atlayın.               |
| `--kagami <path>` | `iroha_kagami` qurmaq əvəzinə açıq `kagami` binar istifadə edin.         |
| `--matrix <path>` | CI mənşəyinin izlənməsi üçün paket metadatasını JSON matrisinə əlavə edin.         |
| `--smoke` | Əsas icra qapısı kimi paketlənmiş paketdən `mochi --help`-i işə salın.      |
| `--stage <dir>` | Hazır paketi (və mövcud olduqda arxivi) bir quruluş qovluğuna kopyalayın. |

`--stage`, hər bir qurma agentinin öz yüklədiyi CI boru kəmərləri üçün nəzərdə tutulub.
artefaktları paylaşılan yerə. Köməkçi paket kataloqunu yenidən yaradır və
yaradılan arxivi tərtib qovluğuna kopyalayır ki, işləri dərc edə bilsin
qabıq skripti olmadan platformaya xas çıxışları toplayın.

Paketin içərisində düzülmə qəsdən sadədir:

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### İcra müddəti ləğv edilir

Paketlənmiş `mochi` icra olunan faylı ən çox əmr satırı ləğvetmələrini qəbul edir
ümumi nəzarətçi parametrləri. Redaktə etmək əvəzinə bu bayraqlardan istifadə edin
Təcrübə edərkən `config/local.toml`:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

İstənilən CLI dəyəri `config/local.toml` girişləri və mühiti üzərində üstünlük təşkil edir
dəyişənlər.

## Snapshot avtomatlaşdırılması

`manifest.json` nəsil vaxt damğasını, hədəf üçlüyü, Yük profilini,
və tam fayl inventar. Boru kəmərləri nə vaxt olduğunu aşkar etmək üçün manifestləri fərqləndirə bilər
yeni artefaktlar görünür, buraxılış aktivləri ilə yanaşı JSON-u yükləyin və ya yoxlayın
paketi operatorlara təqdim etməzdən əvvəl hashlər.

Köməkçi idempotentdir: əmrin təkrar icrası manifest və
`target/mochi-bundle/`-i tək saxlayaraq əvvəlki arxivin üzərinə yazır
cari maşındakı ən son paket üçün həqiqət mənbəyi.