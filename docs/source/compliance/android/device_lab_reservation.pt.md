---
lang: pt
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2026-01-03T18:07:59.245516+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Procedimento de reserva do laboratório de dispositivos Android (AND6/AND7)

Este manual descreve como a equipe do Android reserva, confirma e audita dispositivos
tempo de laboratório para marcos **AND6** (CI e reforço de conformidade) e **AND7**
(prontidão para observabilidade). Complementa o log de contingência
`docs/source/compliance/android/device_lab_contingency.md` garantindo capacidade
as deficiências são evitadas em primeiro lugar.

## 1. Metas e escopo

- Mantenha os pools gerais de dispositivos StrongBox + acima dos 80% exigidos pelo roteiro
  meta de capacidade durante as janelas de congelamento.
- Forneça um calendário determinístico para CI, varreduras de atestado e caos
  os ensaios nunca competem pelo mesmo hardware.
- Capture uma trilha auditável (solicitações, aprovações, notas pós-execução) que alimenta
  a lista de verificação de conformidade AND6 e o registro de evidências.

Este procedimento abrange as pistas Pixel dedicadas, o pool de fallback compartilhado e
o retentor externo do laboratório StrongBox mencionado no roteiro. Emulador ad hoc
o uso está fora do escopo.

## 2. Janelas de Reserva

| Piscina / Pista | Ferragens | Comprimento padrão do slot | Prazo de entrega da reserva | Proprietário |
|---------|----------|---------------------|-------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (Caixa Forte) | 4h | 3 dias úteis | Líder de laboratório de hardware |
| `pixel8a-ci-b` | Pixel8a (CI geral) | 2h | 2 dias úteis | Fundações Android TL |
| `pixel7-fallback` | Piscina compartilhada Pixel7 | 2h | 1 dia útil | Engenharia de Liberação |
| `firebase-burst` | Fila de fumaça do Firebase Test Lab | 1h | 1 dia útil | Fundações Android TL |
| `strongbox-external` | Retentor externo de laboratório StrongBox | 8h | 7 dias corridos | Líder do Programa |

Os slots são reservados em UTC; reservas sobrepostas exigem aprovação explícita
do líder do laboratório de hardware.

## 3. Fluxo de trabalho de solicitação

1. **Prepare o contexto**
   - Atualize `docs/source/sdk/android/android_strongbox_device_matrix.md` com
     os dispositivos que você planeja exercitar e a etiqueta de prontidão
     (`attestation`, `ci`, `chaos`, `partner`).
   - Colete o instantâneo de capacidade mais recente de
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **Enviar solicitação**
   - Arquive um ticket na fila `_android-device-lab` usando o modelo em
     `docs/examples/android_device_lab_request.md` (proprietário, datas, cargas de trabalho,
     requisito de reserva).
   - Anexe quaisquer dependências regulatórias (por exemplo, varredura de atestado AND6, AND7
     exercício de telemetria) e link para a entrada relevante do roteiro.
3. **Aprovação**
   - O líder do laboratório de hardware analisa dentro de um dia útil, confirma a vaga no
     calendário compartilhado (`Android Device Lab – Reservations`) e atualiza o
     Coluna `device_lab_capacity_pct` em
     `docs/source/compliance/android/evidence_log.csv`.
4. **Execução**
   - Execute os jobs agendados; registrar IDs de execução do Buildkite ou logs de ferramentas.
   - Anote quaisquer desvios (trocas de hardware, ultrapassagens).
5. **Encerramento**
   - Comente o ticket com artefatos/links.
   - Se a execução estiver relacionada à conformidade, atualize
     `docs/source/compliance/android/and6_compliance_checklist.md` e adicione uma linha
     para `evidence_log.csv`.

Solicitações que impactem demonstrações de parceiros (AND8) devem enviar cópia para Partner Engineering.

## 4. Alteração e cancelamento- **Reagendar:** reabrir o ticket original, propor um novo horário e atualizar o
  entrada do calendário. Se o novo slot ocorrer dentro de 24 horas, execute ping para líder do laboratório de hardware + SRE
  diretamente.
- **Cancelamento de emergência:** siga o plano de contingência
  (`device_lab_contingency.md`) e registre as linhas de gatilho/ação/acompanhamento.
- **Excessos:** se uma corrida exceder seu intervalo em >15 minutos, publique uma atualização e confirme
  se a próxima reserva pode prosseguir; caso contrário, passe para o substituto
  pool ou pista de explosão do Firebase.

## 5. Evidências e Auditoria

