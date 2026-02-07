---
lang: uz
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

# Referendum paketining ish jarayoni (MINFO-4)

Yoʻl xaritasi bandi **MINFO-4 — Koʻrib chiqish paneli va referendum sintezatorida** hozir
yangi `ReferendumPacketV1` Norito sxemasi va CLI yordamchilari tomonidan bajarilgan
quyida tasvirlangan. Ish jarayoni hakamlar hay'ati siyosati uchun zarur bo'lgan har bir artefaktni birlashtiradi
boshqaruv, auditorlar va shaffoflik uchun yagona JSON hujjatiga ovoz beradi
portallar dalillarni deterministik tarzda takrorlashi mumkin.

## Kirishlar

1. **Kun tartibi taklifi** — xuddi shu JSON `cargo xtask ministry-agenda validate` uchun ishlatilgan.
2. **Ko'ngillilar uchun qisqacha ma'lumotlar** — lintingdan so'ng yaratilgan ma'lumotlar to'plami
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI moderatsiyasi manifest** — boshqaruv tomonidan imzolangan `ModerationReproManifestV1`.
4. **Tartiblash xulosasi** — chiqaradigan deterministik artefakt
   `cargo xtask ministry-agenda sortition`. JSON quyidagicha
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) shuning uchun boshqaruv
   POP snapshot dayjestini va kutish ro'yxatini / uzilish simlarini takrorlang.
5. **Ta'sir hisoboti** — hash-family/hisobot orqali yaratilgan
   `cargo xtask ministry-agenda impact`.

## CLI foydalanish

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

`packet` kichik buyrug'i neytral xulosa sintezatorini (MINFO-4a) ishga tushiradi, qayta foydalanadi
mavjud ixtiyoriy armatura va mahsulotlarni quyidagilar bilan boyitadi:

- `ReferendumSortitionEvidence` - algoritm, urug'lik va ro'yxat hazm qilishlari
  saralash artefakti.
- `ReferendumPanelist[]` - har bir tanlangan kengash a'zosi va Merkle isboti
  ularning qur'asini tekshirish kerak edi.
- `ReferendumImpactSummary` - har bir oila uchun xesh yig'indisi va qarama-qarshiliklar ro'yxati
  ta'sir hisoboti.

Hali ham mustaqil `ReviewPanelSummaryV1` kerak bo'lganda `--summary-out` dan foydalaning
fayl; aks holda paket xulosani `review_summary` ostida joylashtiradi.

## Chiqish tuzilishi

`ReferendumPacketV1` yashaydi
`crates/iroha_data_model/src/ministry/mod.rs` va SDKlarda mavjud.
Asosiy bo'limlarga quyidagilar kiradi:

- `proposal` — asl `AgendaProposalV1` obyekti.
- `review_summary` - MINFO-4a tomonidan chiqarilgan muvozanatli xulosa.
- `sortition` / `panelists` - o'tirgan kengash uchun takrorlanadigan dalillar.
- `impact_summary` - har bir xesh oilasi uchun takroriy/siyosat ziddiyatli dalillari.

Toʻliq namuna uchun `docs/examples/ministry/referendum_packet_example.json` ga qarang.
Yaratilgan paketni imzolangan AI bilan birga har bir referendum fayliga ilova qiling
diqqatga sazovor joylar bo'limida havola qilingan manifest va shaffoflik artefaktlari.