---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md)-аас тохируулсан.

# SoraFS Үйлчилгээ үзүүлэгчийн зар сурталчилгааны төлөвлөгөө

Энэ төлөвлөгөө нь зөвшөөрөгдсөн үйлчилгээ үзүүлэгчийн зар сурталчилгааг таслах ажлыг зохицуулдаг
Олон эх сурвалжийн хэсгүүдэд шаардлагатай бүрэн удирдлагатай `ProviderAdvertV1` гадаргуу
олж авах. Энэ нь гурван үр дүнд анхаарлаа хандуулдаг:

- **Операторын гарын авлага.** Хадгалах үйлчилгээ үзүүлэгч нар алхам алхмаар хийх ёстой
  хаалга бүр эргэхээс өмнө.
- **Телеметрийн хамрах хүрээ.** Ажиглах чадвар болон үйлдлийн системд ашигладаг хяналтын самбар болон анхааруулга
  сүлжээг баталгаажуулахын тулд зөвхөн шаардлагад нийцсэн зар сурталчилгааг хүлээн авдаг.
Энэхүү танилцуулга нь [SoraFS шилжих үеийн SF-2b/2c чухал үе шаттай нийцдэг.
замын зураг](./migration-roadmap) бөгөөд элсэлтийн бодлогыг
[үзүүлэгчийн элсэлтийн журам](./provider-admission-policy) аль хэдийн орсон байна
нөлөө.

## Одоогийн шаардлага

SoraFS нь зөвхөн засаглалын дугтуйтай `ProviderAdvertV1` ачааллыг хүлээн авдаг. The
элсэлтийн үед дараах шаардлагыг мөрдөнө.

- `profile_id=sorafs.sf1@1.0.0`, каноник `profile_aliases` байгаа.
- Олон эх сурвалжийн хувьд `chunk_range_fetch` чадварын даацыг оруулах ёстой
  олж авах.
- Зар сурталчилгаанд хавсаргасан зөвлөлийн гарын үсэг бүхий `signature_strict=true`
  дугтуй.
- `allow_unknown_capabilities`-ийг зөвхөн тодорхой ӨСЛӨХ өрөмдлөгийн үед зөвшөөрнө.
  мөн бүртгэлд оруулах ёстой.

## Операторын хяналтын хуудас

1. **Бараа материалын сурталчилгаа.** Нийтлэгдсэн зар бүрийг жагсааж, тэмдэглэнэ үү:
   - Удирдах дугтуйны зам (`defaults/nexus/sorafs_admission/...` эсвэл үйлдвэрлэлийн эквивалент).
   - `profile_id` болон `profile_aliases` зар сурталчилгаа.
   - Чадварын жагсаалт (хамгийн багадаа `torii_gateway` болон `chunk_range_fetch` байх ёстой).
   - `allow_unknown_capabilities` туг (борлуулагчийн нөөцтэй TLV байгаа үед шаардлагатай).
2. **Үйлчилгээ үзүүлэгчийн хэрэгслээр сэргээнэ үү.**
   - Зар сурталчилгааны үйлчилгээ үзүүлэгчтэйгээ хамт ачааллыг дахин бүтээж, дараахь зүйлийг баталгаажуулна уу.
     - `profile_id=sorafs.sf1@1.0.0`
     - Тодорхойлогдсон `max_span`-тай `capability=chunk_range_fetch`
     - GREASE TLV байгаа үед `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` болон `sorafs_fetch`-ээр дамжуулан баталгаажуулах; үл мэдэгдэх тухай анхааруулга
     чадавхийг шалгах ёстой.
3. **Олон эх сурвалжийн бэлэн байдлыг баталгаажуулах.**
   - `sorafs_fetch`-ийг `--provider-advert=<path>` ашиглан гүйцэтгэх; CLI одоо амжилтгүй боллоо
     `chunk_range_fetch` байхгүй үед үл тоомсорлож буй үл мэдэгдэх анхааруулгыг хэвлэдэг
     чадварууд. JSON тайланг авч, үйлдлийн бүртгэлээр архивлана.
4. **Үе шатыг шинэчлэх.**
   - `ProviderAdmissionRenewalV1` дугтуйг 30-аас доошгүй хоногийн өмнө илгээнэ үү
     хугацаа дуусах. Шинэчлэлт нь каноник бариул болон чадварын багцыг хадгалах ёстой;
     зөвхөн гадас, төгсгөлийн цэг эсвэл мета өгөгдөл өөрчлөгдөх ёстой.
