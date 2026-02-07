---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a206b033b430fc9895f64d402cd53bfea35c3f269b2c18bb12a1f929114423aa
source_last_modified: "2025-12-29T18:16:35.200252+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Orchestrator GA Paritet Hesabatı

Deterministik çoxalma pariteti indi SDK üzrə izlənilir ki, buraxılış mühəndisləri bunu təsdiq edə bilsinlər
faydalı yük baytları, yığın qəbzləri, provayder hesabatları və skorbord nəticələri uyğun olaraq qalır
həyata keçirilməsi. Hər bir qoşqu altındakı kanonik multi-provayder paketini istehlak edir
SF1 planını paketləyən `fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, provayder
metadata, telemetriya snapshot və orkestr seçimləri.

## Pas Baza

- **Əmr:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Əhatə dairəsi:** `MultiPeerFixture` planını prosesdə olan orkestr vasitəsilə iki dəfə işlədir, təsdiqləyir
  yığılmış yük baytları, yığın qəbzləri, provayder hesabatları və skorboard nəticələri. Alətlər
  həmçinin pik paralelliyi və effektiv iş dəstinin ölçüsünü izləyir (`max_parallel × max_chunk_length`).
- **Performans mühafizəsi:** Hər qaçış CI aparatında 2 saniyə ərzində tamamlanmalıdır.
- **İş dəsti tavanı:** SF1 profili ilə qoşqu `max_parallel = 3`-i tətbiq edir,
  ≤196608bayt pəncərə.

Nümunə jurnal çıxışı:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Qoşqu

- **Əmr:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Əhatə dairəsi:** `iroha_js_host::sorafsMultiFetchLocal` vasitəsilə faydalı yükləri müqayisə edərək eyni qurğunu təkrarlayır,
  ardıcıl qaçışlar üzrə qəbzlər, provayder hesabatları və skorbord anlıq görüntüləri.
- **Performans mühafizəsi:** Hər bir icra 2 saniyə ərzində başa çatmalıdır; qoşqu ölçülənləri çap edir
  müddət və qorunan bayt tavanı (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Xülasə xəttinə nümunə:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Qoşqu

- **Əmr:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Əhatə dairəsi:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`-də müəyyən edilmiş paritet dəstini işlədir,
  SF1 qurğusunun Norito körpüsü (`sorafsLocalFetch`) vasitəsilə iki dəfə təkrarlanması. Qoşqu yoxlayır
  faydalı yük baytları, yığın qəbzləri, provayder hesabatları və eyni deterministikdən istifadə edərək skorbord qeydləri
  Rust/JS paketləri kimi provayder metadata və telemetriya snapshotları.
- **Körpü çəkmə kəməri:** Qoşqu tələb və yüklərə görə `dist/NoritoBridge.xcframework.zip` paketini açır
  `dlopen` vasitəsilə macOS dilimi. Xcframework əskik olduqda və ya SoraFS bağlamaları olmadıqda,
  `cargo build -p connect_norito_bridge --release`-ə qayıdır və əks əlaqə yaradır
  `target/release/libconnect_norito_bridge.dylib`, buna görə də CI-də əl ilə quraşdırma tələb olunmur.
- **Performans mühafizəsi:** Hər bir icra CI aparatında 2 saniyə ərzində başa çatmalıdır; qoşqu çap edir
  ölçülmüş müddət və qorunan bayt tavanı (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Xülasə xəttinə nümunə:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bağlama Qoşqu

- **Əmr:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Əhatə dairəsi:** Yüksək səviyyəli `iroha_python.sorafs.multi_fetch_local` sarğısını və onun çapını həyata keçirir
  məlumat sinifləri beləliklə kanonik qurğu təkər istehlakçılarının çağırdığı eyni API vasitəsilə axır. Test
  `providers.json`-dən provayder metadatasını yenidən qurur, telemetriya şəklini daxil edir və yoxlayır
  Rust/JS/Swift kimi faydalı yük baytları, yığın qəbzləri, provayder hesabatları və tablo məzmunu
  suitlər.
- **Öncədən tələb:** `maturin develop --release`-i işə salın (və ya təkəri quraşdırın) beləliklə, `_crypto`
  pytest çağırmadan əvvəl `sorafs_multi_fetch_local` bağlama; bağlama zamanı qoşqu avtomatik olaraq atlar
  mövcud deyil.
- **Performans mühafizəsi:** Rust dəsti ilə eyni ≤2s büdcə; pytest yığılmış bayt sayını qeyd edir
  və buraxılış artefaktı üçün provayderin iştirak xülasəsi.

Buraxılış qapısı hər bir qoşqudan (Rust, Python, JS, Swift) ümumi çıxışı tutmalıdır ki,
arxivləşdirilmiş hesabat, bir quruluşu təşviq etməzdən əvvəl faydalı yük qəbzlərini və ölçüləri vahid şəkildə fərqləndirə bilər. Qaç
`ci/sdk_sorafs_orchestrator.sh` hər bir paritet dəstini (Rust, Python bağlamaları, JS, Swift) yerinə yetirmək üçün
bir keçid; CI artefaktları həmin köməkçidən log çıxarışı və yaradılan əlavə etməlidir
`matrix.md` (SDK/status/müddət cədvəli) buraxılış biletinə göndərin ki, rəyçilər pariteti yoxlaya bilsinlər
matrisi lokal olaraq yenidən işə salmadan.