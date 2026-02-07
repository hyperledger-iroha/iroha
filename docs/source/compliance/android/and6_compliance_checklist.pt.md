---
lang: pt
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-04T11:42:43.489571+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificação de conformidade do Android AND6

Esta lista de verificação rastreia os resultados de conformidade que atingem o marco **AND6 -
Reforço de CI e conformidade**. Consolida os artefatos regulatórios solicitados
em `roadmap.md` e define o layout de armazenamento em
`docs/source/compliance/android/` então Engenharia de Liberação, Suporte e Jurídico
pode fazer referência ao mesmo conjunto de evidências antes de aprovar versões do Android.

## Escopo e proprietários

| Área | Entregáveis ​​| Proprietário principal | Backup/Revisor |
|------|-------------|---------------|--------|
| Pacote regulamentar da UE | Meta de segurança ETSI EN 319 401, resumo GDPR DPIA, atestado SBOM, registro de evidências | Compliance & Jurídico (Sofia Martins) | Engenharia de Liberação (Alexei Morozov) |
| Pacote regulatório do Japão | Lista de verificação de controles de segurança FISC, pacotes de atestado StrongBox bilíngue, registro de evidências | Conformidade e Jurídico (Daniel Park) | Líder do programa Android |
| Preparação do laboratório de dispositivos | Acompanhamento de capacidade, gatilhos de contingência, registro de escalonamento | Líder de laboratório de hardware | Observabilidade do Android TL |

## Matriz de Artefatos| Artefato | Descrição | Caminho de armazenamento | Atualizar cadência | Notas |
|----------|------------|--------------|-----------------|-------|
| Meta de segurança ETSI EN 319 401 | Narrativa que descreve objetivos/suposições de segurança para binários do Android SDK. | `docs/source/compliance/android/eu/security_target.md` | Revalide todas as versões GA + LTS. | Deve citar hashes de origem de construção para o trem de lançamento. |
| Resumo do GDPR DPIA | Avaliação de impacto na proteção de dados abrangendo telemetria/registro. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Anual + antes de alterações na telemetria material. | Política de redação de referência em `sdk/android/telemetry_redaction.md`. |
| Atestado SBOM | Proveniência SBOM mais SLSA assinada para os artefatos Gradle/Maven. | `docs/source/compliance/android/eu/sbom_attestation.md` | Cada versão GA. | Execute `scripts/android_sbom_provenance.sh <version>` para gerar relatórios CycloneDX, pacotes de garantia e somas de verificação. |
| Lista de verificação de controles de segurança FISC | Concluído o mapeamento da lista de verificação dos controles do SDK para os requisitos do FISC. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | Anual + antes dos pilotos parceiros JP. | Fornece títulos bilíngues (EN/JP). |
| Pacote de atestado StrongBox (JP) | Resumo de atestado por dispositivo + cadeia para reguladores JP. | `docs/source/compliance/android/jp/strongbox_attestation.md` | Quando novo hardware entra no pool. | Aponte para artefatos brutos em `artifacts/android/attestation/<device>/`. |
| Memorando de assinatura legal | Resumo do conselho cobrindo o escopo do ETSI/GDPR/FISC, postura de privacidade e cadeia de custódia para artefatos anexados. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Cada vez que o pacote de artefatos muda ou uma nova jurisdição é adicionada. | O memorando faz referência a hashes do registro de evidências e links para o pacote de contingência do laboratório do dispositivo. |
| Registro de evidências | Índice de artefatos enviados com metadados de hash/timestamp. | `docs/source/compliance/android/evidence_log.csv` | Atualizado sempre que qualquer entrada acima for alterada. | Adicionar link do Buildkite + aprovação do revisor. |
| Pacote de instrumentação de laboratório de dispositivos | Evidências de telemetria, fila e atestado específicas do slot registradas com o processo definido em `device_lab_instrumentation.md`. | `artifacts/android/device_lab/<slot>/` (ver `docs/source/compliance/android/device_lab_instrumentation.md`) | Cada slot reservado + exercício de failover. | Capture manifestos SHA-256 e faça referência ao ID do slot no log de evidências + lista de verificação. |
| Registro de reserva do laboratório de dispositivos | Fluxo de trabalho de reserva, aprovações, instantâneos de capacidade e escada de escalonamento usados ​​para manter os pools do StrongBox ≥80% durante congelamentos. | `docs/source/compliance/android/device_lab_reservation.md` | Atualize sempre que as reservas forem criadas/alteradas. | Consulte os IDs de ticket `_android-device-lab` e a exportação de calendário semanal anotados no procedimento. |
| Runbook de failover de laboratório de dispositivos e pacote de detalhamento | Plano de ensaio trimestral e manifesto de artefato demonstrando rotas alternativas, fila de estouro do Firebase e prontidão de retenção externa do StrongBox. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Trimestralmente (ou após alterações na lista de hardware). | Registre os IDs de perfuração no log de evidências e anexe o hash do manifesto + a exportação do PagerDuty anotada no runbook. |

