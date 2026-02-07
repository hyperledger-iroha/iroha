---
lang: az
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 Profilləri

Kagami Iroha 3 şəbəkələri üçün ilkin təyinatları göndərir ki, operatorlar deterministik möhür vura bilsinlər.
genezis hər şəbəkə düymələri ilə hoqqa olmadan özünü göstərir.

- Profillər: `iroha3-dev` (zəncir `iroha3-dev.local`, kollektorlar k=1 r=1, NPoS seçildikdə zəncir id-dən alınan VRF toxumu), `iroha3-testus` (zəncir `iroha3-testus`, kollektorlar k=3r tələb edir) NPoS seçildikdə `--vrf-seed-hex`), `iroha3-nexus` (zəncir `iroha3-nexus`, kollektorlar k=5 r=3, NPoS seçildikdə `--vrf-seed-hex` tələb olunur).
- Konsensus: Sora profil şəbəkələri (Nexus + məlumat məkanları) NPoS tələb edir və mərhələli kəsilmələrə icazə vermir; icazəli Iroha3 yerləşdirmələri Sora profili olmadan işləməlidir.
- Nəsil: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Nexus üçün `--consensus-mode npos` istifadə edin; `--vrf-seed-hex` yalnız NPoS üçün etibarlıdır (testus/nexus üçün tələb olunur). Kagami DA/RBC-ni Iroha3 xəttinə bağlayır və xülasə verir (zəncir, kollektorlar, DA/RBC, VRF toxumu, barmaq izi).
- Doğrulama: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` profil gözləntilərini təkrarlayır (zəncir identifikatoru, DA/RBC, kollektorlar, PoP əhatə dairəsi, konsensus barmaq izi). Yalnız testus/nexus üçün NPoS manifestini yoxlayarkən `--vrf-seed-hex`-i təmin edin.
- Nümunə paketləri: əvvəlcədən yaradılmış paketlər `defaults/kagami/iroha3-{dev,testus,nexus}/` altında yaşayır (genesis.json, config.toml, docker-compose.yml, verify.txt, README). `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` ilə bərpa edin.
- Mochi: `mochi`/`mochi-genesis` `--genesis-profile <profile>` və `--vrf-seed-hex <hex>` (yalnız NPoS) qəbul edir, onları Kagami-ə yönləndirir və istifadə edildiyi zaman eyni Kagami profilini çap edin.

Paketlər BLS PoP-lərini topologiya girişləri ilə birlikdə yerləşdirir, beləliklə `kagami verify` uğur qazanır
qutudan kənarda; yerli üçün lazım olduqda konfiqurasiyalarda etibarlı həmyaşıdları/portları tənzimləyin
tüstü qaçır.