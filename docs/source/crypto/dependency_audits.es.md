---
lang: es
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Auditorías de dependencia criptográfica

## Streebog (caja `streebog`)

- **Versión en árbol:** `0.11.0-rc.2` suministrado bajo `vendor/streebog` (se usa cuando la función `gost` está habilitada).
- **Consumidor:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + hash de mensajes).
- **Estado:** Solo candidato a lanzamiento. Ninguna caja que no sea RC ofrece actualmente la superficie API requerida,
  por lo tanto, reflejamos la caja en el árbol para auditarla mientras realizamos un seguimiento en sentido ascendente para una versión final.
- **Revisar puntos de control:**
  - Salida de hash verificada contra la suite Wycheproof y los dispositivos TC26 a través de
    `cargo test -p iroha_crypto --features gost` (ver `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    ejercita Ed25519/Secp256k1 junto con cada curva TC26 con la dependencia actual.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    compara las mediciones más recientes con las medianas registradas (use `--summary-only` en CI, agregue
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` al rebaselinear).
  - `scripts/gost_bench.sh` envuelve el banco + controla el flujo; pase `--write-baseline` para actualizar el JSON.
    Consulte `docs/source/crypto/gost_performance.md` para conocer el flujo de trabajo de un extremo a otro.
- **Mitigaciones:** `streebog` solo se invoca a través de contenedores deterministas que ponen a cero las claves;
  el firmante protege los nonces con la entropía del sistema operativo para evitar una falla catastrófica del RNG.
- **Próximas acciones:** Siga la versión streebog `0.11.x` de RustCrypto; una vez que la etiqueta aterrice, trate la
  actualizar como un aumento de dependencia estándar (verificar la suma de verificación, revisar la diferencia, registrar la procedencia y
  deje caer el espejo suministrado).