---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: Sora Nexus کی تکنیکی اسپیسفیکیشن
description: `docs/source/nexus.md` کی مکمل عکاسی, جو Iroha 3 (Sora Nexus) لیجر کی معماری اور ڈیزائن پابندیوں کا احاطہ کرتی ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus.md` کی عکاسی کرتا ہے۔ ترجمے کا بیک لاگ پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

#! Iroha 3 - Sora Nexus Ledger: تکنیکی ڈیزائن اسپیسفیکیشن

یہ دستاویز Iroha 3 کے لئے Sora Nexus Ledger کی معماری تجویز کرتی ہے, جو Iroha 2 espaços de dados (DS) são os espaços de dados mais importantes e os espaços de dados mais importantes ہے۔ Espaços de dados مضبوط پرائیویسی ڈومینز ("espaços de dados privados") اور کھلی شمولیت ("espaços de dados públicos") فراہم کرتے ہیں۔ یہ ڈیزائن عالمی لیجر میں composibilidade کو برقرار رکھتے ہوئے private-DS ڈیٹا کے لئے سخت isolamento اور confidencialidade یقینی بناتا ہے، اور Kura (armazenamento em bloco) اور WSV (World State View) میں codificação de eliminação کے ذریعے disponibilidade de dados کو escala کرتا ہے۔

یہی ریپوزٹری Iroha 2 (redes auto-hospedadas) ou Iroha 3 (SORA Nexus) دونوں کو build کرتی ہے۔ A máquina virtual Iroha (IVM) e a cadeia de ferramentas Kotodama são usadas para criar contratos e artefatos de bytecode. میزبان ڈپلائمنٹس اور Nexus عالمی لیجر کے درمیان portátil رہتے ہیں۔

اہداف
- ایک عالمی منطقی لیجر جو بہت سے تعاون کرنے والے validadores e espaços de dados پر مشتمل ہو۔
- آپریشن (مثلاً CBDCs) کے لئے Espaços de dados privados, جہاں ڈیٹا کبھی DS privado سے باہر نہیں جاتا۔
- Espaços de dados públicos کھلی شمولیت کے ساتھ, Ethereum جیسی رسائی۔ sem permissão
- Espaços de dados کے پار contratos inteligentes combináveis, مگر ativos DS privados
- isolamento de desempenho تاکہ público سرگرمی privado-DS اندرونی ٹرانزیکشنز کو متاثر نہ کرے۔
- disponibilidade de dados em escala: Kura com código de eliminação اور WSV تاکہ عملی طور پر لامحدود ڈیٹا سپورٹ ہو جبکہ private-DS ڈیٹا نجی رہے۔

غیر اہداف (ابتدائی مرحلہ)
- economia de tokens یا incentivos para validadores کی تعریف؛ agendamento e piquetagem پالیسیز plugável ہیں۔
- Não ABI ورژن متعارف کرانا؛ تبدیلیاں IVM پالیسی کے مطابق syscall explícito e extensões de ponteiro-ABI کے ساتھ ABI v1 کو ہدف بناتی ہیں۔

اصطلاحات
- Nexus Ledger: عالمی منطقی لیجر جو Data Space (DS) بلاکس کو ایک واحد، مرتب تاریخ اور compromisso estatal میں compose کر کے بنتا ہے۔
- Espaço de dados (DS): ایک محدود execução اور armazenamento ڈومین جس کے اپنے validadores, governança, classe de privacidade, política DA, cotas, política de taxas ہوتے ہیں۔ Qual é a opção: DS público ou DS privado
- Espaço de dados privado: validadores autorizados e controle de acesso; ٹرانزیکشن ڈیٹا اور estado کبھی DS سے باہر نہیں جاتے۔ صرف compromissos/metadados عالمی طور پر âncora ہوتے ہیں۔
- Espaço de dados públicos: sem permissão شمولیت؛ مکمل ڈیٹا اور state عوامی طور پر دستیاب ہیں۔
- Manifesto de Espaço de Dados (Manifesto DS): Manifesto codificado Norito e parâmetros DS (validadores/chaves QC, classe de privacidade, política ISI, parâmetros DA, retenção, cotas, política ZK, taxas) ظاہر کرتا ہے۔ cadeia de nexo de hash manifesto پر âncora ہوتا ہے۔ جب تک substituir نہ ہو، Certificados de quorum DS ML-DSA-87 (classe Dilithium5) کو esquema de assinatura pós-quântica padrão کے طور پر استعمال کرتے ہیں۔
- Diretório de espaço: contrato de diretório on-chain, manifestos DS, versões, eventos de governança/rotação, capacidade de resolução e auditorias, ٹریک کرتا ہے۔
- DSID: Espaço de Dados کے لئے عالمی طور پر منفرد شناخت۔ Os objetos e as referências são o namespace e o namespace.
- Âncora: bloco/cabeçalho DS کا compromisso criptográfico جو histórico DS کو عالمی لیجر میں باندھنے کے لئے cadeia de nexo میں شامل ہوتا ہے۔
- Kura: armazenamento em bloco Iroha۔ یہاں اسے armazenamento de blob com código de eliminação e compromissos کے ساتھ توسیع دی گئی ہے۔
- WSV: Iroha Vista do Estado Mundial۔ یہاں اسے versionado, capaz de capturar instantâneos, segmentos de estado codificados para eliminação کے ساتھ توسیع دی گئی ہے۔
- IVM: execução de contrato inteligente کے لئے Máquina Virtual Iroha (bytecode Kotodama `.to`)۔
  - AIR: Representação Algébrica Intermediária۔ STARK طرز کے provas کے لئے computação کا visão algébrica, جو execução کو rastreamentos baseados em campo میں transição اور restrições de limite کے ساتھ بیان کرتا ہے۔Espaços de dados
- Identidade: `DataSpaceId (DSID)` DS کی شناخت کرتا ہے اور ہر چیز کو namespace کرتا ہے۔ DS ou granularidades para instanciar e executar:
  - Domínio-DS: `ds::domain::<domain_name>` - execução اور estado ایک domínio تک محدود۔
  - Ativo-DS: `ds::asset::<domain_name>::<asset_name>` - execução e estado ایک واحد definição de ativo تک محدود۔
  دونوں شکلیں ساتھ موجود ہیں؛ ٹرانزیکشنز atomicamente متعدد DSIDs کو touch کر سکتی ہیں۔
- Ciclo de vida do manifesto: criação de DS, atualizações (rotação de chaves, mudanças de política), e aposentadoria do Space Directory میں ریکارڈ ہوتے ہیں۔ ہر artefato DS por slot تازہ ترین hash de manifesto کو referência کرتا ہے۔
- Aulas: DS público (participação aberta, DA público) e DS privado (permitido, DA confidencial)۔ sinalizadores de manifesto de políticas híbridas کے ذریعے ممکن ہیں۔
- Políticas por DS: permissões ISI, parâmetros DA `(k,m)`, criptografia, retenção, cotas (por compartilhamento de tx de bloco کی min/max), ZK/política de prova otimista, taxas۔
- Governança: adesão ao DS اور manifesto de rotação do validador کے governança حصے میں متعین ہوتے ہیں (propostas na cadeia, multisig, یا transações nexus اور atestados کے ذریعے governança externa ancorada)۔

Manifestos de capacidade no UAID
- Contas universais: ہر participante کو ایک UAID determinístico (`UniversalAccountId` em `crates/iroha_data_model/src/nexus/manifest.rs`) ملتا ہے جو تمام espaços de dados پر محیط ہے۔ Manifestos de capacidade (`AssetPermissionManifest`) UAID کو مخصوص espaço de dados, épocas de ativação/expiração, e permitir/negar `ManifestEntry` قواعد کی ترتیب شدہ فہرست سے باندھتے ہیں جو `dataspace`, `program_id`, `method`, `asset` اور اختیاری Funções AMX کو محدود کرتی ہے۔ Negar قواعد ہمیشہ غالب رہتے ہیں؛ avaliador یا تو motivo da auditoria کے ساتھ `ManifestVerdict::Denied` جاری کرتا ہے یا metadados de subsídio correspondentes کے ساتھ `Allowed` concessão دیتا ہے۔
- Permissões: ہر permitir entrada میں baldes `AllowanceWindow` determinísticos (`PerSlot`, `PerMinute`, `PerDay`) کے ساتھ ایک اختیاری `max_amount` ہوتا ہے۔ Hosts e SDKs ایک ہی Carga útil Norito استعمال کرتے ہیں, اس لئے hardware de aplicação اور implementações de SDK میں یکساں رہتی ہے۔
- Telemetria de auditoria: Diretório Espacial جب بھی کسی manifesto کا estado بدلتا ہے تو `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) نشر کرتا ہے۔ نئی `SpaceDirectoryEventFilter` superfície Torii/assinantes de eventos de dados کو atualizações de manifesto UAID, revogações, اور deny-wins فیصلوں کی نگرانی encanamento personalizado کے بغیر کرنے دیتی ہے۔

