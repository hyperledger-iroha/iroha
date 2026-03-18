---
lang: az
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
translator: machine-google-reviewed
---

# Nəzərdən keçirmə panelinin xülasəsi (MINFO-4a)

Yol xəritəsi elementi **MINFO-4a — Neytral xülasə generatoru** qəbul edilmiş gündəm təklifini, könüllülərin qısa korpusunu və təsdiq edilmiş AI moderasiya manifestini neytral referendum xülasəsinə çevirən təkrarlana bilən iş axını tələb edir. Çatdırılmalı olmalıdır:

- Nəticəni Norito strukturu (`ReviewPanelSummaryV1`) kimi qeyd edin ki, idarəetmə onu manifestlər və seçki bülletenləri ilə birlikdə arxivləşdirə bilsin.
- Nəzərdən keçirilən paneldə balanslaşdırılmış dəstək/müxalif əhatə olmadıqda və ya faktlarda sitatlar çatışmırsa, mənbə materialını tündləyin.
- Siyasi münsiflər heyətinin səsvermədən əvvəl həm avtomatlaşdırılmış, həm də insan kontekstini görməsini təmin edərək, hər bir məqamda AI manifestinə və təklif sübut paketinə istinad edin.

## CLI istifadəsi

İş axını `cargo xtask`-in bir hissəsi kimi göndərilir:

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

Tələb olunan girişlər:

1. `--proposal` – `AgendaProposalV1`-ə uyğun JSON faydalı yükü. Köməkçi xülasə yaratmazdan əvvəl sxemi təsdiqləyir.
2. `--volunteer` – `docs/source/ministry/volunteer_brief_template.md`-i izləyən JSON könüllü brifinqləri. Mövzudan kənar yazılar avtomatik olaraq nəzərə alınmır.
3. `--ai-manifest` – İdarəetmə tərəfindən imzalanmış `ModerationReproManifestV1` məzmunu yoxlayan AI komitəsini təsvir edir.
4. `--panel-round` – Cari nəzərdən keçirmə mərhələsi üçün identifikator (`RP-YYYY-##`).
5. `--output` – stdout-a axın etmək üçün təyinat faylı və ya `-`. Təklif dilini ləğv etmək üçün `--language` və tarixçəni doldurarkən deterministik Unix vaxt damğası (millisaniyə) təmin etmək üçün `--generated-at` istifadə edin.

Bağımsız xülasə yaradıldıqdan sonra, işə salın
[`cargo xtask ministry-panel packet`](referendum_packet.md) yığmaq üçün köməkçi
tam referendum dosyesi (`ReferendumPacketV1`). Təchizat
`--summary-out` paket əmri eyni xülasə faylını saxlayarkən
onu aşağı axın istehlakçılar üçün paket obyektinin içərisinə yerləşdirmək.

### `ministry-transparency ingest` vasitəsilə avtomatlaşdırma

Artıq rüblük sübut paketləri üçün `cargo xtask ministry-transparency ingest` işlədən komandalar indi baxış panelinin xülasəsini eyni boru xəttinə birləşdirə bilər:

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

Bütün dörd `--panel-*` bayrağı birlikdə təchiz edilməlidir (və `--volunteer` tələb olunur). Komanda nəzərdən keçirmə panelinin xülasəsini `--panel-summary-out`-ə göndərir, təhlil edilmiş faydalı yükü qəbul edilmiş snapşotun içərisinə daxil edir və yoxlama məbləğini qeyd edir ki, aşağı axın alətləri sübutları təsdiq etsin.

## Linting və uğursuzluq rejimləri

`cargo xtask ministry-panel synthesize` xülasəni yazmazdan əvvəl aşağıdakı invariantları tətbiq edir:

- **Balanslı mövqelər:** ən azı bir dəstək brifinqi və bir müxalif brifinq olmalıdır. Çatışmayan əhatə dairəsi təsviri xəta ilə işi dayandırır.
- **Sitatların əhatə dairəsi:** vurğulanan məqamlar yalnız sitatların daxil olduğu fakt sıralarından hazırlanır. Çatışmayan sitatlar heç vaxt quruluşa mane olmur, lakin təsirə məruz qalan hər bir brif çıxışda `warnings[]` altında verilmişdir.
- **Vurğulanan arayışlar:** hər bir vurğuya (a) könüllü fakt sıra(ları), (b) AI manifest ID-si və (c) paketin həmişə imzalanmış artefaktlarla əlaqə saxlaması üçün təklifdən ilk sübut əlavəsinə istinadlar daxildir.Hər hansı bir yoxlama uğursuz olarsa, komanda sıfırdan fərqli bir statusla çıxır və problemli qeydə işarə edir. Uğurlu qaçışlar `ReviewPanelSummaryV1` sxeminə uyğun gələn və idarəetmə manifestlərinə daxil edilə bilən JSON faylı yazır.

## Çıxış strukturu

`ReviewPanelSummaryV1` `crates/iroha_data_model/src/ministry/mod.rs`-də yaşayır və `iroha_data_model` qutusu vasitəsilə hər bir istehlakçıya təqdim olunur. Əsas bölmələrə aşağıdakılar daxildir:

- `overview` – Siyasət jürisi paketi üçün başlıq, neytral xülasə cümləsi və qərar konteksti.
- `stance_distribution` – Hər mövqe üzrə brifinqlərin və fakt sıralarının sayı. Aşağı axın panelləri dərc etməzdən əvvəl əhatə dairəsini təsdiqləmək üçün bunu oxuyur.
- `highlights` – Tam ixtisaslı sitatlarla hər mövqe üçün iki faktın xülasəsi.
- `ai_manifest` – Təkrarlanma manifestindən çıxarılan metadata (manifest UUID, qaçış versiyası, hədlər).
- `volunteer_references` – Audit üçün qısa statistik məlumatlar (dil, mövqe, sətirlər, istinad edilmiş sətirlər).
- `warnings` – Atlanmış elementləri təsvir edən sərbəst formada lint mesajları (məsələn, itkin sitatlarla fakt sıraları).

## Nümunə

`docs/examples/ministry/review_panel_summary_example.json` köməkçi ilə hazırlanmış tam nümunəni ehtiva edir. O, balanslaşdırılmış dəstək/müxalif əhatə dairəsi, sitat naqilləri, açıq istinadlar və diqqət çəkən məqamlara yüksəldilə bilməyən fakt sıraları üçün xəbərdarlıq sətirlərini nümayiş etdirir. Neytral xülasəni istehlak etməli olan tablosunu, idarəetmə manifestlərini və ya SDK alətlərini genişləndirərkən ondan istifadə edin.

> **İpucu:** imzalanmış süni intellekt manifestinə və könüllü qısa həzminə əlavə olaraq referendum sübutlar paketinə daxil olun ki, siyasət münsiflər heyəti baxış panelinin istinad etdiyi hər bir artefaktı yoxlaya bilsin.