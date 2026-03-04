---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Alvo de segurança do Android SDK – Alinhamento ETSI EN 319 401

| Campo | Valor |
|-------|-------|
| Versão do documento | 0,1 (12/02/2026) |
| Escopo | Android SDK (bibliotecas cliente sob `java/iroha_android/` mais scripts/documentos de suporte) |
| Proprietário | Compliance & Jurídico (Sofia Martins) |
| Revisores | Líder do programa Android, engenharia de lançamento, governança de SRE |

## 1. Descrição do dedo do pé

O Target of Evaluation (TOE) compreende o código da biblioteca Android SDK (`java/iroha_android/src/main/java`), sua superfície de configuração (`ClientConfig` + ingestão Norito) e as ferramentas operacionais referenciadas em `roadmap.md` para marcos AND2/AND6/AND7.

Componentes primários:

1. **Ingestão de configuração** — `ClientConfig` encadeia endpoints Torii, políticas TLS, novas tentativas e ganchos de telemetria do manifesto `iroha_config` gerado e impõe imutabilidade pós-inicialização (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`).
2. **Gerenciamento de chaves/StrongBox** — A assinatura apoiada por hardware é implementada via `SystemAndroidKeystoreBackend` e `AttestationVerifier`, com políticas documentadas em `docs/source/sdk/android/key_management.md`. A captura/validação de atestado usa `scripts/android_keystore_attestation.sh` e o auxiliar de CI `scripts/android_strongbox_attestation_ci.sh`.
3. **Telemetria e redação** — A instrumentação canaliza por meio do esquema compartilhado descrito em `docs/source/sdk/android/telemetry_redaction.md`, exportando autoridades com hash, perfis de dispositivos agrupados e substituindo ganchos de auditoria aplicados pelo Manual de suporte.
4. **Runbooks de operações** — `docs/source/android_runbook.md` (resposta do operador) e `docs/source/android_support_playbook.md` (SLA + escalonamento) fortalecem a pegada operacional do TOE com substituições determinísticas, exercícios de caos e captura de evidências.
5. **Procedência da versão** — As compilações baseadas em Gradle usam o plug-in CycloneDX, além de sinalizadores de compilação reproduzíveis, conforme capturado em `docs/source/sdk/android/developer_experience_plan.md` e na lista de verificação de conformidade AND6. Os artefatos de liberação são assinados e referenciados em `docs/source/release/provenance/android/`.

## 2. Ativos e premissas

| Ativo | Descrição | Objetivo de segurança |
|-------|-------------|--------------------|
| Manifestos de configuração | Instantâneos `ClientConfig` derivados de Norito distribuídos com aplicativos. | Autenticidade, integridade e confidencialidade em repouso. |
| Chaves de assinatura | Chaves geradas ou importadas através de provedores StrongBox/TEE. | Preferência do StrongBox, registro de atestado, sem exportação de chave. |
| Fluxos de telemetria | Rastreios/logs/métricas OTLP exportados da instrumentação do SDK. | Pseudonimização (autoridades com hash), PII minimizadas, substituem a auditoria. |
| Interações contábeis | Cargas úteis Norito, metadados de admissão, tráfego de rede Torii. | Autenticação mútua, solicitações resistentes à repetição, novas tentativas determinísticas. |

Suposições:

- O sistema operacional móvel fornece sandboxing padrão + SELinux; Os dispositivos StrongBox implementam a interface keymaster do Google.
- As operadoras fornecem endpoints Torii com certificados TLS assinados por CAs confiáveis.
- A infraestrutura de construção atende aos requisitos de construção reproduzível antes de publicar no Maven.

## 3. Ameaças e controles| Ameaça | Controle | Evidência |
|--------|---------|----------|
| Manifestos de configuração adulterados | `ClientConfig` valida manifestos (hash + esquema) antes de aplicar e registra recargas negadas por meio de `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| Compromisso de assinatura de chaves | Políticas exigidas pelo StrongBox, equipamentos de atestado e auditorias de matriz de dispositivos identificam desvios; substituições documentadas por incidente. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| Vazamento de PII em telemetria | Autoridades com hash Blake2b, perfis de dispositivos agrupados, omissão de operadora, registro de substituição. | `docs/source/sdk/android/telemetry_redaction.md`; Manual de suporte §8. |
| Repetir ou fazer downgrade em Torii RPC | O construtor de solicitações `/v1/pipeline` impõe fixação de TLS, política de canal de ruído e orçamentos de repetição com contexto de autoridade com hash. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (planejado). |
| Lançamentos não assinados ou não reproduzíveis | Atestados CycloneDX SBOM + Sigstore controlados pela lista de verificação AND6; liberar RFCs exigem evidências em `docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| Tratamento incompleto de incidentes | Runbook + playbook definem substituições, exercícios de caos e árvore de escalonamento; substituições de telemetria exigem solicitações Norito assinadas. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. Atividades de avaliação

1. **Revisão de projeto** — Conformidade + SRE verificam se a configuração, o gerenciamento de chaves, a telemetria e os controles de liberação correspondem aos objetivos de segurança do ETSI.
2. **Verificações de implementação** — Testes automatizados:
   - `scripts/android_strongbox_attestation_ci.sh` verifica pacotes capturados para cada dispositivo StrongBox listado na matriz.
   - `scripts/check_android_samples.sh` e Managed Device CI garantem que os aplicativos de amostra cumpram os contratos `ClientConfig`/telemetria.
3. **Validação operacional** — Exercícios de caos trimestrais de acordo com `docs/source/sdk/android/telemetry_chaos_checklist.md` (exercícios de redação + substituição).
4. **Retenção de evidências** — Artefatos armazenados em `docs/source/compliance/android/` (esta pasta) e referenciados em `status.md`.

## 5. Mapeamento ETSI EN 319 401| Cláusula EN 319 401 | Controle SDK |
|-------------------|-------------|
| 7.1 Política de segurança | Documentado neste alvo de segurança + Manual de suporte. |
| 7.2 Segurança organizacional | Propriedade de plantão RACI + no Support Playbook §2. |
| 7.3 Gestão de ativos | Objetivos de configuração, chave e ativos de telemetria definidos no §2 acima. |
| 7.4 Controle de acesso | Políticas do StrongBox + fluxo de trabalho de substituição que exige artefatos Norito assinados. |
| 7.5 Controles criptográficos | Requisitos de geração, armazenamento e atestado de chaves do AND2 (guia de gerenciamento de chaves). |
| 7.6 Segurança das operações | Hashing de telemetria, ensaios de caos, resposta a incidentes e liberação de evidências. |
| 7.7 Segurança das comunicações | Política TLS `/v1/pipeline` + autoridades com hash (documento de redação de telemetria). |
| 7.8 Aquisição/desenvolvimento de sistema | Construções Gradle reproduzíveis, SBOMs e portas de proveniência em planos AND5/AND6. |
| 7.9 Relacionamento com fornecedores | Atestados Buildkite + Sigstore registrados junto com SBOMs de dependência de terceiros. |
| 7.10 Gestão de incidentes | Escalonamento de Runbook/Playbook, registro de substituição, contadores de falhas de telemetria. |

## 6. Manutenção

- Atualize este documento sempre que o SDK introduzir novos algoritmos criptográficos, categorias de telemetria ou alterações na automação de lançamento.
- Vincule cópias assinadas em `docs/source/compliance/android/evidence_log.csv` com resumos SHA-256 e assinaturas do revisor.