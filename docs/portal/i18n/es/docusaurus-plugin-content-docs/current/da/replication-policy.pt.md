---
lang: es
direction: ltr
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
España `docs/source/da/replication_policy.md`. Mantenha como dos versos em
:::

# Política de replicación de Disponibilidad de Datos (DA-4)

_Estado: En progreso - Responsaveis: Core Protocol WG / Equipo de almacenamiento / SRE_

O pipeline de ingest DA agora aplica metas determinísticas de retencao para cada
clase de blob descrita en `roadmap.md` (flujo de trabajo DA-4). Torii persistente persistente
sobres de retencao fornecidos pelo caller que nao correspondem a politica
configurada, garantizando que cada nodo validador/armazenamento retenha o numero
requerido de épocas y réplicas sin depender de la intención del emisor.

## Política padrao

| Clase de blob | Retençao caliente | Retençao frío | Réplicas requeridas | Clase de armazenamento | Etiqueta de gobierno |
|---------------|--------------|---------------|---------------------|-------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (todas como clases anteriores)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |Esses valores sao embutidos em `torii.da_ingest.replication_policy` e aplicados
a todas las presentaciones `/v2/da/ingest`. Torii reescreve manifiesta con el perfil
de retencao imposto e emite una advertencia cuando las personas que llaman necesitan valores divergentes
para que los operadores detectem SDKs desatualizados.

### Clases de disponibilidad Taikai

Manifiestos de roteamento Taikai (`taikai.trm`) declaram `availability_class`
(`hot`, `warm`, o `cold`). Torii aplica a política corresponsal antes de hacerlo
fragmentación para que los operadores puedan escalar contagios de réplicas por stream sem
editar una tabla global. Valores predeterminados:

| Clase de disponibilidad | Retençao caliente | Retençao frío | Réplicas requeridas | Clase de armazenamento | Etiqueta de gobierno |
|---------------------|--------------|---------------|---------------------|-------------------------|-------------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Hints ausentes usam `hot` por padrao para que transmissoes ao vivo retenham a
política más fuerte. Sustituir los valores predeterminados a través de
`torii.da_ingest.replication_policy.taikai_availability` se sua rede usar
también diferentes.

## ConfiguracionA politica vive sollozo `torii.da_ingest.replication_policy` e expoe um template
*predeterminado* más una matriz de anulaciones por clase. Identificadores de clase nao
diferenciam maiusculas/minusculas e aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, o `custom:<u16>` para extensos aprobados por el gobierno.
Clases de armazenamento aceitam `hot`, `warm`, o `cold`.

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

Deixe o bloco intacto para rodar com os defaults acima. Para soportar uma
classe, actualizar o anular correspondiente; para mudar a base de nuevas clases,
editar `default_retention`.

Classes de disponibilidade Taikai podem ser sobrescritas de forma independiente
vía `torii.da_ingest.replication_policy.taikai_availability`:

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

## Semántica de cumplimiento

- Torii sustituto o `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  antes de fragmentar o emitir un manifiesto.
- Manifiestos preconstruidos que declaram um perfil de retencao divergente sao
  rejeitados com `400 schema mismatch` para que clientes obsoletos nao possam
  enfraquecer o contrato.
- Cada evento de override e logado (`blob_class`, politica enviada vs esperada)
  para exportar llamantes nao conforme durante el lanzamiento.

Veja [Plan de ingesta de disponibilidad de datos](ingest-plan.md) (Lista de verificación de validación) para
o puerta atualizado cobrindo cumplimiento de retencao.

## Flujo de trabajo de re-replicación (seguimiento DA-4)Oforcement de retencao e apenas o primeiro passo. Operadores tambem devem
provar que manifests live e órdenes de replicacao permanecem alinhados a politica
configurado para que SoraFS pueda volver a replicar blobs para cumplir con el formato
automática.

1. **Observar deriva.** Torii emite
   `overriding DA retention policy to match configured network baseline` cuando
   um caller submete valores de retencao desatualizados. Combinar esse log con a
   telemetria `torii_sorafs_replication_*` para detectar falta de replicas o
   Se redespliega atrasados.
2. **Diferencia entre intención y réplicas en vivo.** Utilice el nuevo asistente de auditoría:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El comando encargado `torii.da_ingest.replication_policy` de configuración
   fornecida, decodifica cada manifiesto (JSON o Norito), y opcionalmente haz match
   las cargas útiles `ReplicationOrderV1` por resumen del manifiesto. O resumen sinaliza dos
   condicos:

   - `policy_mismatch` - o perfil de retencao do manifest diverge da politica
     imposta (isto nao deveria ocorrer a menos que Torii esteja mal configurado).
   - `replica_shortfall` - a ordem de replicacao live solicita menos replicas do
     que `RetentionPolicy.required_replicas` o fornece menos atribuicos do que
     o alvo.Um status de saya nao zero indica um shortfall ativo para que a automacao de
   CI/on-call puede paginar inmediatamente. Anexo del relato JSON del paquete
   `docs/examples/da_manifest_review_template.md` para votos del Parlamento.
3. **Dispare re-replicacao.** Cuando a auditoria reportar um deficit, emita um
   novo `ReplicationOrderV1` vía como herramientas de gobierno descritas en
   [Mercado de capacidad de almacenamiento SoraFS](../sorafs/storage-capacity-marketplace.md)
   Monté en un auditorio novamente ate que o set de replicas converja. Anulaciones de parámetros
   de emergencia, emparéjelo con dicha CLI con `iroha app da prove-availability` para
   que SRE puede hacer referencia al mesmo resumen y evidencia PDP.

A cobertura de regressao vive em `integration_tests/tests/da/replication_policy.rs`;
a suite envia uma politica de retencao divergente para `/v2/da/ingest` e verifica
que o manifiesto buscado expoe o perfil imposto em vez da intencao do llamante.