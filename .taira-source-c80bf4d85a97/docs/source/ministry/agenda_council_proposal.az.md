---
lang: az
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Gündəlik Şurasının Təklif Sxemi (MINFO-2a)

Yol xəritəsi arayışı: **MINFO-2a — Təklif formatının doğrulayıcısı.**

Gündəlik Şurasının iş prosesi vətəndaşlar tərəfindən təqdim edilən qara siyahı və siyasət dəyişikliyi toplularını birləşdirir
təklifləri idarəetmə panelləri nəzərdən keçirməzdən əvvəl. Bu sənəd müəyyən edir
kanonik faydalı yük sxemi, sübut tələbləri və təkrarlamanın aşkarlanması qaydaları
yeni validator (`cargo xtask ministry-agenda validate`) tərəfindən istehlak edilir
təklif edənlər JSON təqdimatlarını portala yükləməzdən əvvəl yerli olaraq silə bilər.

## Faydalı yükə baxış

Gündəlik təklifləri `AgendaProposalV1` Norito sxemindən istifadə edir
(`iroha_data_model::ministry::AgendaProposalV1`). Sahələr zaman JSON kimi kodlanır
CLI/portal səthləri vasitəsilə təqdim etmək.

| Sahə | Növ | Tələblər |
|-------|------|--------------|
| `version` | `1` (u16) | `AGENDA_PROPOSAL_VERSION_V1`-ə bərabər olmalıdır. |
| `proposal_id` | sətir (`AC-YYYY-###`) | Stabil identifikator; doğrulama zamanı tətbiq edilir. |
| `submitted_at_unix_ms` | u64 | Unix dövründən bəri millisaniyələr. |
| `language` | simli | BCP‑47 etiketi (`"en"`, `"ja-JP"` və s.). |
| `action` | enum (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Nazirlikdən tədbir tələb olunub. |
| `summary.title` | simli | ≤256 simvol tövsiyə olunur. |
| `summary.motivation` | simli | Niyə tədbir tələb olunur. |
| `summary.expected_impact` | simli | Fəaliyyət qəbul edildikdə nəticələr. |
| `tags[]` | kiçik sətirlər | Könüllü triaj etiketləri. İcazə verilən dəyərlər: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `policy-escalation`, ```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```00, Norito. |
| `targets[]` | obyektlər | Bir və ya daha çox hash ailə girişi (aşağıya bax). |
| `evidence[]` | obyektlər | Bir və ya bir neçə sübut əlavəsi (aşağıya bax). |
| `submitter.name` | simli | Görünən ad və ya təşkilat. |
| `submitter.contact` | simli | E-poçt, Matrix sapı və ya telefon; ictimai tablosundan redaktə edilmişdir. |
| `submitter.organization` | string (isteğe bağlı) | Rəyçi UI-də görünür. |
| `submitter.pgp_fingerprint` | string (isteğe bağlı) | 40 hex böyük hərf barmaq izi. |
| `duplicates[]` | strings | Əvvəllər təqdim edilmiş təklif ID-lərinə əlavə istinadlar. |

### Hədəf girişləri (`targets[]`)

Hər bir hədəf təklifin istinad etdiyi hash ailəsi həzmini təmsil edir.

| Sahə | Təsvir | Doğrulama |
|-------|-------------|------------|
| `label` | Rəyçi konteksti üçün dost ad. | Boş deyil. |
| `hash_family` | Hash identifikatoru (`blake3-256`, `sha256` və s.). | ASCII hərfləri/rəqəmləri/`-_.`, ≤48 simvol. |
| `hash_hex` | Həzm kiçik altı hərflə kodlanmışdır. | ≥16 bayt (32 hex simvol) və etibarlı hex olmalıdır. |
| `reason` | Qısa təsviri niyə həzm etmək lazımdır. | Boş deyil. |

Təsdiqləyici eyni müddət ərzində dublikat `hash_family:hash_hex` cütlərini rədd edir
eyni barmaq izi artıq mövcud olduqda təklif və hesabatlar ziddiyyət təşkil edir
dublikat reyestr (aşağıya baxın).

### Sübut əlavələri (`evidence[]`)

Rəyçilərin dəstəkləyici kontekst əldə edə biləcəyi sübut qeydləri sənədi.| Sahə | Növ | Qeydlər |
|-------|------|-------|
| `kind` | enum (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Həzm tələblərini müəyyənləşdirir. |
| `uri` | simli | HTTP(S) URL, Torii hal ID və ya SoraFS URI. |
| `digest_blake3_hex` | simli | `sorafs-cid` və `attachment` növləri üçün tələb olunur; başqaları üçün isteğe bağlıdır. |
| `description` | simli | Rəyçilər üçün əlavə sərbəst formada mətn. |

### Reyestrinin dublikatı

Operatorlar dublikatların qarşısını almaq üçün mövcud barmaq izlərinin reyestrini saxlaya bilərlər
hallar. Təsdiqləyici aşağıdakı formada JSON faylını qəbul edir:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

Təklif hədəfi girişə uyğun gələndə, təsdiqləyici bu hallar istisna olmaqla, fəaliyyəti dayandırır
`--allow-registry-conflicts` göstərilib (xəbərdarlıqlar hələ də verilir).
[`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) istifadə edin
dublikata çarpaz istinad edən referendum üçün hazır xülasə yaratmaq
reyestr və siyasət snapshotları.

## CLI istifadəsi

Tək təklifi silkələyin və onu dublikat reyestrlə yoxlayın:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

Dublikat hitləri xəbərdarlıqlara endirmək üçün `--allow-registry-conflicts` keçin
tarixi yoxlamaların aparılması.

CLI eyni Norito sxeminə və göndərilən doğrulama köməkçilərinə əsaslanır.
`iroha_data_model`, beləliklə SDK/portallar `AgendaProposalV1::validate`-dən təkrar istifadə edə bilər
ardıcıl davranış metodu.

## Çeşidləmə CLI (MINFO-2b)

Yol xəritəsi arayışı: **MINFO-2b — Çox yuvalı çeşidləmə və audit jurnalı.**

Gündəlik Şurasının siyahısı indi vətəndaşlar üçün deterministik çeşidləmə ilə idarə olunur
hər tirajı müstəqil yoxlaya bilər. Yeni əmrdən istifadə edin:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — hər bir uyğun üzvü təsvir edən JSON faylı:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  Nümunə faylı burada yaşayır
  `docs/examples/ministry/agenda_council_roster.json`. Könüllü sahələr (rol,
  təşkilat, əlaqə, metadata) auditorlar üçün Merkle yarpağında tutulur
  heç-heçəni bəsləyən siyahı sübut edə bilər.

- `--slots` — doldurulacaq şura yerlərinin sayı.
- `--seed` — 32 baytlıq BLAKE3 toxumu (64 kiçik hex simvol)
  püşkatma üçün idarəetmə protokolları.
- `--out` — isteğe bağlı çıxış yolu. Buraxıldıqda, JSON xülasəsi çap olunur
  stdout.

### Çıxış xülasəsi

Komanda `SortitionSummary` JSON blob yayır. Nümunə çıxışı burada saxlanılır
`docs/examples/ministry/agenda_sortition_summary_example.json`. Əsas sahələr:

| Sahə | Təsvir |
|-------|-------------|
| `algorithm` | Çeşidləmə etiketi (`agenda-sortition-blake3-v1`). |
| `roster_digest` | Siyahı faylının BLAKE3 + SHA-256 həzmləri (eyni üzv siyahısı üzərində auditlərin işlədiyini təsdiqləmək üçün istifadə olunur). |
| `seed_hex` / `slots` | Auditorların tirajı təkrar edə bilməsi üçün CLI daxiletmələrini əks etdirin. |
| `merkle_root_hex` | Siyahı Merkle ağacının kökü (`hash_node`/`hash_leaf` `xtask/src/ministry_agenda.rs`-də köməkçilər). |
| `selected[]` | Kanonik üzv metadata, uyğun indeks, orijinal siyahı indeksi, deterministik çəkiliş entropiyası, yarpaq hashı və Merkle sübut bacıları daxil olmaqla hər bir yuva üçün girişlər. |

### Heç-heçənin təsdiqlənməsi1. `roster_path` tərəfindən istinad edilən siyahını götürün və onun BLAKE3/SHA-256-nı yoxlayın
   həzmlər xülasəyə uyğun gəlir.
2. CLI-ni eyni toxum/slot/siyahı ilə yenidən işə salın; nəticədə `selected[].member_id`
   sifariş dərc olunmuş xülasəyə uyğun olmalıdır.
3. Xüsusi üzv üçün seriallaşdırılmış üzv JSON-dan istifadə edərək Merkle yarpağını hesablayın
   (`norito::json::to_vec(&sortition_member)`) və hər bir sübut hash-də qatlayın. Final
   həzm `merkle_root_hex`-ə bərabər olmalıdır. Nümunə xülasəsindəki köməkçi göstərir
   `eligible_index`, `leaf_hash_hex` və `merkle_proof[]`-i necə birləşdirmək olar.

Bu artefaktlar yoxlanıla bilən təsadüfilik üçün MINFO-2b tələbini ödəyir,
k-of-m seçimi və zəncirvari API simli olana qədər yalnız audit jurnallarını əlavə edin.

## Doğrulama xətası arayışı

`AgendaProposalV1::validate` `AgendaProposalValidationError` variantlarını yayır
faydalı yük linting uğursuz zaman. Aşağıdakı cədvəl ən çox yayılmışları ümumiləşdirir
Portal rəyçiləri CLI çıxışını icra edilə bilən təlimata çevirə bilsinlər.| Səhv | Məna | Təmir |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | `version` faydalı yükü təsdiqləyicinin dəstəklənən sxemindən fərqlənir. | Ən son sxem paketindən istifadə edərək JSON-u bərpa edin ki, versiya `expected`-ə uyğun olsun. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` boşdur və ya `AC-YYYY-###` formasında deyil. | Yenidən təqdim etməzdən əvvəl sənədləşdirilmiş formata uyğun unikal identifikatoru doldurun. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` sıfırdır və ya yoxdur. | Təqdim olunan vaxt damğasını Unix millisaniyələrində qeyd edin. |
| `InvalidLanguageTag { value }` | `language` etibarlı BCP‑47 teqi deyil. | `en`, `ja-JP` kimi standart teq və ya BCP‑47 tərəfindən tanınan başqa bir dil istifadə edin. |
| `MissingSummaryField { field }` | `summary.title`, `.motivation` və ya `.expected_impact`-dən biri boşdur. | Göstərilən xülasə sahəsi üçün boş olmayan mətn təqdim edin. |
| `MissingSubmitterField { field }` | `submitter.name` və ya `submitter.contact` yoxdur. | Çatışmayan təqdim edən metadata təqdim edin ki, rəyçilər təklif edənlə əlaqə saxlaya bilsinlər. |
| `InvalidTag { value }` | `tags[]` giriş icazəli siyahıda deyil. | Teqi silin və ya sənədləşdirilmiş dəyərlərdən birinə dəyişdirin (`csam`, `malware` və s.). |
| `MissingTargets` | `targets[]` massivi boşdur. | Ən azı bir hədəf hash ailəsi girişini təmin edin. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | Hədəf girişində `label` və ya `reason` sahələri yoxdur. | Yenidən göndərməzdən əvvəl indeksləşdirilmiş giriş üçün tələb olunan sahəni doldurun. |
| `InvalidHashFamily { index, value }` | Dəstəklənməyən `hash_family` etiketi. | Haş ailə adlarını ASCII alfasayısal və `-_` ilə məhdudlaşdırın. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | Digest etibarlı hex deyil və ya 16 baytdan qısadır. | İndekslənmiş hədəf üçün kiçik hərflərlə hex həzm (≥32 hex simvol) təmin edin. |
| `DuplicateTarget { index, fingerprint }` | Hədəf həzm daha əvvəlki giriş və ya qeyd barmaq izini təkrarlayır. | Dublikatları çıxarın və ya dəstəkləyici sübutları bir hədəfə birləşdirin. |
| `MissingEvidence` | Heç bir sübut əlavəsi təqdim edilməmişdir. | Reproduksiya materialı ilə əlaqəli ən azı bir sübut qeydi əlavə edin. |
| `MissingEvidenceUri { index }` | Sübut girişində `uri` sahəsi yoxdur. | İndekslənmiş sübut girişi üçün alına bilən URI və ya iş identifikatorunu təmin edin. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | Həzm tələb edən sübut girişi (SoraFS CID və ya qoşma) yoxdur və ya etibarsız `digest_blake3_hex`-ə malikdir. | İndekslənmiş giriş üçün 64 simvoldan ibarət kiçik hərf BLAKE3 həzmini təmin edin. |

## Nümunələr

- `docs/examples/ministry/agenda_proposal_example.json` - kanonik,
  iki sübut əlavəsi ilə lint-təmiz təklif yükü.
- `docs/examples/ministry/agenda_duplicate_registry.json` — başlanğıc reyestri
  tək BLAKE3 barmaq izini və əsaslandırmanı ehtiva edir.

Portal alətlərini birləşdirərkən və ya CI yazarkən bu faylları şablon kimi yenidən istifadə edin
avtomatlaşdırılmış təqdimatları yoxlayır.