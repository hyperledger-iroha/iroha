---
lang: az
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

# Repo İdarəetmə Paket Şablonu (Yol Xəritəsi F1)

Yol xəritəsi elementinin tələb etdiyi artefakt paketini hazırlayarkən bu şablondan istifadə edin
F1 (repo həyat dövrü sənədləri və alətlər). Məqsəd rəyçiləri təhvil verməkdir a
hər bir giriş, hash və sübut paketini sadalayan tək Markdown faylı
idarəetmə şurası təklifdə istinad edilən baytları təkrarlaya bilər.

> Şablonu öz sübut kataloqunuza köçürün (məsələn
> `artifacts/finance/repo/2026-03-15/packet.md`), yer tutucuları dəyişdirin və
> onu aşağıda istinad edilən hash edilmiş artefaktların yanında yerinə yetirin/yükləyin.

## 1. Metadata

| Sahə | Dəyər |
|-------|-------|
| Razılaşma/dəyişiklik identifikatoru | `<repo-yyMMdd-XX>` |
| / tarix tərəfindən hazırlanmışdır | `<desk lead> – 2026-03-15T10:00Z` |
| Nəzərdən keçirən | `<dual-control reviewer(s)>` |
| Növü dəyişdir | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Qəyyum(lar) | `<custodian id(s)>` |
| Əlaqəli təklif / referendum | `<governance ticket id or GAR link>` |
| Sübut kataloqu | ``artifacts/finance/repo/<slug>/`` |

## 2. Təlimat yükləri

Masaların vasitəsilə imzalanan mərhələli Norito təlimatlarını qeyd edin
`iroha app repo ... --output`. Hər bir giriş emissiyanın hashını daxil etməlidir
fayl və səsvermədən sonra təqdim ediləcək hərəkətin qısa təsviri
keçir.

| Fəaliyyət | Fayl | SHA-256 | Qeydlər |
|--------|------|---------|-------|
| Təşəbbüs | `instructions/initiate.json` | `<sha256>` | Masa + qarşı tərəf tərəfindən təsdiqlənmiş pul/girov ayaqlarını ehtiva edir. |
| Marja zəngi | `instructions/margin_call.json` | `<sha256>` | Zəngə səbəb olan kadans + iştirakçı identifikatorunu çəkir. |
| Açın | `instructions/unwind.json` | `<sha256>` | Şərtlər yerinə yetirildikdən sonra tərs ayağın sübutu. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Qəyyumun təşəkkürləri (yalnız üçtərəfli)

Repo `--custodian` istifadə etdikdə bu bölməni tamamlayın. İdarəetmə paketi
hər bir qəyyumun imzalı təsdiqi və hash daxil edilməlidir
`docs/source/finance/repo_ops.md` §2.8-də istinad edilən fayl.

| Qəyyum | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Qəyyumluq pəncərəsi, marşrutlaşdırma hesabı və qazma kontaktını əhatə edən imzalanmış SLA. |

> Təsdiqi digər sübutların yanında saxlayın (`artifacts/finance/repo/<slug>/`)
> beləliklə, `scripts/repo_evidence_manifest.py` faylı eyni ağacda qeyd edir
> mərhələli təlimatlar və konfiqurasiya fraqmentləri. Bax
> Doldurmağa hazır üçün `docs/examples/finance/repo_custodian_ack_template.md`
> idarəetmə sübutu müqaviləsinə uyğun gələn şablon.

## 3. Konfiqurasiya Snippet

Klasterə enəcək `[settlement.repo]` TOML blokunu yapışdırın (o cümlədən
`collateral_substitution_matrix`). Haşi-ni fraqmentin yanında saxlayın
auditorlar repo sifarişi zamanı aktiv olan icra müddəti siyasətini təsdiq edə bilərlər
təsdiq olundu.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Təsdiqdən Sonra Konfiqurasiya Snapshotları

