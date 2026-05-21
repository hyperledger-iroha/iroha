---
lang: az
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hyperledger Iroha

[![Lisenziya](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha icazəli və konsorsium yerləşdirmələri üçün deterministik blokçeyn platformasıdır. O, Iroha Virtual Maşın (IVM) vasitəsilə hesab/aktivin idarə edilməsi, zəncir üzərində icazələr və ağıllı müqavilələr təqdim edir.

> İş sahəsinin vəziyyəti və son dəyişikliklər [`status.md`](./status.md) daxilində izlənilir.

## Musiqiləri buraxın

Bu depo eyni kod bazasından iki yerləşdirmə trekini göndərir:

- **Iroha 2**: öz-özünə yerləşdirilən icazəli/konsorsium şəbəkələri.
- **Iroha 3 (SORA Nexus)**: eyni əsas qutulardan istifadə edərək Nexus yönümlü yerləşdirmə treki.

Hər iki trek eyni əsas komponentləri paylaşır, o cümlədən Norito serializasiya, Sumeragi konsensus və Kotodama -> IVM alətlər silsiləsi.

## Repository Layout

- [`crates/`](./crates): əsas Pas qutuları (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `irohad`, I108NIX, I108NIX, `norito` və s.).
- [`integration_tests/`](./integration_tests): komponentlər arası şəbəkə/inteqrasiya testləri.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK paketi.
- [`java/iroha_android/`](./java/iroha_android): Android SDK paketi.
- [`docs/`](./docs): istifadəçi/operator/developer sənədləri.

## Sürətli başlanğıc

### İlkin şərtlər

- [Pas sabit](https://www.rust-lang.org/tools/install)
- İsteğe bağlı: Docker + Docker Yerli multi-peer qaçışları üçün tərtib edin

### Yaradın və Sınayın (İş sahəsi)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Qeydlər:

- Tam iş sahəsinin qurulması təxminən 20 dəqiqə çəkə bilər.
- Tam iş sahəsi testləri bir neçə saat çəkə bilər.
- İş sahəsi hədəfləri `std` (WASM/no-std quruluşları dəstəklənmir).

### Məqsədli Test Əmrləri

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK Test Əmrləri

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Yerli Şəbəkəni işə salın

Təqdim olunan Docker Compose şəbəkəsini işə salın:

```bash
docker compose -f defaults/docker-compose.yml up
```

CLI-ni standart müştəri konfiqurasiyasına qarşı istifadə edin:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Demona xas yerli yerləşdirmə addımları üçün baxın [`crates/irohad/README.md`](./crates/irohad/README.md).

## API və müşahidə qabiliyyəti

Torii həm Norito, həm də JSON API-lərini ifşa edir. Ümumi operator son nöqtələri:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Tam son nöqtə istinadına baxın:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Əsas qutular

- [`crates/iroha`](./crates/iroha): müştəri kitabxanası.
- [`crates/irohad`](./crates/irohad): həmyaşıd demon ikili faylları.
- [`crates/iroha_cli`](./crates/iroha_cli): istinad CLI.
- [`crates/iroha_core`](./crates/iroha_core): kitab/əsas icra mühərriki.
- [`crates/iroha_config`](./crates/iroha_config): tipli konfiqurasiya modeli.
- [`crates/iroha_data_model`](./crates/iroha_data_model): kanonik məlumat modeli.
- [`crates/iroha_crypto`](./crates/iroha_crypto): kriptoqrafik primitivlər.
- [`crates/norito`](./crates/norito): deterministik seriallaşdırma kodek.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Maşın.
- [`crates/iroha_kagami`](./crates/iroha_kagami): açar/genesis/konfiqurasiya aləti.

## Sənədləşmə xəritəsi

- Əsas sənədlər indeksi: [`docs/README.md`](./docs/README.md)
- Yaradılış: [`docs/genesis.md`](./docs/genesis.md)
- Konsensus (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Əməliyyat kəməri: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P daxili: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM sistem zəngləri: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama qrammatikası: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito tel formatı: [`norito.md`](./norito.md)
- Cari iş izləmə: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Tərcümələr

Yapon icmalı: [`README.ja.md`](./README.ja.md)

Digər icmallar:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Tərcümə iş axını: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Töhfə və Yardım

- Töhfə bələdçisi: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- İcma/dəstək kanalları: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Lisenziya

Iroha Apache-2.0 altında lisenziyalıdır. Baxın [`LICENSE`](./LICENSE).

Sənədlər CC-BY-4.0 altında lisenziyalaşdırılıb: http://creativecommons.org/licenses/by/4.0/