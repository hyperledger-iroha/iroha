---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-spec
כותרת: Especificacao tecnica da Sora Nexus
תיאור: Espelho completo de `docs/source/nexus.md`, cobrindo a arquitetura e as restricoes de design para o Ledger Iroha 3 (Sora Nexus).
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/nexus.md`. Mantenha ambas as copias alinhadas ate que o backlog de traducao chegue ao פורטל.
:::

#! Iroha 3 - Sora Nexus ספר חשבונות: Especificacao tecnica de design

מסמך זה מציע ארכיטקטורה של Sora Nexus Ledger עבור Iroha 3, evoluindo o Iroha 2 עבור Ledger Global Unico e Logicamente Unificado Organizado in Torno de Data Spaces (DS). מרחבי נתונים fornecem dominios fortes de privacidade ("מרחבי נתונים פרטיים") e participacao aberta ("מרחבי נתונים ציבוריים"). או שמירה על עיצוב והרכבה ארוכת טווח לעשות ספר עולמי אקוואנטו אסטרטגי estrito e confidencialidade para dados de private-DS, e introduz escala de disponibilidade de dados via codificacao de apagamento em Kura (אחסון בלוק) ו-WSV (World State View).

O mesmo repositorio compila tanto Iroha 2 (Redes auto-hospedadas) quanto Iroha 3 (SORA Nexus). מכונה וירטואלית של Iroha (IVM) כוללת שרשרת הכלים של Kotodama. Nexus.

אובייקטיבוס
- ניהול חשבונות עולמי של שיתוף פעולה ומרחבי נתונים.
- Data Spaces privados para operacao permissionada (לדוגמה, CBDCs), com dodos que nunca saem do DS privado.
- Data Spaces publicos com participacao aberta, accesso sem permissao estilo Ethereum.
- Contratos inteligentes composaveis intre Data Spaces, sojeitos and permissoes explicitas para accesso and ativos de private-DS.
- Isolamento de desempenho para que a atividade publica nao degrade transacoes internas de private-DS.
- Disponibilidade de dados em escala: Kura e WSV com codificacao de apagamento para suportar dados efetivamente ilimitados mantendo dados de private-DS privados.

Nao objetivos (שלב ראשוני)
- Definir economia de token ou incentivos de validadores; politicas de scheduling e staking sao pluggable.
- Introduzir uma nova versao de ABI; הצגת מידע על ABI v1 com הרחבות מפורשת של syscalls e pointer-ABI תואם לפוליטיקה של IVM.טרמינולוגיה
- Nexus ספר חשבונות: ספר חשבונות הגלובלי פורמאדו או חיבור גוש הנתונים של מרחב הנתונים (DS) עם אומה היסטורית רגילה יחידה ועם פשרה.
- מרחב נתונים (DS): Dominio delimitado de execucao e armazenamento com seus proprios validadores, governanca, classe de privacidade, politica de DA, quotas and politica de taxas. שיעורי דואס קיימים: DS ציבוריים ו-DS פרטיים.
- מרחב נתונים פרטי: Validadores permissionados e control de acesso; dados de transacao e estado nunca saem do DS. Apenas compromissos/metadados sao ancorados globalmente.
- מרחב מידע ציבורי: Participacao sem permissao; dados completos e estado sao publicos.
- Manifest Space (DS Manifest): Manifest codificado em Norito que declara parametros DS (validadores/chaves QC, classe de privacidade, politica ISI, parametros DA, retencao, quotas, politica ZK, taxas). הו האש עשה מניפסט e ancorado na cadeia nexus. החלפת סלבו, תעודות קוורום DS usam ML-DSA-87 (מחלקה Dilithium5) como esquema de assinatura post-quantum por padrao.
- ספריית החלל: Contrato de diretorio Global on-chain que rastreia manifestes DS, versoes e eventos de governanca/rotacao para resolucao e auditorias.
- DSID: מזהה גלובלי יחיד עבור מרחב נתונים. Usado para namespace de todos או objetos e references.
- עוגן: Compromisso criptografico de um bloco/header DS כולל קשר קדום להיסטוריית DS או Ledger Global.
- Kura: Armazenamento de blocos Iroha. Estendido aqui com armazenamento de blobs codificados com apagamento e compromissos.
- WSV: Iroha World State View. Estendido aqui com segmentos de estado versionados, com תמונות, e codificados com apagamento.
- IVM: Iroha Machine Virtual Para Execucao de contratos inteligentes (קוד בייט Kotodama `.to`).
  - AIR: ייצוג ביניים אלגברי. Visao algebrica de computacao para provas estilo STARK, decrevendo a execucao como tracos baseados em campos com restricoes de transicao e fronteira.דגם מרחבי נתונים
- זיהוי: `DataSpaceId (DSID)` זיהוי DS ורווח שמות. DS podem ser instanciados em duas granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - execucao e estado delimitados a um dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execucao e estado delimitados a uma definicao de ativo unica.
  אמבס כפורמס דו קיום; transacoes podem tocar multiplos DSIDs de forma atomica.
- Ciclo de vida do manifest: criacao de DS, atualizacoes (rotacao de chaves, mudancas de politica) e aposentadoria sao registradas no Space Directory. Cada artefato DS ל-Slot Reference o Hash do manifest mais recente.
- שיעורים: DS ציבורי (participacao aberta, DA publica) e פרטי DS (אישור, DA סודי). Politicas hibridas sao possiveis באמצעות דגלים אכן מופיעים.
- Politicas por DS: permissoes ISI, parametros DA `(k,m)`, criptografia, retencao, quotas (participacao min/max de tx por bloco), politica de prova ZK/otimista, taxas.
- Governanca: חברות DS e rotacao de validadores definida pela secao de governanca do manifest (propostas on-chain, multisig ou governanca externa ancorada por transacoes nexus e atestacoes).

Manifests de capacidades e UAID
- תכונות אוניברסליות: קביעת UAID (`UniversalAccountId` ו-`crates/iroha_data_model/src/nexus/manifest.rs`) כדי לשנות את מרחבי הנתונים. Manifests de capacidades (`AssetPermissionManifest`) vinculam um UAID a um dataspace especifico, epocas de ativacao/expiracao ו-uma list orderada de regras לאפשר/הכחשה `ManifestEntry` que delimitam Sumeragi000, I108NI, I008NI,000X `method`, `asset` תפקידי AMX אופציונליים. Regras מכחיש סמפר גאנהם; o avaliador emite `ManifestVerdict::Denied` com uma razao de auditoria ou um grant `Allowed` com a metadata de allowance correspondente.
- קצבאות: cada entrada לאפשר carrega buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) אם `max_amount` אופציונלי. מארח SDKs e consomem o mesmo payload Norito, eao aplicacao permaneence identica entre hardware e SDK.
- Telemetria de auditoria: ספריית החלל משדרת את `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) כרוכה בגילוי נאות. שטח חדשני `SpaceDirectoryEventFilter` מאפשר להתערב Torii/data-event לפקח על UAID המניפסט, חזרות והחלטות הכחשות-זכיות בהתאמה אישית של אינסטלציה.

