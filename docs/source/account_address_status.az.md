---
lang: az
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Hesab Ünvanına Uyğunluq Statusu (ADDR-2)

Status: Qəbul edilib 30-03-2026  
Sahiblər: Data Model Team / QA Guild  
Yol xəritəsi arayışı: ADDR-2 — Dual-Format Compliance Suite

### 1. İcmal

- Qurğu: `fixtures/account/address_vectors.json` (IH58 (üstünlük verilir) + sıxılmış (`sora`, ikinci ən yaxşı) + multisig müsbət/mənfi hallar).
- Əhatə dairəsi: qeyri-defolt, Lokal-12, Qlobal reyestr və tam səhv taksonomiyası olan multisig nəzarətçiləri əhatə edən deterministik V1 yükləri.
- Paylanma: Rust data-modeli, Torii, JS/TS, Swift və Android SDK-ları arasında paylaşılır; Hər hansı bir istehlakçı sapdıqda CI uğursuz olur.
- Həqiqət mənbəyi: generator `crates/iroha_data_model/src/account/address/compliance_vectors.rs`-də yaşayır və `cargo xtask address-vectors` vasitəsilə ifşa olunur.
### 2. Regenerasiya və Doğrulama

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Bayraqlar:

- `--out <path>` — ad-hoc paketləri istehsal edərkən isteğe bağlı ləğvetmə (defolt olaraq `fixtures/account/address_vectors.json`).
- `--stdout` — diskə yazmaq əvəzinə JSON-u stdout-a buraxın.
- `--verify` — cari faylı təzə yaradılmış məzmunla müqayisə edin (driftdə tez uğursuz olur; `--stdout` ilə istifadə edilə bilməz).

### 3. Artefakt Matrisi

| Səthi | İcra | Qeydlər |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON-u təhlil edir, kanonik faydalı yükləri yenidən qurur və IH58 (üstünlük verilir)/sıxılmış (`sora`, ikinci ən yaxşı)/kanonik çevrilmələri + strukturlaşdırılmış səhvləri yoxlayır. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Torii səhv formalaşmış IH58 (üstünlük verilir)/sıxılmış (`sora`, ikinci ən yaxşı) faydalı yüklərdən qəti şəkildə imtina etməsi üçün server tərəfindəki kodekləri təsdiqləyir. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Güzgülər V1 qurğuları (IH58 üstünlük verilir/sıxılmış (`sora`) ikinci ən yaxşı/tam genişlikdə) və hər bir mənfi hal üçün Norito tipli xəta kodlarını təsdiqləyir. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Apple platformalarında IH58 (üstünlük verilir)/sıxılmış (`sora`, ikinci ən yaxşı) dekodlaşdırma, multisig faydalı yüklər və səhvlərin aradan qaldırılması məşqləri. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Kotlin/Java bağlamalarının kanonik qurğu ilə uyğunluğunu təmin edir. |

### 4. Monitorinq və Görkəmli İş- Status hesabatı: bu sənəd `status.md` və yol xəritəsi ilə əlaqələndirilir ki, həftəlik nəzərdən keçirmə qurğunun sağlamlığını yoxlaya bilsin.
- Tərtibatçı portalının xülasəsi: xaricə baxan xülasə üçün sənədlər portalında (`docs/portal/docs/reference/account-address-status.md`) **İstinad → Hesab ünvanına uyğunluq**-a baxın.
- Prometheus və idarə panelləri: SDK nüsxəsini yoxladığınız zaman köməkçini `--metrics-out` (və istəyə görə `--metrics-label`) ilə işə salın ki, Prometheus mətn faylı toplayıcısı I100NI000. Grafana tablosuna **Hesab Ünvanı Quraşdırma Vəziyyəti** (`dashboards/grafana/account_address_fixture_status.json`) hər səth üzrə keçmə/uğursuzluq saylarını göstərir və audit sübutu üçün kanonik SHA-256 həzmini üzə çıxarır. Hər hansı bir hədəf `0` məlumat verdikdə xəbərdar olun.
- Torii ölçüləri: `torii_address_domain_total{endpoint,domain_kind}` indi `torii_address_invalid_total`/`torii_address_local8_total` əks etdirən hər bir uğurla təhlil edilmiş hesab hərfi üçün emissiya edir. İstehsalda hər hansı `domain_kind="local12"` trafiki barədə xəbərdarlıq edin və sayğacları SRE `address_ingest` idarə panelinə əks etdirin ki, Local-12 pensiya qapısının yoxlanıla bilən sübutları olsun.
- Quraşdırma köməkçisi: `scripts/account_fixture_helper.py` kanonik JSON-u endirir və ya yoxlayır ki, SDK buraxılışının avtomatlaşdırılması isteğe bağlı olaraq Prometheus ölçülərini yazarkən paketi əl ilə kopyalama/yapışdırmadan ala/yoxlaya bilsin. Misal:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  Hədəf uyğun gələndə köməkçi `account_address_fixture_check_status{target="android"} 1` yazır, üstəlik SHA-256 həzmlərini ifşa edən `account_address_fixture_remote_info` / `account_address_fixture_local_info` ölçüləri. Çatışmayan fayllar `account_address_fixture_local_missing` hesabatı.
  Avtomatlaşdırma sarğısı: cron/CI-dən `ci/account_fixture_metrics.sh` nömrəsinə zəng edərək birləşdirilmiş mətn faylı (defolt olaraq `artifacts/account_fixture/address_fixture.prom`). Təkrarlanan `--target label=path` daxiletmələrini keçin (istəyə görə mənbəni ləğv etmək üçün hər hədəfə `::https://mirror/...` əlavə edin) beləliklə, Prometheus hər SDK/CLI nüsxəsini əhatə edən bir faylı silir. GitHub iş axını `address-vectors-verify.yml` artıq bu köməkçini kanonik qurğuya qarşı işlədir və SRE qəbulu üçün `account-address-fixture-metrics` artefaktını yükləyir.