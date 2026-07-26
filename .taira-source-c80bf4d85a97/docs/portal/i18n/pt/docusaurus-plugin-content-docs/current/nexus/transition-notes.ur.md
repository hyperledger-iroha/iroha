---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: Nexus ٹرانزیشن نوٹس
description: `docs/source/nexus_transition_notes.md` کا آئینہ, جو Fase B ٹرانزیشن ثبوت, آڈٹ شیڈول, اور میٹیگیشنز کو کور کرتا ہے۔
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus ٹرانزیشن نوٹس

یہ لاگ **Fase B - Nexus Fundações de Transição** چیک لسٹ مکمل نہ ہو جائے۔ یہ `roadmap.md` میں marcos اندراجات کی تکمیل کرتا ہے اور B1-B4 میں حوالہ دی گئی evidência کو ایک جگہ رکھتا ہے تاکہ governança, SRE اور SDK لیڈز ایک ہی fonte da verdade شیئر کر سکیں۔

## اسکوپ اور cadência

- routed-trace آڈٹس اور guarda-corpos de telemetria (B1/B2), governança سے منظور شدہ config delta سیٹ (B3) ، اور acompanhamentos de ensaio de lançamento em várias pistas (B4) کو کور کرتا ہے۔
- یہ عارضی cadência نوٹ کی جگہ لیتا ہے جو پہلے یہاں تھا؛ Primeiro trimestre de 2026 mitigação رجسٹر رکھتا ہے۔
- ہر routed-trace ونڈو، governança ووٹ، یا ensaio de lançamento کے بعد ٹیبلز اپ ڈیٹ کریں۔ Você pode usar artefatos para obter documentos downstream (status, painéis, portais SDK) مستحکم âncora سے لنک کر سکیں۔

## Instantâneo de evidências (1º a 2º trimestre de 2026)

| ورک اسٹریم | ثبوت | Mala | اسٹیٹس | Não |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Semana (1º trimestre de 2026) | تین آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` کا TLS lag Q2 rerun میں بند ہوا۔ |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | مکمل | pacote de alerta, política de bot diff, tamanho do lote OTLP (log `nexus.scheduler.headroom` + painel de headroom Grafana) فراہم ہو چکے ہیں؛ کوئی واورز اوپن نہیں۔ |
| **B3 - Aprovações delta de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governança | مکمل | GOV-2026-03-19 e ریکارڈ ہے؛ pacote assinado نیچے e pacote de telemetria کو feed کرتا ہے۔ |
| **B4 - Ensaio de lançamento em várias pistas** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Semana (2º trimestre de 2026) | Reexecução canário do segundo trimestre com mitigação de atraso TLS manifesto do validador + `.sha256` نے slots 912-936, semente de carga de trabalho `NEXUS-REH-2026Q2` e reexecução میں ریکارڈ شدہ hash de perfil TLS کو captura کیا۔ |

## سہ ماہی routed-trace آڈٹ شیڈول

| ID de rastreamento | Tóquio (UTC) | Não | Não |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | پاس | Admissão de fila P95 ہدف <=750 ms سے کافی نیچے رہا۔ کوئی ایکشن درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | پاس | Hashes de repetição OTLP `status.md` کے ساتھ منسلک ہیں؛ Paridade do bot diff do SDK com desvio zero |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | حل شدہ | Repetição do TLS lag Q2 `NEXUS-REH-2026Q2` کے pacote de telemetria میں hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ریکارڈ ہے (دیکھیں `artifacts/nexus/tls_profile_rollout_2026q2/`) اور صفر retardatários ہیں۔ |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | پاس | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pacote de telemetria + manifesto/resumo `artifacts/nexus/rehearsals/2026q1/` میں (intervalo de slots 912-936) e agenda `artifacts/nexus/rehearsals/2026q2/` میں ہے۔ |

آنے والے سہ ماہیوں میں نئی قطاریں شامل کریں اور جب ٹیبل موجودہ سہ ماہی سے بڑا ہو جائے تو مکمل اندراجات کو apêndice میں منتقل کریں۔ routed-trace رپورٹس یا minutos de governança سے اس سیکشن کو `#quarterly-routed-trace-audit-schedule` âncora کے ذریعے ریفرنس کریں۔