| Artefato | Localização | Notas |
|----------|----------|-------|
| Reserva de bilhetes | Fila `_android-device-lab` (Jira) | Exportar resumo semanal; vincular IDs de tickets no registro de evidências. |
| Exportação de calendário | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | Execute `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` todas as sextas-feiras; o auxiliar salva o arquivo `.ics` filtrado mais um resumo JSON para a semana ISO para que as auditorias possam anexar ambos os artefatos sem downloads manuais. |
| Instantâneos de capacidade | `docs/source/compliance/android/evidence_log.csv` | Atualização após cada reserva/fechamento. |
| Notas pós-execução | `docs/source/compliance/android/device_lab_contingency.md` (se contingência) ou comentário do ticket | Obrigatório para auditorias. |

Durante as revisões trimestrais de conformidade, anexe a exportação do calendário, o resumo do ticket,
e trecho do registro de evidências para o envio da lista de verificação AND6.

### Automação de exportação de calendário

1. Obtenha o URL do feed ICS (ou baixe um arquivo `.ics`) para “Android Device Lab – Reservas”.
2. Executar

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   O script grava `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   e `...-calendar.json`, capturando a semana ISO selecionada.
3. Carregue os arquivos gerados com o pacote de evidências semanais e faça referência ao
   Resumo JSON em `docs/source/compliance/android/evidence_log.csv` quando
   registrar a capacidade do laboratório do dispositivo.

## 6. Escada de escalada

1. Líder de laboratório de hardware (primário)
2. Fundações Android TL
3. Líder do programa/Engenharia de liberação (para janelas congeladas)
4. Contato externo do laboratório StrongBox (quando o retentor é invocado)

Os escalonamentos devem ser registrados no ticket e espelhados no Android semanal
correio de status.

## 7. Documentos Relacionados

- `docs/source/compliance/android/device_lab_contingency.md` — registro de incidentes para
  deficiências de capacidade.
- `docs/source/compliance/android/and6_compliance_checklist.md` — mestre
  lista de verificação de entregas.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — hardware
  rastreador de cobertura.
-`docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  Evidência de atestado do StrongBox referenciada por AND6/AND7.

A manutenção deste procedimento de reserva satisfaz o item de ação do roteiro “definir
procedimento de reserva de laboratório de dispositivos” e mantém artefatos de conformidade voltados para o parceiro
em sincronia com o restante do plano de preparação do Android.

## 8. Procedimento e contatos de simulação de failover

O item AND6 do roteiro também requer um ensaio trimestral de failover. O completo,
instruções passo a passo ao vivo em
`docs/source/compliance/android/device_lab_failover_runbook.md`, mas o alto
O fluxo de trabalho de nível está resumido abaixo para que os solicitantes possam planejar exercícios juntamente com
reservas de rotina.1. **Agende o exercício:** Bloqueie as pistas afetadas (`pixel8pro-strongbox-a`,
   pool de fallback, `firebase-burst`, retentor StrongBox externo) no compartilhado
   calendário e fila `_android-device-lab` pelo menos 7 dias antes do exercício.
2. **Simular interrupção:** Depool a pista primária, acione o PagerDuty
   (`AND6-device-lab`) e anote os trabalhos dependentes do Buildkite com
   o ID do exercício anotado no runbook.
3. **Fail over:** Promova a via de fallback do Pixel7, inicie o Firebase burst
   suíte e envolva o parceiro externo do StrongBox dentro de 6 horas. Capturar
   URLs de execução do Buildkite, exportações do Firebase e reconhecimentos de retenção.
4. **Validar e restaurar:** Verifique o atestado + tempos de execução de CI, restabeleça o
   pistas originais e atualização `device_lab_contingency.md` mais o registro de evidências
   com o caminho do pacote + somas de verificação.

### Referência de contato e escalonamento

| Função | Contato Primário | Canais | Ordem de escalonamento |
|------|-----------------|------------|------------------|
| Líder de laboratório de hardware | Priya Ramanathan | `@android-lab` Folga · +81-3-5550-1234 | 1 |
| Operações de laboratório de dispositivos | Mateus Cruz | Fila `_android-device-lab` | 2 |
| Fundações Android TL | Elena Vorobeva | Folga `@android-foundations` | 3 |
| Engenharia de Liberação | Alexei Morozov | `release-eng@iroha.org` | 4 |
| Laboratório externo de StrongBox | Sakura Instrumentos NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

Escale sequencialmente se o exercício descobrir problemas de bloqueio ou se houver algum substituto
A pista não pode ser colocada online em 30 minutos. Sempre registre o escalonamento
notas no ticket `_android-device-lab` e espelhe-as no log de contingência.