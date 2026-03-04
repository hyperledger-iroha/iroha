---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: Modelo de pistas de Nexus
descrição: Taxonomia lógica de pistas, geometria de configuração e regras de mesclagem de estado mundial para Sora Nexus.
---

# Modelo de pistas de Nexus e particionado de WSV

> **Estado:** entregable NX-1 - taxonomia de pistas, geometria de configuração e layout de listas de armazenamento para implementação.  
> **Proprietários:** Nexus GT Central, GT de Governança  
> **Referência de roteiro:** NX-1 em `roadmap.md`

Esta página do portal reflete o breve canônico `docs/source/nexus_lanes.md` para que operadores de Sora Nexus, proprietários de SDK e revisores possam ler o guia de pistas sem entrar no arbol mono-repo. O objetivo da arquitetura de manter o estado determinista mundial permite que espaços de dados (lanes) individuais executem conjuntos de validadores públicos ou privados com cargas de trabalho isoladas.

## Conceitos

- **Lane:** fragmento lógico do razão de Nexus com seu próprio conjunto de validadores e backlog de execução. Identificado por um `LaneId` estável.
- **Data Space:** balde de governança que agrupa uma ou mais vias que comparam políticas de conformidade, roteamento e liquidação.
- **Lane Manifest:** metadados controlados por governança que descrevem validadores, política de DA, token de gás, regras de liquidação e permissões de roteamento.
- **Compromisso Global:** comprovante emitido por uma via que resume novas raízes de estado, dados de liquidação e transferências cross-lane opcionais. O anel NPoS ordena compromissos globais.

## Taxonomia de pistas

Os tipos de via descrevem de forma canônica sua visibilidade, superfície de governança e ganchos de liquidação. A geometria de configuração (`LaneConfig`) captura esses atributos para que nós, SDKs e ferramentas possam razonar sobre o layout sem lógica sob medida.

| Tipo de pista | Visibilidade | Membros de validadores | Exposição WSV | Governança por defeito | Política de liquidação | Uso típico |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | público | Sem permissão (participação global) | Réplica de estado completo | SORA Parlamento | `xor_global` | Base pública do razão |
| `public_custom` | público | Sem permissão ou protegido por estacas | Réplica de estado completo | Módulo ponderado por estaca | `xor_lane_weighted` | Aplicações públicas de alto rendimento |
| `private_permissioned` | restrito | Set fijo de validadores (aprovado por governança) | Compromissos e provas | Conselho Federado | `xor_hosted_custody` | CBDC, cargas de trabalho de consórcio |
| `hybrid_confidential` | restrito | Membresia mixta; envuelve provas ZK | Compromissos + divulgação seletiva | Módulo de dinheiro programável | `xor_dual_fund` | Dinheiro programável com preservação de privacidade |

Todos os tipos de pista devem declarar:

- Alias de dataspace - agrupamento legível por humanos que vincula políticas de conformidade.
- Lidar com governança - identificador de resultado via `Nexus.governance.modules`.
- Handle de liquidação - identificador consumido pelo roteador de liquidação para debitar buffers XOR.
- Metadados opcionais de telemetria (descrição, contato, domínio de negócio) expostos via `/status` e dashboards.

## Geometria de configuração de pistas (`LaneConfig`)

`LaneConfig` é a geometria runtime derivada do catálogo de pistas validado. Não reemplaza os manifestos de governança; em seu lugar, forneça identificadores de deterministas de armazenamento e pistas de telemetria para cada pista configurada.

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

- `LaneConfig::from_catalog` recalcula a geometria quando a configuração é carregada (`State::set_nexus`).
- Los aliases são higienizados em lesmas e minúsculas; caracteres sem caracteres alfanuméricos consecutivos são colapsados ​​em `_`. Se o alias produzir um slug vacio, use `lane{id}`.
- `shard_id` é derivado da chave de metadados `da_shard_id` (por defeito `lane_id`) e conduz o diário do cursor do shard persistido para manter a repetição do DA determinista entre reinicializações/resharding.
- Os prefijos de clave garantem que o WSV mantenha rangos de claves por faixa disjuntos quando você compara o mesmo backend.
- Os nomes de segmentos Kura são deterministas entre hosts; auditores podem verificar diretórios de segmento e manifestos sem ferramentas sob medida.
- Os segmentos merge (`lane_{id:03}_merge`) guardam as últimas raízes de merge-hint e compromissos de estado global para essa pista.