עבור הוכחות תפעוליות מקצה לקצה, הוראות ביצוע של SDK ורשימות תיוג לפרסום, espelhe esta secao com מדריך חשבון אוניברסלי (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados semper que a politica ou as ferramentas UAID mudarem.Arquitetura de alto nivel
1) Camada de composicao global (Nexus שרשרת)
- Mantem uma unica ordenacao canonica de blocos Nexus de 1 segundo que finalizam transacoes atomicas abrangendo um ou mais Data Spaces (DS). Cada transacao התחייבה לאיחוד עולמי של המדינה העולמית (וטור שורשי DS).
- Contem metadados minimos mais provas/QCs agregados para garantir composabilidade, finalidade e deteccao de fraude (DSIDs tocados, roots de estado por DS antes/depois, compromissos DA, provas de validade por DS, e o certificado de quorum ML-DSA usan). Nenhum dado privado e incluido.
- קונצנזו: comite BFT global em pipeline de tamanho 22 (3f+1 com f=7), selecionado de um pool de ate ~200k validadores potenciais por um mecanismo de VRF/stake por epoca. O comite nexus sequencia transacoes e finaliza o bloco em 1s.

2) Camada de Data Space (ציבורי/פרטי)
- Executa fragmentos por DS de transacoes globais, atualiza WSV local do DS e produz artefatos de validade por bloco (provas por DS agregadas e compromissos DA) que se acumulam no bloco Nexus de 1 segundo.
- פרטי DS criptografam dados em repouso e em transito entre validadores autorizados; אפנאס פשרות e provas de validade PQ saem do DS.
- Public DS exportam corpos completos de dados (via DA) e provas de validade PQ.

