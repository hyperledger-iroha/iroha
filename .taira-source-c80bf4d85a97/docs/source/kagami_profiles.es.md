---
lang: es
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Perfiles Iroha3

Kagami envía ajustes preestablecidos para redes Iroha 3 para que los operadores puedan estampar deterministas
La génesis se manifiesta sin hacer malabarismos con las perillas por red.

- Perfiles: `iroha3-dev` (cadena `iroha3-dev.local`, recolectores k=1 r=1, semilla VRF derivada de la identificación de la cadena cuando se selecciona NPoS), `iroha3-taira` (cadena `iroha3-taira`, recolectores k=3 r=3, requiere `--vrf-seed-hex` cuando Se selecciona NPoS), `iroha3-nexus` (cadena `iroha3-nexus`, colectores k=5 r=3, requiere `--vrf-seed-hex` cuando se selecciona NPoS).
- Consenso: las redes de perfil Sora (Nexus + espacios de datos) requieren NPoS y no permiten cortes por etapas; Las implementaciones autorizadas de Iroha3 deben ejecutarse sin un perfil de Sora.
- Generación: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Utilice `--consensus-mode npos` para Nexus; `--vrf-seed-hex` solo es válido para NPoS (requerido para taira/nexus). Kagami fija DA/RBC en la línea Iroha3 y emite un resumen (cadena, recolectores, DA/RBC, semilla VRF, huella digital).
- Verificación: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` reproduce las expectativas del perfil (identificación de cadena, DA/RBC, recopiladores, cobertura PoP, huella digital de consenso). Proporcione `--vrf-seed-hex` solo cuando verifique un manifiesto NPoS para taira/nexus.
- Paquetes de muestra: los paquetes pregenerados se encuentran en `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verificar.txt, README). Regenerar con `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` acepta `--genesis-profile <profile>` e `--vrf-seed-hex <hex>` (solo NPoS), los reenvía a Kagami e imprime el mismo resumen de Kagami en stdout/stderr cuando se utiliza un perfil.

Los paquetes incorporan BLS PoP junto con entradas de topología para que `kagami verify` tenga éxito
fuera de la caja; ajuste los pares/puertos confiables en las configuraciones según sea necesario para local
el humo corre.