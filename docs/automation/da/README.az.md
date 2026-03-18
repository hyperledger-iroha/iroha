---
lang: az
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Məlumat Əlçatanlığı Təhlükə Modelinin Avtomatlaşdırılması (DA-1)

Yol xəritəsi elementi DA-1 və `status.md` deterministik avtomatlaşdırma dövrəsini tələb edir ki,
üzə çıxan Norito PDP/PoTR təhlükə modeli xülasələrini hazırlayır.
`docs/source/da/threat_model.md` və Docusaurus güzgü. Bu kataloq
istinad edilən artefaktları çəkir:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (`scripts/docs/render_da_threat_model_tables.py` ilə işləyir)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Axın

1. **Hesabat yaradın**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON xülasəsi simulyasiya edilmiş replikasiya uğursuzluq dərəcəsini, chunkeri qeyd edir
   hədlər və PDP/PoTR tərəfindən aşkar edilən hər hansı siyasət pozuntuları
   `integration_tests/src/da/pdp_potr.rs`.
2. **Markdown cədvəllərini göstərin**
   ```bash
   make docs-da-threat-model
   ```
   Bu, yenidən yazmaq üçün `scripts/docs/render_da_threat_model_tables.py` işləyir
   `docs/source/da/threat_model.md` və `docs/portal/docs/da/threat-model.md`.
3. JSON hesabatını (və isteğe bağlı CLI jurnalını) kopyalayaraq **Artefaktı arxivləşdirin**
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Nə vaxt
   idarəetmə qərarları müəyyən bir işə əsaslanır, git commit hash və daxildir
   bir qardaş `<timestamp>-metadata.md`-də simulyator toxumu.

## Sübut Gözləntiləri

- JSON faylları <100 KiB qalmalıdır ki, onlar git-də yaşaya bilsinlər. Daha böyük icra
  izlər xarici yaddaşa aiddir - metadatada onların imzalanmış heşinə istinad edin
  lazım olduqda qeyd edin.
- Hər bir arxivləşdirilmiş fayl toxumu, konfiqurasiya yolunu və simulyator versiyasını sadalamalıdır
  DA buraxılış qapılarını yoxlayarkən təkrarlar tam olaraq təkrarlana bilər.
- İstənilən vaxt `status.md`-dən arxivləşdirilmiş fayla və ya yol xəritəsi girişinə keçid edin
  DA-1 qəbul meyarlarını qabaqcadan nəzərdən keçirənlərin yoxlaya bilməsini təmin edir
  kəməri yenidən işə salmadan əsas xətt.

## Öhdəliyin uzlaşdırılması (Sequencer buraxılması)

DA qəbulu qəbzlərini müqayisə etmək üçün `cargo xtask da-commitment-reconcile` istifadə edin
DA öhdəlik qeydləri, sequencer buraxılması və ya dəyişdirilməsi:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito və ya JSON formasında qəbz və öhdəlikləri qəbul edir
  `SignedBlockWire`, `.norito` və ya JSON paketləri.
- Blok jurnalında hər hansı bir bilet əskik olduqda və ya heşlər ayrıldıqda uğursuz olur;
  Siz qəsdən əhatə etdiyiniz zaman `--allow-unexpected` yalnız bloklanan biletlərə məhəl qoymur
  qəbz dəsti.
- Emissiya edilmiş JSON-u idarəetmə paketlərinə/Alertmanagerə əlavə edin
  xəbərdarlıqlar; defolt olaraq `artifacts/da/commitment_reconciliation.json`.

## İmtiyaz Auditi (Rüblük Giriş İcmalı)

DA manifest/replay kataloqlarını skan etmək üçün `cargo xtask da-privilege-audit` istifadə edin
(plus əlavə əlavə yollar) itkin, kataloq olmayan və ya dünya üçün yazıla bilən üçün
girişlər:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Təqdim olunan Torii konfiqurasiyasından DA qəbul yollarını oxuyur və Unix-i yoxlayır
  icazələr mövcud olduqda.
- Çatışmayan/kataloq olmayan/dünyada yazıla bilən yolları qeyd edir və sıfırdan fərqli çıxışı qaytarır
  problemlər mövcud olduqda kod.
- JSON paketini imzalayın və əlavə edin (`artifacts/da/privilege_audit.json` by
  defolt) rüblük giriş-nəzərdən paketlərə və tablolara.