3) Transacoes atomicas cross-data-space (AMX)
- דגם: cada transacao de usuario pode tocar multiplos DS (לדוגמה, תחום DS e um ou mais asset DS). Ela e committed atomicamente em um unico bloco Nexus ou aborta; nao ha efeitos parciais.
- Prepare-Commit 1s: para cada transacao candidata, DS tocados executam em paralelo contra o mesmo snapshot (roots DS no inicio do slot) e produzem provas de validade PQ por DS (FASTPQ-ISI) ו-DA פשרה. O comite nexus commita a transacao apenas se todas as provas DS exigidas verificarem e os certificados DA chegarem a tempo (alvo <=300 ms); caso contrario, a transacao e reprogramada para o proximo חריץ.
- Consistencia: conjuntos de leitura/escrita sao declarados; deteccao de conflitos ocorre no commit contra roots do inicio do slot. Execucao otimista sem locks por DS evita דוכנים globais; atomicide e imposta pela regra de commit nexus (tudo ou nada entre DS).
- פרטיות: פרטי DS exportam apenas provas/compromissos vinculados aos roots DS pre/post. Nenhum dado privado cru sai do DS.

4) Disponibilidade de dados (DA) com codificacao de apagamento
- Kura armazena corpos de blocos e תמונות WSV como blobs codificados com apagamento. Blobs publicos sao amplamente shardados; blobs privados sao armazenados apenas em validadores private-DS, com chunks criptografados.
- Compromissos DA sao registrados tanto em artefatos DS quanto em blocos Nexus, possibilitando amostragem e garantias de recuperacao sem revelar conteudo privado.Estrutura de bloco e commit
- Artefato de prova de Data Space (פור חריץ de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - פרטי-DS exporta artefatos sem corpos de dados; הציבור DS permite recuperacao de corpos via DA.

- Nexus Block (cadencia de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacoes atomicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcao: finaliza todas as transacoes atomicas cujos artefatos DS requeridos verificam; atualiza o vetor de roots DS לעשות מדינה עולמית גלובלית em um passo.

קונצנזו ותזמון
- קונסנסו של Nexus שרשרת: BFT Global em pipeline (מחלקה Sumeragi) comite de 22 nos (3f+1 com f=7) mirando blocos de 1s e finalidade de 1s. רשומות מבצעים את הבחירה ב-VRF/הימור של ~200,000 מועמדים; a rotacao mantem descentralizacao e resistencia a censura.
- הסכמה של מרחב נתונים: DS executa Seu proprio BFT entre validadores para produzir artefatos por slot (provas, compromissos DA, DS QC). Comites ליין-relay sao dimensionados em `3f+1` usando `fault_tolerance` לעשות מרחב נתונים e sao amostrados de forma deterministica por epoca a partir do pool de validadores do dataspace usando a seed VRF ligada a I018NI070X. פרטי DS sao permissionados; הציבור DS מתיר חיים אברטה סוjeita א פוליטיקה אנטי סיביל. O comite nexus global permanece inalterado.
- תזמון טרנסאקציות: Usuarios submetem transacoes atomicas declarando DSIDs tocados e conjuntos de leitura/escrita. DS executam em parlelo dentro do חריץ; o comite nexus inclui a transacao no bloco de 1s se todos os artefatos DS verificarem e os certificados DA forem pontuais (<=300 אלפיות השנייה).
- Isolamento de desempenho: cada DS tem mempools e execucao independentes. מכסות של DS מוגבלות לטרנסאקויז ל-DS podem ser committed por bloco para evitar head-of-line blocking e proteger and latecia de private DS.

מודלים של מרווחי שמות
- IDs qualificados por DS: todas as entidades (dominios, contas, ativos, roles) sao qualificadas por `dsid`. דוגמה: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias global: uma referencia global e uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain with camada nexus או emcritors AMX para uso cross-DS.
- Serializacao Norito: todas as mensagens cross-DS (מתאר AMX, provas) usam codec Norito. Sem uso de serde em caminhos de producao.Contratos inteligentes e extensoes IVM
- Contexto de execucao: Adicionar `dsid` או Contexto de Execucao IVM. Contratos Kotodama מבצעים את הספציפיות של מרחב הנתונים.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitam uma transacao atomica multi-DS ללא מארח IVM.
  - `amx_touch(dsid, key)` declara intencao de leitura/escrita para deteccao de conflitos contra roots snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> תוצאה (operacao permitida apenas se a politica permitir e o handle for valido)
- הנכס מטפל במסים אלקטרוניים:
  - Operacoes de ativos sao autorizadas pelas politicas ISI/rolle do DS; taxas sao pagas no token de gas do DS. אסימוני יכולת אופציונליים ופוליטיים רבים (מרבים מאשרים, מגבלות קצב, גיאופנסינג)
- Determinismo: todas as novas syscalls sao puras e deterministicas dadas as entradas e os conjuntos de leitura/escrita AMX declarados. Sem efeitos ocultos de tempo ou ambiente.Provas de validade post-quantum (ISI generalizados)
- FASTPQ-ISI (PQ, הגדרה מהימנה): מבוססת טיעון hash que generaliza o design de transfer for todas as familias ISI equinto mira prova sub-segundo para lotes em Escala 20k em hardware class GPU.
  - פרפיל תפעולי:
    - Nos de producao constroem o prover via `fastpq_prover::Prover::canonical`, que agora semper inicializa o backend de producao; o לעג לדטרמיניסטיקו foi removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) ו-`irohad --fastpq-execution-mode` מאפשרים הפעלת מעבד/GPU של מעבד/GPU deterministica deforma deterministica או צופה registra triplas solicitadas/resolvidas/backend para auditorias de frota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/888.rs:8
- Aritmetizacao:
  - KV-Update AIR: WSV como of mapa key-value tipado comprometido דרך Poseidon2-SMT. Cada ISI se expande para um pequeno conjunto de linhas read-check-write sobre chaves (contas, ativos, roles, dominios, metadata, supply).
  - Restricoes com Gates de opcode: uma unica tabela AIR com colunas seletoras impone regras por ISI (conservacao, contadores monotonic, permissoes, בדיקות טווח, atualizacoes de metadata limitadas).
  - טיעונים לחיפוש: טבלאות שקופות של הרשאות/תפקידים, טבלאות שקופות ופורמטים של הרשאות/תפקידים.
- Compromissos e atualizacoes de estado:
  - Prova SMT agregada: todas as chaves tocadas (pre/post) sao provadas contra `old_root`/`new_root` usando um frontier comprimido com brothers deduplicados.
  - Invariantes: invariantes globais (לדוגמה, אספקה ​​כוללת por ativo) sao impostas via igualdade de multiconjuntos entre linhas de efeito e contadores rastreados.
- סיסטמה דה פרובה:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) com alta aridade (8/16) e blow-up 8-16; hashes Poseidon2; תמלול פיאט-שמיר com SHA-2/3.
  - Recursao אופציונלי: agregacao recursiva local DS para comprimir micro lotes ema prova por slot se necessario.
