---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito Codec Referansı

Norito, Iroha-in kanonik serializasiya qatıdır. Hər bir tel mesajı, diskdə
faydalı yük və çarpaz komponent API Norito istifadə edir, beləliklə qovşaqlar eyni baytlarda razılaşır
hətta müxtəlif aparatlarda işləyərkən belə. Bu səhifə hərəkət edən hissələri ümumiləşdirir
və `norito.md`-də tam spesifikasiyaya işarə edir.

## Əsas düzən

| Komponent | Məqsəd | Mənbə |
| --- | --- | --- |
| **Başlıq** | Xüsusiyyətləri müzakirə edir (paketli strukturlar/ardıcıllıqlar, yığcam uzunluqlar, sıxılma bayraqları) və CRC64 yoxlama məbləğini daxil edir, beləliklə deşifrədən əvvəl yükün bütövlüyü yoxlanılır. | `norito::header` — bax `norito.md` (“Başlıq və Bayraqlar”, depo kökü) |
| **Çılpaq faydalı yük** | Hashing/müqayisə üçün istifadə edilən deterministik dəyər kodlaması. Eyni layout nəqliyyat üçün başlıq ilə bükülmüşdür. | `norito::codec::{Encode, Decode}` |
| **Sıxılma** | Könüllü Zstd (və eksperimental GPU sürətləndirilməsi) `compression` bayt baytı vasitəsilə aktivləşdirildi. | `norito.md`, “Sıxılma danışıqları” |

Tam bayraq reyestri (paket-struct, packed-seq, kompakt uzunluqlar, sıxılma)
`norito::header::flags`-də yaşayır. `norito::header::Flags` rahatlığı ortaya qoyur
iş vaxtının yoxlanılmasını yoxlayır; qorunan layout bitləri dekoderlər tərəfindən rədd edilir.

## Dəstək əldə edin

`norito_derive` `Encode`, `Decode`, `IntoSchema` göndərir və JSON köməkçisi gəlir.
Əsas konvensiyalar:

- `packed-struct` xüsusiyyəti olduqda strukturlar/saxlamalar qablaşdırılmış planlar əldə edir
  aktivdir (standart). Tətbiq `crates/norito_derive/src/derive_struct.rs`-də yaşayır
  və davranış `norito.md` (“Paketlənmiş planlar”) sənədində sənədləşdirilir.
- Paketlənmiş kolleksiyalar v1-də sabit enli ardıcıllıq başlıqlarından və ofsetlərdən istifadə edir; yalnız
  hər dəyər uzunluğu prefiksləri `COMPACT_LEN` tərəfindən təsirlənir.
- JSON köməkçiləri (`norito::json`) üçün deterministik Norito dəstəklənən JSON təmin edir.
  API-ləri açın. `norito::json::{to_json_pretty, from_json}` istifadə edin — heç vaxt `serde_json`.

## Multikodek və identifikator cədvəlləri

Norito multikodek təyinatlarını `norito::multicodec`-də saxlayır. İstinad
cədvəl (heshlər, əsas növlər, faydalı yük təsvirləri) `multicodec.md`-də saxlanılır
depo kökündə. Yeni identifikator əlavə edildikdə:

1. `norito::multicodec::registry`-i yeniləyin.
2. `multicodec.md`-də cədvəli genişləndirin.
3. Aşağı axın bağlamaları (Python/Java) xəritədən istifadə edərsə, bərpa edin.

## Sənədlərin və qurğuların bərpası

Portalda hazırda nəsr xülasəsi var, yuxarı Markdown-dan istifadə edin
həqiqət mənbəyi kimi mənbələr:

- **Spec**: `norito.md`
- **Multicodec cədvəli**: `multicodec.md`
- **Bençmarklar**: `crates/norito/benches/`
- **Qızıl testlər**: `crates/norito/tests/`

Docusaurus avtomatlaşdırılması işə salındıqda, portal yenilənəcək.
bunlardan məlumatları çıxaran sinxronizasiya skripti (`docs/portal/scripts/`-də izlənilir)
fayllar. O vaxta qədər, spesifikasiyalar dəyişəndə ​​bu səhifəni əl ilə düzülmüş saxlayın.