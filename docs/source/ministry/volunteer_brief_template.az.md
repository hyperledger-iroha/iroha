---
lang: az
direction: ltr
source: docs/source/ministry/volunteer_brief_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cae4747782524b545fdcd52e7523cce0f5b60ddb85f32c747c5f57a63f85ccdc
source_last_modified: "2025-12-29T18:16:35.984008+00:00"
translation_last_reviewed: 2026-02-07
title: Volunteer Brief Template
summary: Structured template for roadmap item MINFO-3a covering balanced briefs, fact tables, conflict disclosures, and moderation tags.
translator: machine-google-reviewed
---

# Könüllü Qısa Şablonu (MINFO-3a)

Yol xəritəsi arayışı: **MINFO-3a — Balanslaşdırılmış qısa şablonlar və münaqişənin açıqlanması.**

Könüllülərin qısa təqdimatları qara siyahıya dəyişikliklər və ya digər Nazirlik tətbiqi təklifləri təklif edildikdə vətəndaş panellərinin idarəçiliyin nəzərdən keçirilməsini istədikləri mövqeləri ümumiləşdirir. MINFO-3a tələb edir ki, şəffaflıq kəməri (1) müqayisə edilə bilən fakt cədvəlləri təqdim edə, (2) maraqların toqquşmasının açıqlandığını təsdiq edə və (3) mövzudan kənar təqdimatları avtomatik olaraq buraxa və ya qeyd edə bilsin. Bu səhifə `cargo xtask ministry-transparency`-də göndərilən alətlər tərəfindən gözlənilən kanonik sahələri, CSV-stil fakt cədvəlinin tərtibatını və moderasiya teqlərini müəyyən edir.

> **Norito sxemi:** `iroha_data_model::ministry::VolunteerBriefV1` strukturu (`1` versiyası) indi bütün təqdimatlar üçün səlahiyyətli sxemdir. Alətlər və portal təsdiqləyiciləri brifinqi dərc etməzdən və ya panel xülasələrində ona istinad etməzdən əvvəl `VolunteerBriefV1::validate`-ə zəng edirlər.

## Təqdimat yükü strukturu

| Bölmə | Sahələr | Tələblər |
|---------|--------|--------------|
| **Zərf** | `version` (u16) | `1` olmalıdır. Versiya qoruyucusu Nazirliyə qeyri-müəyyənlik olmadan sxemi təkmilləşdirməyə imkan verir. |
| **Şəxsiyyət və mövqe** | `brief_id` (sətir, təqvim ili üçün unikal), `proposal_id` (qara siyahıya və ya siyasət hərəkətinə keçidlər), `language` (BCP-47), `stance` (`support`/`oppose`/`context`), `submitted_at` (RFC3339) | Bütün sahələr tələb olunur. `stance` idarə panellərini təqdim edir və icazə verilən lüğətə uyğun olmalıdır. |
| **Müəllif məlumatı** | `author.name`, `author.organization` (isteğe bağlı), `author.contact`, `author.no_conflicts_certified` (bool) | `author.contact` ictimai idarə panellərindən redaktə olunub, lakin xam artefaktda saxlanılır. `no_conflicts_certified: true` yalnız müəllif heç bir açıqlamanın tətbiq olunmadığını təsdiqlədikdə təyin edin. |
| **Xülasə** | `summary.title`, `summary.abstract`, `summary.requested_action` | Mətn icmalı faktlar cədvəlinin yanında görünür. `summary.abstract`-i ≤2000 simvolla məhdudlaşdırın. |
| **Fakt cədvəli** | `fact_table` massivi (növbəti bölməyə bax) | Hətta qısa brifinqlər üçün də tələb olunur. CLI və şəffaflıq işi fakt cədvəli olmadan təqdim etməyi rədd edir. |
| **Açıqlamalar** | `disclosures` massivi VEYA `author.no_conflicts_certified: true` | Hər bir açıqlama sırasına `type` (`financial`, `employment`, `governance`, `family`, `other`), `other`, Norito, Norito daxil edilməlidir. `relationship` və `details`. |
| **Moderasiya metadata** | `moderation.off_topic` (bool), `moderation.tags` (enum sətirləri massivi), `moderation.notes` | Rəyçilər tərəfindən astroturfing və ya əlaqəli olmayan təqdimatların qarşısını almaq üçün istifadə olunur. Mövzudan kənar girişlər tablosuna kömək etmir. |

