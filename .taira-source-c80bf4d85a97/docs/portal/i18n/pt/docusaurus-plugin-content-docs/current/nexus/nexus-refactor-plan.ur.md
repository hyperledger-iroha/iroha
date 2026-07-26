---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
título: Sora Nexus لیجر ری فیکٹر پلان
description: `docs/source/nexus_refactor_plan.md` کا آئینہ، جو Iroha 3 کوڈ بیس کی مرحلہ وار صفائی کے کام کی تفصیل دیتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_refactor_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Plano de refatoração do razão Sora Nexus

یہ دستاویز Sora Nexus Ledger ("Iroha 3") کے ری فیکٹر کے لئے فوری roadmap کو محفوظ کرتی ہے۔ یہ موجودہ ریپوزٹری layout اور genesis/WSV bookkeeping, consenso Sumeragi, gatilhos de contrato inteligente, consultas de snapshot, ligações de host ponteiro-ABI e codecs Norito میں دیکھی گئی regressões کی عکاسی کرتی ہے۔ مقصد یہ ہے کہ تمام correções کو ایک بڑے patch monolítico میں اتارنے کے بجائے ایک مربوط, قابل ٹیسٹ arquitetura تک پہنچا جائے۔

## 0. رہنما اصول
- مختلف ہارڈویئر پر determinístico رویہ برقرار رکھیں; aceleração صرف sinalizadores de recurso opt-in کے ذریعے اور ایک جیسے substitutos کے ساتھ استعمال کریں۔
- Camada de serialização Norito ہے۔ کسی بھی estado/esquema تبدیلی میں Norito codifica/decodifica testes de ida e volta e atualizações de equipamentos شامل ہونے چاہئیں۔
- configuração `iroha_config` (usuário -> real -> padrões) کے ذریعے گزرتی ہے۔ پروڈکشن caminhos سے ambiente ad-hoc alterna ہٹا دیں۔
- Política ABI V1 hosts são tipos de ponteiro/syscalls são determinísticos.
- `cargo test --workspace` اور testes de ouro (`ivm`, `norito`, `integration_tests`) ہر marco کے لئے بنیادی portão رہیں گے۔

## 1. ریپوزٹری ٹاپولوجی اسنیپ شاٹ
- `crates/iroha_core`: atores Sumeragi, WSV, carregador de gênese, pipelines (consulta, sobreposição, pistas zk), e cola de host de contrato inteligente۔
- `crates/iroha_data_model`: dados on-chain e consultas کے لئے esquema autoritativo۔
- `crates/iroha`: API do cliente e CLI, testes, SDK میں استعمال ہوتا ہے۔
- `crates/iroha_cli`: آپریٹر CLI, جو اس وقت `iroha` کی متعدد APIs کو espelho کرتا ہے۔
- `crates/ivm`: Kotodama bytecode VM, pontos de entrada de integração de host ponteiro-ABI۔
- `crates/norito`: codec de serialização com adaptadores JSON e backends AoS/NCB instalados
- `integration_tests`: asserções de componentes cruzados جو genesis/bootstrap, Sumeragi, gatilhos, paginação وغیرہ کو کور کرتے ہیں۔
- Docs پہلے ہی Sora Nexus Ledger اہداف (`nexus.md`, `new_pipeline.md`, `ivm.md`) بیان کرتے ہیں، مگر implementação ٹکڑوں میں ہے اور کوڈ کے مقابلے میں جزوی طور پر پرانی ہے۔

## 2. ری فیکٹر ستون اور marcos

### Fase A - Fundações e Observabilidade
1. **Telemetria WSV + Instantâneos**
   - `state` میں API de instantâneo canônico (traço `WorldStateSnapshot`) قائم کریں جسے consultas, Sumeragi e CLI استعمال کریں۔
   - `scripts/iroha_state_dump.sh` استعمال کریں تاکہ `iroha state dump --format norito` کے ذریعے instantâneos determinísticos بنیں۔
2. **Gênesis/Determinismo Bootstrap**
   - ingestão de genesis
   - cobertura de integração/regressão (ٹریک: `integration_tests/tests/genesis_replay_determinism.rs`)۔
