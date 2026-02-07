---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2026-01-03T18:07:59.202230+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Resumo GDPR DPIA – Telemetria Android SDK (AND7)

| Campo | Valor |
|-------|-------|
| Data de avaliação | 12/02/2026 |
| Atividade de processamento | Exportação de telemetria do Android SDK para back-ends OTLP compartilhados |
| Controladores/Processadores | SORA Nexus Operações (controlador), operadores parceiros (controladores conjuntos), Hyperledger Iroha Contribuintes (processador) |
| Documentos de referência | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Descrição do processamento

- **Objetivo:** Fornecer telemetria operacional necessária para dar suporte à observabilidade AND7 (latência, novas tentativas, integridade do atestado) enquanto espelha o esquema do nó Rust (§2 de `telemetry_redaction.md`).
- **Sistemas:** instrumentação Android SDK -> exportador OTLP -> coletor de telemetria compartilhado gerenciado pelo SRE (consulte o Support Playbook §8).
- **Titulares dos dados:** Equipe da operadora usando aplicativos baseados em Android SDK; pontos de extremidade Torii downstream (as cadeias de caracteres de autoridade são criptografadas por política de telemetria).

## 2. Inventário de dados e mitigações

| Canal | Campos | Risco de PII | Mitigação | Retenção |
|---------|--------|----------|------------|-----------|
| Traços (`android.torii.http.request`, `android.torii.http.retry`) | Rota, status, latência | Baixo (sem PII) | Hash de autoridade com Blake2b + sal rotativo; nenhum corpo de carga útil foi exportado. | 7–30 dias (por documento de telemetria). |
| Eventos (`android.keystore.attestation.result`) | Rótulo de alias, nível de segurança, resumo de atestado | Médio (dados operacionais) | Hash de alias (`alias_label`), `actor_role_masked` registrado para substituições com tokens de auditoria Norito. | 90 dias para sucesso, 365 dias para substituições/falhas. |
| Métricas (`android.pending_queue.depth`, `android.telemetry.export.status`) | Contagem de filas, status do exportador | Baixo | Somente contagens agregadas. | 90 dias. |
| Medidores de perfil de dispositivo (`android.telemetry.device_profile`) | SDK principal, nível de hardware | Médio | Bucketing (emulador/consumidor/empresa), sem OEM ou números de série. | 30 dias. |
| Eventos de contexto de rede (`android.telemetry.network_context`) | Tipo de rede, sinalizador de roaming | Médio | O nome da operadora foi totalmente eliminado; suporta requisitos de conformidade para evitar dados de assinantes. | 7 dias. |

## 3. Base legal e direitos

- **Base jurídica:** Interesse legítimo (artigo 6.º, n.º 1, alínea f)) — garantir o funcionamento fiável dos clientes do livro-razão regulamentado.
- **Teste de necessidade:** Métricas limitadas à saúde operacional (sem conteúdo do usuário); A autoridade hash garante paridade com nós Rust por meio de mapeamento reversível disponível apenas para pessoal de suporte autorizado (por meio de fluxo de trabalho de substituição).
- **Teste de balanceamento:** A telemetria tem como escopo dispositivos controlados pelo operador, não dados do usuário final. As substituições exigem artefatos Norito assinados revisados ​​pelo Support + Compliance (Support Playbook §3 + §9).
- **Direitos do titular dos dados:** Os operadores entram em contato com a Engenharia de Suporte (manual §2) para solicitar exportação/exclusão de telemetria. Substituições e registros de redação (documento de telemetria §Signal Inventory) permitem o cumprimento em 30 dias.

## 4. Avaliação de risco| Risco | Probabilidade | Impacto | Mitigação Residual |
|------|------------|--------|---------------------|
| Reidentificação através de autoridades com hash | Baixo | Médio | Rotação de sal registrada através de `android.telemetry.redaction.salt_version`; sais armazenados em cofre seguro; substitui auditados trimestralmente. |
| Impressão digital de dispositivos por meio de grupos de perfil | Baixo | Médio | Somente nível + SDK principal exportado; O Support Playbook proíbe solicitações de escalonamento para dados OEM/serial. |
| Substituir o uso indevido de vazamento de PII | Muito Baixo | Alto | As solicitações de substituição Norito registradas expiram em 24 horas e exigem aprovação do SRE (`docs/source/android_runbook.md` §3). |
| Armazenamento de telemetria fora da UE | Médio | Médio | Coletores destacados nas regiões UE + JP; política de retenção aplicada por meio da configuração de back-end OTLP (documentada no Support Playbook §8). |

O risco residual é considerado aceitável dados os controles acima e o monitoramento contínuo.

## 5. Ações e acompanhamentos

1. **Revisão trimestral:** Validar esquemas de telemetria, rotações salt e registros de substituição; documento em `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`.
2. **Alinhamento entre SDKs:** Coordene com os mantenedores de Swift/JS para manter regras de hashing/bucketing consistentes (rastreadas no roteiro AND7).
3. **Comunicações para parceiros:** Incluir o resumo da DPIA nos kits de integração do parceiro (Manual de suporte §9) e link para este documento de `status.md`.