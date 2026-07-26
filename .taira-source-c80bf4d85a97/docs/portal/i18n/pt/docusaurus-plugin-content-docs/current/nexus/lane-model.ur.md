---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: modelo de pista Nexus
description: Sora Nexus کے لئے pistas کی منطقی taxonomia, geometria de configuração, e fusão de estado mundial کے اصول۔
---

# Modelo de pista Nexus e particionamento WSV

> **Status:** Entrega NX-1 - taxonomia de pista, geometria de configuração, e layout de armazenamento نفاذ کے لئے تیار ہیں۔  
> **Proprietários:** Nexus GT Central, GT de Governança  
> **Referência do roteiro:** `roadmap.md` Modelo NX-1

یہ پورٹل صفحہ canonical `docs/source/nexus_lanes.md` brief کی عکاسی کرتا ہے تاکہ Sora Nexus آپریٹرز, proprietários de SDK e revisores árvore mono-repo میں جائے بغیر orientação de faixa پڑھ سکیں۔ ہدفی arquitetura estado mundial کی determinismo برقرار رکھتا ہے جبکہ انفرادی espaços de dados (faixas) کو conjuntos de validadores públicos e privados کے ساتھ cargas de trabalho isoladas چلانے دیتا ہے۔

## Conceitos

- **Lane:** Nexus ledger کا منطقی shard, اپنے conjunto de validadores e backlog de execução کے ساتھ۔ O modelo `LaneId` é um produto de alta qualidade
- **Espaço de dados:** balde de governança جو ایک یا زیادہ faixas کو گروپ کرتا ہے جو conformidade, roteamento, políticas de liquidação شیئر کرتے ہیں۔
- **Lane Manifest:** metadados controlados por governança e validadores, política DA, token de gás, regras de liquidação, e permissões de roteamento بیان کرتا ہے۔
- **Compromisso Global:** ایک prova جو lane جاری کرتی ہے, نئے raízes estaduais, dados de liquidação, اور transferências cruzadas opcionais کا خلاصہ دیتی ہے۔ compromissos de anel NPoS globais

## Taxonomia de pista

Tipos de pista اپنی visibilidade, superfície de governança اور ganchos de liquidação کو canônico طور پر بیان کرتے ہیں۔ geometria de configuração (`LaneConfig`) ان atributos کو captura کرتی ہے تاکہ nós, SDKs اور ferramentas بغیر lógica sob medida کے layout کو سمجھ سکیں۔

| Tipo de pista | Visibilidade | Associação do validador | Exposição ao WSV | Governança padrão | Política de liquidação | Uso típico |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | público | Sem permissão (participação global) | Réplica completa do estado | SORA Parlamento | `xor_global` | Livro-razão público de base |
| `public_custom` | público | Sem permissão ou controlado por estacas | Réplica completa do estado | Módulo ponderado por estaca | `xor_lane_weighted` | Aplicações públicas de alto rendimento |
| `private_permissioned` | restrito | Conjunto de validadores fixo (aprovado pela governança) | Compromissos e provas | Conselho Federado | `xor_hosted_custody` | CBDC, cargas de trabalho do consórcio |
| `hybrid_confidential` | restrito | Associação mista; envolve provas ZK | Compromissos + divulgação seletiva | Módulo monetário programável | `xor_dual_fund` | Dinheiro programável que preserva a privacidade |

Os tipos de pista que você pode declarar são:

- Alias do espaço de dados - انسان کے لئے پڑھنے کے قابل agrupamento جو políticas de conformidade کو vincular کرتی ہے۔
- Identificador de governança - ایسا identificador جو `Nexus.governance.modules` کے ذریعے resolver ہوتا ہے۔
- Identificador de liquidação - Identificador جو buffers XOR do roteador de liquidação کو débito کرنے کے لئے استعمال کرتا ہے۔
- Metadados de telemetria opcionais (descrição, contato, domínio comercial) جو `/status` اور painéis کے ذریعے ظاہر ہوتے ہیں۔

## Geometria de configuração da pista (`LaneConfig`)