## Fakt cədvəlinin spesifikasiyası

Hər bir `fact_table` cərgəsi maşın tərəfindən oxuna bilən iddianı əks etdirir. Sətirləri JSON obyektləri kimi aşağıdakı sahələrlə saxlayın:| Sahə | Təsvir |
|-------|-------------|
| `claim_id` | Stabil identifikator (məsələn, `VB-2026-04-F1`). |
| `claim` | Bir cümlədən ibarət fakt və ya təsir ifadəsi. |
| `status` | `corroborated`, `disputed`, `context-only`-dən biri. |
| `impact` | `governance`, `technical`, `compliance`, `community`-dən bir və ya daha çoxunu ehtiva edən massiv. |
| `citations` | Sətirlərin boş olmayan massivi. URL-lər, Torii iş ID-ləri və ya CID arayışları qəbul edilir. |
| `evidence_digest` | Əlavə sənədlərin BLAKE3 yoxlama cəmi. |

Avtomatlaşdırma qeydləri:
- Nəşr kartlarını yaratmaq üçün qəbul edilən iş `fact_rows` və `fact_rows_with_citation`-ni hesablayır. Sitatsız sətirlər hələ də insan tərəfindən oxuna bilən cədvəldə görünür, lakin itkin sübut kimi izlənir.
- İddiaları qısa saxlayın və idarəetmə təkliflərində istifadə edilən eyni identifikatorlara istinad edin ki, çarpaz əlaqə deterministik olsun.

## Münaqişənin açıqlanması tələbləri

1. Maliyyə, məşğulluq, idarəetmə və ya ailə əlaqəsi mövcud olduqda, ən azı bir açıqlama qeydini təqdim edin.
2. `author.no_conflicts_certified: true`-dən istifadə edərək “məlum münaqişə yoxdur”. Təqdimatlar ya açıqlama girişini, ya da `true` sertifikatını daxil etməlidir; əks halda, qəbul zamanı onlar işarələnir.
3. İctimai sənədlər mövcud olduqda (məsələn, korporativ sənədlər, DAO səsləri) `disclosures[i].evidence` daxil edin. Sübut “heç biri” sertifikatları üçün isteğe bağlıdır, lakin şiddətlə tövsiyə olunur.

## Moderasiya teqləri və mövzudan kənar işləmə

Moderasiya rəyçiləri şəffaflıq xəttinə daxil olmamışdan əvvəl təqdimatları etiketləyə bilərlər:

- `moderation.off_topic: true` `off_topic_rejections` sayğacını artırarkən girişi ümumi saylardan silir. Sətir hələ də audit üçün xam arxivlərdə mövcuddur.
- `moderation.tags` enum dəyərlərini qəbul edir: `duplicate`, `needs-translation`, `needs-follow-up`, `spam`, `astroturf`, `astroturf`, Norito. Teqlər tam brifinqi yenidən oxumadan aşağı istiqamətli rəyçilərə sınaqdan keçməyə kömək edir.
- `moderation.notes` moderasiya qərarı üçün qısa əsaslandırma saxlayır (≤512 simvol).

## Təqdimat siyahısı

1. Bu şablondan və ya aşağıda təsvir edilən köməkçi CLI-dən istifadə edərək JSON yükünü doldurun.
2. Ən azı bir fakt cədvəli sırasını doldurun; hər sətir üçün sitatlar daxil edin.
3. Açıqlamalar təqdim edin və ya açıq şəkildə `author.no_conflicts_certified: true` təyin edin.
4. Rəyçilər tez sınaqdan keçirə bilməsi üçün moderasiya metadatasını (defolt `off_topic: false`) əlavə edin.
5. Yükləmədən əvvəl faydalı yükü `cargo xtask ministry-transparency ingest --volunteer <file>` və ya hər hansı Norito təsdiqləyicisi ilə yoxlayın.

## Validasiya CLI (MINFO-3)

