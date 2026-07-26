---
lang: pt
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificação de lançamento do Android (AND6)

Esta lista de verificação captura as portas **AND6 — CI & Compliance Hardening** de
`roadmap.md` (§Prioridade 5). Ele alinha as versões do Android SDK com o Rust
liberar as expectativas da RFC explicando os trabalhos de CI, artefatos de conformidade,
evidências de laboratório de dispositivo e pacotes de proveniência que devem ser anexados antes de um GA,
LTS, ou trem de hotfix, avança.

Use este documento junto com:

- `docs/source/android_support_playbook.md` — calendário de lançamento, SLAs e
  árvore de escalada.
- `docs/source/android_runbook.md` — runbooks operacionais diários.
- `docs/source/compliance/android/and6_compliance_checklist.md` — regulador
  inventário de artefatos.
- `docs/source/release_dual_track_runbook.md` — governança de liberação de via dupla.

## 1. Visão geral dos portões do palco

| Palco | Portões necessários | Evidência |
|-------|----------------|----------|
| **T−7 dias (pré-congelamento)** | Noturno `ci/run_android_tests.sh` verde por 14 dias; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` e `ci/check_android_docs_i18n.sh` passando; verificações de lint/dependência enfileiradas. | Painéis do Buildkite, relatório de comparação de equipamentos, exemplos de capturas de tela. |
| **T−3 dias (promoção RC)** | Reserva do laboratório do dispositivo confirmada; Execução de CI de atestado StrongBox (`scripts/android_strongbox_attestation_ci.sh`); Conjuntos robolétricos/instrumentados exercidos em hardware programado; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` limpo. | CSV da matriz do dispositivo, manifesto do pacote de atestado, relatórios Gradle arquivados em `artifacts/android/lint/<version>/`. |
| **T-1 dia (vá/não vá)** | Pacote de status de redação de telemetria atualizado (`scripts/telemetry/check_redaction_status.py --write-cache`); artefatos de conformidade atualizados de acordo com `and6_compliance_checklist.md`; ensaio de proveniência concluído (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, JSON de status de telemetria, log de simulação de procedência. |
| **T0 (transferência GA/LTS)** | `scripts/publish_android_sdk.sh --dry-run` concluído; procedência + SBOM assinado; lista de verificação de liberação exportada e anexada aos minutos de entrada/saída; `ci/sdk_sorafs_orchestrator.sh` fumaça verde. | Liberar anexos RFC, pacote Sigstore, artefatos de adoção sob `artifacts/android/`. |
| **T+1 dia (pós-transição)** | Prontidão do hotfix verificada (`scripts/publish_android_sdk.sh --validate-bundle`); diferenças do painel revisadas (`ci/check_android_dashboard_parity.sh`); pacote de evidências carregado em `status.md`. | Exportação de comparação do painel, link para entrada `status.md`, pacote de lançamento arquivado. |

## 2. CI e Matriz de Portão de Qualidade| Portão | Comando(s) / Script | Notas |
|------|--------------------|-------|
| Testes unitários + integração | `ci/run_android_tests.sh` (envolve `ci/run_android_tests.sh`) | Emite `artifacts/android/tests/test-summary.json` + log de teste. Inclui codec Norito, fila, fallback do StrongBox e testes de chicote do cliente Torii. Obrigatório todas as noites e antes da marcação. |
| Paridade de jogos | `ci/check_android_fixtures.sh` (envolve `scripts/check_android_fixtures.py`) | Garante que os fixtures Norito regenerados correspondam ao conjunto canônico Rust; anexe a comparação JSON quando o portão falhar. |
| Exemplos de aplicativos | `ci/check_android_samples.sh` | Constrói `examples/android/{operator-console,retail-wallet}` e valida capturas de tela localizadas por meio de `scripts/android_sample_localization.py`. |
| Documentos/I18N | `ci/check_android_docs_i18n.sh` | Guards README + guias de início rápido localizados. Execute novamente depois que as edições do documento chegarem ao branch de lançamento. |
| Paridade do painel | `ci/check_android_dashboard_parity.sh` | Confirma que as métricas CI/exportadas estão alinhadas com as contrapartes do Rust; exigido durante a verificação T+1. |
| Fumaça de adoção do SDK | `ci/sdk_sorafs_orchestrator.sh` | Exercita as ligações do orquestrador Sorafs de várias fontes com o SDK atual. Obrigatório antes de enviar artefatos preparados. |
| Verificação de atestado | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Agrega os pacotes de atestado StrongBox/TEE em `artifacts/android/attestation/**`; anexe o resumo aos pacotes GA. |
| Validação de slot de laboratório de dispositivo | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Valida pacotes de instrumentação antes de anexar evidências aos pacotes de liberação; O CI é executado no slot de amostra em `fixtures/android/device_lab/slot-sample` (telemetria/atestado/fila/logs + `sha256sum.txt`). |

> **Dica:** adicione esses trabalhos ao pipeline `android-release` Buildkite para que
> congelar semanas reexecuta automaticamente cada portão com a ponta do ramo de liberação.

A tarefa consolidada `.github/workflows/android-and6.yml` executa o lint,
verificações de conjunto de testes, resumo de atestado e slot de laboratório de dispositivo em cada PR/push
tocando fontes do Android, enviando evidências sob `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Verificações de lint e dependências

Execute `scripts/android_lint_checks.sh --version <semver>` na raiz do repositório. O
script é executado:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Relatórios e saídas de proteção de dependência são arquivados em
  `artifacts/android/lint/<label>/` e um link simbólico `latest/` para lançamento
  oleodutos.
- As falhas nas descobertas do lint exigem correção ou uma entrada na versão
  RFC documentando o risco aceito (aprovado pelo Release Engineering + Program
  Chumbo).
- `dependencyGuardBaseline` regenera o bloqueio de dependência; anexe o diferencial
  para o pacote go/no-go.

## 4. Laboratório de dispositivos e cobertura do StrongBox

1. Reserve dispositivos Pixel + Galaxy usando o rastreador de capacidade mencionado em
   `docs/source/compliance/android/device_lab_contingency.md`. Bloqueia lançamentos
   se ` para atualizar o relatório de atestado.
3. Execute a matriz de instrumentação (documente a lista de suíte/ABI no dispositivo
   rastreador). Capture falhas no log de incidentes mesmo se as novas tentativas forem bem-sucedidas.
4. Registre um ticket se for necessário recorrer ao Firebase Test Lab; vincular o ingresso
   na lista de verificação abaixo.

## 5. Artefatos de conformidade e telemetria- Siga `docs/source/compliance/android/and6_compliance_checklist.md` para a UE
  e submissões JP. Atualizar `docs/source/compliance/android/evidence_log.csv`
  com hashes + URLs de trabalho do Buildkite.
- Atualizar evidências de redação de telemetria por meio de
  `scripts/telemetria/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Armazene o JSON resultante em
  `artifacts/android/telemetry/<version>/status.json`.
- Registre a saída de comparação do esquema de
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  para provar a paridade com os exportadores de Rust.

## 6. Proveniência, SBOM e Publicação

1. Execute o pipeline de publicação:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Gerar proveniência SBOM + Sigstore:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Anexe `artifacts/android/provenance/<semver>/manifest.json` e assine
   `checksums.sha256` para a RFC de lançamento.
4. Ao promover para o repositório Maven real, execute novamente
   `scripts/publish_android_sdk.sh` sem `--dry-run`, capture o console
   log e carregue os artefatos resultantes em `artifacts/android/maven/<semver>`.

## 7. Modelo de pacote de envio

Cada versão GA/LTS/hotfix deve incluir:

1. **Lista de verificação preenchida** — copie a tabela deste arquivo, marque cada item e vincule
   para suportar artefatos (Buildkite run, logs, doc diffs).
2. **Evidência de laboratório do dispositivo** — resumo do relatório de atestado, registro de reserva e
   quaisquer ativações de contingência.
3. **Pacote de telemetria** — status de redação JSON, comparação de esquema, link para
   Atualizações `docs/source/sdk/android/telemetry_redaction.md` (se houver).
4. **Artefatos de conformidade** — entradas adicionadas/atualizadas na pasta de conformidade
   além do CSV de log de evidências atualizado.
5. **Pacote de proveniência** — SBOM, assinatura Sigstore e `checksums.sha256`.
6. **Resumo da versão** — visão geral de uma página anexada ao resumo `status.md`
   acima (data, versão, destaque de quaisquer portões dispensados).

Armazene o pacote em `artifacts/android/releases/<version>/` e faça referência a ele
em `status.md` e no RFC de lançamento.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` automaticamente
  copia o arquivo lint mais recente (`artifacts/android/lint/latest`) e o
  evidência de conformidade faça login em `artifacts/android/releases/<version>/` para que o
  o pacote de envio sempre tem uma localização canônica.

---

**Lembrete:** atualize esta lista de verificação sempre que novos trabalhos de CI, artefatos de conformidade,
ou requisitos de telemetria são adicionados. O item AND6 do roteiro permanece aberto até o
lista de verificação e automação associada permanecem estáveis por dois lançamentos consecutivos
trens.