5. **Харилцааны багуудтай харилцах.**
   - SDK эзэмшигчид операторуудад анхааруулга өгөх хувилбаруудыг гаргах ёстой
     сурталчилгаа татгалзсан.
   - DevRel нь үе шат болгоны шилжилтийг зарладаг; хяналтын самбарын холбоосууд болон
     босго логик доор байна.
6. **Хяналтын самбар болон анхааруулга суулгах.**
   - Grafana экспортыг оруулж ирээд **SoraFS / Үйлчилгээ үзүүлэгчийн доор байрлуулна уу.
     UID `sorafs-provider-admission` хяналтын самбартай Rollout**.
   - Анхааруулгын дүрмүүд нь хуваалцсан `sorafs-advert-rollout` руу чиглэж байгаа эсэхийг шалгаарай
     үе шат, үйлдвэрлэл дэх мэдэгдлийн суваг.

## Телеметр ба хяналтын самбар

Дараах хэмжүүрүүд `iroha_telemetry`-ээр аль хэдийн илэрсэн байна:

- `torii_sorafs_admission_total{result,reason}` - хүлээн зөвшөөрсөн, татгалзсан тоо,
  болон анхааруулах үр дүн. Шалтгаан нь `missing_envelope`, `unknown_capability`,
  `stale`, `policy_violation`.

Grafana экспорт: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Файлыг хуваалцсан хяналтын самбарын репозитор руу импортлох (`observability/dashboards`)
мөн нийтлэхээс өмнө зөвхөн өгөгдлийн эх сурвалжийн UID-г шинэчилнэ үү.

Удирдах зөвлөл нь Grafana хавтас **SoraFS / Provider Rollout** дор нийтэлдэг.
тогтвортой UID `sorafs-provider-admission`. Анхааруулах дүрэм
`sorafs-admission-warn` (анхааруулга) ба `sorafs-admission-reject` (чухал)
`sorafs-advert-rollout` мэдэгдлийн бодлогыг ашиглахаар урьдчилан тохируулсан; тохируулах
-ийг засварлахын оронд очих жагсаалт өөрчлөгдвөл тэр холбоо барих цэг
JSON хяналтын самбар.

Санал болгож буй Grafana хавтангууд:

| Самбар | Асуулга | Тэмдэглэл |
|-------|-------|-------|
| **Элсэлтийн үр дүнгийн хувь хэмжээ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Зөвшөөрөх, анхааруулах, татгалзах зэргийг дүрслэн харуулах диаграммыг стек. Анхааруулах > 0.05 * нийт (анхааруулах) эсвэл татгалзах > 0 (чухал) үед дохио өгнө. |
| **Анхааруулах харьцаа** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Пэйжерийн босгыг хангадаг нэг мөрт цагийн цуврал (5% анхааруулах хурд 15 минут). |
| **Татгалзах шалтгаан** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Runbook triage-ийг хөтчүүд; бууруулах алхмуудын холбоосыг хавсаргана уу. |
| **Өрийг сэргээх** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Үйлчилгээ үзүүлэгчдийг шинэчлэх хугацааг дутуу байгааг харуулж байна; илрүүлэх кэш логуудтай хөндлөн лавлагаа. |

Гарын авлагын хяналтын самбарт зориулсан CLI олдворууд:

- `sorafs_fetch --provider-metrics-out` `failures`, `successes` бичих ба
  Үйлчилгээ үзүүлэгч бүрт `disabled` тоолуур. Хянахын тулд түр хяналтын самбар руу импорт хийнэ үү
  Үйлдвэрлэгчийг солихын өмнө найруулагч хуурай гүйлт.
- JSON тайлангийн `chunk_retry_rate` болон `provider_failure_rate` талбарууд
  ихэвчлэн элсэлтийн өмнөх ачааллыг бууруулах эсвэл хуучирсан шинж тэмдгийг онцлон тэмдэглэ
  татгалзал.

### Grafana хяналтын самбарын зохион байгуулалт

Observability нь тусгай самбарыг нийтэлдэг — **SoraFS Үйлчилгээ үзүүлэгчийн элсэлт
Rollout** (`sorafs-provider-admission`) — **SoraFS / Provider Rollout**-ийн дагуу
дараах каноник самбар ID-тай:

