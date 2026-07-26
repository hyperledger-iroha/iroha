---
lang: pt
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Ganchos de instrumentação de laboratório de dispositivos Android (AND6)

Esta referência encerra a ação do roteiro “preparar o restante dispositivo-laboratório /
ganchos de instrumentação antes do início do AND6”. Explica como cada reserva
slot de laboratório de dispositivo deve capturar artefatos de telemetria, fila e atestado para que o
A lista de verificação de conformidade AND6, o registro de evidências e os pacotes de governança compartilham o mesmo
fluxo de trabalho determinístico. Combine esta nota com o procedimento de reserva
(`device_lab_reservation.md`) e o runbook de failover ao planejar ensaios.

## Metas e escopo

- **Evidência determinística** – todas as saídas de instrumentação vivem sob
  `artifacts/android/device_lab/<slot-id>/` com SHA-256 se manifesta para auditores
  pode diferenciar pacotes sem executar novamente os testes.
- **Fluxo de trabalho baseado em script** – reutilize os auxiliares existentes
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  em vez de comandos adb personalizados.
- **As listas de verificação permanecem sincronizadas** – cada execução faz referência a este documento do
  Lista de verificação de conformidade AND6 e anexa os artefatos a
  `docs/source/compliance/android/evidence_log.csv`.

## Layout do artefato

1. Escolha um identificador de slot exclusivo que corresponda ao bilhete de reserva, por exemplo.
   `2026-05-12-slot-a`.
2. Propaie os diretórios padrão:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Salve cada registro de comando dentro da pasta correspondente (por exemplo,
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Capture os manifestos SHA-256 assim que o slot for fechado:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Matriz de Instrumentação

| Fluxo | Comando(s) | Local de saída | Notas |
|------|------------|------|-------|
| Redação de telemetria + pacote de status | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Execute no início e no final do slot; anexe stdout CLI a `status.log`. |
| Fila pendente + preparação para o caos | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | Espelha CenárioD de `readiness/labs/telemetry_lab_01.md`; estenda o env var para cada dispositivo no slot. |
| Substituir resumo do razão | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Obrigatório mesmo quando nenhuma substituição estiver ativa; provar o estado zero. |
| Atestado StrongBox / TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Repita para cada dispositivo reservado (corresponda aos nomes em `android_strongbox_device_matrix.md`). |
| Regressão de atestado de chicote de CI | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | Captura as mesmas evidências que o CI carrega; incluir em execuções manuais para simetria. |
| Linha de base de lint/dependência | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Execute uma vez por janela de congelamento; cite o resumo nos pacotes de conformidade. |

## Procedimento de slot padrão1. **Pré-voo (T-24h)** – Confirme se o bilhete da reserva faz referência a este
   documento, atualize a entrada da matriz do dispositivo e propague a raiz do artefato.
2. **Durante o horário**
   - Execute primeiro os comandos de pacote de telemetria + exportação de fila. Passe
     `--note <ticket>` a `ci/run_android_telemetry_chaos_prep.sh` para que o log
     faz referência ao ID do incidente.
   - Acione os scripts de atestado por dispositivo. Quando o arnês produz um
     `.zip`, copie-o na raiz do artefato e registre o Git SHA impresso em
     o final do roteiro.
   - Execute `make android-lint` com o caminho de resumo substituído, mesmo que CI
     já correu; os auditores esperam um log por slot.
3. **Pós-corrida**
   - Gere `sha256sum.txt` e `README.md` (notas de formato livre) dentro do slot
     pasta resumindo os comandos executados.
   - Anexe uma linha a `docs/source/compliance/android/evidence_log.csv` com o
     ID do slot, caminho do manifesto hash, referências do Buildkite (se houver) e o mais recente
     porcentagem de capacidade do laboratório de dispositivos da exportação do calendário de reservas.
   - Vincular a pasta slot no ticket `_android-device-lab`, o AND6
     lista de verificação e relatório de lançamento `docs/source/android_support_playbook.md`.

## Tratamento e escalonamento de falhas

- Se algum comando falhar, capture a saída stderr em `logs/` e siga o
  escada de escalada em `device_lab_reservation.md` §6.
- As deficiências de fila ou telemetria devem anotar imediatamente o status de substituição em
  `docs/source/sdk/android/telemetry_override_log.md` e faça referência ao ID do slot
  para que a governação possa seguir o procedimento.
- As regressões de atestado devem ser registradas em
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  com as séries de dispositivos com falha e os caminhos do pacote registrados acima.

## Lista de verificação de relatórios

Antes de marcar o slot como concluído, verifique se as seguintes referências estão atualizadas:

- `docs/source/compliance/android/and6_compliance_checklist.md` — marque o
  linha de instrumentação completa e anote o ID do slot.
- `docs/source/compliance/android/evidence_log.csv` — adicione/atualize a entrada com
  o hash do slot e a leitura da capacidade.
- Ticket `_android-device-lab` – anexe links de artefatos e IDs de trabalho do Buildkite.
- `status.md` — inclua uma breve nota no próximo resumo de preparação do Android para
  os leitores do roteiro sabem qual slot produziu as evidências mais recentes.

Seguir esse processo mantém os “ganchos de laboratório de dispositivos + instrumentação” do AND6
marco auditável e evita divergência manual entre reserva, execução,
e relatórios.