Referendum və ya idarəetmə səsverməsi başa çatdıqdan və `[settlement.repo]`
Dəyişiklik yayıldı, hər bir həmyaşıddan `/v1/configuration` anlıq görüntüləri çəkin
auditorlar təsdiq edilmiş siyasətin bütün klasterdə canlı olduğunu sübut edə bilərlər (bax
Sübut iş axını üçün `docs/source/finance/repo_ops.md` §2.9).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / mənbə | Fayl | SHA-256 | Blok hündürlüyü | Qeydlər |
|-------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot konfiqurasiya təqdim edildikdən dərhal sonra çəkildi. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]`-in mərhələli TOML ilə uyğunluğunu təsdiq edir. |

`hashes.txt`-də (və ya ekvivalentində) həmyaşıd identifikatorları ilə birlikdə həzmləri qeyd edin
xülasə) beləliklə rəyçilər hansı qovşaqların dəyişikliyi qəbul etdiyini izləyə bilsinlər. Ani görüntülər
TOML fraqmentinin yanında `config/peers/` altında yaşayır və götürüləcək
avtomatik olaraq `scripts/repo_evidence_manifest.py` tərəfindən.

## 4. Deterministik Test Artefaktları

Ən son çıxışları əlavə edin:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Jurnal paketləri və ya CI tərəfindən hazırlanmış JUnit XML üçün fayl yollarını + hashları qeyd edin
sistemi.

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Həyat dövrü sübut jurnalı | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` çıxışı ilə çəkilib. |
| İnteqrasiya test jurnalı | `tests/repo_integration.log` | `<sha256>` | Əvəzetmə + marja kadans əhatəsi daxildir. |

## 5. Lifecycle Proof Snapshot

Hər bir paketə ixrac edilən deterministik həyat dövrü snapshotı daxil edilməlidir
`repo_deterministic_lifecycle_proof_matches_fixture`. ilə kəməri işə salın
ixrac düymələri aktivləşdirilib ki, rəyçilər JSON çərçivəsini fərqləndirə və ona qarşı həzm edə bilsinlər
armatur `crates/iroha_core/tests/fixtures/`-də izlənilir (bax
`docs/source/finance/repo_ops.md` §2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Və ya armaturları bərpa etmək və onları özünüzə köçürmək üçün bərkidilmiş köməkçidən istifadə edin
bir addımda sübut paketi:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Kanonik həyat dövrü çərçivəsi sübut qoşqu tərəfindən yayılır. |
| Digest faylı | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`-dən əks olunmuş böyük hərf altıbucaqlı həzm; dəyişməmiş olsa belə əlavə edin. |

## 6. Sübut Manifesti

Bütün sübut kataloqu üçün manifest yaradın ki, auditorlar yoxlaya bilsinlər
arxivi açmadan hashlər. Köməkçi təsvir olunan iş prosesini əks etdirir
`docs/source/finance/repo_ops.md`-də §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Sübut manifest | `manifest.json` | `<sha256>` | Yoxlama məbləğini idarəetmə biletinə/referendum qeydlərinə daxil edin. |

## 7. Telemetriya və Hadisə Snapshot

Müvafiq `AccountEvent::Repo(*)` qeydlərini və istənilən tablosunu və ya CSV-ni ixrac edin
`docs/source/finance/repo_ops.md`-də istinad edilən ixrac. Faylları qeyd edin +
rəyçilər birbaşa dəlillərə keçə bilsinlər ki, burada hash edir.

| İxrac | Fayl | SHA-256 | Qeydlər |
|--------|------|---------|-------|
| Repo hadisələri JSON | `evidence/repo_events.ndjson` | `<sha256>` | Raw Torii hadisə axını masa hesablarına süzülüb. |
| Telemetriya CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo Marja panelindən istifadə edərək Grafana-dən ixrac edilib. |

## 8. Təsdiqlər və İmzalar

- **İkili nəzarət imzalayanlar:** `<names + timestamps>`
- **GAR / dəqiqə həzm:** İmzalanmış GAR PDF sənədinin `<sha256>` və ya yükləmə dəqiqələri.
- **Saxlama yeri:** `governance://finance/repo/<slug>/packet/`

## 9. Yoxlama siyahısı

Hər bir elementi tamamlandıqdan sonra qeyd edin.

- [ ] Təlimat yükləri mərhələli, hashed və əlavə edildi.
- [ ] Konfiqurasiya snippet hash qeydə alınıb.
- [ ] Deterministik test qeydləri tutuldu + hashed.
- [ ] Həyat dövrü snapshot + həzm ixrac edildi.
- [ ] Sübut manifest yaradıldı və hash qeydə alındı.
- [ ] Hadisə/temetriya ixracı çəkildi + hashed.
- [ ] İkili nəzarət təsdiqləri arxivləşdirildi.
- [ ] GAR/dəqiqə yüklənib; yuxarıda qeyd olunan həzm.

Bu şablonu hər bir paketlə birlikdə saxlamaq DAG idarəçiliyini saxlayır
deterministikdir və auditorlara repo həyat dövrü üçün portativ manifest təqdim edir
qərarlar.