Catálogo de pistas validadas `LaneConfig` سے geometria de tempo de execução derivada ہے۔ یہ manifestos de governança کو substituir نہیں کرتا؛ اس کے بجائے ہر faixa configurada کے لئے identificadores de armazenamento determinísticos اور dicas de telemetria فراہم کرتا ہے۔

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- Geometria `LaneConfig::from_catalog` کو دوبارہ computação کرتا ہے جب carga de configuração ہو (`State::set_nexus`).
- Aliases کو slugs minúsculos میں higienizar کیا جاتا ہے؛ مسلسل caracteres não alfanuméricos `_` میں colapso ہوتے ہیں۔ اگر alias vazio slug دے تو ہم `lane{id}` fallback کرتے ہیں۔
- Chave de metadados do catálogo `shard_id` `da_shard_id` سے derivar ہوتا ہے (padrão `lane_id`) اور diário de cursor de shard persistente کو unidade کرتا ہے تاکہ reinicia/resharding میں DA reproduzir رہے۔ determinístico
- Prefixos de chave یہ یقینی بناتے ہیں کہ Intervalos de chaves WSV por pista کو جدا رکھے, چاہے backend compartilhado ہو۔
- Nomes de segmentos Kura hosts کے درمیان determinístico ہوتے ہیں؛ auditores بغیر ferramentas sob medida کے diretórios de segmento اور verificação cruzada de manifestos کر سکتے ہیں۔
- Mesclar segmentos (`lane_{id:03}_merge`) اس lane کے últimas raízes de dicas de mesclagem اور compromissos estatais globais محفوظ کرتے ہیں۔

## Particionamento do estado mundial- Espaços de estado por faixa de estado mundial lógico Nexus کا união ہے۔ O estado completo das vias públicas persiste کرتی ہیں؛ pistas privadas/confidenciais Merkle/raízes de compromisso کو razão de mesclagem میں exportação کرتی ہیں۔
- Chave de armazenamento MV کو `LaneConfigEntry::key_prefix` کے Prefixo de 4 bytes سے prefixo کرتا ہے, جس سے `[00 00 00 01] ++ PackedKey` جیسے chaves بنتے ہیں۔
- Entradas de tabelas compartilhadas (contas, ativos, gatilhos, registros de governança) کو prefixo de pista کے حساب سے grupo کرتی ہیں، جس سے varreduras de intervalo determinísticas رہتے ہیں۔
- Metadados do livro-razão de mesclagem اسی layout کو espelho کرتا ہے: ہر lane `lane_{id:03}_merge` میں raízes de dica de mesclagem اور raízes de estado globais reduzidas لکھتی ہے, جس سے lane retirar ہونے پر retenção direcionada یا despejo ممکن ہوتی ہے۔
- Índices cruzados (aliases de contas, registros de ativos, manifestos de governança) prefixos de faixa explícitos armazenam کرتے ہیں تاکہ entradas de operadores جلد reconciliar کر سکیں۔
- **Política de retenção** - vias públicas مکمل bloquear corpos رکھتی ہیں؛ pontos de verificação de faixas somente para compromisso کے بعد پرانے corpos compactos کر سکتی ہیں کیونکہ compromissos autoritativos ہیں۔ Diários de texto cifrado de faixas confidenciais کو segmentos dedicados میں رکھتی ہیں تاکہ دوسرے bloco de cargas de trabalho نہ ہوں۔
- **Ferramentas** - utilitários de manutenção (`kagami`, comandos de administração CLI) کو métricas expõem کرتے, rótulos Prometheus بناتے یا Arquivo de segmentos Kura کرتے وقت namespace slugged consulte کرنا چاہیے۔

## Roteamento e APIs

- Torii endpoints REST/gRPC opcionais `lane_id` قبول کرتے ہیں؛ عدم موجودگی `lane_default` کو ظاہر کرتی ہے۔
- SDKs seletores de pistas فراہم کرتے ہیں اور aliases fáceis de usar کو catálogo de pistas کے ذریعے `LaneId` سے mapa کرتے ہیں۔
- Catálogo validado de regras de roteamento پر operar کرتے ہیں اور pista اور espaço de dados دونوں منتخب کر سکتے ہیں۔ Painéis `LaneConfig` e logs کے لئے aliases compatíveis com telemetria فراہم کرتا ہے۔

