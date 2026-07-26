---
lang: az
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage Paket Şablonu (SN13-C)

Yol xəritəsi elementi **SN13-C — Manifestlər və SoraNS ankerləri** hər bir ləqəb tələb edir
deterministik sübut paketini göndərmək üçün fırlanma. Bu şablonu özünüzə kopyalayın
artefakt kataloqu (məsələn
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) və dəyişdirin
paketi idarəetməyə təqdim etməzdən əvvəl yer tutanlar.

## 1. Metadata

| Sahə | Dəyər |
|-------|-------|
| Hadisə ID | `<taikai.event.launch-2026-07-10>` |
| Yayım / ifa | `<main-stage>` |
| Alias ​​ad sahəsi / ad | `<sora / docs>` |
| Sübut kataloqu | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Operatorla əlaqə | `<name + email>` |
| GAR / RPT bilet | `<governance ticket or GAR digest>` |

## Paket köməkçisi (isteğe bağlı)

Spool artefaktlarını kopyalayın və əvvəl JSON (istəyə görə imzalanmış) xülasəsi buraxın
qalan bölmələrin doldurulması:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Köməkçi `taikai-anchor-request-*`, `taikai-trm-state-*`,
Taikai makara kataloqundan `taikai-lineage-*`, zərflər və gözətçilər
(`config.da_ingest.manifest_store_dir/taikai`) artıq sübut qovluğu
aşağıda istinad edilən dəqiq faylları ehtiva edir.

## 2. Nəsil dəftəri və göstəriş

Həm diskdəki nəsil kitabçasını və bunun üçün yazdığı JSON Torii işarəsini əlavə edin
pəncərə. Bunlar birbaşa olaraq gəlir
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` və
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Nəsil dəftəri | `taikai-trm-state-docs.json` | `<sha256>` | Əvvəlki manifest həzmini/pəncərəni sübut edir. |
| Nəsil işarəsi | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS ankerinə yükləməzdən əvvəl çəkilib. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Lövbər yükünün tutulması

Torii anker xidmətinə çatdırılan POST yükünü qeyd edin. Faydalı yük
`envelope_base64`, `ssm_base64`, `trm_base64` və inline daxildir
`lineage_hint` obyekti; Auditlər bu ələ keçirilməsinə əsaslanan ipucunu sübut edir
SoraNS-ə göndərildi. Torii indi bu JSON-u avtomatik olaraq yazır
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool kataloqunun içərisində (`config.da_ingest.manifest_store_dir/taikai/`), belə ki
operatorlar HTTP qeydlərini qırmaq əvəzinə birbaşa kopyalaya bilərlər.

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Xam sorğu `taikai-anchor-request-*.json`-dən kopyalandı (Taikai spool). |

## 4. Manifest həzm etirafı

| Sahə | Dəyər |
|-------|-------|
| Yeni manifest digest | `<hex digest>` |
| Əvvəlki manifest həzm (işarədən) | `<hex digest>` |
| Pəncərənin başlanğıcı / sonu | `<start seq> / <end seq>` |
| Qəbul vaxt damğası | `<ISO8601>` |

Yuxarıda qeyd olunan kitaba/işarə hashlərinə istinad edin ki, rəyçilər bunu yoxlaya bilsinlər
əvəz edilmiş pəncərə.

## 5. Metriklər / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (ləqəb üzrə): `<file path + hash>`

Sayğacı göstərən Prometheus/Grafana ixracını və ya `curl` çıxışını təmin edin
artım və bu ləqəb üçün `/status` massivi.

## 6. Sübut kataloqu üçün manifest

Sübut qovluğunun deterministik manifestini yaradın (spool faylları,
faydalı yükün tutulması, ölçülərin anlıq görüntüləri) beləliklə idarəetmə hər hash olmadan yoxlaya bilər
arxivin açılması.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefakt | Fayl | SHA-256 | Qeydlər |
|----------|------|---------|-------|
| Sübut manifest | `manifest.json` | `<sha256>` | Bunu idarəetmə paketinə / GAR-a əlavə edin. |

## 7. Yoxlama siyahısı

- [ ] Nəsil kitabçası kopyalandı + hashed.
- [ ] Nəsil işarəsi kopyalandı + hashed.
- [ ] Anchor POST faydalı yükü tutuldu və hashing edildi.
- [ ] Manifest həzm cədvəli dolduruldu.
- [ ] Metrik snapşotları ixrac edildi (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifest `scripts/repo_evidence_manifest.py` ilə yaradılıb.
- [ ] Hash + əlaqə məlumatı ilə idarəetməyə yüklənmiş paket.

Hər ləqəb fırlanması üçün bu şablonun saxlanması SoraNS idarəçiliyini saxlayır
reproduktiv paketi birləşdirin və nəsil göstərişlərini birbaşa GAR/RPT sübutlarına bağlayır.