---
lang: az
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito SwiftUI Demo İştirakçı Bələdçisi

Bu sənəd SwiftUI demosunu a
yerli Torii node və saxta kitab. `docs/norito_bridge_release.md` ilə tamamlayır
gündəlik inkişaf vəzifələrinə diqqət yetirir. İnteqrasiyanı daha dərindən öyrənmək üçün
Norito körpüsü/Steki Xcode layihələrinə birləşdirin, baxın `docs/connect_swift_integration.md`.

## Ətraf mühitin qurulması

1. `rust-toolchain.toml`-də müəyyən edilmiş Rust alətlər silsiləsi quraşdırın.
2. MacOS-da Swift 5.7+ və Xcode komanda xətti alətlərini quraşdırın.
3. (İstəyə görə) Xətt üçün [SwiftLint](https://github.com/realm/SwiftLint) quraşdırın.
4. `cargo build -p irohad`-i işə salın ki, qovşağın hostunuzda yığılmasını təmin edin.
5. `examples/ios/NoritoDemoXcode/Configs/demo.env.example`-i `.env`-ə kopyalayın və
   mühitinizə uyğun dəyərlər. Proqram işə salındıqda bu dəyişənləri oxuyur:
   - `TORII_NODE_URL` — əsas REST URL (WebSocket URL-ləri ondan əldə edilir).
   - `CONNECT_SESSION_ID` — 32 baytlıq sessiya identifikatoru (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — `/v1/connect/session` tərəfindən qaytarılan tokenlər.
   - `CONNECT_CHAIN_ID` — nəzarət əl sıxma zamanı elan edilmiş zəncir identifikatoru.
   - `CONNECT_ROLE` — UI-də defolt rol əvvəlcədən seçilmişdir (`app` və ya `wallet`).
   - Əllə sınaq üçün əlavə köməkçilər: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`.

## Bootstrapping Torii + saxta kitab

Anbar yaddaşdaxili kitabçası ilə Torii qovşağını işə salan köməkçi skriptləri göndərir.
demo hesablarla yüklənmiş:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

Skript yayır:

- Torii qovşağı `artifacts/torii.log`-ə daxil olur.
- Ledger ölçüləri (Prometheus formatı) - `artifacts/metrics.prom`.
- `artifacts/torii.jwt`-ə müştəri giriş nişanları.

`start.sh` siz `Ctrl+C` düyməsinə basana qədər demo peer-i davam etdirir. Hazır vəziyyətdə yazır
`artifacts/ios_demo_state.json`-ə snapshot (digər artefaktlar üçün həqiqət mənbəyi),
aktiv Torii stdout jurnalını kopyalayır, Prometheus sıyrılana qədər `/metrics` sorğularını keçirir
mövcuddur və konfiqurasiya edilmiş hesabları `torii.jwt`-ə təqdim edir (şəxsi açarlar daxil olmaqla)
konfiqurasiya onları təmin etdikdə). Skript çıxışı ləğv etmək üçün `--artifacts`-i qəbul edir
kataloqu, `--telemetry-profile` xüsusi Torii konfiqurasiyalarına uyğundur və
Qeyri-interaktiv CI işləri üçün `--exit-after-ready`.

`SampleAccounts.json`-dəki hər giriş aşağıdakı sahələri dəstəkləyir:

- `name` (string, isteğe bağlı) — `alias` hesab metadatası kimi saxlanılır.
- `public_key` (multihash sətri, tələb olunur) — hesabı imzalayan kimi istifadə olunur.
- `private_key` (isteğe bağlı) — müştəri etimadnaməsinin yaradılması üçün `torii.jwt`-ə daxildir.
- `domain` (isteğe bağlı) — buraxılıbsa, aktiv domeninə defolt edilir.
- `asset_id` (sətir, tələb olunur) — hesab üçün aktivin tərifi.
- `initial_balance` (sətir, tələb olunur) — hesaba daxil edilmiş rəqəmli məbləğ.

## SwiftUI demosunun işlədilməsi

1. `docs/norito_bridge_release.md`-də təsvir olunduğu kimi XCFramework qurun və onu yığın
   demo layihəsinə (istinadlar layihədə `NoritoBridge.xcframework`-ni gözləyir
   kök).
2. Xcode-da `NoritoDemoXcode` layihəsini açın.
3. `NoritoDemo` sxemini seçin və iOS simulyatorunu və ya cihazını hədəfləyin.
4. `.env` faylına sxemin mühit dəyişənləri vasitəsilə istinad edildiyinə əmin olun.
   `/v1/connect/session` tərəfindən ixrac edilmiş `CONNECT_*` dəyərlərini doldurun ki, UI olsun
   proqram işə salındıqda əvvəlcədən doldurulur.
5. Avadanlıq sürətləndirilməsinin defoltlarını yoxlayın: `App.swift` zəngləri
   `DemoAccelerationConfig.load().apply()` beləliklə demo ya seçir
   `NORITO_ACCEL_CONFIG_PATH` mühitin ləğvi və ya paketləşdirilmiş
   `acceleration.{json,toml}`/`client.{json,toml}` faylı. Əgər varsa, bu girişləri çıxarın/tənzimləyin
   işə başlamazdan əvvəl CPU geri qaytarılmasını məcbur etmək istəyirəm.
6. Proqramı qurun və işə salın. Əsas ekran Torii URL/token tələb edir
   artıq `.env` vasitəsilə quraşdırılmışdır.
7. Hesab yeniləmələrinə abunə olmaq və ya sorğuları təsdiqləmək üçün "Qoşul" sessiyasına başlayın.
8. IRH köçürməsini təqdim edin və Torii qeydləri ilə birlikdə ekrandakı jurnal çıxışını yoxlayın.

### Avadanlıq sürətləndiricisi (Metal / NEON)

`DemoAccelerationConfig` Rust node konfiqurasiyasını əks etdirir ki, tərtibatçılar məşq edə bilsinlər
Sərt kodlaşdırma eşikləri olmayan metal/NEON yolları. Yükləyici aşağıdakıları axtarır
işə salındığı yerlər:

1. `NORITO_ACCEL_CONFIG_PATH` (`.env`/sxem arqumentlərində müəyyən edilir) — mütləq yol və ya
   `iroha_config` JSON/TOML faylına `tilde` genişləndirilmiş göstərici.
2. `acceleration.{json,toml}` və ya `client.{json,toml}` adlı birləşdirilmiş konfiqurasiya faylları.
3. Mənbələrdən heç biri mövcud deyilsə, standart parametrlər (`AccelerationSettings()`) qalır.

Nümunə `acceleration.toml` fraqmenti:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

`nil` sahələrini tərk etmək iş sahəsinin defolt parametrlərini miras alır. Mənfi nömrələr nəzərə alınmır,
və çatışmayan `[accel]` bölmələri deterministik CPU davranışına qayıdır. Çalışarkən
Metal dəstəyi olmayan bir simulyator körpü səssizcə skalyar yolu saxlasa belə
konfiqurasiya sorğuları Metal.

## İnteqrasiya testləri

- İnteqrasiya testləri `Tests/NoritoDemoTests`-də yerləşir (macOS CI olduqdan sonra əlavə olunacaq
  mövcuddur).
- Testlər yuxarıdakı skriptlərdən istifadə edərək Torii-i fırladır və WebSocket abunəliklərini həyata keçirir, token
  balansları və Swift paketi vasitəsilə köçürmə axınlarını.
- Sınaq işlərinin qeydləri ölçülərlə yanaşı `artifacts/tests/<timestamp>/`-də saxlanılır və
  nümunə dəftəri zibilləri.

## CI paritet yoxlamaları

- Demoya və ya paylaşılan qurğulara toxunan PR göndərməzdən əvvəl `make swift-ci`-i işə salın. The
  hədəf armatur paritet yoxlamalarını həyata keçirir, tablosuna verilən xəbərləri təsdiqləyir və təqdim edir
  yerli xülasələr. CI-də eyni iş axını Buildkite metadatasından asılıdır
  (`ci/xcframework-smoke:<lane>:device_tag`) beləliklə tablosuna nəticələri aid edə bilər
  düzgün simulyator və ya StrongBox zolağı - parametrləri tənzimləsəniz, metadatanın mövcud olduğunu yoxlayın
  boru kəməri və ya agent etiketləri.
- `make swift-ci` uğursuz olduqda, `docs/source/swift_parity_triage.md`-dəki addımları izləyin
  və hansı zolağın tələb olunduğunu müəyyən etmək üçün göstərilən `mobile_ci` çıxışını nəzərdən keçirin
  regenerasiya və ya hadisənin təqibi.

## Problemlərin aradan qaldırılması

- Demo Torii-ə qoşula bilmirsə, node URL və TLS parametrlərini yoxlayın.
- JWT tokeninin (lazım olduqda) etibarlı olduğundan və müddəti bitmədiyindən əmin olun.
- Server tərəfində səhvlər üçün `artifacts/torii.log` yoxlayın.
- WebSocket problemləri üçün müştəri jurnalı pəncərəsini və ya Xcode konsol çıxışını yoxlayın.