Evidência de operador ponta a ponta, Notas de migração do SDK, Listas de verificação de publicação de manifesto, Guia de conta universal (`docs/source/universal_accounts_guide.md`) e Espelho de espelho. Política UAID یا ferramentas میں تبدیلی ہو تو دونوں دستاویزات ہم آہنگ رکھیں۔

اعلی سطحی معماری
1) Camada de Composição Global (Cadeia Nexus)
- 1 bloco Nexus blocos کی ایک واحد، کینونیکل ترتیب برقرار رکھتا ہے جو ایک یا زیادہ Data Spaces (DS) پر پھیلی atomic ٹرانزیکشنز کو finalize کرتا ہے۔ ہر comprometido ٹرانزیکشن متحد عالمی estado mundial کو اپ ڈیٹ کرتی ہے (raízes por DS کا vetor)۔
- کم سے کم metadados کے ساتھ provas agregadas/QCs شامل ہوتے ہیں تاکہ composição, finalidade, e detecção de fraude یقینی ہو (toque em کیے گئے DSIDs، raízes de estado por DS پہلے/بعد, compromissos DA, provas de validade por DS, ML-DSA-87 e certificado de quorum DS)۔ Dados privados شامل نہیں ہوتا۔
- Consenso: ایک واحد global, comitê BFT pipeline جس کا سائز 22 (3f + 1 com f = 7) ہے، جو ~ 200k ممکنہ validadores کے pool سے mecanismo VRF/stake de época کے ذریعے منتخب ہوتا ہے۔ comitê do nexo ٹرانزیکشنز کو sequência کرتا ہے اور بلاک کو 1s میں finalizar کرتا ہے۔