3. **Testes de fixação entre caixas**
   - `integration_tests/tests/genesis_json.rs` کو بڑھائیں تاکہ WSV, pipeline e invariantes ABI کو ایک chicote میں validar کیا جا سکے۔
   - Andaime `cargo xtask check-shape` متعارف کریں جو desvio de esquema پر pânico کرے (backlog de ferramentas DevEx میں ٹریک; `scripts/xtask/README.md` کی item de ação دیکھیں)۔

### Fase B - WSV e superfície de consulta
1. **Transações de armazenamento estatal**
   - `state/storage_transactions.rs` کو ایک adaptador transacional میں سمیٹیں جو commit ordenação اور detecção de conflitos نافذ کرے۔
   - testes unitários.
2. **Refator do modelo de consulta**
   - lógica de paginação/cursor کو `crates/iroha_core/src/query/` کے تحت componentes reutilizáveis میں منتقل کریں۔ `iroha_data_model` میں Representações Norito کو alinhar کریں۔
   - gatilhos, ativos e funções کے لئے ordenação determinística کے ساتھ consultas de instantâneo شامل کریں (cobertura de موجودہ `crates/iroha_core/tests/snapshot_iterable.rs` میں ٹریک ہے)۔
3. **Consistência do Instantâneo**
   - یقینی بنائیں کہ `iroha ledger query` CLI e caminho do snapshot استعمال کرے جو Sumeragi/fetchers استعمال کرتے ہیں۔
   - Testes de regressão de instantâneo CLI `tests/cli/state_snapshot.rs` میں ہیں (execuções lentas کے لئے dependentes de recursos)۔### Fase C - Pipeline Sumeragi
1. **Topologia e Gerenciamento de Época**
   - `EpochRosterProvider` کو ایک trait میں نکالیں جس کی implementações WSV stake snapshots پر مبنی ہوں۔
   - Bancos/testes `WsvEpochRosterAdapter::from_peer_iter` کے لئے ایک سادہ construtor mock-friendly فراہم کرتا ہے۔
2. **Simplificação do Fluxo de Consenso**
   - `crates/iroha_core/src/sumeragi/*` کو ماڈیولز میں ری آرگنائز کریں: `pacemaker`, `aggregation`, `availability`, `witness` Os tipos de tipos کو `consensus` کے تحت رکھیں۔
   - passagem de mensagem ad-hoc کو envelopes Norito digitados سے بدلیں اور testes de propriedade de alteração de visualização متعارف کریں (Sumeragi backlog de mensagens میں ٹریک)۔
3. **Integração de pista/prova**
   - provas de pista کو compromissos DA کے ساتھ alinhar کریں اور یقینی بنائیں کہ RBC gating یکساں ہو۔
   - teste de integração ponta a ponta `integration_tests/tests/extra_functional/seven_peer_consistency.rs` اب caminho habilitado para RBC کو verificar کرتا ہے۔

### Fase D - Contratos Inteligentes e Hosts Pointer-ABI
1. **Auditoria de limite do anfitrião**
   - verificações do tipo ponteiro (`ivm::pointer_abi`) e adaptadores de host (`iroha_core::smartcontracts::ivm::host`) e consolidar کریں۔
   - expectativas da tabela de ponteiros کور کرتے ہیں, جو mapeamentos TLV dourados کو exercício کرتے ہیں۔
2. **Sandbox de execução de gatilho**
   - gatilhos کو اس طرح ری فیکٹر کریں کہ وہ مشترکہ `TriggerExecutor` کے ذریعے چلیں gás, validação de ponteiro e registro de eventos نافذ کرتا ہے۔
   - gatilhos de chamada/tempo کے لئے testes de regressão شامل کریں جو caminhos de falha کو کور کرتے ہیں (ٹریک: `crates/iroha_core/tests/trigger_failure.rs`)۔
3. **CLI e alinhamento do cliente**
   - یقینی بنائیں کہ Operações CLI (`audit`, `gov`, `sumeragi`, `ivm`) drift سے بچنے کے لئے compartilhado Funções do cliente `iroha`
   - Testes de instantâneo CLI JSON `tests/cli/json_snapshot.rs` میں ہیں؛ انہیں اپ ٹو ڈیٹ رکھیں تاکہ saída de comando principal referência JSON canônica سے correspondência کرتا رہے۔

