---
lang: az
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-12-29T18:16:35.071058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sənədə baxış rəy forması (W1 partnyor dalğası)

W1 rəyçilərindən rəy toplayan zaman bu şablondan istifadə edin. Onu dublikat edin
tərəfdaş, metadatanı doldurun və tamamlanmış nüsxəni altında saxlayın
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Rəyçi metadatası

- **Tərəfdaş ID-si:** `partner-w1-XX`
- **Bilet tələb edin:** `DOCS-SORA-Preview-REQ-PXX`
- **Dəvət göndərildi (UTC):** `YYYY-MM-DD hh:mm`
- **Təsdiq edilmiş yoxlama məbləği (UTC):** `YYYY-MM-DD hh:mm`
- **Əsas diqqət sahələri:** (məsələn, _SoraFS orkestrator sənədləri_, _Torii ISO axınları_)

## Telemetriya və artefakt təsdiqləri

| Yoxlama siyahısı elementi | Nəticə | Sübut |
| --- | --- | --- |
| Yoxlama məbləğinin yoxlanılması | ✅ / ⚠️ | Giriş yolu (məsələn, `build/checksums.sha256`) |
| Proksi duman testini sınayın | ✅ / ⚠️ | `npm run manage:tryit-proxy …` transkript parçası |
| Grafana tablosuna baxış | ✅ / ⚠️ | Ekran görüntüsü yol(ları) |
| Portal araşdırma hesabatının nəzərdən keçirilməsi | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Rəyçinin yoxladığı hər hansı əlavə SLO üçün sətirlər əlavə edin.

## Əlaqə qeydi

| Ərazi | Ciddilik (info/minor/major/blocker) | Təsvir | Təklif edilən düzəliş və ya sual | İzləyici problemi |
| --- | --- | --- | --- | --- |
| | | | | |

Son sütunda GitHub məsələsinə və ya daxili biletə istinad edin
izləyici remediasiya elementlərini yenidən bu forma bağlaya bilər.

## Sorğunun xülasəsi

1. **Yoxlama məbləği təlimatı və dəvət prosesinə nə dərəcədə əminsiniz?** (1–5)
2. **Hansı sənədlər ən çox/ən az faydalı oldu?** (qısa cavab)
3. **Try it proksi və ya telemetriya panellərinə daxil olan blokerlər var idi?**
4. **Əlavə lokalizasiya və ya əlçatanlıq məzmunu tələb olunurmu?**
5. **GA-dan əvvəl hər hansı digər şərh varmı?**

Qısa cavabları çəkin və xarici formadan istifadə edirsinizsə, xam sorğu ixracını əlavə edin.

## Bilik yoxlanışı

- Qiymət: `__/10`
- Yanlış suallar (əgər varsa): `[#1, #4, …]`
- Təqib tədbirləri (əgər xal < 9/10 olarsa): remediasiya çağırışı planlaşdırılıb? y/n

## Çıxış

- Rəyçi adı və vaxt möhürü:
- Sənədlər/DevRel rəyçisi və vaxt damğası:

İmzalanmış nüsxəni əlaqəli artefaktlarla birlikdə saxlayın ki, auditorlar onu təkrarlaya bilsinlər
əlavə kontekst olmadan dalğa.