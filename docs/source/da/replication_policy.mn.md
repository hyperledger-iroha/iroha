---
lang: mn
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Өгөгдлийн олдоц хуулбарлах бодлого (DA-4)

_Төлөв: Ажиллаж байна — Эзэмшигчид: Үндсэн протоколын ажлын хэсэг / Хадгалах баг / SRE_

DA залгих дамжуулах хоолой нь одоо тодорхой хадгалах зорилтуудыг хэрэгжүүлж байна
`roadmap.md` (ажлын урсгал DA-4) -д тодорхойлсон blob анги бүр. Torii татгалзаж байна
Тохируулсантай таарахгүй байгаа дуудлага хийгчийн өгсөн хадгалах дугтуйг хадгалах
Баталгаажуулагч/хадгалах зангилаа бүр шаардлагатайг хадгалж үлдэхийг баталгаажуулсан бодлого
илгээгчийн хүсэлд найдахгүйгээр эрин үе болон хуулбарын тоо.

## Өгөгдмөл бодлого

| Blob анги | Халуун хадгалах | Хүйтэн хадгалах | Шаардлагатай хуулбар | Хадгалах анги | Засаглалын шошго |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 цаг | 14 хоног | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 цаг | 7 хоног | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 цаг | 180 хоног | 3 | `cold` | `da.governance` |
| _Өгөгдмөл (бусад бүх ангиуд)_ | 6 цаг | 30 хоног | 3 | `warm` | `da.default` |

Эдгээр утгыг `torii.da_ingest.replication_policy`-д суулгаж, ашигласан болно
бүх `/v1/da/ingest` мэдүүлэг. Torii нь манифестуудыг хэрэгжүүлэхтэй хамт дахин бичдэг
хадгалах профайл ба дуудлага хийгчид таарахгүй утгыг өгөх үед анхааруулга гаргадаг
операторууд хуучирсан SDK-г илрүүлэх боломжтой.

### Тайкай ашиглах боломжтой ангиуд

Taikai чиглүүлэлтийн манифестууд (`taikai.trm` мета өгөгдөл) одоо
`availability_class` зөвлөмж (`Hot`, `Warm`, эсвэл `Cold`). Байгаа үед Torii
`torii.da_ingest.replication_policy`-аас тохирох хадгалах профайлыг сонгоно
ачааллыг хуваахаас өмнө үйл явдлын операторуудад идэвхгүй байдлыг бууруулах боломжийг олгоно
дэлхийн бодлогын хүснэгтийг засварлахгүйгээр орчуулга. Өгөгдмөл нь:

| Боломжийн ангилал | Халуун хадгалах | Хүйтэн хадгалах | Шаардлагатай хуулбар | Хадгалах анги | Засаглалын шошго |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 цаг | 14 хоног | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 цаг | 30 хоног | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 цаг | 180 хоног | 3 | `cold` | `da.taikai.archive` |

Хэрэв манифест нь `availability_class`-г орхигдуулсан бол залгих зам буцаад
`hot` профайл нь шууд дамжуулалтууд нь бүрэн хуулбарыг нь хадгалдаг. Операторууд чадна
шинийг засварлах замаар эдгээр утгыг дарна уу
`torii.da_ingest.replication_policy.taikai_availability` блок тохиргоонд байна.

## Тохиргоо