- Escopo e exemplos cobertos:
  - Ativos: העברה, טביעה, צריבה, רישום/ביטול רישום של הגדרות נכסים, הגדרת דיוק (limitado), הגדרת מטא נתונים.
  - Contas/Dominios: צור/הסר, הגדר מפתח/סף, הוספה/הסרה של חותמים (apenas estado; verificacoes de assinatura sao atestadas por validadores DS, nao provadas dentro do AIR).
  - תפקידים/הרשאות (ISI): הענק/שלילת הרשאות e של תפקידים; impostas por tabelas de lookup e checks de politica monotonic.
  - Contratos/AMX: מרקדורים מתחילים/מבצעים AMX, יכולת נחוש/בטל את הגישה; provados como transicoes de estado e contadores de politica.
- בדיקות פורה לעשות AIR לשמירה על חוסר זמן:- Assinaturas e criptografia pesada (לדוגמה, assinaturas ML-DSA de usuario) sao verificadas por validadores DS e atestadas no DS QC; a prova de validade cobre apenas consistencia de estado e conformidade de politica. Isso mantem provas PQ e rapidas.
- Metas desempenho (איורים, מעבד 32 ליבות + uma GPU moderna):
  - 20k ISIs mistos com key-touch pequeno (<=8 chaves/ISI): ~0.4-0.9 s de prova, ~150-450 KB de prova, ~5-15 ms de verificacao.
  - ISIs mais pesadas (mais chaves/contrastes ricos): מיקרו-לוטה (לדוגמה, 10x2k) + recursao para manter <1 s por slot.