2) Camada de Espaço de Dados (Público/Privado)
- global ٹرانزیکشنز کے per-DS fragmentos executam کرتا ہے, DS-local WSV کو update کرتا ہے، اور artefatos de validade por bloco (provas agregadas por DS e compromissos DA) بناتا ہے جو 1 segundo Bloco Nexus میں roll up ہوتے ہیں۔
- Dados DS privados em repouso ou dados em voo کو validadores autorizados کے درمیان criptografar کرتے ہیں؛ صرف compromissos اور PQ provas de validade DS سے باہر جاتے ہیں۔
- DS público مکمل corpos de dados (via DA) e exportação de provas de validade PQ کرتے ہیں۔3) Transações atômicas entre dados e espaço (AMX)
- Modelo: ہر transação do usuário متعدد DS کو touch کر سکتی ہے (مثلاً domínio DS اور ایک یا زیادہ ativo DS)۔ یہ ایک ہی Bloco Nexus میں confirmar atomicamente ہوتی ہے یا abortar; جزوی اثرات نہیں۔
- Prepare-Commit dentro de 1s: ہر transação candidata کے لئے DS tocado ایک ہی instantâneo (slot کے آغاز کے raízes DS) کے خلاف execução paralela کرتے ہیں اور por-DS PQ provas de validade (FASTPQ-ISI) اور DA compromissos بناتے ہیں۔ comitê do nexo ٹرانزیکشن کو تبھی commit کرتا ہے جب تمام مطلوبہ Provas DS verificam ہوں اور certificados DA وقت پر پہنچیں (ہدف <=300 senhora)؛ ورنہ ٹرانزیکشن اگلے slot کے لئے reagendar ہوتی ہے۔
- Consistência: conjuntos de leitura e gravação declaram ہوتے ہیں؛ raízes de início de slot de detecção de conflito کے خلاف commit پر ہوتی ہے۔ execução otimista sem bloqueio por paralisações globais do DS سے بچاتی ہے؛ regra de commit do nexo de atomicidade
- Privacidade: DS privado صرف raízes pré/pós DS سے بندھے exportação de provas/compromissos کرتے ہیں۔ کوئی dados privados brutos DS سے باہر نہیں جاتا۔

4) Disponibilidade de dados (DA) com codificação de eliminação
- Corpos de bloco Kura e instantâneos WSV کو blobs codificados para eliminação کے طور پر ذخیرہ کرتا ہے۔ blobs públicos وسیع پیمانے پر shard ہوتے ہیں؛ blobs privados صرف validadores DS privados میں، pedaços criptografados کے ساتھ محفوظ ہوتے ہیں۔
- Compromissos DA Artefatos DS اور Nexus Blocos دونوں میں ریکارڈ ہوتے ہیں, جس سے amostragem اور garantias de recuperação ممکن ہوتی ہیں بغیر conteúdos privados ظاہر کیے۔

بلاک اور کمٹ ڈھانچہ
- Artefato à prova de espaço de dados (1s slot, ہر DS)
  - فیلڈز: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Corpos de dados de artefatos DS privados کے بغیر exportar کرتے ہیں؛ público DS DA کے ذریعے recuperação de corpo کی اجازت دیتے ہیں۔

