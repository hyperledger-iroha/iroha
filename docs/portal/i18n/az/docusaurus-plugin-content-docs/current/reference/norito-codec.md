---
lang: az
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Codec Referansı

Norito, Iroha-in kanonik serializasiya qatıdır. Hər bir tel mesajı, diskdə
faydalı yük və çarpaz komponent API Norito istifadə edir, beləliklə qovşaqlar eyni baytlarda razılaşır
hətta müxtəlif aparatlarda işləyərkən belə. Bu səhifə hərəkət edən hissələri ümumiləşdirir
və `norito.md`-də tam spesifikasiyaya işarə edir.

## Əsas düzən

| Komponent | Məqsəd | Mənbə |
| --- | --- | --- |
| **Başlıq** | Faydalı yükləri sehrli/versiya/şema hash, CRC64, uzunluq və sıxılma etiketi ilə çərçivələr; v1 `VERSION_MINOR = 0x00` tələb edir və dəstəklənən maskaya qarşı başlıq bayraqlarını doğrulayır (defolt `0x00`). | `norito::header` — bax `norito.md` (“Başlıq və Bayraqlar”, depo kökü) |
| **Çılpaq faydalı yük** | Hashing/müqayisə üçün istifadə edilən deterministik dəyər kodlaması. Tellə daşıma həmişə başlıqdan istifadə edir; çılpaq baytlar yalnız daxilidir. | `norito::codec::{Encode, Decode}` |
| **Sıxılma** | Könüllü Zstd (və eksperimental GPU sürətləndirilməsi) başlıq sıxılma baytı vasitəsilə seçilir. | `norito.md`, “Sıxılma danışıqları” |

Layout bayraq reyestri (paketli struktur, paketlənmiş seq, sahə bit dəsti, yığcam
uzunluqlar) `norito::header::flags`-də yaşayır. V1 standart olaraq `0x00` bayraqlarıdır, lakin
dəstəklənən maska daxilində açıq başlıq bayraqlarını qəbul edir; naməlum bitlərdir
rədd edildi. `norito::header::Flags` daxili yoxlama üçün saxlanılır və
gələcək versiyalar.

## Dəstək əldə edin

`norito_derive` `Encode`, `Decode`, `IntoSchema` göndərir və JSON köməkçisi gəlir.
Əsas konvensiyalar:

- Törəmələr həm AoS, həm də paketlənmiş kod yollarını yaradır; v1 defolt olaraq AoS-ə uyğundur
  layout (bayraqlar `0x00`) başlıq bayraqları dolu variantlara daxil olmadıqda.
  İcra `crates/norito_derive/src/derive_struct.rs`-də yaşayır.
- Dizayna təsir edən xüsusiyyətlər (`packed-struct`, `packed-seq`, `compact-len`)
  başlıq bayraqları vasitəsilə daxil olun və həmyaşıdları arasında ardıcıl olaraq kodlaşdırılmalı/deşifrə edilməlidir.
- JSON köməkçiləri (`norito::json`) üçün deterministik Norito dəstəkli JSON təmin edir.
  API-ləri açın. `norito::json::{to_json_pretty, from_json}` istifadə edin — heç vaxt `serde_json`.

## Multikodek və identifikator cədvəlləri

Norito multikodek təyinatlarını `norito::multicodec`-də saxlayır. İstinad
cədvəl (heshlər, əsas növlər, faydalı yük təsvirləri) `multicodec.md`-də saxlanılır
depo kökündə. Yeni identifikator əlavə edildikdə:

1. `norito::multicodec::registry` yeniləyin.
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