## Mitigações de itens do backlog| آئٹم | تفصیل | Mal | ہدف | اسٹیٹس / نوٹس |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` کے دوران پیچھے رہ جانے والے perfil TLS کی propagação مکمل کریں, reexecutar captura de evidências کریں, e log de mitigação بند کریں۔ | @release-eng, @sre-core | Rastreamento roteado do segundo trimestre de 2026 | بند - Hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کو `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` میں captura کیا گیا؛ reexecutar نے zero retardatários کی تصدیق کی۔ |
| Preparação `TRACE-MULTILANE-CANARY` | Ensaio do segundo trimestre شیڈول کریں, pacote de telemetria کے ساتھ luminárias منسلک کریں, اور یقینی بنائیں کہ Chicotes SDK validar شدہ reutilização auxiliar کریں۔ | @telemetry-ops, Programa SDK | 30/04/2026 chamada de planejamento | مکمل - agenda `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے جس میں metadados de slot/carga de trabalho شامل ہے؛ rastreador de reutilização de chicote میں نوٹ ہے۔ |
| Rotação de resumo do pacote de telemetria | ہر ensaio/lançamento سے پہلے `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں اور digests کو config delta tracker کے ساتھ لاگ کریں۔ | @telemetria-ops | ہر candidato a lançamento | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (intervalo de slots `912-936`, semente `NEXUS-REH-2026Q2`); rastreador de resumos e índice de evidências میں کاپی کیے گئے۔ |

## Configurar integração do pacote delta