- Bloco Nexus (cadência de 1s)
  - فیلڈز: block_number, parent_hash, slot_time, tx_list (DSIDs tocados em transações atômicas entre DS), ds_artifacts[], nexus_qc.
  - فنکشن: وہ تمام atomic ٹرانزیکشنز finalizar کرتا ہے جن کے مطلوبہ Artefatos DS verificar ہوں؛ vetor de estado mundial global کے Raízes DS کو ایک ہی قدم میں atualização کرتا ہے۔

Consenso e agendamento
- Consenso de cadeia Nexus: Único global, BFT em pipeline (classe Sumeragi) ایک Comitê de 22 nós (3f + 1 com f = 7) کے ساتھ blocos de 1s e finalidade de 1s کو ہدف بناتا ہے۔ membros do comitê ~ 200 mil candidatos کے pool سے VRF/stake de época کے ذریعے منتخب ہوتے ہیں؛ rotação descentralização اور resistência à censura برقرار رکھتی ہے۔
- Consenso de espaço de dados: ہر DS اپنے validadores کے درمیان اپنا BFT چلاتا ہے تاکہ artefatos por slot (provas, compromissos DA, DS QC) بنائے جا سکیں۔ Comitês de retransmissão de pista `3f+1` سائز میں espaço de dados `fault_tolerance` سیٹنگ استعمال کرتے ہیں اور ہر época میں conjunto de validadores de espaço de dados سے amostra deterministicamente ہوتے ہیں, Semente de época VRF کو `(dataspace_id, lane_id)` کے ساتھ bind کر کے۔ DS privado com permissão ہیں؛ DS público anti-Sybil پالیسیز کے تحت vivacidade aberta دیتے ہیں۔ Comitê do Nexus Global تبدیل نہیں ہوتا۔
- Agendamento de transações: صارفین atomic ٹرانزیکشنز submit کرتے ہیں جن میں DSIDs tocados اور conjuntos de leitura e gravação declarados ہوتے ہیں۔ Slot DS کے اندر execução paralela کرتے ہیں؛ comitê do nexo ٹرانزیکشن کو 1s بلاک میں تبھی شامل کرتا ہے جب تمام Artefatos DS verificam ہوں اور Certificados DA بروقت ہوں (<=300 senhor)۔
- Isolamento de desempenho: ہر DS کے پاس mempools independentes e execução ہوتی ہے۔ cotas por DS یہ حد باندھتی ہیں کہ کسی DS کو touch کرنے والی کتنی ٹرانزیکشنز فی بلاک commit ہو سکتی ہیں تاکہ bloqueio direto سے بچا جا سکے اور latência DS privada محفوظ رہے۔

Namespace e Namespace
- IDs qualificados para DS: entidades importantes (domínios, contas, ativos, funções) `dsid` کے ساتھ qualificadas ہوتے ہیں۔ Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências globais: ایک referência global ایک tupla `(dsid, object_id, version_hint)` ہے اور اسے camada nexus پر on-chain یا cross-DS استعمال کے لئے descritores AMX میں رکھا جا سکتا ہے۔
- Serialização Norito: mensagens cross-DS (descritores AMX, provas) Codecs Norito استعمال کرتے ہیں۔ caminhos de produção میں serde کا استعمال نہیں ہوتا۔Contratos inteligentes por IVM
- Contexto de execução: Contexto de execução IVM میں `dsid` شامل کریں۔ Contratos Kotodama ہمیشہ کسی مخصوص Espaço de dados کے اندر executar ہوتے ہیں۔
- Primitivos Atômicos Cross-DS:
  - `amx_begin()` / `amx_commit()` IVM host میں atomic multi-DS ٹرانزیکشن کو delinear کرتے ہیں۔
  - Detecção de conflito `amx_touch(dsid, key)` کے لئے raízes de instantâneo de slot کے خلاف declaração de intenção de leitura/gravação کرتا ہے۔
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (operação تبھی permitida ہے جب política اجازت دے اور identificador válido ہو)
- Tratamento de ativos e taxas:
  - Operações de ativos DS کے ISI/políticas de função کے تحت autorizar ہوتے ہیں؛ taxas DS کے token de gás میں ادا ہوتی ہیں۔ tokens de capacidade opcionais اور زیادہ políticas ricas (aprovador múltiplo, limites de taxa, delimitação geográfica)
