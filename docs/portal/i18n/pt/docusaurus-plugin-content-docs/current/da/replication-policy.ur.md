---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ریٹائر ہونے تک دونوں ورژنز کو sincronização رکھیں۔
:::

# Política de replicação de disponibilidade de dados (DA-4)

_حالت: Em andamento - Proprietários: Core Protocol WG / Storage Team / SRE_

Pipeline de ingestão de DA اب `roadmap.md` (workstream DA-4) میں بیان کردہ ہر classe blob
کے لئے metas de retenção determinísticas نافذ کرتا ہے۔ Retenção Torii ایسے
envelopes کو persist کرنے سے انکار کرتا ہے جو chamador فراہم کرے لیکن configurado
política سے correspondência نہ کریں، تاکہ ہر validador/nó de armazenamento مطلوبہ épocas اور
réplicas کی تعداد reter کرے بغیر intenção do remetente پر انحصار کے۔

## Política padrão

| Classe de blob | Retenção quente | Retenção de frio | Réplicas necessárias | Classe de armazenamento | Etiqueta de governança |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Padrão (todas as outras classes)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں incorporado ہیں اور تمام
Submissões `/v2/da/ingest` پر لاگو ہوتی ہیں۔ Perfil de retenção imposta Torii
کے ساتھ manifestos دوبارہ لکھتا ہے اور جب valores incompatíveis dos chamadores فراہم کرتے ہیں
تو warning دیتا ہے تاکہ operadores SDKs obsoletos پکڑ سکیں۔

### Aulas de disponibilidade de Taikai

Manifestos de roteamento Taikai (`taikai.trm`) ایک `availability_class` (`hot`, `warm`,
یا `cold`) declara کرتے ہیں۔ Torii chunking سے پہلے política de correspondência نافذ کرتا
ہے تاکہ edição de tabela global de operadores کئے بغیر fluxo کے لحاظ سے contagens de réplicas
escala کر سکیں۔ Padrões:

| Classe de disponibilidade | Retenção quente | Retenção de frio | Réplicas necessárias | Classe de armazenamento | Etiqueta de governança |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Dicas ausentes padrão طور پر `hot` رکھتے ہیں تاکہ transmissões ao vivo سب سے مضبوط
política reter کریں۔ اگر آپ کا rede مختلف alvos استعمال کرتا ہے تو
`torii.da_ingest.replication_policy.taikai_availability` کے ذریعے padrões
substituir کریں۔

## Configuração

یہ política `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک *padrão*
template کے ساتھ substituições por classe کا array فراہم کرتی ہے۔ Identificadores de classe
ہیں اور `taikai_segment`, `nexus_lane_sidecar`, que não diferencia maiúsculas de minúsculas,
`governance_artifact`, ou `custom:<u16>` (extensões aprovadas pela governança)
کرتے ہیں۔ Classes de armazenamento `hot`, `warm`, یا `cold` قبول کرتی ہیں۔

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Padrões کے ساتھ چلنے کے لئے bloco کو ویسا ہی رہنے دیں۔ کسی classe کو apertar
کرنے کے لئے متعلقہ substituir atualização کریں؛ نئی classes کے linha de base کو بدلنے کے لئے
`default_retention` editar کریں۔

Classes de disponibilidade de Taikai کو الگ سے substituir کیا جا سکتا ہے via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Semântica de aplicação

- Torii fornecido pelo usuário `RetentionPolicy` کو perfil aplicado سے substituir کرتا ہے
  chunking یا emissão de manifesto سے پہلے۔
- Manifestos pré-construídos e declaração de perfil de retenção de incompatibilidade کریں, `400 schema mismatch`
  کے ساتھ rejeitar ہوتے ہیں تاکہ contrato de cliente obsoleto کو enfraquecer نہ کر سکیں۔
- ہر substituir log de eventos ہوتا ہے (`blob_class`, política enviada versus política esperada)
  Implementação de تاکہ کے دوران chamadores não compatíveis سامنے آئیں۔

Gate atualizado کے لئے [Plano de ingestão de disponibilidade de dados](ingest-plan.md)
(Lista de verificação de validação) دیکھیں جو aplicação de retenção کو cobertura کرتا ہے۔

## Fluxo de trabalho de nova replicação (acompanhamento DA-4)

Aplicação de retenção Operadores کو یہ بھی ثابت کرنا ہوگا کہ ao vivo
manifestos e ordens de replicação configuradas política کے مطابق رہیں تاکہ SoraFS
blobs não compatíveis کو خودکار طور پر replicar novamente کر سکے۔

1. **Drift پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` emite
   Valores de retenção obsoletos do chamador بھیجتا ہے۔ اس log کو
   Telemetria `torii_sorafs_replication_*` کے ساتھ جوڑ کر deficiências de réplica یا
   reimplantações atrasadas کو پکڑیں۔
2. **Intent بمقابلہ live replicas کا diff۔** نیا audit helper استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   یہ comando `torii.da_ingest.replication_policy` کو config سے لوڈ کرتا ہے، ہر
   manifesto (JSON یا Norito) decodificar کرتا ہے، اور اختیاری طور پر `ReplicationOrderV1`
   cargas úteis کو resumo do manifesto سے correspondência کرتا ہے۔ Aqui está o sinalizador de condições کرتا ہے:- `policy_mismatch` - política aplicada de perfil de retenção de manifesto سے مختلف ہے
     (یہ Torii configuração incorreta کے بغیر نہیں ہونا چاہئے).
   - `replica_shortfall` - pedido de replicação ao vivo `RetentionPolicy.required_replicas`
     سے کم réplicas طلب کرتا ہے یا alvo سے کم atribuições دیتا ہے۔

   Status de saída diferente de zero فعال déficit کی نشاندہی کرتا ہے تاکہ CI/de plantão
   página de automação فوراً کر سکے۔ JSON رپورٹ کو
   Pacote `docs/examples/da_manifest_review_template.md` کے ساتھ anexar کریں
   تاکہ O Parlamento vota کے لئے دستیاب ہو۔
3. **Acionador de nova replicação کریں۔** جب deficiência de auditoria رپورٹ کرے، ایک نیا
   `ReplicationOrderV1` جاری کریں por meio de ferramentas de governança
   [Mercado de capacidade de armazenamento SoraFS](../sorafs/storage-capacity-marketplace.md)
   میں بیان ہے، اور auditoria دوبارہ چلائیں جب تک replica set converge نہ کرے۔
   Substituições de emergência کے لئے saída CLI کو `iroha app da prove-availability` کے
   ساتھ جوڑیں تاکہ SREs وہی resumo اور Evidência PDP consulte کر سکیں۔

Cobertura de regressão `integration_tests/tests/da/replication_policy.rs` میں ہے؛
suíte `/v2/da/ingest` کو política de retenção incompatível بھیجتی ہے اور verificar کرتی ہے
کہ intenção do chamador do manifesto buscado کے بجائے exposição de perfil imposta کرے۔