- Configuracao do DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (מאומת של DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (padrao; alternativas devem ser declaradas explicitamente)
- נפילות:
  - ISIs complexas/personalizadas podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com prova adiada e finalidade de 1 s via atestacao QC + slashing em provas invalidas.
  - Opcoes nao PQ (לדוגמה, Plonk com KZG) Exigem Trusted Setup e nao sao mais suportadas no build padrao.

Introducao ao AIR (para Nexus)
- Traco de execucao: matriz com largura (colunas de registros) e comprimento (passos). Cada linha e um passo logico do processamento ISI; colunas armazenam valores pre/post, seletores e flags.
- מגבלות:
  - Restricoes de transicao: impoem relacoes de linha a linha (לדוגמה, post_balance = pre_balance - כמות para uma linha de debito quando `sel_transfer = 1`).
  - Restricoes de fronteira: ligam I/O publico (old_root/new_root, contadores) כמו primeiras/ultimas linhas.
  - חיפושים/תמורות: חברות garantem e igualdade de multiconjuntos contra tabelas comprometidas (permissoes, parametros de ativos) sem circuitos pesados ​​de bits.
- Compromisso e verificacao:
  - O prover compromete traces via codificacoes hash e constroi polinomios de baixo grau que sao validos se as restricoes forem satisfeitas.
  - O Verifier checa baixo grau באמצעות FRI (מבוסס גיבוב, פוסט-קוונטי) com poucas aberturas Merkle; o custo e logaritmico nos passos.
- דוגמה (העברה): רישום כולל pre_balance, amount, post_balance, nonce e seletores. Restricoes impem nao negatividade/intervalo, conservacao e monotonicidade de nonce, enquanto uma multiprova SMT agregada liga folhas pre/post a roots old/new.

Evolucao de ABI e syscalls (ABI v1)
- Syscall מפרט (nomes illustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos pointer-ABI מוסיף:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Atualisacoes necessarias:
  - Adicionar a `ivm::syscalls::abi_syscall_list()` (manter ordenacao), gate por politica.
  - Mapear numeros desconhecidos para `VMError::UnknownSyscall` ללא מארחים.
  - Atualizar testes: syscall list golden, ABI hash, pointer type ID goldens e בדיקות מדיניות.
  - מסמכים: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.דגם פרטיות
- Contencao de dados privados: corpos de transacao, diffs de estado e WSV de private DS nunca deixam o subconjunto privado de validadores.
- Exposicao publica: כותרות apenas, compromissos DA e provas de validade PQ sao exportados.
- Provas ZK opcionais: פרטי DS podem produzir provas ZK (לדוגמה, saldo suficiente, politica satisfeita) habilitando acoes cross-DS sem revelar estado interno.
- Controle de acesso: autorizacao e imposta por politicas ISI/rolle dentro do DS. אסימוני יכולת סאו אופציונאיים e podem ser introduzidos depois.

Isolamento de desempenho e QoS
- קונצנזו, מפולס ו-Armazenamento Separados por DS.
- מכסות של תזמון קשר ל-DS להגביל או קצב הכלל של עוגנים וחסימת ראש קו.
- Orcamentos de recursos de contrato por DS (מחשוב/זיכרון/IO), מארח מארח IVM. Contencao em public DS nao pode consumir orcamentos de private DS.
- Chamadas cross-DS assincronas evitam esperas sincronas longas dentro da execucao פרטי-DS.

Disponibilidade de dados e design de armazenamento
1) Codificacao de apagamento
- Usar Reed-Solomon sistematico (לדוגמה, GF(2^16)) para codificacao de apagamento em nivel de blob de blocos Kura e snapshots WSV: parametros `(k, m)` com `n = k + m` shards.
- Parametros padrao (propostos, DS ציבורי): `k=32, m=16` (n=48), permitindo recuperacao de ate 16 shards perdidos com ~1.5x de expansao. עבור DS פרטי: `k=16, m=8` (n=24) dentro do conjunto permissionado. Ambos configuravis por DS Manifest.
- Blobs publicos: shards distribuidos por muitos nos DA/validadores com checagens de disponibilidade por amostragem. פשרות DA עם כותרות מאפשרות אימות ללקוחות קלים.
- Blobs privados: shards criptografados e distribuidos apenas entre validadores private-DS (ou custodios designados). A cadeia global carrega apenas compromissos DA (sem localizacoes de shards ou chaves).

2) Compromissos e amostragem
- Para cada blob: calcular uma raiz Merkle sobre shards e inclui-la em `*_da_commitment`. Manter PQ evitando compromissos de curva eliptica.
- מציינים של DA: מציינים אזורי אמוסטרדם ל-VRF (לדוגמה, 64 לרשות) emitem um certificado ML-DSA-87 atestando amostragem bem sucedida de shards. Meta de latencia de atestacao DA <=300 ms. O comite nexus valida certificados em vez de buscar shards.