- Determinismo: contém entradas de syscalls e conjuntos de leitura/gravação AMX declarados, puros e determinísticos. وقت یا ماحول کے efeitos ocultos نہیں ہوتے۔Provas de validade pós-quântica (ISIs generalizadas)
- FASTPQ-ISI (PQ, sem configuração confiável): ایک kernelizado, argumento baseado em hash, transferência ڈیزائن کو تمام famílias ISI تک عام کرتا ہے جبکہ hardware de classe GPU پر lotes em escala de 20k کے لئے prova de subsegundos کو ہدف بناتا ہے۔
  - Perfil operacional:
    - Provador de nós de produção کو `fastpq_prover::Prover::canonical` کے ذریعے construir کرتے ہیں, جو اب ہمیشہ inicializar backend de produção کرتا ہے؛ simulação determinística ہٹا دیا گیا ہے۔ [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) اور `irohad --fastpq-execution-mode` operadores کو Execução de CPU/GPU کو pino determinístico کرنے دیتے ہیں جبکہ auditorias de frota de gancho de observador کے لئے triplos solicitados/resolvidos/backend کو ریکارڈ کرتا ہے۔ [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: WSV کو Poseidon2-SMT کے ذریعے mapa de valor-chave digitado confirmado کے طور پر treat کرتا ہے۔ ہر Chaves ISI (contas, ativos, funções, domínios, metadados, fornecimento) پر ler-verificar-escrever linhas کے چھوٹے سیٹ میں expandir ہوتا ہے۔
  - Restrições controladas por Opcode: colunas seletoras کے ساتھ ایک ہی Tabela AIR por ISI قواعد نافذ کرتی ہے (conservação, contadores monotônicos, permissões, verificações de intervalo, atualizações limitadas de metadados)۔
  - Argumentos de pesquisa: permissões/funções, precisões de ativos, e parâmetros de política کے لئے tabelas transparentes comprometidas com hash, restrições bit a bit pesadas سے بچتی ہیں۔
- Compromissos e atualizações do Estado:
  - Prova SMT agregada: تمام teclas tocadas (pré/pós) `old_root`/`new_root` کے خلاف ایک fronteira compactada کے ساتھ provar ہوتے ہیں جس میں irmãos desduplicados ہوں۔
  - Invariantes: invariantes globais (مثلاً ہر ativo کی fornecimento total) linhas de efeito اور contadores rastreados کے درمیان igualdade multiset کے ذریعے نافذ ہوتے ہیں۔
- Sistema de prova:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) alta aridade (8/16) e explosão 8-16 کے ساتھ؛ Hashes Poseidon2; Transcrição Fiat-Shamir SHA-2/3 کے ساتھ۔
  - Recursão opcional: اگر ضرورت ہو تو microlotes کو ایک prova فی compressão de slot کرنے کے لئے agregação recursiva local DS۔
- Escopo e exemplos abordados:
  - Ativos: transferência, cunhagem, gravação, registro/cancelamento de definições de ativos, definição de precisão (limitada), definição de metadados۔
  - Contas/domínios: criar/remover, definir chave/limite, adicionar/remover signatários (apenas estado; verificações de assinatura DS validadores atestam کرتے ہیں, AIR کے اندر provar نہیں ہوتے)۔
  - Funções/Permissões (ISI): conceder/revogar funções e permissões; tabelas de pesquisa اور verificações de política monotônicas سے impor ہوتے ہیں۔
  - Contratos/AMX: marcadores de início/confirmação AMX, capacidade de cunhagem/revogação ativada ہو؛ transições de estado اور contadores de política کے طور پر provar ہوتے ہیں۔
- Verificações fora do AIR para preservar a latência:
  - Assinaturas com criptografia pesada (como assinaturas de usuário ML-DSA) Os validadores DS verificam کرتے ہیں e DS QC میں atesta ہوتے ہیں؛ prova de validade صرف consistência do estado اور conformidade com a política کو cobertura کرتا ہے۔ یہ provas کو PQ اور تیز رکھتا ہے۔
- Metas de desempenho (CPU ilustrativa de 32 núcleos + GPU única moderna):
  - 20k ISIs mistos com toque de tecla pequeno (<= 8 teclas/ISI): prova de ~0,4-0,9 s, prova de ~150-450 KB, verificação de ~5-15 ms۔
  - ISIs mais pesados ​​(mais chaves/restrições ricas): microlote (como 10x2k) + recursão تاکہ por slot <1 s رہے۔
- Configuração do manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas DS QC verificam کرتا ہے)
  - `attestation.qc_signature = "ml_dsa_87"` (padrão؛ alternativas کو explícitas para declarar کرنا ہوگا)
- Subsídios:
  - ISIs complexos/personalizados عمومی STARK (`zk.policy = "stark_fri_general"`) استعمال کر سکتے ہیں جس میں prova diferida اور 1 s finalidade atestado QC + corte کے ذریعے ہوتی ہے۔
  - Opções não-PQ (مثلاً Plonk com KZG) configuração confiável چاہتی ہیں اور compilação padrão میں اب compatível نہیں ہیں۔AIR تعارف (Nexus کے لئے)
