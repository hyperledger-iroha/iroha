---
lang: uz
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo boshqaruv paketi shabloni (Yo'l xaritasi F1)

Yoʻl xaritasi elementi talab qiladigan artefaktlar toʻplamini tayyorlashda ushbu shablondan foydalaning
F1 (repo hayotiy tsikli hujjatlari va asboblari). Maqsad - sharhlovchilarni topshirish a
Har bir kirish, xesh va dalillar to'plamini ko'rsatadigan yagona Markdown fayli
boshqaruv kengashi taklifda ko'rsatilgan baytlarni takrorlashi mumkin.

> Shablonni o'zingizning dalillar katalogingizga nusxalang (masalan
> `artifacts/finance/repo/2026-03-15/packet.md`), to'ldirgichlarni almashtiring va
> uni quyida havola qilingan xeshlangan artefaktlar yonida bajaring/yuklang.

## 1. Metadata

| Maydon | Qiymat |
|-------|-------|
| Kelishuv/identifikatorni o'zgartirish | `<repo-yyMMdd-XX>` |
| Tayyorlangan / sana | `<desk lead> – 2026-03-15T10:00Z` |
| Ko'rib chiqqan | `<dual-control reviewer(s)>` |
| Turni o'zgartirish | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Vasiy(lar)i | `<custodian id(s)>` |
| Bog'langan taklif / referendum | `<governance ticket id or GAR link>` |
| Dalillar katalogi | ``artifacts/finance/repo/<slug>/`` |

## 2. Yo'riqnomaning foydali yuklari

Stollar orqali imzolangan Norito bosqichli ko'rsatmalarini yozib oling.
`iroha app repo ... --output`. Har bir yozuv emissiya qilingan xeshni o'z ichiga olishi kerak
fayl va ovoz berishdan keyin taqdim etiladigan harakatning qisqacha tavsifi
o'tadi.

| Harakat | Fayl | SHA-256 | Eslatmalar |
|--------|------|---------|-------|
| Boshlash | `instructions/initiate.json` | `<sha256>` | Stol + kontragent tomonidan tasdiqlangan naqd pul/garov oyoqlarini o'z ichiga oladi. |
| Marja chaqiruvi | `instructions/margin_call.json` | `<sha256>` | Chaqiruvga sabab bo‘lgan kadans + ishtirokchi identifikatorini yozib oladi. |
| Dam olish | `instructions/unwind.json` | `<sha256>` | Shartlar bajarilgandan keyin teskari oyoqning isboti. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Vasiylik bildirishnomalari (faqat uch tomonli)

Repo `--custodian` dan foydalanganda ushbu bo'limni to'ldiring. Boshqaruv paketi
har bir qo'riqchi tomonidan imzolangan tasdiqnoma va uning xeshini o'z ichiga olishi kerak
`docs/source/finance/repo_ops.md` §2.8 da havola qilingan fayl.

| Qo'riqchi | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Himoya oynasi, marshrutlash hisobi va burg'ulash kontaktlarini qamrab olgan imzolangan SLA. |

> Tasdiqni boshqa dalillar yonida saqlang (`artifacts/finance/repo/<slug>/`)
> shuning uchun `scripts/repo_evidence_manifest.py` faylni xuddi shu daraxtga yozadi
> bosqichli ko'rsatmalar va konfiguratsiya parchalari. Qarang
> `docs/examples/finance/repo_custodian_ack_template.md` to'ldirishga tayyor
> boshqaruvni tasdiqlovchi shartnomaga mos keladigan shablon.

## 3. Konfiguratsiya parchasi

Klasterga tushadigan `[settlement.repo]` TOML blokini joylashtiring (jumladan,
`collateral_substitution_matrix`). Xeshni parchaning yonida shunday saqlang
auditorlar repo bron qilish paytida faol bo'lgan ish vaqti siyosatini tasdiqlashlari mumkin
tasdiqlandi.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Tasdiqdan keyingi konfiguratsiya suratlari

Referendum yoki boshqaruv ovoz berish tugagandan so'ng va `[settlement.repo]`
o'zgartirish amalga oshirildi, har bir tengdoshdan `/v1/configuration` suratlarini oling
auditorlar tasdiqlangan siyosat klaster bo'ylab amalda ekanligini isbotlashlari mumkin (qarang
Dalillar ish jarayoni uchun `docs/source/finance/repo_ops.md` §2.9).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Tengdosh / manba | Fayl | SHA-256 | Blok balandligi | Eslatmalar |
|-------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot konfiguratsiyani ishga tushirgandan so'ng darhol olingan. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` bosqichli TOMLga mos kelishini tasdiqlaydi. |