Anbar indi könüllü brifinqlər üçün xüsusi təsdiqləyici göndərir:

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input docs/examples/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

Əsas davranış:- Fərdi JSON obyektlərini *və ya* brifinq massivlərini qəbul edir; bir qaçışda bir neçə faylı silmək üçün `--input`-dən bir neçə dəfə keçin.
- Səhvlərin və xəbərdarlıqların sayını göstərən qısa xülasə verir; xəbərdarlıqlar boş sitat siyahılarını və ya həddən artıq uzun qeydləri vurğulayır, səhvlər isə nəşri bloklayır.
- Tələb olunan sahələrin (`brief_id`, `proposal_id`, `stance`, faktlar cədvəlinin məzmunu, açıqlamalar və ya `no_conflicts_certified`) bu şablona uyğun olmasını və enum dəyərlərinin sənədləşdirilmiş lüğətlər daxilində qalmasını təmin edir.
- `--json-output <path>` qurulduqda, validator hər bir brifinqi (təklif identifikatoru, mövqe, status, səhvlər/xəbərdarlıqlar) ümumiləşdirən maşın tərəfindən oxuna bilən manifest yazır. Portalın `npm run generate:volunteer-lint` əmri hər bir təklif səhifəsinin yanında lint statusunu göstərmək üçün bu manifestdən istifadə edir.

Könüllü təqdimatları şəffaflığın qəbulu işinə çatmazdan əvvəl **MINFO-3**-ə uyğun saxlamaq üçün əmri portal iş axınlarına və ya CI-yə inteqrasiya edin.

## Nümunə yükü

Fakt cədvəli sətirləri, açıqlamalar və moderasiya teqləri daxil olmaqla, tam doldurulmuş nümunə üçün `docs/examples/ministry/volunteer_brief_template.json`-ə baxın. Aşağı axın panelləri xam JSON-u istehlak edir və avtomatik olaraq hesablayır:

- `total_briefs` (mövzudan kənar təqdimatlar istisna olunur)
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

Yeni sahələr tələb olunarsa, bu sənədi və qəbul edilmiş xülasəni (`xtask/src/ministry.rs`) eyni dəyişikliklə yeniləyin ki, idarəetmə sübutları təkrarlana bilsin.

## Nəşr SLA və portal səthi (MINFO-3)

Vətəndaş müraciətlərini şəffaf saxlamaq üçün portal indi təsdiqləmədən keçdikdən sonra sabit kadans haqqında qısa məlumat dərc edir:

1. **T+0–6saat:** təqdimatlar könüllü qəbul forması və ya `cargo xtask ministry-transparency ingest` vasitəsilə verilir. Təsdiqləyicilər `VolunteerBriefV1::validate`-i işə salır, səhv formalaşmış faydalı yükləri rədd edir və lint hesabatları (çatışmayan açıqlamalar, dublikat fakt identifikatorları və s.) yayır.
2. **T+6–24saat:** qəbul edilən brifinqlər tərcümə/triaj üçün növbəyə qoyulur. Moderasiya teqləri (`needs-translation`, `duplicate`, `policy-escalation`, …) tətbiq edilir və mövzudan kənar qeydlər arxivləşdirilir, lakin ümumi saylardan xaric edilir.
3. **T+24–48saat:** portal müvafiq təklif səhifəsinin yanında qısa məlumatı dərc edir. Hər dərc edilmiş təklif indi “Könüllülərin Rəyləri” ilə əlaqələndirilir ki, rəyçilər xam JSON açmadan dəstək/müxalifət/kontekst brifinqlərini oxuya bilsinlər.

Təqdimat `policy-escalation` və ya `astroturf` olaraq qeyd edilirsə, SLA **12 saat** müddətinə sərtləşir ki, idarəetmə tez cavab verə bilsin. Operatorlar sənədlər portalında (`docs/portal/docs/ministry/volunteer-briefs.md`) ən son nəşr pəncərələrini, lint statusunu və Norito artefaktlarına keçidləri sadalayan **Könüllülük haqqında qısa məlumat** səhifəsi vasitəsilə SLA-nı yoxlaya bilərlər.