---
lang: pt
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2026-01-03T18:07:59.262670+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook de simulação de failover do laboratório de dispositivos Android (AND6/AND7)

Este runbook captura o procedimento, os requisitos de evidências e a matriz de contato
usado ao exercer o **plano de contingência do laboratório do dispositivo** mencionado em
`roadmap.md` (§“Aprovações regulatórias de artefatos e contingência de laboratório”). Complementa
o fluxo de trabalho de reserva (`device_lab_reservation.md`) e o log de incidentes
(`device_lab_contingency.md`) para que revisores de conformidade, consultores jurídicos e SRE
temos uma única fonte de verdade sobre como validamos a prontidão para failover.

## Objetivo e cadência

- Demonstrar que os pools gerais de dispositivos do Android StrongBox + podem fazer failover
  para as pistas de pixel substitutas, pool compartilhado, fila intermitente do Firebase Test Lab e
  retentor externo do StrongBox sem falta de SLAs AND6/AND7.
- Produzir um pacote de evidências que o departamento jurídico possa anexar às submissões do ETSI/FISC
  antes da revisão de conformidade de fevereiro.
- Execute pelo menos uma vez por trimestre, além de sempre que a lista de hardware do laboratório mudar
  (novos dispositivos, desativação ou manutenção superior a 24h).

| ID de perfuração | Data | Cenário | Pacote de evidências | Estado |
|----------|------|----------|------|--------|
| DR-2026-02-Q1 | 20/02/2026 | Interrupção simulada da pista Pixel8Pro + backlog de atestado com ensaio de telemetria AND7 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Concluído — hashes de pacote registrados em `docs/source/compliance/android/evidence_log.csv`. |
| DR-2026-05-Q2 | 2026-05-22 (agendado) | Sobreposição de manutenção do StrongBox + ensaio Nexus | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(pendente)* — bilhete `_android-device-lab` **AND6-DR-202605** mantém as reservas; o pacote será preenchido após o exercício. | 🗓 Agendado — bloco de calendário adicionado ao “Android Device Lab – Reservas” por cadência AND6. |

## Procedimento

### 1. Preparação pré-perfuração

1. Confirme a capacidade da linha de base em `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. Exporte o calendário de reservas para a semana ISO alvo via
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. Arquivo do tíquete `_android-device-lab`
   `AND6-DR-<YYYYMM>` com escopo (“exercício de failover”), slots planejados e afetados
   cargas de trabalho (atestado, fumaça de CI, caos de telemetria).
4. Atualize o modelo de log de contingência em `device_lab_contingency.md` com um
   linha de espaço reservado para a data do exercício.

### 2. Simular condições de falha

1. Desative ou desassocia a pista primária (`pixel8pro-strongbox-a`) dentro do laboratório
   agendador e marque a entrada da reserva como “drill”.
2. Acione um alerta simulado de interrupção no PagerDuty (serviço `AND6-device-lab`) e
   capture a exportação de notificação para o pacote de evidências.
3. Anote os trabalhos do Buildkite que normalmente consomem a pista
   (`android-strongbox-attestation`, `android-ci-e2e`) com o ID da broca.

### 3. Execução de failover1. Promova a pista Pixel7 substituta para o alvo de CI primário e agende o
   cargas de trabalho planejadas contra ele.
2. Acione o pacote burst do Firebase Test Lab por meio da via `firebase-burst` para
   os testes de fumaça da carteira de varejo enquanto a cobertura do StrongBox muda para o compartilhado
   pista. Capture a invocação da CLI (ou exportação do console) no ticket para auditoria
   paridade.
3. Engate o retentor de laboratório externo do StrongBox para uma breve varredura de atestado;
   registre a confirmação do contato conforme descrito abaixo.
4. Registre todos os IDs de execução do Buildkite, URLs de trabalho do Firebase e transcrições de retenção em
   o ticket `_android-device-lab` e o manifesto do pacote de evidências.

### 4. Validação e reversão

1. Compare os tempos de execução do atestado/CI com a linha de base; sinalizar deltas >10% para o
   Líder de laboratório de hardware.
2. Restaure a via primária e atualize o instantâneo de capacidade mais a prontidão
   matriz assim que a validação for aprovada.
3. Anexe a linha final a `device_lab_contingency.md` com gatilho, ações,
   e acompanhamentos.
4. Atualize `docs/source/compliance/android/evidence_log.csv` com:
   caminho do pacote, manifesto SHA-256, IDs de execução do Buildkite, hash de exportação do PagerDuty e
   aprovação do revisor.

## Layout do pacote de evidências

| Arquivo | Descrição |
|------|-------------|
| `README.md` | Resumo (ID do exercício, escopo, proprietários, cronograma). |
| `bundle-manifest.json` | Mapa SHA-256 para cada arquivo do pacote. |
| `calendar-export.{ics,json}` | Calendário de reservas semanais ISO do script de exportação. |
| `pagerduty/incident_<id>.json` | Exportação de incidentes do PagerDuty mostrando alerta + cronograma de confirmação. |
| `buildkite/<job>.txt` | Buildkite executa URLs e logs para trabalhos afetados. |
| `firebase/burst_report.json` | Resumo da execução intermitente do Firebase Test Lab. |
| `retainer/acknowledgement.eml` | Confirmação do laboratório externo do StrongBox. |
| `photos/` | Fotos/capturas de tela opcionais da topologia do laboratório se o hardware tiver sido re-cabeado. |

Armazene o pacote em
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` e registro
a soma de verificação do manifesto dentro do registro de evidências mais a lista de verificação de conformidade AND6.

## Matriz de contato e escalonamento

| Função | Contato Primário | Canais | Notas |
|------|-----------------|------------|-------|
| Líder de laboratório de hardware | Priya Ramanathan | `@android-lab` Folga · +81-3-5550-1234 | Possui ações no local e atualizações de calendário. |
| Operações de laboratório de dispositivos | Mateus Cruz | Fila `_android-device-lab` | Coordena reservas de ingressos + uploads de pacotes. |
| Engenharia de Liberação | Alexei Morozov | Liberar folga de engenharia · `release-eng@iroha.org` | Valida evidências do Buildkite + publica hashes. |
| Laboratório externo de StrongBox | Sakura Instrumentos NOC | `noc@sakura.example` · +81-3-5550-9876 | Contato de retenção; confirme a disponibilidade dentro de 6h. |
| Coordenador do Firebase Burst | Tessa Wright | Folga `@android-ci` | Aciona a automação do Firebase Test Lab quando um substituto é necessário. |

Escale na seguinte ordem se um exercício descobrir problemas de bloqueio:
1. Líder de laboratório de hardware
2. Fundações Android TL
3. Líder do Programa/Engenharia de Liberação
4. Líder de conformidade + consultor jurídico (se o exercício revelar risco regulatório)

## Relatórios e acompanhamentos- Vincule este runbook ao procedimento de reserva sempre que fizer referência
  prontidão de failover em `roadmap.md`, `status.md` e pacotes de governança.
- Envie por e-mail o resumo trimestral do exercício para Compliance + Jurídico com o pacote de evidências
  tabela hash e anexe a exportação do ticket `_android-device-lab`.
- Espelhe as principais métricas (tempo até failover, cargas de trabalho restauradas, ações pendentes)
  dentro de `status.md` e do rastreador de lista de procurados AND7 para que os revisores possam rastrear o
  dependência de um ensaio concreto.