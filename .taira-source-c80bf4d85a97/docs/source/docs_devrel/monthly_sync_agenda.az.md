---
lang: az
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2025-12-29T18:16:35.952009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sənədlər/DevRel Aylıq Sinxronizasiya Gündəliyi

Bu gündəm, istinad edilən aylıq Sənədlər/DevRel sinxronizasiyasını rəsmiləşdirir
`roadmap.md` (bax: “Aylıq Sənədlərə/DevRel-ə lokalizasiya üzrə kadr icmalı əlavə et”
sync”) və Android AND5 i18n planı. Onu kanonik yoxlama siyahısı kimi istifadə edin və
yol xəritəsi çıxışları gündəmə maddələr əlavə etdikdə və ya ləğv etdikdə onu yeniləyin.

## Cadence & Logistics

- **Tezlik:** aylıq (adətən ikinci cümə axşamı, 16:00UTC)
- **Müddət:** 45 dəqiqə + dərin dalışlar üçün isteğe bağlı 15 dəqiqə geri çəkilmə
- **Yer:** Paylaşılan ilə böyütmə (`https://meet.sora.dev/docs-devrel-sync`).
  HackMD və ya `docs/source/docs_devrel/minutes/<yyyy-mm>.md`-də qeydlər
- **Auditoriya:** Sənədlər/DevRel meneceri (sədr), Sənəd mühəndisləri, lokalizasiya
  proqram meneceri, SDK DX TLs (Android, Swift, JS), Məhsul Sənədləri, Buraxılış
  Mühəndislik nümayəndəsi, Dəstək/QA müşahidəçiləri
- **Fasilitator:** Sənədlər/DevRel meneceri; fırlanan bir katib təyin etsin
  dəqiqələri 24 saat ərzində repoya köçürün

## İşdən əvvəl yoxlama siyahısı

| Sahibi | Tapşırıq | Artefakt |
|-------|------|----------|
| Yazı | Aşağıdakı şablondan istifadə edərək ayın qeydləri faylını (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) yaradın. | Qeydlər faylı |
| Lokallaşdırma PM | `docs/source/sdk/android/i18n_plan.md#translation-status` və kadr jurnalını yeniləyin; təklif olunan qərarları əvvəlcədən doldurun. | i18n planı |
| DX TLs | `ci/check_android_docs_i18n.sh` və ya `scripts/sync_docs_i18n.py --dry-run`-i işə salın və müzakirə üçün həzmlər əlavə edin. | CI artefaktları |
| Sənəd alətləri | `docs/i18n/manifest.json` həzmlərini + `docs/source/sdk/android/i18n_requests/`-dən görkəmli bilet siyahısını ixrac edin. | Manifest və bilet xülasəsi |
| Dəstək / Buraxılış | Docs/DevRel əməliyyatını tələb edən hər hansı eskalasiyaları toplayın (məsələn, gözləyən ilk baxış dəvətləri, rəyçi rəyini bloklamaq). | Status.md və ya eskalasiya sənədi |

## Gündəlik Blokları1. **Vəkilli zəng və məqsədlər (5 dəq)**
   - Yetərsay, katib və logistikanı təsdiqləyin.
   - Hər hansı təcili insidentləri vurğulayın (sənədlərin önizləməsinin dayandırılması, lokalizasiya bloku).
2. **Lokallaşdırma işçi heyətinin nəzərdən keçirilməsi (15 dəqiqə)**
   - Kadr qərarları ilə bağlı girişi nəzərdən keçirin
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Açıq PO-ların statusunu (`DOCS-L10N-*`) və aralıq əhatə dairəsini təsdiqləyin.
   - CI təzəlik çıxışını tərcümə statusu cədvəli ilə müqayisə edin; hər hansı bir zəng edin
     yerli SLA (>5 iş günü) növbəti gündən əvvəl pozulacaq sənəd
     sinxronizasiya.
   - Artırmanın tələb edilib-edilməməsinə qərar verin (Məhsul Əməliyyatları, Maliyyə, podratçı
     idarəetmə). Qərarı həm kadr jurnalında, həm də aylıq qeyd edin
     dəqiqə, o cümlədən sahibi + son tarix.
   - Əgər işçi heyəti sağlamdırsa, yol xəritəsi hərəkəti mümkün olması üçün təsdiqi sənədləşdirin
     dəlillərlə 🈺/🈴-a qayıdın.
3. **Sənəd/yol xəritəsi yeniləmələri (10 dəqiqə)**
   - DOCS-SORA portal işinin statusu, Try-It proksisi və SoraFS nəşri
     hazırlıq.
   - Cari buraxılış qatarları üçün lazım olan sənəd borcunu və ya rəyçiləri vurğulayın.