- Rastreamento de execução: ایک matriz جس کی largura (colunas de registro) اور comprimento (etapas) ہوتی ہے۔ Processamento ISI de linha کا منطقی قدم ہے؛ colunas میں valores pré/pós, seletores, e sinalizadores ہوتے ہیں۔
- Restrições:
  - Restrições de transição: relações linha a linha
  - Restrições de limite: E/S pública (old_root/new_root, contadores) کو پہلی/آخری linhas سے bind کرتی ہیں۔
  - Pesquisas/permutações: tabelas comprometidas (permissões, parâmetros de ativos) کے خلاف associação اور igualdades multiset کو یقینی بناتی ہیں بغیر circuitos com muitos bits کے۔
- Compromisso e verificação:
  - Rastreamentos de provador کو codificações baseadas em hash کے ذریعے commit کرتا ہے اور polinômios de baixo grau بناتا ہے جو تبھی válido ہوں جب restrições پوری ہوں۔
  - Verificador FRI (baseado em hash, pós-quântico) کے ذریعے verificação de baixo grau کرتا ہے، چند aberturas Merkle کے ساتھ؛ etapas de custo کے log پر منحصر ہے۔
- Exemplo (Transferência): registros میں pre_balance, amount, post_balance, nonce, e seletores شامل ہیں۔ Restrições não-negatividade/intervalo, conservação, اور monotonicidade nonce نافذ کرتی ہیں, جبکہ folhas pré/pós multi-prova SMT agregadas کو raízes antigas/novas سے جوڑتی ہے۔

ABI e Syscall (ABI v1)
- Syscalls a adicionar (nomes ilustrativos):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos Pointer-ABI para adicionar:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Atualizações necessárias:
  - `ivm::syscalls::abi_syscall_list()` میں شامل کریں (pedindo برقرار رکھیں), política کے ذریعے portão کریں۔
  - números desconhecidos کو hosts میں `VMError::UnknownSyscall` پر mapa کریں۔
  - atualização de testes کریں: lista syscall golden, hash ABI, ID de tipo de ponteiro goldens, e testes de política ۔
  - documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Privacidade
- Contenção de dados privados: DS privado کے لئے órgãos de transação, diferenças de estado, e instantâneos WSV کبھی subconjunto de validador privado سے باہر نہیں جاتے۔
- Exposição pública: صرف cabeçalhos, compromissos DA, e exportação de provas de validade PQ ہوتے ہیں۔
- Provas ZK opcionais: Provas DS ZK privadas (saldo مثلاً کافی ہے، política پوری ہوئی) بنا سکتے ہیں تاکہ estado interno ظاہر کئے بغیر ações cross-DS ممکن ہوں۔
- Controle de acesso: autorização DS کے اندر ISI/políticas de função کے ذریعے نافذ ہوتی ہے۔ tokens de capacidade

Isolamento de desempenho e QoS
- ہر DS کے لئے consenso, mempools, e armazenamento de dados
- Nexus cotas de agendamento por tempo de inclusão de âncora DS
- Orçamentos de recursos de contrato por DS (computação/memória/IO) host IVM کے ذریعے نافذ ہوتے ہیں۔ Contenção de DS público Orçamentos de DS privados استعمال نہیں کر سکتی۔
- Chamadas assíncronas de DS cruzado, execução de DS privado کے اندر لمبی esperas síncronas سے بچاتی ہیں۔

Disponibilidade de dados e design de armazenamento
1) Codificação de apagamento
- Kura bloqueia instantâneos WSV کے codificação de eliminação em nível de blob کے لئے Reed-Solomon sistemático (مثلاً GF (2 ^ 16)) استعمال کریں: parâmetros `(k, m)` ou `n = k + m` fragmentos ہیں۔
- Parâmetros padrão (DS público proposto): `k=32, m=16` (n=48), جس سے ~1,5x expansão کے ساتھ 16 shards تک recuperação ممکن ہے۔ DS privado کے لئے: `k=16, m=8` (n=24) conjunto permitido کے اندر۔ دونوں DS Manifest کے ذریعے configurável ہیں۔
- Blobs públicos: fragmentos بہت سے nós/validadores DA میں distribuir ہوتے ہیں اور verificações de disponibilidade baseadas em amostragem ہوتے ہیں۔ cabeçalhos میں compromissos DA clientes leves کو verificar کرنے دیتے ہیں۔
- Blobs privados: fragmentos criptografados ہوتے ہیں اور صرف validadores DS privados (یا custodiantes designados) کے اندر distribuir ہوتے ہیں۔ cadeia global صرف Compromissos DA رکھتی ہے (locais de fragmentos یا chaves نہیں)۔