- Resumo de diferenças canônicas `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ہے۔ جب نئے `defaults/nexus/*.toml` یا mudanças de gênese آئیں تو پہلے rastreador اپ ڈیٹ کریں اور پھر خلاصہ یہاں شامل کریں۔
- Pacote de telemetria de ensaio de pacotes de configuração assinados کو feed کرتے ہیں۔ یہ pack, جو `scripts/telemetry/validate_nexus_telemetry_pack.py` سے validar ہوتا ہے, configurar evidência delta کے ساتھ publicar ہونا چاہیے تاکہ operadores B4 میں استعمال ہونے والے بالکل وہی reprodução de artefatos کر سکیں۔
- Iroha 2 pistas de pacotes کے بغیر رہتے ہیں: `nexus.enabled = false` e configurações de faixa/espaço de dados/substituições de roteamento کو rejeitar کرتی ہیں جب تک Perfil Nexus (`--sora`) ativar نہ ہو، اس لئے modelos de pista única سے seções `nexus.*` ہٹا دیں۔
- registro de votação de governança (GOV-2026-03-19) کو rastreador اور اس نوٹ دونوں سے لنک رکھیں تاکہ مستقبل کے votos اسی فارمیٹ کو دوبارہ دریافت کیے بغیر استعمال کر سکیں۔

## Lançar acompanhamentos de ensaio

- `docs/source/runbooks/nexus_multilane_rehearsal.md` canário پلان، lista de participantes e etapas de reversão کو محفوظ کرتا ہے؛ جب topologia de pista یا exportadores de telemetria بدلیں تو runbook اپ ڈیٹ کریں۔
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 اپریل کے ensaio کے ہر artefato کو لسٹ کرتا ہے اور اب notas de preparação/agenda do segundo trimestre بھی رکھتا ہے۔ مستقبل کے ensaios اسی rastreador میں شامل کریں تاکہ evidência monotônica رہے۔
- Trechos de coletor OTLP اور Grafana exportações (دیکھیں `docs/source/telemetry.md`) تب publicar کریں جب orientação de lote do exportador بدلے؛ Atualização do primeiro trimestre com alertas de headroom سے بچنے کے لئے tamanho do lote کو 256 amostras تک بڑھایا۔
- evidência de teste/CI multipista `pytests/nexus/test_multilane_pipeline.py` ریفرنس کو ریٹائر کیا؛ `defaults/nexus/config.toml` کے hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) کو rastreador کے ساتھ sincronização رکھیں جب pacotes de ensaio ریفریش ہوں۔

## Ciclo de vida da pista de tempo de execução

- Planos de ciclo de vida da pista de tempo de execução اب vinculações de espaço de dados validadas کرتے ہیں اور Kura/reconciliação de armazenamento em camadas falham ہونے پر abortar کر دیتے ہیں, جس سے catálogo بغیر تبدیلی کے رہتا ہے۔ ajudantes ریٹائر pistas کے relés em cache کو podar کرتے ہیں تاکہ síntese de merge-ledger پرانی reutilização de provas نہ کرے۔
- Auxiliares de configuração/ciclo de vida Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) کے ذریعے planos aplicam-se کریں تاکہ faixas کو reiniciar کے بغیر adicionar/retirar کیا جا سکے؛ roteamento, instantâneos TEU e registros de manifesto کامیاب plano کے بعد خودکار طور پر recarregar ہوتے ہیں۔
- Operadores کے لئے رہنمائی: اگر falha no plano ہو تو espaços de dados ausentes یا raízes de armazenamento چیک کریں جو بن نہیں سکتے (diretórios de raiz fria/Kura lane em camadas)۔ caminhos base کامیاب diferenças de telemetria de pista/espaço de dados دوبارہ emitir کرتے ہیں تاکہ painéis نئی topologia دکھا سکیں۔

## Telemetria NPoS e evidência de contrapressão

Ensaio de lançamento da Fase B retro نے capturas de telemetria determinística مانگے تھے جو ثابت کریں کہ Marcapasso NPoS اور camadas de fofoca اپنی contrapressão حدود میں رہتے ہیں۔ `integration_tests/tests/sumeragi_npos_performance.rs` é um conjunto de integração de cenários e cenários que contém métricas e resumos JSON (`sumeragi_baseline_summary::<scenario>::...`) emitem کرتا ہے۔ O que você precisa saber:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` یا `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کریں تاکہ زیادہ estresse e topologias دیکھ سکیں؛ valores padrão B4 میں استعمال ہونے والے 1 s/`k=3` coletor پروفائل کو refletir کرتے ہیں۔| Cenário/teste | Cobertura | Telemetria chave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | tempo de bloco de ensaio کے ساتھ 12 rodadas چلاتا ہے تاکہ Envelopes de latência EMA, profundidades de fila e medidores de envio redundante ریکارڈ ہوں, پھر serialização de pacote de evidências کیا جاتا ہے۔ | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | fila de transações کو بھر کر یقینی بناتا ہے کہ diferimentos de admissão acionados deterministicamente ہوں اور capacidade da fila/exportação de contadores de saturação کرے۔ | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | jitter do marcapasso اور visualizar amostra de tempos limite کرتا ہے جب تک +/-125 permille | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | بڑے Cargas úteis RBC کو armazenar کی soft/hard حدود تک push کرتا ہے تاکہ sessões اور contadores de bytes کا بڑھنا, واپس آنا، اور بغیر overflow کے estabilizar ہونا دکھایا جا سکے۔ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | retransmite força کرتا ہے تاکہ medidores de taxa de envio redundante اور contadores de coletores no alvo آگے بڑھیں, اور ظاہر ہو کہ retro والی telemetria ponta a ponta جڑی ہے۔ | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | pedaços deterministicamente espaçados caem کرتا ہے تاکہ monitores de backlog خاموش dreno کے بجائے falhas aumentam کریں۔ | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

جب بھی governança یہ ثبوت مانگے کہ alarmes de contrapressão topologia de ensaio سے correspondência کرتے ہیں, chicote کے پرنٹ شدہ linhas JSON کے ساتھ Prometheus raspar بھی منسلک کریں۔

## Lista de verificação de atualização

1. O routed-trace é um recurso de rastreamento roteado que pode ser usado para configurar o recurso de rastreamento
2. Acompanhamento do Alertmanager کے بعد tabela de mitigação اپ ڈیٹ کریں, چاہے ticket de ação بند کرنا ہی کیوں نہ ہو۔
3. جب config deltas بدلیں، tracker, یہ نوٹ، اور telemetry pack digests کی فہرست کو اسی pull request میں اپ ڈیٹ کریں۔
4. کسی بھی نئے artefato de ensaio/telemetria کو یہاں لنک کریں تاکہ مستقبل کی atualizações de roteiro ایک ہی دستاویز کو ریفرنس کریں, بکھری ہوئی ad-hoc نوٹس نہیں۔

## Índice de evidências

| اثاثہ | مقام | Não |
|-------|----------|-------|
| Relatório de auditoria de rastreamento roteado (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Evidência da Fase B1 کے لئے fonte canônica؛ پورٹل پر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` Espelho de espelho ہے۔ |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Resumos de comparação TRACE-CONFIG-DELTA, revisores کے iniciais, اور GOV-2026-03-19 registro de votação شامل ہیں۔ |
| Plano de remediação de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | pacote de alerta, tamanho do lote OTLP, e B2 سے جڑی grades de proteção do orçamento de exportação کو دستاویز کرتا ہے۔ |
| Rastreador de ensaio com várias pistas | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9. Artefatos de ensaio, manifesto/resumo do validador, notas/agenda do segundo trimestre e evidências de reversão. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | intervalo de slots 912-936, seed `NEXUS-REH-2026Q2` e pacotes de governança کے hashes de artefatos ریکارڈ کرتا ہے۔ |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Reexecução do segundo trimestre کے دوران captura شدہ منظور شدہ hash de perfil TLS؛ apêndices de rastreamento roteado میں citar کریں۔ |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Planejamento de ensaio do segundo trimestre نوٹس (gama de slots, semente de carga de trabalho, proprietários de ação) ۔ |
| Lançar manual de ensaios | `docs/source/runbooks/nexus_multilane_rehearsal.md` | teste -> execução -> reversão کے لئے عملی checklist؛ topologia de pista یا orientação do exportador بدلنے پر اپ ڈیٹ کریں۔ |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro میں حوالہ دیا گیا CLI؛ pacote بدلنے پر resumos کو rastreador کے ساتھ آرکائیو کریں۔ |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | configurações multi-lane کے لئے `nexus.enabled = true` ثابت کرتا ہے, hashes de catálogo Sora محفوظ رکھتا ہے, اور `ConfigLaneRouter` کے ذریعے lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) provision کر کے artefato digests شائع کرتا ہے۔ |