Бодлого нь `torii.da_ingest.replication_policy`-ийн дагуу ажилладаг бөгөөд a
*өгөгдмөл* загвар дээр нэмээд анги тус бүрийн дарангуйллын массив. Анги танигч нь
том жижиг үсэг үл хамаарах ба `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, эсвэл `custom:<u16>` засаглалаар батлагдсан өргөтгөлүүдийн хувьд.
Хадгалах ангиуд нь `hot`, `warm`, эсвэл `cold` зэргийг хүлээн зөвшөөрдөг.

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

Дээр дурдсан өгөгдмөл тохиргоотой ажиллахын тулд блокыг хөндөөгүй орхи. чангалах а
анги, тохирох дарж бичихийг шинэчлэх; шинэ ангиудын суурь үзүүлэлтийг өөрчлөх,
`default_retention` засварлах.Тайкай ашиглах боломжтой тодорхой ангиудыг тохируулахын тулд доор оруулгуудыг нэмнэ үү
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Хэрэгжүүлэх семантик

- Torii нь хэрэглэгчийн нийлүүлсэн `RetentionPolicy`-г албадан профайлаар сольсон.
  бөөгнөрөл эсвэл ил тод ялгаралтын өмнө.
- Тохиромжгүй хадгалалтын профайлыг зарласан, урьдчилан бэлтгэсэн манифестуудыг үгүйсгэдэг
  `400 schema mismatch`-тэй тул хуучирсан үйлчлүүлэгчид гэрээг сулруулж чадахгүй.
- Дарсан үйл явдал бүрийг бүртгэсэн (`blob_class`, хүлээгдэж буй бодлогын эсрэг илгээсэн)
  нэвтрүүлэх явцад шаардлага хангаагүй дуудагчийг илрүүлэх.

Шинэчлэгдсэн хаалгыг `docs/source/da/ingest_plan.md` (Баталгаажуулалтын хяналтын хуудас) -аас үзнэ үү.
хадгалалтын хэрэгжилтийг хамарсан.

## Дахин хуулбарлах ажлын урсгал (DA-4 дагах)

Хадгалах нь зөвхөн эхний алхам юм. Үүнийг операторууд ч батлах ёстой
шууд манифест болон хуулбарлах дараалал нь тохируулсан бодлоготой нийцэж байх болно
SoraFS стандартын шаардлага хангаагүй бөмбөлгийг автоматаар дахин хуулбарлах боломжтой.

1. **Дрифтийг ажиглаарай.** Torii ялгаруулдаг
   `overriding DA retention policy to match configured network baseline`
   дуудлага хийгч нь хуучирсан хадгалалтын утгыг илгээдэг. Тэр бүртгэлтэй хослуул
   `torii_sorafs_replication_*` телеметрийн хуулбарын дутагдал эсвэл саатлыг илрүүлэх
   дахин байршуулалт.
2. **Ялгаатай зорилго ба шууд хуулбар.** Аудитын шинэ туслахыг ашиглана уу:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Энэ тушаал нь өгсөнөөс `torii.da_ingest.replication_policy`-г ачаална
   config, манифест бүрийн кодыг тайлж (JSON эсвэл Norito), сонголтоор дурын
   `ReplicationOrderV1` манифест дижестээр ачааллыг тооцдог. Дүгнэлт нь хоёрыг тэмдэглэнэ
   нөхцөл:

   - `policy_mismatch` – ил тод хадгалалтын профайл нь хэрэгжсэнээс ялгаатай
     бодлого (Torii буруу тохируулаагүй бол энэ нь хэзээ ч болохгүй).
   - `replica_shortfall` – шууд хуулбарлах захиалга нь түүнээс цөөн хуулбарыг шаарддаг.
     `RetentionPolicy.required_replicas` буюу түүнээс цөөн даалгавар өгдөг
     зорилтот.

   Гаралтын төлөв нь тэг биш байгаа нь идэвхтэй дутагдлыг илтгэдэг тул CI/дуудлагын автоматжуулалт
   нэн даруй хуудас хийж болно. JSON тайланг хавсаргана уу
   Парламентын санал хураалтад зориулсан `docs/examples/da_manifest_review_template.md` багц.
3. **Дахин хуулбарлах үйлдлийг өдөөх.** Аудит дутагдлын талаар мэдээлэх үед шинэ мэдэгдэл гарга.
   `ReplicationOrderV1` -д тайлбарласан засаглалын хэрэгслээр дамжуулан
   `docs/source/sorafs/storage_capacity_marketplace.md` ба аудитыг дахин явуулна уу
   хуулбарын багц нэгдэх хүртэл. Онцгой байдлын үед хүчингүй болгохын тулд CLI гаралтыг холбоно уу
   `iroha app da prove-availability`-тэй, ингэснээр SRE-ууд нь ижил хайлтыг лавлах боломжтой
   болон PDP нотлох баримт.

Регрессийн хамрах хүрээ нь `integration_tests/tests/da/replication_policy.rs`;
иж бүрдэл нь `/v1/da/ingest`-д тохирохгүй хадгалах бодлогыг илгээж, баталгаажуулдаг
татаж авсан манифест нь дуудлага хийгчийн оронд албадуулсан профайлыг ил гаргадаг
зорилго.

## Эрүүл мэндийн телеметр ба хяналтын самбар (DA-5 гүүр)

Замын зургийн **DA-5** зүйлд PDP/PoTR хэрэгжилтийн үр дүнг аудит хийх боломжтой байхыг шаарддаг.
бодит цаг. `SorafsProofHealthAlert` үйл явдлууд одоо зориулалтын багцыг жолооддог
Prometheus хэмжүүр:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Health** Grafana самбар
(`dashboards/grafana/sorafs_pdp_potr_health.json`) одоо эдгээр дохиог харуулж байна:- *Триггерээр баталгаажсан эрүүл мэндийн сэрэмжлүүлэг* дохиоллын хувь хэмжээг гох/торгуулийн тугаар графикаар харуулдаг.
  Taikai/CDN операторууд зөвхөн PDP, зөвхөн PoTR эсвэл давхар анхааруулга эсэхийг нотолж чадна.
  буудах.
- *Cooldown дахь үйлчилгээ үзүүлэгчид* нь одоогийн байдлаар үйлчилгээ үзүүлэгчдийн нийлбэрийг мэдээлдэг
  SorafsProofHealthAlert хөргөх хугацаа.
- *Proof Health Window Snapshot* нь PDP/PoTR тоолуур, торгуулийн хэмжээ,
  үйлчилгээ үзүүлэгч тус бүрээр хөргөлтийн дарцаг, цохилтын цонхны төгсгөлийн эрин үе байх тул засаглалын тоймчид
  ослын пакетуудад хүснэгтийг хавсаргаж болно.

Runbook нь DA-ийн хэрэгжилтийн нотлох баримтыг танилцуулахдаа эдгээр самбарыг холбох ёстой; тэд
CLI proof-stream алдааг гинжин торгуулийн мета өгөгдөлтэй шууд холбох ба
замын зурагт заасан ажиглалтын дэгээгээр хангана.