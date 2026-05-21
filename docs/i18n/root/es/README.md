---
lang: es
direction: ltr
source: README.md
status: complete
translator: manual
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:00:00+00:00"
translation_last_reviewed: 2026-02-07
---

# Hyperledger Iroha

[![Licencia](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha es una plataforma de blockchain determinista para despliegues permissionados y de consorcio. Ofrece gestión de cuentas y activos, permisos on-chain y contratos inteligentes mediante la Iroha Virtual Machine (IVM).

> El estado del workspace y los cambios recientes se registran en [`status.md`](./status.md).

## Líneas de lanzamiento

Este repositorio publica dos líneas de despliegue desde la misma base de código:

- **Iroha 2**: redes permissionadas/de consorcio autogestionadas.
- **Iroha 3 (SORA Nexus)**: línea orientada a Nexus usando los mismos crates base.

Ambas líneas comparten los mismos componentes centrales, incluidos la serialización Norito, el consenso Sumeragi y la cadena de herramientas Kotodama -> IVM.

## Estructura del repositorio

- [`crates/`](./crates): crates principales de Rust (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito`, etc.).
- [`integration_tests/`](./integration_tests): pruebas de integración y de red entre componentes.
- [`IrohaSwift/`](./IrohaSwift): paquete SDK de Swift.
- [`java/iroha_android/`](./java/iroha_android): paquete SDK de Android.
- [`docs/`](./docs): documentación para usuarios, operaciones y desarrollo.

## Inicio rápido

### Requisitos previos

- [Rust estable](https://www.rust-lang.org/tools/install)
- Opcional: Docker + Docker Compose para ejecuciones locales multi-peer

### Compilar y probar (workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Notas:

- La compilación completa del workspace puede tardar unos 20 minutos.
- Las pruebas completas del workspace pueden tardar varias horas.
- El workspace apunta a `std` (no se soportan builds WASM/no-std).

### Comandos de prueba dirigidos

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### Comandos de prueba de SDK

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

## Ejecutar una red local

Inicia la red de Docker Compose incluida:

```bash
docker compose -f defaults/docker-compose.yml up
```

Usa el CLI con la configuración de cliente por defecto:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Para los pasos de despliegue nativo del daemon, consulta [`crates/irohad/README.md`](./crates/irohad/README.md).

## API y observabilidad

Torii expone APIs Norito y JSON. Endpoints operativos comunes:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Consulta la referencia completa de endpoints en:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Crates principales

- [`crates/iroha`](./crates/iroha): biblioteca cliente.
- [`crates/irohad`](./crates/irohad): binarios del daemon peer.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI de referencia.
- [`crates/iroha_core`](./crates/iroha_core): motor de ejecución y núcleo del ledger.
- [`crates/iroha_config`](./crates/iroha_config): modelo de configuración tipado.
- [`crates/iroha_data_model`](./crates/iroha_data_model): modelo de datos canónico.
- [`crates/iroha_crypto`](./crates/iroha_crypto): primitivas criptográficas.
- [`crates/norito`](./crates/norito): códec de serialización determinista.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami): herramientas de claves, génesis y configuración.

## Mapa de documentación

- Índice principal: [`docs/README.md`](./docs/README.md)
- Génesis: [`docs/genesis.md`](./docs/genesis.md)
- Consenso (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Pipeline de transacciones: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- Internals P2P: [`docs/source/p2p.md`](./docs/source/p2p.md)
- Syscalls de IVM: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Gramática de Kotodama: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Formato wire de Norito: [`norito.md`](./norito.md)
- Seguimiento del trabajo actual: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Traducciones

Resumen en japonés: [`README.ja.md`](./README.ja.md)

Otros resúmenes:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Flujo de traducción: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Contribución y ayuda

- Guía de contribución: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Canales de comunidad/soporte: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Licencia

Iroha se distribuye bajo Apache-2.0. Consulta [`LICENSE`](./LICENSE).

La documentación se distribuye bajo CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/
