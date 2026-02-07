---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: Modelo de pistas Nexus
descrição: Taxonomia lógica das pistas, geometria de configuração e regras de fusão do estado mundial para Sora Nexus.
---

# Modelo de pistas Nexus e particionamento WSV

> **Estatuto:** NX-1 disponível - taxonomia de pistas, geometria de configuração e layout de estoque prets para implementação.  
> **Proprietários:** Nexus GT Central, GT de Governança  
> **Roteiro de referência:** NX-1 em `roadmap.md`

Esta página do portal reflete o breve canônico `docs/source/nexus_lanes.md` para que os operadores Sora Nexus, proprietários SDK e leitores possam ler as pistas de orientação sem o explorador da árvore mono-repo. A arquitetura cível guarda o estado mundial determinado por todos os espaços de dados (faixas) e executa conjuntos de validadores públicos ou privados com cargas de trabalho isoladas.

## Conceitos

- **Lane:** shard logique du ledger Nexus com seu próprio conjunto de validadores e backlog de execução. Identificado por um `LaneId` estável.
- **Espaço de dados:** balde de governo reagrupando uma ou mais pistas que compartilham políticas de conformidade, roteamento e liquidação.
- **Lane Manifest:** metadados controlados por validadores decrivantes de governo, DA política, token de gás, regras de liquidação e permissões de roteamento.
- **Compromisso Global:** emise par une lane qui resume as novas raízes de estado, os donnees de liquidação e as transferências de opções cross-lane. Anneau NPoS global ordena os compromissos.

## Taxonomia das pistas

Os tipos de via canônica descritiva têm visibilidade, superfície de governo e ganchos de liquidação. A geometria de configuração (`LaneConfig`) captura esses atributos para os nódulos, SDKs e ferramentas que podem ser racionais no layout sem lógica na medida.

| Tipo de pista | Visibilidade | Associação de validadores | Exposição WSV | Governança por padrão | Política de liquidação | Tipo de uso |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | público | Sem permissão (participação global) | Réplica de estado completa | SORA Parlamento | `xor_global` | Razão pública de base |
| `public_custom` | público | Sem permissão ou protegido por estacas | Réplica de estado completa | Módulo pondere par stake | `xor_lane_weighted` | Aplicações publicam um alto rendimento |
| `private_permissioned` | restrito | Conjunto fixo de validadores (aprovado por governo) | Compromissos e provas | Conselho Federado | `xor_hosted_custody` | CBDC, cargas de trabalho de consórcio |
| `hybrid_confidential` | restrito | Mistura de membros; revestir as provas ZK | Compromissos + divulgação seletiva | Módulo de mês programável | `xor_dual_fund` | Monnaie programável preserva a privacidade |

Todos os tipos de pista devem ser declarados:

- Alias ​​de dataspace - reagrupamento lisível por humanos dependentes de políticas de conformidade.
- Lidar com a governança - resolução identificadora via `Nexus.governance.modules`.
- Handle de liquidação - identificador consumido pelo roteador de liquidação para debitar os buffers XOR.
- Metadados de opções de telemetria (descrição, contato, domínio comercial) expostos via `/status` e painéis.

## Geometria de configuração das pistas (`LaneConfig`)

`LaneConfig` é o tempo de execução da geometria derivado do catálogo de pistas válido. Il ne substituta os manifestos de governo; ele fornece muitos identificadores de armazenamento determinados e dicas de telemetria para cada faixa configurada.

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

- `LaneConfig::from_catalog` recalcula a geometria quando a configuração é cobrada (`State::set_nexus`).
- Os aliases são higienizados, em slugs e minúsculos; Os caracteres não alfanuméricos consecutivos são compactados em `_`. Se o alias produzir um slug vide, reviente em `lane{id}`.
- `shard_id` é derivado da chave de metadados `da_shard_id` (por padrão `lane_id`) e o piloto do diário de cursor do shard persiste até que você guarde a leitura DA que determina entre redemarrages/resharding.
- Os prefixos de cle garantem que o WSV mantenha as placas de cles par lane disjointes meme se o backend estiver compartilhado.
- Os nomes dos segmentos Kura são determinados entre hosts; Os auditores podem verificar os repertórios de segmento e os manifestos sem ferramentas sob medida.
- Les segmentos merge (`lane_{id:03}_merge`) armazenam as últimas raízes da sugestão de fusão e os compromissos de estado global para esta via.## Particionamento do estado mundial

