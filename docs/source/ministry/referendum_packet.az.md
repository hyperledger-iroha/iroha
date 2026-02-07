---
lang: az
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# Referendum Paketi İş axını (MINFO-4)

Yol xəritəsi elementi **MINFO-4 — Baxış panelində və referendum sintezatorunda** indi
yeni `ReferendumPacketV1` Norito sxemi və CLI köməkçiləri ilə yerinə yetirilir
aşağıda təsvir edilmişdir. İş axını siyasət-münsiflər heyəti üçün tələb olunan hər bir artefaktı birləşdirir
idarəetmə, auditorlar və şəffaflıq üçün vahid JSON sənədinə səs verir
portallar sübutları deterministik şəkildə təkrarlaya bilər.

## Girişlər

1. **Gündəm təklifi** — `cargo xtask ministry-agenda validate` üçün istifadə edilən eyni JSON.
2. **Könüllü brifinqləri** — lintingdən sonra hazırlanmış məlumat dəsti
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI moderasiya manifest** — idarəetmə tərəfindən imzalanmış `ModerationReproManifestV1`.
4. **Çeşidləmə xülasəsi** — tərəfindən buraxılan deterministik artefakt
   `cargo xtask ministry-agenda sortition`. JSON izləyir
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) beləliklə idarəetmə
   POP snapshot həzmini və gözləmə siyahısı/failover naqillərini təkrarlayın.
5. **Təsir hesabatı** — hash-ailə/hesabat vasitəsilə yaradılır
   `cargo xtask ministry-agenda impact`.

## CLI istifadəsi

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` alt əmri neytral xülasə sintezatorunu (MINFO-4a) işlədir, təkrar istifadə edir.
mövcud könüllü armaturları və məhsulu aşağıdakılarla zənginləşdirir:

- `ReferendumSortitionEvidence` - alqoritm, toxum və siyahıdan həzmlər
  çeşidləmə artefaktı.
- `ReferendumPanelist[]` — seçilmiş hər bir şura üzvü və Merkle sübutu
  onların tirajını yoxlamaq lazımdır.
- `ReferendumImpactSummary` - hər ailəyə görə ümumi və münaqişə siyahıları
  təsir hesabatı.

Hələ də müstəqil `ReviewPanelSummaryV1` ehtiyacınız olduqda `--summary-out` istifadə edin
fayl; əks halda paket xülasəni `review_summary` altında yerləşdirir.

## Çıxış strukturu

`ReferendumPacketV1` yaşayır
`crates/iroha_data_model/src/ministry/mod.rs` və SDK-larda mövcuddur.
Əsas bölmələrə aşağıdakılar daxildir:

- `proposal` — orijinal `AgendaProposalV1` obyekti.
- `review_summary` — MINFO-4a tərəfindən yayılan balanslaşdırılmış xülasə.
- `sortition` / `panelists` - oturan şura üçün təkrarlana bilən sübutlar.
- `impact_summary` — hash ailəsi üçün dublikat/siyasət münaqişəsi sübutu.

Tam nümunə üçün baxın `docs/examples/ministry/referendum_packet_example.json`.
Yaradılmış paketi imzalanmış AI ilə birlikdə hər referendum faylına əlavə edin
aydınlıq və şəffaflıq artefaktları vurğulananlar bölməsində istinad edilir.