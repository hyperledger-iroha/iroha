---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM e atestado de procedência - Android SDK

| Campo | Valor |
|-------|-------|
| Escopo | Android SDK (`java/iroha_android`) + aplicativos de amostra (`examples/android/*`) |
| Proprietário do fluxo de trabalho | Engenharia de Liberação (Alexei Morozov) |
| Última verificação | 11/02/2026 (Buildkite `android-sdk-release#4821`) |

## 1. Fluxo de trabalho de geração

Execute o script auxiliar (adicionado para automação AND6):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

O script executa o seguinte:

1. Executa `ci/run_android_tests.sh` e `scripts/check_android_samples.sh`.
2. Invoca o wrapper Gradle em `examples/android/` para construir SBOMs CycloneDX para
   `:android-sdk`, `:operator-console` e `:retail-wallet` com o fornecido
   `-PversionName`.
3. Copia cada SBOM em `artifacts/android/sbom/<sdk-version>/` com nomes canônicos
   (`iroha-android.cyclonedx.json`, etc.).

## 2. Proveniência e Assinatura

O mesmo script assina cada SBOM com `cosign sign-blob --bundle <file>.sigstore --yes`
e emite `checksums.txt` (SHA-256) no diretório de destino. Defina o `COSIGN`
variável de ambiente se o binário residir fora de `$PATH`. Depois que o script terminar,
registre os caminhos do pacote/soma de verificação mais o ID de execução do Buildkite em
`docs/source/compliance/android/evidence_log.csv`.

## 3. Verificação

Para verificar um SBOM publicado:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Compare o SHA de saída com o valor listado em `checksums.txt`. Os revisores também comparam o SBOM com a versão anterior para garantir que os deltas de dependência sejam intencionais.

## 4. Instantâneo de evidências (11/02/2026)

| Componente | SBOM | SHA-256 | Pacote Sigstore |
|-----------|------|---------|-----------------|
| SDK Android (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | Pacote `.sigstore` armazenado ao lado do SBOM |
| Amostra do console do operador | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Amostra de carteira de varejo | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Hashes capturados do Buildkite são executados `android-sdk-release#4821`; reproduzidos por meio do comando de verificação acima.)*

## 5. Trabalho Excelente

- Automatize as etapas de SBOM + cosign dentro do pipeline de lançamento antes do GA.
- Espelhe SBOMs no balde de artefato público assim que AND6 marcar a lista de verificação como concluída.
- Coordenar com o Docs para vincular locais de download do SBOM a partir de notas de lançamento voltadas para parceiros.