- A lógica do estado mundial de Nexus é a união de espaços de estado por via. Les lanes publiques persistente l etat complet; les lanes privees/confidencial exportador de raízes Merkle/compromisso com o livro de mesclagem.
- O prefixo MV de armazenamento cada um com o prefixo 4 octetos de `LaneConfigEntry::key_prefix`, produzindo códigos como `[00 00 00 01] ++ PackedKey`.
- As tabelas compartilhadas (contas, ativos, gatilhos, registros de governo) armazenam entradas agrupadas por prefixo de pista, observando os intervalos de varreduras deterministas.
- Os metadados do merge-ledger refletem o layout do meme: cada pista escrita das raízes da dica de mesclagem e das raízes do estado global reduit em `lane_{id:03}_merge`, permitindo uma retenção ou despejo ciblee quando uma pista se aposenta.
- Os índices cruzados (aliases de contas, registros de ativos, manifestos de governança) armazenam prefixos de faixa explícitos para que os operadores reconciliem rapidamente as entradas.
- **Política de retenção** - as vias públicas conservam os corpos dos blocos completos; os compromissos das pistas apenas podem compactar os corpos antigos após os pontos de verificação dos carros, os compromissos são autorizados. As faixas confidenciais armazenam diários importantes em segmentos dedicados para não bloquear outras cargas de trabalho.
- **Ferramentas** - Os utilitários de manutenção (`kagami`, comandos admin CLI) devem referenciar o namespace com slug durante a exposição de métricas, rótulos Prometheus ou arquivamento de segmentos Kura.

## Roteamento e APIs

- Os endpoints Torii REST/gRPC aceitam uma opção `lane_id`; l ausência implícita `lane_default`.
- Os SDKs expõem os seletores de pista e mapeiam os aliases compatíveis com `LaneId` usando o catálogo de pistas.
- As regras de roteamento operam no catálogo válido e podem escolher a via e o espaço de dados. `LaneConfig` fornece aliases amigáveis ​​para telemetria em painéis e registros.

## Liquidação e frete

- Cada pista paga XOR com conjunto de validadores globais. As vias podem coletar tokens de gás nativos, mas também depositar equivalentes XOR com compromissos.
- As provas de liquidação incluem o montante, os metadados de conversão e a garantia de depósito (por exemplo, transferência para o cofre global de dinheiro).
- O roteador de liquidação unificado (NX-3) debita os buffers e utiliza os prefixos de memes da pista, fazendo com que a telemetria de liquidação seja alinhada com a geometria de estoque.

## Governança

- As pistas declaram seu módulo de governança por meio do catálogo. `LaneConfigEntry` porta o alias e o slug de origem para proteger a telemetria e as trilhas de auditoria visíveis.
- O registro Nexus distribui sinais de manifestos de pista que incluem `LaneId`, vinculação de espaço de dados, identificador de governança, identificador de liquidação e metadados.
- Os ganchos de atualização em tempo de execução continuam aplicando as políticas de governo (`gov_upgrade_id` por padrão) e registrando as diferenças por meio da ponte de telemetria (eventos `nexus.config.diff`).

## Telemetria e status

- `/status` expõe os aliases de pista, ligações de espaço de dados, identificadores de governança e perfis de liquidação, derivados do catálogo e de `LaneConfig`.
- As métricas do agendador (`nexus_scheduler_lane_teu_*`) exibem aliases/slugs para que os operadores possam mapear rapidamente o backlog e a pressão do TEU.
- `nexus_lane_configured_total` calcula o nome das entradas da pista derivadas e recalcula quando a alteração da configuração. A telemetria emite sinais diferentes quando a geometria das pistas muda.
- As medições do espaço de dados do backlog incluem o alias/descrição dos metadados para ajudar os operadores a associar a pressão do arquivo aos domínios de negócios.

## Configuração e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e fornecem estruturas no formato Norito para manifestos e SDKs.
- `LaneConfig` é encontrado em `iroha_config::parameters::actual::Nexus` e é derivado automaticamente do catálogo; ele não requer codificação Norito carro c é um auxiliar de tempo de execução interno.
- A configuração do código do usuário (`iroha_config::parameters::user::Nexus`) continua aceitando os descritores declarados de pista e espaço de dados; a análise deriva mantendo a geometria e rejeita os aliases inválidos ou os IDs das pistas duplicadas.

## Travail restante- Integre as atualizações do roteador de liquidação (NX-3) com a nova geometria para que os débitos e o recurso de buffer XOR sejam etiquetados por slug de lane.
- Crie o administrador de ferramentas para listar as famílias de colunas, compactar as faixas removidas e inspecionar os logs de blocos por faixa por meio do slugge do namespace.
- Finalizador do algoritmo de mesclagem (ordenação, poda, detecção de conflitos) e anexo dos acessórios de regressão para a repetição da faixa cruzada.
- Adiciona ganchos de conformidade para listas brancas/listas negras e políticas de dinheiro programáveis ​​(sujeito ao NX-12).

---

*Esta página continua seguindo os acompanhamentos do NX-1 na medida em que o NX-2 é igual ao NX-18 confirmado. Por favor, remonte as perguntas abertas em `roadmap.md` ou o rastreador de governo para que o portal esteja alinhado com os documentos canônicos.*