2) Compromissos e Amostragem
- ہر blob کے لئے shards پر Merkle root نکال کر `*_da_commitment` میں شامل کریں۔ compromissos de curva elíptica سے بچ کر PQ رہیں۔
- Atestadores DA: atestadores regionais amostrados por VRF (região مثلاً ہر میں 64) amostragem de shard bem-sucedida کی atestado کے لئے certificado ML-DSA-87 جاری کرتے ہیں۔ ہدف Latência de atestado DA <=300 ms ہے۔ fragmentos do comitê do nexo لینے کے بجائے certificados validam کرتی ہے۔

3) Integração Kura
- Bloqueia corpos de transação کو compromissos Merkle کے ساتھ blobs codificados para eliminação کے طور پر loja کرتے ہیں۔
- Compromissos de blob de cabeçalhos رکھتے ہیں؛ órgãos públicos DS کے لئے rede DA اور privado DS کے لئے canais privados کے ذریعے قابلِ بازیافت ہیں۔4) Integração WSV
- Instantâneo WSV: periódico طور پر Estado DS کو instantâneos fragmentados e codificados para eliminação میں ponto de verificação کریں اور cabeçalhos de compromissos میں ریکارڈ کریں۔ instantâneos کے درمیان logs de alterações برقرار رہتے ہیں۔ instantâneos públicos وسیع پیمانے پر shard ہوتے ہیں؛ instantâneos privados validadores privados کے اندر رہتے ہیں۔
- Acesso de transporte de provas: contrata provas estaduais (Merkle/Verkle) فراہم یا طلب کر سکتے ہیں جو compromissos instantâneos سے ancorados ہوں۔ Provas privadas de DS خام کے بجائے atestados de conhecimento zero دے سکتے ہیں۔

5) Retenção e Poda
- DS público کے لئے poda نہیں: تمام Kura corpos اور instantâneos WSV DA کے ذریعے reter کریں (escala horizontal)۔ A retenção interna do DS privado define کر سکتے ہیں، مگر compromissos exportados imutáveis ​​رہتے ہیں۔ Camada Nexus تمام Nexus Blocos e compromissos de artefato DS برقرار رکھتی ہے۔

Rede e funções de nó
- Validadores globais: consenso do nexo میں حصہ لیتے ہیں, Nexus Blocos اور artefatos DS validam کرتے ہیں, DS público کے لئے verificações DA کرتے ہیں۔
- Validadores de espaço de dados: Consenso DS چلاتے ہیں، contratos executados کرتے ہیں, gerenciamento local Kura/WSV کرتے ہیں, اپنے DS کے لئے identificador DA کرتے ہیں۔
- Nós DA (opcional): armazenamento/publicação de blobs públicos کرتے ہیں, amostragem میں مدد دیتے ہیں۔ DS privado کے لئے، validadores de nós DA یا custodiantes confiáveis ​​کے ساتھ co-localizar ہوتے ہیں۔

Melhorias no nível do sistema
- Desacoplamento de sequenciamento/mempool: DAG mempool (estilo Narwhal) اپنائیں جو camada de nexo میں pipeline BFT کو feed کرے تاکہ latência کم اور taxa de transferência بہتر ہو، بغیر منطقی ماڈل بدلے۔
- Cotas DS اور justiça: cotas por DS por bloco اور limites de peso bloqueio head-of-line سے بچانے اور DS privado کے لئے قابلِ پیش گوئی latência یقینی بنانے کے لئے۔
- Atestado DS (PQ): certificados de quorum DS padrão ML-DSA-87 (classe Dilithium5) استعمال کرتے ہیں۔ یہ pós-quântica ہے اور assinaturas EC سے بڑا ہے مگر ایک QC فی slot قابلِ قبول ہے۔ DS گر Manifesto DS میں declarar کریں تو ML-DSA-65/44 (چھوٹا) یا Assinaturas CE منتخب کر سکتے ہیں؛ Public DS کے لئے ML-DSA-87 برقرار رکھنے کی سخت سفارش ہے۔
- Atestadores DA: DS públicos کے لئے Atestadores regionais com amostragem VRF Certificados DA جاری کرتے ہیں۔ amostragem de fragmentos brutos do comitê do nexo کے بجائے certificados validados کرتی ہے؛ DS privado اندرونی atestados DA رکھتے ہیں۔
- Provas de recursão e época: اختیاری طور پر ایک DS میں متعدد microlotes کو ایک prova recursiva por slot/época میں agregado کریں تاکہ tamanhos de prova اور verificar tempo de alta carga پر مستحکم رہیں۔
- Escala de pista (اگر ضرورت ہو): اگر ایک gargalo do comitê global بن جائے تو K pistas de sequenciamento paralelo متعارف کریں جن کی mesclagem determinística ہو۔ اس سے ordem global única برقرار رہتا ہے جبکہ escala horizontal ہو جاتی ہے۔
- Aceleração determinística: hashing/FFT کے لئے Kernels controlados por recursos SIMD/CUDA دیں اور substituto de CPU com bit exato رکھیں تاکہ determinismo entre hardware برقرار رہے۔
- Limites de ativação de faixa (proposta): 2-4 faixas فعال کریں اگر (a) p95 finalidade 1,2 s سے زیادہ ہو >3 مسلسل منٹ، یا (b) ocupação por bloco 85% سے زیادہ ہو >5 منٹ، یا (c) taxa de transmissão de entrada sustentada سطحوں پر capacidade de bloco کی >1.2x ضرورت رکھتی ہو۔ faixas determinísticas طور پر DSID hash کے ذریعے transações کو bucket کرتی ہیں اور bloco de nexo میں mesclagem ہوتی ہیں۔