## Liquidação e taxas

- ہر conjunto de validador global de pista کو taxas XOR ادا کرتی ہے۔ Tokens de gás nativos de faixas جمع کر سکتی ہیں مگر compromissos کے ساتھ Garantia de equivalentes XOR کرنا لازم ہے۔
- Provas de liquidação میں valor, metadados de conversão, اور prova de garantia شامل ہوتے ہیں (مثلا cofre de taxa global کو transferência)۔
- Buffers do roteador de liquidação unificado (NX-3) کو انہی prefixos de pista کے ساتھ débito کرتا ہے, لہذا geometria de armazenamento de telemetria de liquidação کے ساتھ alinhar ہوتی ہے۔

## Governança

- Faixas اپنی módulo de governança کو catálogo کے ذریعے declarar کرتی ہیں۔ `LaneConfigEntry` اصل alias اور slug ساتھ رکھتا ہے تاکہ telemetria اور trilhas de auditoria legíveis رہیں۔
- Manifestos de pista assinada do registro Nexus
- Políticas de governança de ganchos de atualização em tempo de execução (padrão `gov_upgrade_id` بطور) ہیں۔

## Telemetria e status

- `/status` aliases de pista, ligações de espaço de dados, identificadores de governança e perfis de liquidação کو expor کرتا ہے, جو catálogo اور `LaneConfig` سے derivar ہوتے ہیں۔
- Métricas do agendador (`nexus_scheduler_lane_teu_*`) aliases/slugs de pista دکھاتے ہیں تاکہ backlog de operadores اور pressão TEU کو جلد mapa کر سکیں۔
- Entradas de pista derivadas `nexus_lane_configured_total` Geometria da pista de telemetria بدلنے پر diferenças assinadas emitem کرتی ہے۔
- Dataspace backlog mede metadados de alias/descrição شامل کرتے ہیں تاکہ pressão de fila de operadores کو domínios de negócios

## Configuração e tipos Norito

- `LaneCatalog`, `LaneConfig`, اور `DataSpaceCatalog` `iroha_data_model::nexus` میں رہتے ہیں اور manifestos e SDKs کے لئے estruturas de formato Norito فراہم کرتے ہیں۔
- `LaneConfig` `iroha_config::parameters::actual::Nexus` میں رہتا ہے اور catálogo سے خودکار طور پر derivar ہوتا ہے؛ اسے Codificação Norito کی ضرورت نہیں کیونکہ یہ auxiliar de tempo de execução interno ہے۔
- Configuração voltada para o usuário (`iroha_config::parameters::user::Nexus`) via declarativa e descritores de espaço de dados کو قبول کرتی رہتی ہے؛ análise de derivação de geometria کرتا ہے اور aliases inválidos یا IDs de pista duplicados کو rejeitar کرتا ہے۔

## Excelente trabalho

- atualizações do roteador de liquidação (NX-3) کو نئی geometria کے ساتھ integrar کریں تاکہ Débitos de buffer XOR اور slug de pista de recibos کے مطابق tag ہوں۔
- Ferramentas de administração کو estender کریں تاکہ lista de famílias de colunas ہوں، pistas aposentadas compactas ہوں، اور espaço de nomes slugged کے ساتھ inspecionar logs de blocos por pista ہوں۔
- Algoritmo de mesclagem (ordenação, remoção, detecção de conflitos) finalização کریں اور replay cross-lane کے لئے acessórios de regressão شامل کریں۔
- Listas brancas/listas negras e políticas monetárias programáveis ​​کے لئے ganchos de conformidade شامل کریں (NX-12 میں ٹریک)۔---

*یہ صفحہ NX-2 سے NX-18 کے اترنے کے ساتھ Acompanhamentos NX-1 کو ٹریک کرتا رہے گا۔ براہ کرم کھلے سوالات `roadmap.md` یا rastreador de governança میں سامنے لائیں تاکہ پورٹل canonical docs کے ساتھ alinhado رہے۔*