3) Integracao com Kura
- Blocos armazenam corpos de transacao como blobs codificados com apagamento e compromissos Merkle.
- Headers carregam compromissos de blob; corpos sao recuperaveis via rede DA para public DS e via canais privados para private DS.4) Integracao com WSV
- Snapshots WSV: נקודת הביקורת התקופתית של DS היא תצלומי מצב חתוכים וקודים בחתיכות ובין כותרות פשרות. הזן צילומי מצב, יומני שינוי מנדט. תמונות Snapshots publicos sao amplamente shardados; צילומי מצב פרטיים קבועים דנטרו validadores privados.
- Acesso com prova: contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por compromissos de snapshot. פרטי DS podem fornecer atestacoes אפס ידע ב-vez de provas cruas.

5) גיזום Retencao e
- סם גיזום עבור DS ציבורי: reter todos os corpos Kura e snapshots WSV via DA (escalabilidade horizontal). Podem DS פרטי מגדיר retencao interna, mas compromissos exportados permanecem imutavis. קשר חברתי שומר על מערכות ההפעלה Nexus ופשרה של DS.

Rede e papeis de nos
- Validadores globais: participam do consenso nexus, validam blocos Nexus e artefatos DS, realizam checagens DA para public DS.
- Validadores de Data Space: executam consenso DS, executam contratos, gerenciam Kura/WSV local, lidam com DA para seu DS.
- Nos DA (אופציונלי): armazenam/publicam blobs publicos, facilitam amostragem. עבור DS פרטית, אין שיתוף פעולה מקומי עם אישורי שמירה או תמיכה.Melhorias e consideracoes de sistema
- Desacoplamento רצף/מפול: adotar um mempool DAG (לדוגמה, estilo Narwhal) alimentando um BFT עם צינור עם חיבור camada para reduzir latencia e melhorar תפוקה דומה או מודל לוגיקה.
- מכסות DS והוגנות: מכסות פור DS por bloco e caps de peso para evitar head-of-line blocking e garantir latencia previsivel para private DS.
- Atestacao DS (PQ): תעודות קוורום DS usam ML-DSA-87 (classe Dilithium5) por padrao. לאחר קוואנטום e maior que assinaturas EC, mas aceitavel com um QC por חריץ. DS podem optar explicitamente por ML-DSA-65/44 (menores) ou assinaturas EC se declarado no DS Manifest; הציבור DS sao fortemente encorajados a manter ML-DSA-87.
- אישורי DA: para public DS, usar attesters regionais amostrados por VRF que emitem certificados DA. O comite nexus valida certificados em vez de amostragem bruta de shards; פרטי DS mantem atestacoes DA internas.
- Recursao e provas por epoca: opcionalmente agregar varios micro-lotes dentro de um DS em uma prova recursiva por slot/epoca para manter tamanho de prova e tempo de verificacao estaveis sob alta carga.
- Escalonamento de lanes (לפי הכרחי): se um unico comite global virar gargalo, introduzir K lanes de sequenciamento paralelas com merge deterministico. יש לשמור על אומה אוניקה אומנותית גלובלית.
- Aceleracao deterministica: גרעיני SIMD/CUDA com תכונה דגלים עבור hashing/FFT com fallback CPU bit-exato לשמירה על חומרה צולבת.
- Limiares de ativacao de lanes (proposta): habilitar 2-4 מסלולים se (א) a finalidade p95 exceder 1.2 s por >3 minutes consecutivos, ou (b) a occupacao por bloco exceder 85% por >5 minutes, ou (c) a taxa de entradair a capacida de >1. niveis sustentados. נתיבים fazem bucket de transacoes de forma deterministica por hash de DSID e se fazem merge no bloco nexus.