Taxas e Economia (padrões)
- Unidade de gás: token de gás por DS جس میں computação/IO medido ہوتا ہے؛ taxas DS کے ativo de gás nativo میں ادا ہوتی ہیں۔ Aplicativo de conversão DS کے درمیان کی ذمہ داری ہے۔
- Prioridade de inclusão: round-robin entre DS کے ساتھ cotas por DS تاکہ justiça اور 1s SLOs برقرار رہیں؛ DS کے اندر taxa de desempate de licitação کر سکتی ہے۔
- Futuro: mercado de taxas globais opcional یا Políticas de minimização de MEV explorar کی جا سکتی ہیں بغیر atomicidade یا Design à prova de PQ بدلے۔Fluxo de trabalho entre espaços de dados (exemplo)
1) ایک صارف AMX ٹرانزیکشن submit کرتا ہے جو public DS P اور private DS S کو touch کرتی ہے: ativo X کو S سے beneficiário B تک منتقل کریں جس کی conta P میں ہے۔
2) slot کے اندر، P اور S ہر ایک اپنا instantâneo de slot de fragmento کے خلاف executar کرتے ہیں۔ Autorização S اور verificação de disponibilidade کرتا ہے، اپنا atualização de estado interno کرتا ہے، اور prova de validade PQ اور compromisso DA بناتا ہے (کوئی vazamento de dados privados نہیں ہوتا)۔ P متعلقہ atualização de estado تیار کرتا ہے (مثلاً política کے مطابق P میں hortelã/queimadura/bloqueio) اور اپنی prova۔
3) comitê do nexo دونوں Provas DS e verificação de certificados DA کرتی ہے؛ اگر دونوں slot کے اندر verificar ہوں تو ٹرانزیکشن 1s Nexus Bloco میں commit atomicamente ہوتی ہے, vetor de estado mundial global میں دونوں Atualização de raízes DS ہوتے ہیں۔
4). O que você precisa saber sobre isso کسی مرحلے پر S سے کوئی dados privados باہر نہیں جاتا۔

- سکیورٹی پر غور e فکر
- Execução Determinística: IVM syscalls determinísticos رہتے ہیں؛ cross-DS نتائج AMX commit اور finalidade سے چلتے ہیں, relógio de parede یا tempo de rede سے نہیں۔
- Controle de acesso: DS privado میں permissões ISI محدود کرتی ہیں کہ کون ٹرانزیکشن enviar کر سکتا ہے اور کون سی operações permitidas ہیں۔ tokens de capacidade cross-DS استعمال کے لئے codificação refinada حقوق کرتے ہیں۔
- Confidencialidade: dados DS privados کے لئے criptografia ponta a ponta, fragmentos codificados para eliminação صرف membros autorizados میں ذخیرہ, بیرونی atestados کے لئے provas ZK opcionais۔
- Resistência DoS: camadas mempool/consenso/armazenamento میں isolamento congestionamento público کو progresso DS privado پر اثر انداز ہونے سے روکتی ہے۔

Componentes Iroha میں تبدیلیاں
- iroha_data_model: `DataSpaceId`, identificadores qualificados para DS, descritores AMX (conjuntos de leitura/gravação), tipos de compromisso de prova/DA متعارف کریں۔ serialização Norito۔
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA, syscalls e tipos ABI de ponteiro, شامل کریں؛ Testes/documentos ABI کو v1 پالیسی کے مطابق اپ ڈیٹ کریں۔