`hashes.txt` (yoki ekvivalenti) da dayjestlarni tengdosh identifikatorlari bilan birga yozib oling
xulosa) shunday qilib, sharhlovchilar oʻzgarishlarni qaysi tugunlar qabul qilganligini kuzatishi mumkin. Suratlar
TOML snippeti yonida `config/peers/` ostida yashaydi va olinadi
avtomatik ravishda `scripts/repo_evidence_manifest.py` tomonidan.

## 4. Deterministik sinov artefaktlari

Quyidagilardan eng so'nggi natijalarni biriktiring:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Jurnal to'plamlari yoki CI tomonidan ishlab chiqarilgan JUnit XML uchun fayl yo'llari + xeshlarni yozib oling
tizimi.

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Hayotiy tsiklni tekshirish jurnali | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` chiqishi bilan olingan. |
| Integratsiya sinovlari jurnali | `tests/repo_integration.log` | `<sha256>` | O'zgartirish + marja kadansini qoplashni o'z ichiga oladi. |

## 5. Lifecycle Proof Snapshot

Har bir paket eksport qilingan deterministik hayot tsiklining suratini o'z ichiga olishi kerak
`repo_deterministic_lifecycle_proof_matches_fixture`. bilan jabduqni boshqaring
Eksport tugmalari yoqilgan, shuning uchun sharhlovchilar JSON ramkasini farqlashi va unga qarshi hazm qilishlari mumkin
`crates/iroha_core/tests/fixtures/` da kuzatilgan armatura (qarang
`docs/source/finance/repo_ops.md` §2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Yoki moslamalarni qayta tiklash va ularni nusxalash uchun mahkamlangan yordamchidan foydalaning
dalillar to'plami bir bosqichda:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Kanonik hayot aylanishi ramkasi isbotlovchi jabduqlar tomonidan chiqariladi. |
| Dijest fayli | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` dan aks ettirilgan katta olti burchakli dayjest; o'zgarmagan holda ham biriktiring. |

## 6. Dalillar manifesti

Auditorlar tekshirishi uchun barcha dalillar katalogi uchun manifestni yarating
arxivni ochmasdan xeshlar. Yordamchi tasvirlangan ish jarayonini aks ettiradi
`docs/source/finance/repo_ops.md` §3.2 da.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Dalillar manifest | `manifest.json` | `<sha256>` | Boshqaruv biletiga/referendum eslatmalariga nazorat summasini kiriting. |

## 7. Telemetriya va voqealar surati

Tegishli `AccountEvent::Repo(*)` yozuvlarini va har qanday asboblar panelini yoki CSVni eksport qiling
`docs/source/finance/repo_ops.md` da havola qilingan eksport. Fayllarni yozib oling +
Bu yerda xeshlar, shuning uchun sharhlovchilar to'g'ridan-to'g'ri dalillarga o'tishlari mumkin.

| Eksport | Fayl | SHA-256 | Eslatmalar |
|--------|------|---------|-------|
| Repo hodisalari JSON | `evidence/repo_events.ndjson` | `<sha256>` | Xom Torii hodisa oqimi stol hisoblariga filtrlangan. |
| Telemetriya CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo margin paneli yordamida Grafana dan eksport qilindi. |

## 8. Tasdiqlashlar va imzolar

- **Dual-control signers:** `<names + timestamps>`
- **GAR / daqiqalar dayjesti:** Imzolangan GAR PDF hujjatining `<sha256>` yoki yuklangan daqiqalar.
- **Saqlash joyi:** `governance://finance/repo/<slug>/packet/`

## 9. Tekshirish ro'yxati

Har bir elementni tugallangandan keyin belgilang.

- [ ] Yo'riqnomaning foydali yuklari bosqichma-bosqich, xeshlangan va biriktirilgan.
- [ ] Konfiguratsiya parchasi xeshi qayd etildi.
- [ ] Deterministik test jurnallari olingan + xeshlangan.
- [ ] Hayot siklining surati + dayjest eksport qilindi.
- [ ] Dalil manifest yaratildi va xesh qayd etildi.
- [ ] Hodisa/telemetriya eksporti yozib olindi + xeshlangan.
- [ ] Ikkilamchi boshqaruv tasdiqlari arxivlandi.
- [ ] GAR/daqiqa yuklangan; yuqorida qayd etilgan dayjest.

Ushbu shablonni har bir paket bilan birga saqlash DAG boshqaruvini saqlab qoladi
deterministik va auditorlarga repo hayot aylanishi uchun portativ manifestni taqdim etadi
qarorlar.