- 1-р самбар — *Элсэлтийн үр дүнгийн хувь хэмжээ* (овоолсон талбай, нэгж “ops/min”).
- Самбар 2 — *Анхааруулах харьцаа* (нэг цуврал), илэрхийлэл ялгаруулна
  `нийлбэр(хүн(torii_sorafs_admission_total{үр дүн="анхааруулах"}[5м])) /
   нийлбэр(хувь(torii_sorafs_элсэлтийн_нийт[5м]))`.
- Самбар 3 — *Татгалзсан шалтгаан* (цаг хугацааны цуваа `reason`-р бүлэглэсэн), эрэмбэлсэн
  `rate(...[5m])`.
- Самбар 4 — *Өрийг сэргээх* (стат), дээрх хүснэгтэд байгаа асуулга болон
  Шилжилт хөдөлгөөний дэвтэрээс авсан сурталчилгааны шинэчлэлтийн эцсийн хугацааг тэмдэглэсэн.

Дэд бүтцийн хяналтын самбарын репо дахь JSON араг ясыг хуулах (эсвэл үүсгэх).
`observability/dashboards/sorafs_provider_admission.json`, дараа нь зөвхөн шинэчилнэ үү
өгөгдлийн эх сурвалж UID; самбар ID болон дохиоллын дүрмийг runbook-ээс иш татсан болно
доор байгаа тул энэ баримт бичгийг засварлахгүйгээр дугаарлахаас зайлсхий.

Тохиромжтой болгох үүднээс репозитор одоо лавлах самбарын тодорхойлолтыг хаягаар илгээж байна
`docs/source/grafana_sorafs_admission.json`; Хэрэв та үүнийг Grafana хавтсандаа хуулж ав
танд орон нутгийн туршилтын эхлэх цэг хэрэгтэй.

### Prometheus дохиоллын дүрэм

Дараах дүрмийн бүлгийг `observability/prometheus/sorafs_admission.rules.yml` дээр нэмнэ үү
(хэрэв энэ нь SoraFS дүрмийн эхний бүлэг бол файлыг үүсгээд)
таны Prometheus тохиргоо. `<pagerduty>`-г бодит чиглүүлэлтээр солино уу
таны дуудлага дээр сэлгэх шошго.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` ажиллуулна уу
синтакс `promtool check rules` дамждагийг баталгаажуулахын тулд өөрчлөлтийг түлхэхээс өмнө.

## Элсэлтийн үр дүн

- `chunk_range_fetch` чадвар дутуу → `reason="missing_capability"`-ээр татгалз.
- `allow_unknown_capabilities=true`-гүй үл мэдэгдэх чадвартай TLVs → татгалз
  `reason="unknown_capability"`.
- `signature_strict=false` → татгалзах (тусгаарлагдсан оношлогоонд зориулагдсан).
- Хугацаа дууссан `refresh_deadline` → татгалзах.

## Харилцаа холбоо ба ослыг зохицуулах

- **Долоо хоног тутмын статусын шуудан.** DevRel элсэлтийн товч тоймыг тараадаг
  хэмжигдэхүүн, үл мэдэгдэх анхааруулга, удахгүй болох хугацаа.
- **Осол гарсан хариу арга хэмжээ.** Хэрэв `reject` галын дохио өгвөл дуудлагын инженерүүд:
  1. Torii илрүүлэлт (`/v2/sorafs/providers`)-ээр зөрчигдсөн зарыг татаж аваарай.
  2. Үйлчилгээ үзүүлэгчийн шугамд зар сурталчилгааны баталгаажуулалтыг дахин ажиллуулж, харьцуулна уу
     Алдааг дахин гаргахын тулд `/v2/sorafs/providers`.
  3. Зар сурталчилгааг дараагийн шинэчлэлтээс өмнө эргүүлэхийн тулд үйлчилгээ үзүүлэгчтэй хамтран ажиллана уу
     эцсийн хугацаа.
- **Өөрчлөлт хөлддөг.** R1/R2-ын үед ямар ч чадамжийн схем газар өөрчлөхгүй
  сонгон шалгаруулах хороо гарын үсэг зурсан; GREASE-ийн туршилтыг энэ үеэр хийх ёстой
  долоо хоног тутмын засвар үйлчилгээний цонх болон шилжилт хөдөлгөөний дэвтэрт нэвтэрсэн.

## Лавлагаа

- [SoraFS зангилаа/Клиент протокол](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Үйлчилгээ үзүүлэгчийн элсэлтийн бодлого](./provider-admission-policy)
- [Шилжилт хөдөлгөөний замын зураг](./migration-roadmap)
- [Үйлчилгээ үзүүлэгчийн зарын олон эх сурвалжийн өргөтгөлүүд](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)