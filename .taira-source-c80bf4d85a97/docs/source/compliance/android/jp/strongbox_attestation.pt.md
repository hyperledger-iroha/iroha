---
lang: pt
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Evidência de atestado do StrongBox - implantações no Japão

| Campo | Valor |
|-------|-------|
| Janela de Avaliação | 10/02/2026 – 12/02/2026 |
| Localização do artefato | `artifacts/android/attestation/<device-tag>/<date>/` (formato de pacote por `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Ferramentas de captura | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Revisores | Líder de laboratório de hardware, conformidade e jurídico (JP) |

## 1. Procedimento de captura

1. Em cada dispositivo listado na matriz do StrongBox, gere um desafio e capture o pacote de atestado:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Confirme os metadados do pacote configurável (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) na árvore de evidências.
3. Execute o auxiliar de CI para verificar novamente todos os pacotes off-line:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Resumo do dispositivo (12/02/2026)

| Etiqueta do dispositivo | Modelo / Caixa Forte | Caminho do pacote | Resultado | Notas |
|--------|--------|---------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Aprovado (com suporte de hardware) | Desafio limitado, patch do sistema operacional 2025/03/05. |
| `pixel7-strongbox-a` | Pixel 7/Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Aprovado | Candidato primário à faixa CI; temperatura dentro das especificações. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro/Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Aprovado (reteste) | Hub USB-C substituído; Buildkite `android-strongbox-attestation#221` capturou o pacote de passagem. |
| `s23-strongbox-a` | Galaxy S23/Snapdragon 8 Geração 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Aprovado | Perfil de atestado Knox importado em 09/02/2026. |
| `s24-strongbox-a` | Galaxy S24/Snapdragon 8 Geração 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Aprovado | Perfil de atestado Knox importado; Pista CI agora verde. |

As tags do dispositivo são mapeadas para `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Lista de verificação do revisor

- [x] Verifique se `result.json` mostra `strongbox_attestation: true` e a cadeia de certificados para raiz confiável.
- [x] Confirme se os bytes de desafio correspondem ao Buildkite executando `android-strongbox-attestation#219` (varredura inicial) e `#221` (reteste do Pixel 8 Pro + captura S24).
- [x] Execute novamente a captura do Pixel 8 Pro após correção de hardware (proprietário: líder do laboratório de hardware, concluído em 13/02/2026).
- [x] Captura completa do Galaxy S24 assim que a aprovação do perfil Knox chegar (proprietário: Device Lab Ops, concluído em 13/02/2026).

## 4. Distribuição

- Anexe este resumo e o arquivo de texto do relatório mais recente aos pacotes de conformidade do parceiro (lista de verificação FISC §Residência de dados).
- Caminhos de pacotes de referência ao responder a auditorias de reguladores; não transmita certificados brutos fora dos canais criptografados.

## 5. Registro de alterações

| Data | Alterar | Autor |
|------|--------|--------|
| 12/02/2026 | Captura inicial do pacote JP + relatório. | Operações de laboratório de dispositivos |