### Fase E - Endurecimento do Codec Norito
1. **Registro de esquema**
   - `crates/norito/src/schema/` کے تحت Registro de esquema Norito بنائیں تاکہ tipos de dados principais کے لئے codificações canônicas دستیاب ہوں۔
   - amostra de codificação de carga útil کو verificar کرنے والے testes de documento شامل کریں (`norito::schema::SamplePayload`)۔
2. **Atualização de luminárias douradas**
   - `crates/norito/tests/*` کے luminárias douradas کو اپ ڈیٹ کریں تاکہ refatorar کے بعد نئی esquema WSV سے correspondência ہوں۔
   - Auxiliar `scripts/norito_regen.sh` `norito_regen_goldens` کے ذریعے Norito JSON goldens کو determinístico طور پر regenerar کرتا ہے۔
3. **Integração IVM/Norito**
   - Serialização de manifesto Kotodama کو Norito کے ذریعے validação de ponta a ponta کریں, تاکہ ponteiro metadados ABI مستقل رہے۔
   - Manifestos `crates/ivm/tests/manifest_roundtrip.rs` کے لئے Paridade de codificação/decodificação Norito برقرار رکھتا ہے۔

## 3. Corte transversal
- **Estratégia de teste**: ہر testes unitários de fase -> testes de caixa -> testes de integração کو فروغ دیتا ہے۔ testes com falha موجودہ regressões کو پکڑتے ہیں؛ نئے testes انہیں واپس آنے سے روکتے ہیں۔
- **Documentação**: ہر fase کے اترنے کے بعد `status.md` اپ ڈیٹ کریں اور کھلے itens کو `roadmap.md` میں منتقل کریں, جبکہ مکمل شدہ کام prune کریں۔
- **Benchmarks de desempenho**: `iroha_core`, `ivm` e `norito` Bancos de bancada برقرار رکھیں؛ refatorar کے بعد medições de linha de base شامل کریں تاکہ regressões نہ ہوں۔
- **Feature Flags**: alterna no nível da caixa صرف ان backends کے لئے رکھیں جنہیں بیرونی cadeias de ferramentas چاہیے (`cuda`, `zk-verify-batch`)۔ Caminhos CPU SIMD ہمیشہ build ہوتے اور runtime پر منتخب ہوتے ہیں؛ hardware não suportado کے لئے substitutos escalares determinísticos فراہم کریں۔

## 4. فوری اگلے اقدامات
- Andaime da Fase A (característica instantânea + fiação de telemetria) - atualizações de roteiro میں tarefas acionáveis دیکھیں۔
- `sumeragi`, `state`, اور `ivm` کی حالیہ auditoria de defeitos نے درج ذیل destaques سامنے لائے:
  - `sumeragi`: permissões de código morto, transmissão à prova de alteração de visualização, estado de repetição VRF, exportação de telemetria EMA e proteção کرتے ہیں۔ یہ Fase C کے simplificação do fluxo de consenso اور entregas de integração de faixa/prova اترنے تک gated رہیں گے۔
  - `state`: Limpeza `Cell` e roteamento de telemetria Fase A کے Trilha de telemetria WSV پر منتقل ہوتے ہیں, SoA/aplicação paralela Não backlog de otimização de pipeline da Fase C میں شامل ہوتے ہیں۔
  - `ivm`: exposição de alternância CUDA, validação de envelope, cobertura Halo2/Metal Fase D کے limite de host کام اور tema transversal de aceleração de GPU سے میپ ہوتے ہیں؛ kernels تیار ہونے تک backlog de GPU dedicado پر رہتے ہیں۔
- alterações invasivas de código## 5. کھلے سوالات
- کیا RBC کو P1 کے بعد بھی opcional رہنا چاہیے, یا Nexus pistas de razão کے لئے لازمی ہے؟ parte interessada فیصلہ درکار ہے۔
- کیا ہم P1 میں Grupos de composição DS نافذ کریں یا provas de pista کے maduro ہونے تک انہیں desativar رکھیں؟
- Parâmetros ML-DSA-87 کی localização canônica کیا ہے؟ امیدوار: نیا crate `crates/fastpq_isi` (criação pendente)۔

---

_آخری اپ ڈیٹ: 2025-09-12_