> **Dica:** ao anexar PDFs ou artefatos assinados externamente, armazene um breve
> Wrapper Markdown no caminho tabelado que vincula ao artefato imutável em
> a quota de governação. Isso mantém o repositório leve enquanto preserva o
> trilha de auditoria.

## Pacote Regulatório da UE (ETSI/GDPR)O pacote da UE une os três artefatos acima, além do memorando jurídico:

- Atualize `security_target.md` com o identificador de lançamento, hash de manifesto Torii,
  e resumo SBOM para que os auditores possam combinar os binários com o escopo declarado.
- Manter o resumo da DPIA alinhado com a política de redação de telemetria mais recente e
  anexe o trecho de comparação Norito referenciado em `docs/source/sdk/android/telemetry_redaction.md`.
- A entrada do atestado SBOM deve incluir: hash CycloneDX JSON, procedência
  hash do pacote, instrução cosign e o URL do trabalho Buildkite que os gerou.
- `legal_signoff_memo.md` deve capturar o conselho/data, listar cada artefato +
  SHA-256, descreva quaisquer controles de compensação e vincule à linha do registro de evidências
  mais o ID do ticket do PagerDuty que acompanhou a aprovação.

## Pacote Regulatório do Japão (FISC/StrongBox)

Os reguladores do Japão esperam um pacote paralelo com documentação bilíngue:

- `fisc_controls_checklist.md` espelha a planilha oficial; preencha ambos os
  Colunas EN e JA e referência à seção específica de `sdk/android/security.md`
  ou o pacote de atestado StrongBox que satisfaz cada controle.
- `strongbox_attestation.md` resume as últimas execuções de
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (envelopes JSON + Norito por dispositivo). Incorporar links para os artefatos imutáveis
  sob `artifacts/android/attestation/<device>/` e observe a cadência de rotação.
- Registre o modelo de carta de apresentação bilíngue que acompanha os envios dentro
  `docs/source/compliance/android/jp/README.md` para que o suporte possa reutilizá-lo.
- Atualize o registro de evidências com uma única linha que faça referência à lista de verificação, o
  hash do pacote de atestado e quaisquer IDs de ticket de parceiro JP vinculados à entrega.

## Fluxo de trabalho de envio

1. **Rascunho** - O proprietário prepara o artefato, registra o nome do arquivo planejado
   a tabela acima e abre um PR contendo o stub Markdown atualizado mais um
   soma de verificação do anexo externo.
2. **Revisão** – A Engenharia de Liberação confirma que os hashes de origem correspondem ao estágio
   binários; A conformidade verifica a linguagem regulatória; O suporte garante SLAs e
   as políticas de telemetria são referenciadas corretamente.
3. **Aprovação** – Os aprovadores adicionam seus nomes e datas à tabela `Sign-off`
   abaixo. O log de evidências é atualizado com o PR URL e a execução do Buildkite.
4. **Publicar** – Após a aprovação da governança do SRE, vincule o artefato em
   `status.md` e atualize as referências do Android Support Playbook.

### Registro de assinatura

| Artefato | Revisado por | Data | RP / Evidências |
|----------|------------|------|---------------|
| *(pendente)* | - | - | - |

## Reserva de laboratório de dispositivos e plano de contingência

Para mitigar o risco de **disponibilidade do laboratório de dispositivos** indicado no roteiro:- Rastreie a capacidade semanal em `docs/source/compliance/android/evidence_log.csv`
  (coluna `device_lab_capacity_pct`). Engenharia de liberação de alerta se houver disponibilidade
  cai abaixo de 70% durante duas semanas consecutivas.