Taxas e Economia (padroes iniciais)
- Unidade de gas: token de gas por DS com compute/IO medido; taxas sao pagas no ativo de gas nativo do DS. Conversao entre DS e Responsabilidade da aplicacao.
- Prioridade de inclusao: round-robin entre DS com quotas por DS para preservar fairness e SLOs de 1s; dentro de um DS, הצעת שכר טרחה pode desempatar.
- Futuro: Mercado Global de taxas או Politicas que Minimizem MEV podem ser explorados sem mudar a atomicide או o design de provas PQ.Fluxo Cross-Data-Space (דוגמה)
1) Um usuario envia uma transacao AMX tocando public DS P e private DS S: mover o ativo X de S para o beneficiario B cuja conta esta em P.
2) Dentro do חריץ, P e S executam seu fragmento contra o snapshot do חריץ. S verifica autorizacao e disponibilidade, atualiza seu estado interno e produz uma prova de validade PQ e compromisso DA (nenhum dado privado vaza). P prepara a atualizacao de estado correspondente (לדוגמה, מנטה/צריבה/נעילה em P conforme politica) e sua prova.
3) O comite nexus verifica ambas as provas DS e certificados DA; se ambas verificarem dentro do slot, a transacao e committed atomicamente no bloco Nexus de 1s, atualizando ambos roots DS לא וטור de world state global.
4) Se qualquer prova ou certificado DA estiver faltando/invalido, a transacao aborta (sem efeitos) e o cliente pode reenviar para o proximo slot. Nenhum dado privado sai de S em nenhum passo.

- Consideracoes de seguranca
- Execucao deterministica: syscalls IVM permanecem deterministicas; resultados cross-DS sao guiados por commit AMX e finalizacao, nao por relogio ou timing de rede.
- Controle de acesso: הרשאות ISI em private DS restringem quem pode submeter transacoes e quais operacoes sao permitidas. אסימוני יכולת קודים קודים לפינוסים לשימוש חוצה DS.
- Confidencialidade: criptografia מקצה לקצה para dados private-DS, shards codificados com apagamento armazenados apenas entre membros autorizados, provas ZK opcionais para atestacoes externas.
- Resistencia a DoS: isolamento em mempool/consenso/armazenamento impede que congestionamento publico impacte o progresso de private DS.

Mudancas nos componentes Iroha
- iroha_data_model: introduzir `DataSpaceId`, IDs qualificados por DS, decritores AMX (conjuntos leitura/escrita), tipos de prova/compromissos DA. Serializacao somente Norito.
- ivm: adicionar syscalls e tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA; atualizar testes/docs ABI conforme politica v1.