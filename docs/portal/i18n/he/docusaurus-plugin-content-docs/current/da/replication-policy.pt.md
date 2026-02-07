---
lang: he
direction: rtl
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Espelha `docs/source/da/replication_policy.md`. Mantenha as duas versoes em
:::

# Politica de replicacao de Data Availability (DA-4)

_סטטוס: Em progresso -- תשובות: Core Protocol WG / Storage Team / SRE_

O pipeline de ingest DA agora aplica metas deterministicas de retencao para cada
classe de blob decrita em `roadmap.md` (זרם עבודה DA-4). Torii recusa persistir
מעטפות דה retencao fornecidos pelo מתקשר que nao correspondem a politica
configurada, garantindo que cada node validador/armazenamento retenha o numero
requerido de epocas e replicas sem depender da intencao do emissor.

## פוליטיקה פאדראו

| Classe de blob | Retencao חם | Retencao קר | רפליקות requeridas | Classe de armazenamento | Tag de governanca |
|---------------|--------------------------------------|
| `taikai_segment` | 24 שעות | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 הורות | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 הורות | 180 dias | 3 | `cold` | `da.governance` |
| _ברירת מחדל (טודאס כשיעורי demais)_ | 6 הורות | 30 dias | 3 | `warm` | `da.default` |

Esses valores sao embutidos em `torii.da_ingest.replication_policy` e aplicados
a todas כמו הגשות `/v1/da/ingest`. Torii reescreve manifests com o perfil
de retencao imposto e emite um אזהרה quando callers fornecem valores divergentes
עבור מפעילי זיהוי SDKs desatualizados.

### Classes de disponibilidade Taikai

Manifests de roteamento Taikai (`taikai.trm`) הכריז `availability_class`
(`hot`, `warm`, או `cold`). Torii אפליקציית א פוליטיקה קורורנדנטה אנטס לעשות
chunking para que operatores possam escalar contagens de replicas por stream sem
עריכת טבלה גלובלית. ברירות מחדל:

| Classe de disponibilidade | Retencao חם | Retencao קר | רפליקות requeridas | Classe de armazenamento | Tag de governanca |
|----------------------------|----------------|---------------------------------|
| `hot` | 24 שעות | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 הורות | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 הורה | 180 dias | 3 | `cold` | `da.taikai.archive` |

רמזים ausentes usam `hot` por padrao para que transmissoes ao vivo retenham a
פוליטיקה מאי פורטה. Substitua os ברירת המחדל באמצעות
`torii.da_ingest.replication_policy.taikai_availability` הוא משתמש
alvos diferentes.

## Configuracao

A politica vive sob `torii.da_ingest.replication_policy` e expoe um template
*ברירת מחדל* מערך ה-um מבטל את המחלקה. זיהוי מעמד נאו
diferenciam maiusculas/minusculas e aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para extensoes aprovadas por governanca.
Classes de armazenamento aceitam `hot`, `warm`, או `cold`.

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
```Deixe o bloco intacto para Rodar com os ברירת המחדל של acima. Para endurecer uma
classe, לממש או לעקוף correspondente; שיעורי para mudar a base de Novas,
ערוך `default_retention`.

Classes de disponibilidade Taikai podem ser sobrescritas de forma independente
דרך `torii.da_ingest.replication_policy.taikai_availability`:

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

## סמנטיקה דה אכיפה

- Torii substitui o `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  אנטים עושים chunking ou da emisso de manifest.
- Manifests preconstruidos que declaram um perfil de retencao divergente sao
  rejeitados com `400 schema mismatch` עבור לקוחות מיושנים נאו פוסאם
  enfraquecer o contrato.
- Cada evento de override e logado (`blob_class`, politica enviada vs esperada)
  מתקשרי para expor נאו תואמים את משך ההשקה.

Veja [תוכנית הטמעת זמינות נתונים](ingest-plan.md) (רשימת אימות) סעיף
o gate atualizado cobrindo enforcement de retencao.

## זרימת עבודה של רפליקאו (סיגמנטו DA-4)

O אכיפה de retencao e apenas o primeiro passo. Operadores tambem devem
provar que manifests live e ordens de replicacao permanecem alinhados a politica
configurada para que SoraFS possa re-replicar blobs fora de conformidade de forma
אוטומטי.

1. **התבונן בהסחף.** Torii emite
   `overriding DA retention policy to match configured network baseline` quando
   אום המתקשר submete valores de retencao desatualizados. שלב esse log com a
   telemetria `torii_sorafs_replication_*` עבור detectar falta de replicas ou
   פורס מחדש atrasados.
2. **הכוונה שונה לעומת העתקים חיים.** השתמש ב-o novo helper de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   O comando carrega `torii.da_ingest.replication_policy` da configuracao
   fornecida, decodifica cada manifest (JSON ou Norito), ואופציונלי התאמה אישית
   de payloads `ReplicationOrderV1` por digest de manifest. O resumo sinaliza duas
   קונדיקוס:

   - `policy_mismatch` - o perfil de retencao do manifest diverge da politica
     imposta (isto nao deveria ocorrer a menos que Torii esteja mal configurado).
   - `replica_shortfall` - סדרה דה רפליקאו בשידור חי solicita menos העתקים לעשות
     que `RetentionPolicy.required_replicas` ou fornece menos atribuicoes do que
     או אלבו.

   אום סטטוס דה סאדה נאו אפס אינדיקה אום חסרון ativo para que a automacao de
   CI/on-call possa pager immediatamente. תוספת ל-JSON או פאקוטה
   `docs/examples/da_manifest_review_template.md` להצבעות על פרלמנטו.
3. **הסר רפליקאו מחדש.** ראה אודיטוריה דיווחים, מחסור,
   novo `ReplicationOrderV1` via as ferramentas de governanca descritas em
   [שוק קיבולת אחסון SoraFS](../sorafs/storage-capacity-marketplace.md)
   אני רכבתי על אודיטוריה novamente ate que o set de replicas converja. פארה עוקפת
   de emergencia, emparelhe a saida da CLI com `iroha app da prove-availability` para
   que SREs possam referenciar o mesmo digest e Evidencia PDP.

A cobertura de regressao vive em `integration_tests/tests/da/replication_policy.rs`;
a suite envia uma politica de retencao divergente para `/v1/da/ingest` e verifica
que o manifest buscado expoe o perfil imposto em vez da intencao do caller.