- Reserve StrongBox/pistas gerais seguindo
  `docs/source/compliance/android/device_lab_reservation.md` à frente de todos
  congelamento, ensaio ou varredura de conformidade para que solicitações, aprovações e artefatos
  são capturados na fila `_android-device-lab`. Vincule os IDs dos tickets resultantes
  no registro de evidências ao registrar instantâneos de capacidade.
- **Pools substitutos:** burst para o pool de Pixel compartilhado primeiro; se ainda estiver saturado,
  agendar execuções de fumaça do Firebase Test Lab para validação de CI.
- **Retentor de laboratório externo:** mantenha o retentor com o parceiro StrongBox
  laboratório para que possamos reservar hardware durante janelas congeladas (mínimo de 7 dias).
- **Escalonamento:** levanta o incidente `AND6-device-lab` no PagerDuty quando ambos os
  os pools primários e de reserva ficam abaixo de 50% da capacidade. O líder do laboratório de hardware
  coordena com o SRE para redefinir a prioridade dos dispositivos.
- **Pacotes de evidências de failover:** armazene todos os ensaios em
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` com a reserva
  solicitação, exportação do PagerDuty, manifesto de hardware e transcrição de recuperação. Referência
  o pacote de `device_lab_contingency.md` e adicione o SHA-256 ao log de evidências
  para que o Departamento Jurídico possa provar que o fluxo de trabalho de contingência foi exercido.
- **Exercícios trimestrais:** exercite o runbook em
  `docs/source/compliance/android/device_lab_failover_runbook.md`, anexe o
  caminho do pacote resultante + hash de manifesto para o ticket `_android-device-lab` e
  espelhe o ID do exercício no log de contingência e no log de evidências.

Documente cada ativação do plano de contingência em
`docs/source/compliance/android/device_lab_contingency.md` (incluir data,
gatilho, ações e acompanhamentos).

## Protótipo de análise estática

- `make android-lint` envolve `ci/check_android_javac_lint.sh`, compilando
  `java/iroha_android` e as fontes compartilhadas `java/norito_java` com
  `javac --release 21 -Xlint:all -Werror` (com as categorias sinalizadas indicadas em
- Após a compilação, o script impõe a política de dependência AND6 com
  `jdeps --summary`, falhando se algum módulo estiver fora da lista de permissões aprovadas
  (`java.base`, `java.net.http`, `jdk.httpserver`) aparece. Isto mantém o
  Superfície Android alinhada com “nenhuma dependência oculta de JDK” do conselho do SDK
  requisito antes das revisões de conformidade do StrongBox.
- CI agora executa o mesmo portão via
  `.github/workflows/android-lint.yml`, que invoca
  `ci/check_android_javac_lint.sh` em cada push/PR que toca o Android ou
  fontes e uploads Java Norito compartilhados `artifacts/android/lint/jdeps-summary.txt`
  para que as revisões de conformidade possam fazer referência a uma lista de módulos assinados sem executar novamente o
  script localmente.
- Defina `ANDROID_LINT_KEEP_WORKDIR=1` quando precisar reter o temporário
  espaço de trabalho. O script já copia o resumo do módulo gerado para
  `artifacts/android/lint/jdeps-summary.txt`; definir
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (ou similar) quando você precisar de um artefato versionado adicional para auditorias.
  Os engenheiros ainda devem executar o comando localmente antes de enviar PRs do Android
  que tocam fontes Java e anexam o resumo/log registrado à conformidade
  comentários. Referencie-o nas notas de lançamento como “Android javac lint + dependência
  digitalizar”.

## Evidência de CI (Lint, testes, atestado)- `.github/workflows/android-and6.yml` agora executa todas as portas AND6 (javac lint +
  verificação de dependência, conjunto de testes Android, verificador de atestado StrongBox e
  validação de slot de laboratório de dispositivo) em cada PR/push tocando a superfície do Android.
- `ci/run_android_tests.sh` envolve `ci/run_android_tests.sh` e emite
  um resumo determinístico em `artifacts/android/tests/test-summary.json` enquanto
  persistindo o log do console para `artifacts/android/tests/test.log`. Anexe ambos
  arquivos para pacotes de conformidade ao fazer referência a execuções de CI.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` produz
  `artifacts/android/attestation/ci-summary.json`, validando o pacote
  cadeias de atestado sob `artifacts/android/attestation/**` para StrongBox e
  Piscinas TEE.
