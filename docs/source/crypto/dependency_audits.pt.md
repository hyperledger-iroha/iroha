---
lang: pt
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Auditorias de dependência criptográfica

## Streebog (caixa `streebog`)

- **Versão na árvore:** `0.11.0-rc.2` vendido sob `vendor/streebog` (usado quando o recurso `gost` está habilitado).
- **Consumidor:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + hashing de mensagem).
- **Status:** Somente candidato a lançamento. Nenhuma caixa não RC oferece atualmente a superfície API necessária,
  portanto, espelhamos a caixa na árvore para auditabilidade enquanto rastreamos o upstream para uma versão final.
- **Revisar pontos de verificação:**
  - Saída de hash verificada em relação ao conjunto Wycheproof e aos equipamentos TC26 via
    `cargo test -p iroha_crypto --features gost` (ver `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  -`cargo bench -p iroha_crypto --bench gost_sign --features gost`
    exercita Ed25519/Secp256k1 ao longo de cada curva TC26 com a dependência atual.
  -`cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    compara as medições mais recentes com as medianas verificadas (use `--summary-only` no CI, adicione
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` ao rebasear).
  - `scripts/gost_bench.sh` envolve a bancada + verifica fluxo; passe `--write-baseline` para atualizar o JSON.
    Consulte `docs/source/crypto/gost_performance.md` para obter o fluxo de trabalho de ponta a ponta.
- **Mitigações:** `streebog` só é invocado por meio de wrappers determinísticos que zeram chaves;
  o signatário protege os nonces com entropia do sistema operacional para evitar falhas catastróficas do RNG.
- **Próximas ações:** Siga o lançamento do streebog `0.11.x` do RustCrypto; assim que a etiqueta chegar, trate o
  atualizar como um aumento de dependência padrão (verificar a soma de verificação, revisar a diferença, registrar a procedência e
  solte o espelho vendido).