## Particionado do estado mundial- O estado lógico mundial de Nexus é a união de espaços de estado por pista. As pistas públicas persistem em estado completo; las lanes privadas/confidenciais raízes exportadoras Merkle/commitment al merge ledger.
- O armazenamento MV prefira cada chave com o prefijo de 4 bytes de `LaneConfigEntry::key_prefix`, gerando chaves como `[00 00 00 01] ++ PackedKey`.
- As tabelas compartilhadas (contas, ativos, gatilhos, registros de governança) armazenam entradas agrupadas por prefijo de pista, mantendo as varreduras de alcance deterministas.
- Os metadados do merge-ledger refletem o mesmo layout: cada pista descreve as raízes da sugestão de mesclagem e as raízes do estado global reduzidas em `lane_{id:03}_merge`, permitindo a retenção ou o despejo direcionado quando uma pista é retirada.
- Os índices cross-lane (aliases de contas, registros de ativos, manifestos de governança) armazenam prefijos de lane explícitos para que os operadores reconciliem as entradas rapidamente.
- **Politica de retencion** - las lanes publicas retienen cuerpos de block completes; las lanes solo de compromissos podem compactar corpos antigos após pontos de verificação porque los compromissos são autorizados. As faixas confidenciais protegem periódicos cifrados em segmentos dedicados para não bloquear outras cargas de trabalho.
- **Tooling** - utilidades de manutenção (`kagami`, comandos admin de CLI) devem referenciar o namespace com slug nas métricas do expositor, etiquetas Prometheus ou no arquivamento de segmentos Kura.

## Roteamento e APIs

- Endpoints Torii REST/gRPC aceitam um `lane_id` opcional; a ausência implica `lane_default`.
- Os SDKs expõem seletores de pista e mapeiam aliases amigáveis ​​para `LaneId` usando o catálogo de pistas.
- As regras de roteamento operam sobre o catálogo validado e podem selecionar pista e espaço de dados. `LaneConfig` fornece aliases amigáveis ​​para telemetria em painéis e logs.

## Taxas de liquidação e

- Cada pista paga taxas XOR no conjunto global de validadores. As vias podem cobrar tokens de gás nativos, mas devem fazer o depósito de equivalentes XOR junto com compromissos.
- As provas de liquidação incluem valores, metadados de conversão e verificação de depósito (por exemplo, transferência para o cofre global de taxas).
- O roteador de liquidação unificado (NX-3) buffers de débito usando os mesmos prefijos de pista, assim como a telemetria de liquidação se alinha com a geometria de armazenamento.

## Governança

- As vias declaram seu módulo de governança por meio do catálogo. `LaneConfigEntry` removerá o pseudônimo e o slug original para manter a telemetria e as trilhas de auditoria legíveis.
- O registro de Nexus distribui manifestos de pista firmados que incluem o `LaneId`, vinculação de espaço de dados, tratamento de governança, tratamento de liquidação e metadados.
- Os ganchos de atualização de tempo de execução seguem aplicando políticas de governança (`gov_upgrade_id` por defeito) e registram diferenças por meio da ponte de telemetria (eventos `nexus.config.diff`).

## Telemetria e status

- `/status` expõe aliases de pista, ligações de espaço de dados, identificadores de governança e perfis de liquidação, resultados de catálogo e `LaneConfig`.
- As métricas do agendador (`nexus_scheduler_lane_teu_*`) exibem aliases/slugs para que os operadores mapeem o backlog e presionem o TEU rapidamente.
- `nexus_lane_configured_total` conta o número de entradas da pista derivada e se recalcula quando a configuração muda. A telemetria emite diferenças firmadas quando a geometria das pistas muda.
- Os medidores de backlog do espaço de dados incluem metadados de alias/descrição para ajudar os operadores a associar pressão de cola com domínios de negócios.

## Configuração e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e provam estruturas no formato Norito para manifestos e SDKs.
- `LaneConfig` vive em `iroha_config::parameters::actual::Nexus` e é derivado automaticamente do catálogo; não requer codificação Norito porque é um auxiliar interno de tempo de execução.
- A configuração de cara do usuário (`iroha_config::parameters::user::Nexus`) segue aceitando os descritores declarativos de pista e espaço de dados; a análise agora deriva da geometria e recupera aliases inválidos ou IDs de pista duplicados.

## Trabalho pendente- Integrar atualizações do roteador de liquidação (NX-3) com a nova geometria para que os débitos e recibos de buffer XOR sejam etiquetados por slug de lane.
- Extender tooling admin para listar famílias de colunas, compactar faixas retiradas e inspecionar logs de blocos por faixa usando o namespace com slug.
- Finalizar o algoritmo de mesclagem (ordenação, remoção, detecção de conflitos) e adicionar acessórios de regressão para reprodução cross-lane.
- Agregar ganchos de conformidade para listas brancas/listas negras e políticas de dinheiro programáveis ​​(seguido abaixo do NX-12).

---

*Esta página seguirá rastreando follow-ups de NX-1 a medida que aterrissou NX-2 até NX-18. Por favor, exponha perguntas abertas em `roadmap.md` ou no tracker de governança para que o portal siga alinhado com os documentos canônicos.*