-`scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  verifica o slot de amostra (`slot-sample/`) usado no CI e pode ser apontado para
  real é executado sob `artifacts/android/device_lab/<slot-id>/` com
  `--require-slot --json-out <dest>` para provar que os pacotes de instrumentação seguem
  o layout documentado. CI escreve o resumo de validação para
  `artifacts/android/device_lab/summary.json`; o slot de amostra inclui
  extrações de telemetria/atestado/fila/log de espaço reservado mais um registro
  `sha256sum.txt` para hashes reproduzíveis.

## Fluxo de trabalho de instrumentação de laboratório de dispositivos

Cada ensaio de reserva ou failover deve seguir o
Guia `device_lab_instrumentation.md` para telemetria, fila e atestado
artefatos se alinham com o registro de reservas:

1. **Artefatos de slot de sementes.** Criar
   `artifacts/android/device_lab/<slot>/` com as subpastas padrão e execute
   `shasum` após o fechamento do slot (consulte a seção “Layout do artefato” do novo
   guia).
2. **Execute comandos de instrumentação.** Execute a captura de telemetria/fila,
   substituir o resumo, o chicote do StrongBox e a verificação de lint/dependência exatamente como
   documentado para que as saídas espelhem o CI.
3. **Arquivar evidências.** Atualização
   `docs/source/compliance/android/evidence_log.csv` e o bilhete de reserva
   com o ID do slot, caminho do manifesto SHA-256 e painel/Buildkite correspondente
   links.

Anexe a pasta do artefato e o manifesto hash ao pacote de lançamento AND6 para
a janela de congelamento afetada. Os revisores de governança rejeitarão listas de verificação que não
não cite um identificador de slot mais o guia de instrumentação.

### Evidência de reserva e prontidão para failover

O item do roteiro “Aprovações regulatórias de artefatos e contingência de laboratório” exige mais
do que instrumentação. Cada pacote AND6 também deve fazer referência ao protocolo proativo
fluxo de trabalho de reserva e ensaio trimestral de failover:- **Manual de reservas (`device_lab_reservation.md`).** Siga a reserva
  tabela (prazos de entrega, proprietários, comprimentos de slots), exporte o calendário compartilhado via
  `scripts/android_device_lab_export.py` e registro `_android-device-lab`
  IDs de tickets junto com instantâneos de capacidade em `evidence_log.csv`. O manual
  explica a escala de escalada e os gatilhos de contingência; copie esses detalhes
  na entrada da lista de verificação quando as reservas mudam ou a capacidade cai abaixo do
  Meta de roteiro de 80%.
- **Runbook de simulação de failover (`device_lab_failover_runbook.md`).** Execute o
  ensaio trimestral (simular interrupção → promover faixas alternativas → envolver
  Firebase burst + parceiro StrongBox externo) e armazene os artefatos em
  `artifacts/android/device_lab_contingency/<drill-id>/`. Cada pacote deve
  contém o manifesto, exportação do PagerDuty, links de execução do Buildkite, explosão do Firebase
  relatório e reconhecimento de retenção anotado no runbook. Faça referência ao
  ID de perfuração, manifesto SHA-256 e ticket de acompanhamento no registro de evidências e
  esta lista de verificação.

Juntos, esses documentos provam que o planejamento da capacidade do dispositivo, os ensaios de interrupção,
e os pacotes de instrumentação compartilham a mesma trilha auditada exigida pelo
roteiro e revisores jurídicos.

## Revise a cadência

- **Trimestralmente** - Validar se os artefatos da UE/JP estão atualizados; atualizar
  hashes de registro de evidências; ensaiar a captura de proveniência.
- **Pré-lançamento** - Execute esta lista de verificação durante cada transição do GA/LTS e anexe
  o log concluído para a RFC de lançamento.
- **Pós-incidente** - Se um incidente de setembro 1/2 envolver telemetria, assinatura ou
  atestado, atualize os stubs de artefato relevantes com notas de remediação e
  capture a referência no registro de evidências.