4. **SDK məqamları (10 dəq)**
   - Android AND5/AND7 sənəd hazırlığı, Swift IOS5 pariteti, JS GA tərəqqi.
   - Sənədlərə təsir edəcək paylaşılan qurğuları və ya sxem fərqlərini çəkin.
5. **Fəaliyyətə baxış və dayanacaq (5 dəq)**
   - Əvvəlki sinxronizasiyadan açıq elementlərə yenidən baxın; bağlanmasını təsdiqləyin.
   - Açıq sahibləri və son tarixləri ilə qeydlər faylında yeni hərəkətləri qeyd edin.

## Lokallaşdırma Kadrlarının İcmal Şablonu

Aşağıdakı cədvəli hər ayın protokoluna daxil edin:

| Yerli | Tutum (FTE) | Öhdəliklər və PO-lar | Risklər / Eskalasiyalar | Qərar və Sahib |
|--------|----------------|-------------------|---------------------|------------------|
| JP | məsələn, 0,5 podratçı + 0,1 Sənəd ehtiyat nüsxəsi | PO `DOCS-L10N-4901` (imza gözləyir) | “Müqavilə 2026-03-04 tarixində imzalanmamışdır” | “Məhsul əməliyyatlarına yüksəldin — @docs-devrel, 2026-03-02 tarixinə qədər” |
| HE | məsələn, 0.1 Sənəd mühəndisi | Rotasiya PTO-ya daxil olur 2026-03-18 | “Yedək rəyçi lazımdır” | “@docs-lead 2026-03-05 tarixinədək ehtiyat nüsxəsini müəyyən etməyə imkan verir” |

Həm də əhatə edən qısa hekayəni qeyd edin:

- **SLA proqnozu:** Hər hansı bir sənədin beş iş günü SLA və
  azaldılması (mübadilə prioriteti, ehtiyat nüsxə satıcısını cəlb etmək və s.).
- **Bilet və aktivlərin sağlamlığı:** Görkəmli girişlər
  `docs/source/sdk/android/i18n_requests/` və ekran görüntülərinin/aktivlərin olub-olmaması
  tərcüməçilər üçün hazırdır.

### Lokallaşdırma İşçilərinin Baxış Qeydi

- **Dəqiqələr:** Ştat cədvəlini + hekayəni kopyalayın
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (bütün yerlilər
  Eyni qovluq altında lokallaşdırılmış fayllar vasitəsilə ingilis dili dəqiqələri). Girişi əlaqələndirin
  yenidən gündəmə (`docs/source/docs_devrel/monthly_sync_agenda.md`) belə
  idarəetmə sübutları izləyə bilər.
- **i18n planı:** Kadr qərarları jurnalını və tərcümə statusu cədvəlini yeniləyin
  görüşdən dərhal sonra `docs/source/sdk/android/i18n_plan.md`-də.
- **Status:** Kadr qərarları yol xəritəsi qapılarına təsir etdikdə, qısa bir giriş əlavə edin
  Dəqiqə faylına və i18n planına istinad edən `status.md` (Sənədlər/DevRel bölməsi)
  yeniləmə.

## Dəqiqə Şablonu

Bu skeleti `docs/source/docs_devrel/minutes/<yyyy-mm>.md`-ə kopyalayın:

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```Görüşdən dərhal sonra qeydləri PR vasitəsilə dərc edin və onları `status.md`-dən əlaqələndirin
risk və ya kadr qərarlarına istinad edərkən.

## Təqib Gözləntiləri

1. **Təqdim olunan dəqiqələr:** 24 saat ərzində (`docs/source/docs_devrel/minutes/`).
2. **i18n planı yeniləndi:** kadr jurnalını və tərcümə cədvəlini uyğunlaşdırın
   yeni öhdəlikləri və ya eskalasiyaları əks etdirir.
3. **Status.md girişi:** yol xəritəsini saxlamaq üçün yüksək riskli qərarları ümumiləşdirin
   sinxronizasiyada.
4. **Ekkalasiyalar təqdim edildi:** baxış eskalasiya tələb etdikdə, yaradın/yeniləyin
   müvafiq bilet (məsələn, Məhsul Əməliyyatları, Maliyyə təsdiqi, satıcının işə qəbulu)
   və həm dəqiqələrdə, həm də i18n planında istinad edin.

Bu gündəmi təqib edərək, yol xəritəsinin yerliləşdirmə tələbini ehtiva edir
Sənədlər/DevRel aylıq sinxronizasiyada kadr icmalı yoxlanıla və aşağı axın olaraq qalır
komandalar həmişə sübutları haradan tapacaqlarını bilirlər.