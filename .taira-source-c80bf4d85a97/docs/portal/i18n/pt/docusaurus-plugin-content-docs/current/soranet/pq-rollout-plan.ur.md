---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: Manual de implementação pós-quântica SNNet-16G
sidebar_label: Plano de implementação PQ
descrição: SoraNet کے handshake híbrido X25519 + ML-KEM کو canary سے padrão تک relés, clientes اور SDKs میں promover کرنے کے لئے عملی رہنمائی۔
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentação retirar نہ ہو، دونوں کاپیاں sincronização رکھیں۔
:::

Transporte SNNet-16G SoraNet para implementação pós-quântica Operadores de botões `rollout_phase` کو coordenada de promoção determinística کرنے دیتے ہیں جو موجودہ Requisito de guarda do Estágio A سے Cobertura majoritária do Estágio B اور Postura PQ estrita do Estágio C تک جاتی ہے، بغیر ہر superfície کے JSON/TOML bruto کو editar کئے۔

یہ playbook درج ذیل چیزیں capa کرتا ہے:

- Definições de fase com botões de configuração (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) e base de código com fio (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeamento de sinalizadores SDK ou CLI تاکہ ہر implementação do cliente کو faixa کر سکے۔
- Expectativas de agendamento canário de retransmissão/cliente اور painéis de governança جو promoção کو portão کرتے ہیں (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de reversão e referências de runbook de simulação de incêndio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases

| `rollout_phase` | Estágio de anonimato efetivo | Efeito padrão | Uso típico |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | فلیٹ کے گرم ہونے تک ہر circuito کے لئے کم از کم ایک Guarda PQ لازم کریں۔ | Linha de base اور ابتدائی canário ہفتے۔ |
| `ramp` | `anon-majority-pq` (Estágio B) | Seus relés PQ têm polarização کریں تاکہ >= دو تہائی cobertura ہو؛ fallback de relés clássicos رہیں۔ | Canários de retransmissão região por região; A visualização do SDK alterna ۔ |
| `default` | `anon-strict-pq` (Estágio C) | Circuitos somente PQ impõem alarmes de کریں اور downgrade سخت کریں۔ | Telemetria e aprovação de governança مکمل ہونے کے بعد آخری promoção۔ |

اگر کوئی superfície explícita `anonymity_policy` بھی set کرے تو وہ اس componente کے لئے fase کو override کرتا ہے۔ Estágio explícito نہ ہو تو اب `rollout_phase` valor پر defer ہوتا ہے تاکہ operadores ہر ambiente میں ایک بار phase flip کریں اور clientes اسے herdar کریں۔

## Referência de configuração

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Tempo de execução do carregador do orquestrador میں resolução de estágio de fallback کرتا ہے (`crates/sorafs_orchestrator/src/lib.rs:2229`) اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے superfície کرتا ہے۔ `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` contém trechos prontos para aplicação.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب registro de fase analisado کرتا ہے (`crates/iroha/src/client.rs:2315`) تاکہ comandos auxiliares (مثال کے طور پر `iroha_cli app sorafs fetch`) موجودہ fase کو política de anonimato padrão کے ساتھ relatório کر سکیں۔

## Automação

دو `cargo xtask` ajudantes agendam geração اور captura de artefato automatiza کرتے ہیں۔

1. **Programação regional gera کریں**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Durações `s`, `m`, `h`, یا `d` sufixo قبول کرتے ہیں۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` e Markdown summary (`artifacts/soranet_pq_rollout_plan.md`) emite کرتی ہے جسے solicitação de alteração کے ساتھ بھیجا جا سکتا ہے۔

2. **Perfurar artefatos کو assinaturas کے ساتھ capturar کریں**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   کمانڈ فراہم کردہ فائلیں `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں کاپی کرتی ہے، ہر artefato کے لئے BLAKE3 digere computação کرتی ہے، Para `rollout_capture.json`, você precisa de metadados e carga útil para assinatura Ed25519. اسی chave privada کو استعمال کریں جو minuto de treinamento de incêndio assinar کرتی ہے تاکہ governança جلدی validar کر سکے۔

## Matriz de sinalizadores SDK e CLI

| Superfície | Canário (Fase A) | Rampa (Etapa B) | Padrão (Fase C) |
|---------|------------------|----------------|------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` fase de fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuração do orquestrador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuração do cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (padrão) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos assinados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ajudantes do orquestrador JavaScript | `rolloutPhase: "canary"` ou `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تمام SDK alterna اسی analisador de estágio سے mapa ہوتی ہیں جو orquestrador استعمال کرتا ہے (`crates/sorafs_orchestrator/src/lib.rs:365`), لہذا implantações multilíngues configuradas fase کے ساتھ lock-step رہتے ہیں۔

## Lista de verificação de agendamento canário

1. **Pré-voo (T menos 2 semanas)**

- Confirmar کریں کہ Taxa de queda de energia do Estágio A پچھلے دو ہفتوں میں =70% ہے (`sorafs_orchestrator_pq_candidate_ratio`).
   - Cronograma de slots de revisão de governança کریں جو janela canário aprovar کرتا ہے۔
   - Preparação da atualização `sorafs.gateway.rollout_phase = "ramp"` کریں (edição JSON do orquestrador کر کے reimplantação) اور pipeline de promoção کو simulação de execução کریں۔

2. **Retransmissão canário (dia T)**

   - ایک وقت میں ایک promoção da região کریں, `rollout_phase = "ramp"` کو orquestrador اور manifestos de retransmissão participantes پر conjunto کرتے ہوئے۔
   - Painel PQ Ratchet پر "Eventos de política por resultado" اور "Brownout Rate" کو guard cache TTL کے دوگنے عرصے تک monitor کریں (painel اب painel de implementação دکھاتا ہے)۔
   - Armazenamento de auditoria کے لئے `sorafs_cli guard-directory fetch` snapshots پہلے اور بعد میں لیں۔

3. **Cliente/SDK canário (T mais 1 semana)**

   - Configurações do cliente میں `rollout_phase = "ramp"` flip کریں یا منتخب SDK cohorts کے لئے `stage-b` substituições دیں۔
   - Captura de diferenças de telemetria کریں (`sorafs_orchestrator_policy_events_total` کو `client_id` اور `region` کے حساب سے grupo کریں) اور انہیں log de incidentes de implementação کے ساتھ anexar کریں۔

4. **Promoção padrão (T mais 3 semanas)**- Aprovação de governança کے بعد orquestrador اور configurações do cliente دونوں کو `rollout_phase = "default"` پر switch کریں اور lista de verificação de prontidão assinada کو liberar artefatos میں girar کریں۔

## Lista de verificação de governança e evidências

| Mudança de fase | Portão de promoção | Pacote de evidências | Painéis e alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | Taxa de brownout do estágio A پچھلے 14 دن میں = 0,7 فی região promovida, verificação de ticket Argon2 p95  Padrão *(aplicação do Estágio C)* | Burn-in de telemetria SN16 de 30 dias مکمل, `sn16_handshake_downgrade_total` linha de base پر flat, cliente canário کے دوران `sorafs_orchestrator_brownouts_total` صفر, اور proxy toggle log de ensaio۔ | Transcrição `sorafs_cli proxy set-mode --mode gateway|direct`, saída `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log `sorafs_cli guard-directory verify`, e pacote `cargo xtask soranet-rollout-capture --label default` assinado | Placa de catraca PQ e painéis de downgrade SN16 e `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json` Documentados ہیں۔ |
| Preparação para rebaixamento/reversão de emergência | Gatilho تب ہوتا ہے جب pico de contadores de downgrade کریں, falha na verificação do diretório de proteção ہو، یا Buffer `/policy/proxy-toggle` مسلسل eventos de downgrade ریکارڈ کرے۔ | Lista de verificação `docs/source/ops/soranet_transport_rollback.md`, registros `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes, modelos de notificação | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, outros pacotes de alerta (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- ہر artefato کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں store کریں، `rollout_capture.json` gerado کے ساتھ، تاکہ pacotes de governança میں placar, rastreamentos de promtool, اور digests شامل ہوں۔
- Evidência carregada (PDF de minutos, pacote de captura, instantâneos de proteção) کے SHA256 resume minutos de promoção کے ساتھ anexar کریں تاکہ Cluster de preparação de aprovações do Parlamento تک رسائی کے بغیر repetição ہو سکیں۔
- Bilhete de promoção میں plano de telemetria کا حوالہ دیں تاکہ ثابت ہو کہ `docs/source/soranet/snnet16_telemetry_plan.md` rebaixar vocabulários اور limites de alerta کے لئے fonte canônica ہے۔

## Atualizações de painel e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` اب Painel de anotações "Plano de implementação" کے ساتھ enviar ہوتا ہے جو اس playbook سے link کرتا ہے اور fase atual ظاہر کرتا ہے تاکہ análises de governança ativas estágio کی تصدیق کر سکیں۔ Descrição do painel کو botões de configuração کی مستقبل تبدیلیوں کے ساتھ sincronização رکھیں۔

Alertando کے لئے یقینی بنائیں کہ موجودہ regras `stage` rótulo استعمال کریں تاکہ canário اور fases padrão الگ limites de política acionador کریں (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversão

### Padrão -> Rampa (Estágio C -> Estágio B)1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` کے ذریعے orquestrador کو rebaixar کریں (configuração SDK میں وہی espelho de fase کریں) تاکہ Estágio B پورے frota میں دوبارہ لاگو ہو۔
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے clientes کو perfil de transporte seguro پر مجبور کریں, اور captura de transcrição کریں تاکہ Fluxo de trabalho de remediação `/policy/proxy-toggle` auditável رہے۔
3. `cargo xtask soranet-rollout-capture --label rollback-default` چلائیں تاکہ guard-directory diffs, saída do promtool, e capturas de tela do painel e `artifacts/soranet_pq_rollout/` میں arquivo کیا جا سکے۔

### Rampa -> Canário (Estágio B -> Estágio A)

1. Promoção سے پہلے captura کیا گیا instantâneo do diretório de proteção `sorafs_cli guard-directory import --guard-directory guards.json` کے ذریعے importação کریں اور `sorafs_cli guard-directory verify` دوبارہ چلائیں Pacote de rebaixamento تاکہ hashes شامل ہوں۔
2. Orquestrador para configurações do cliente میں `rollout_phase = "canary"` set کریں (یا `anonymity_policy stage-a` override) اور پھر [PQ catraca runbook](./pq-ratchet-runbook.md) سے PQ catraca broca repetição کریں O pipeline de downgrade prova ہو۔
3. Capturas de tela de telemetria PQ Ratchet e SN16 atualizadas کے ساتھ resultados de alerta کو registro de incidentes میں anexar کریں, پھر governança کو notificar کریں۔

### Lembretes de proteção

- جب بھی rebaixamento ہو `docs/source/ops/soranet_transport_rollback.md` consulte کریں اور mitigação temporária کو rastreador de lançamento میں `TODO:` کے طور پر log کریں تاکہ acompanhamento ہو سکے۔
- `dashboards/alerts/soranet_handshake_rules.yml` ou `dashboards/alerts/soranet_privacy_rules.yml` کو rollback سے پہلے اور بعد میں `promtool test rules` cobertura میں رکھیں تاکہ pacote de captura de desvio de alerta کے Documento ساتھ ہو۔