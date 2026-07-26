---
lang: mn
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
:::

SNNet-16G нь SoraNet тээвэрт зориулсан квантын дараах хувилбарыг дуусгаж байна. `rollout_phase` товчлуурууд нь операторуудад одоо байгаа А шатлалын хамгаалалтын шаардлагаас B шатлалын дийлэнх хамрах хүрээ болон C шатлалын хатуу PQ байрлал хүртэлх тодорхой дэвшлийг бүх гадаргуу дээр түүхий JSON/TOML засварлахгүйгээр зохицуулах боломжийг олгодог.

Энэхүү тоглоомын ном нь:

- Фазын тодорхойлолт ба шинэ тохиргооны товчлуурууд (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) кодын санд (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`) холбогдсон.
- SDK болон CLI тугны зураглалтай тул үйлчлүүлэгч бүр нэвтрүүлэлтийг хянах боломжтой.
- Буухиа/үйлчлүүлэгчийн канарын хуваарийн хүлээлт, дээр нь сурталчилгаанд хүргэдэг засаглалын хяналтын самбарууд (`dashboards/grafana/soranet_pq_ratchet.json`).
- Буцах дэгээ ба галын өрөмдлөгийн дэвтэрийн лавлагаа ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Үе шатын зураг

| `rollout_phase` | Үр дүнтэй нэрээ нууцлах үе шат | Өгөгдмөл нөлөө | Ердийн хэрэглээ |
|-----------------|--------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (A шат) | Флот дулаарч байх үед хэлхээ бүрт дор хаяж нэг PQ хамгаалагч шаардлагатай. | Суурь ба эхэн үеийн канарын долоо хоногууд. |
| `ramp` | `anon-majority-pq` (B шат) | >= хамрах хүрээний гуравны хоёрын хувьд PQ реле рүү хазайсан сонголт; Сонгодог реле нь нөөц бололцоо хэвээр байна. | Бүс нутгийн буухиа канар; SDK урьдчилан харахыг асаадаг. |
| `default` | `anon-strict-pq` (С шат) | Зөвхөн PQ-д хамаарах хэлхээг мөрдүүлж, зэрэглэл буурах дохиоллыг чангатгаарай. | Телеметр болон засаглалд гарын үсэг зурж дууссаны дараа ахиулна. |

Хэрэв гадаргуу нь мөн тодорхой `anonymity_policy`-г тохируулсан бол тухайн бүрэлдэхүүн хэсгийн үе шатыг дарна. Тодорхой үе шатыг орхигдуулсан нь одоо `rollout_phase` утгыг өөрчилсөн тул операторууд орчин бүрд фазыг нэг удаа эргүүлж, үйлчлүүлэгчдэд үүнийг өвлүүлэх боломжтой болно.

## Тохируулгын лавлагаа

### Оркестр (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Оркестрийн дуудагч нь ажиллах үед (`crates/sorafs_orchestrator/src/lib.rs:2229`) буцаах үе шатыг шийдэж, `sorafs_orchestrator_policy_events_total` болон `sorafs_orchestrator_pq_ratio_*`-ээр дамжуулдаг. Хэрэглэхэд бэлэн хэсгүүдийг `docs/examples/sorafs_rollout_stage_b.toml` болон `docs/examples/sorafs_rollout_stage_c.toml`-с харна уу.

### Rust клиент / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` одоо задалсан үе шатыг (`crates/iroha/src/client.rs:2315`) бүртгэдэг тул туслах командууд (жишээ нь `iroha_cli app sorafs fetch`) үндсэн нэрээ нууцлах бодлогын зэрэгцээ одоогийн үе шатыг мэдээлэх боломжтой.

## Автоматжуулалт

Хоёр `cargo xtask` туслагч нь хуваарь үүсгэх болон олдвор авах ажиллагааг автоматжуулдаг.

1. **Бүс нутгийн хуваарийг гаргах**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Хугацаа нь `s`, `m`, `h`, эсвэл `d` дагаваруудыг хүлээн зөвшөөрдөг. Тус тушаал нь `artifacts/soranet_pq_rollout_plan.json` болон Markdown хураангуй (`artifacts/soranet_pq_rollout_plan.md`) -ийг ялгаруулж, өөрчлөх хүсэлтийн хамт илгээгдэж болно.

2. ** Өрөмдлөгийн олдворуудыг гарын үсгээр нь авах**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Тушаал нь нийлүүлсэн файлуудыг `artifacts/soranet_pq_rollout/<timestamp>_<label>/` руу хуулж, олдвор тус бүрээр BLAKE3 дижестийг тооцоолж, мета өгөгдлийг агуулсан `rollout_capture.json` болон ачааллын дээр Ed25519 гарын үсгийг бичдэг. Галын сургуулилтын тэмдэглэлд гарын үсэг зурдаг ижил нууц түлхүүрийг ашигла, ингэснээр засаг захиргаа зураг авалтыг хурдан баталгаажуулна.

## SDK & CLI тугны матриц

| Гадаргуу | Канар (А шат) | Налуу зам (Б шат) | Өгөгдмөл (С шат) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` авах | `--anonymity-policy stage-a` эсвэл үе шат дээр тулгуурлана | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Оркестрийн тохиргоо JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust клиентийн тохиргоо (`iroha.toml`) | `rollout_phase = "canary"` (анхдагч) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` гарын үсэг зурсан командууд | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, сонголтоор `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, сонголтоор `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, сонголтоор `.ANON_STRICT_PQ` |
| JavaScript оркестрын туслахууд | `rolloutPhase: "canary"` эсвэл `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Бүх SDK нь найруулагчийн ашигладаг ижил үе шат задлагч руу шилжүүлдэг (`crates/sorafs_orchestrator/src/lib.rs:365`), тиймээс холимог хэл дээрх суулгацууд тохируулсан үе шаттай хамт цоожтой хэвээр үлдэнэ.

## Канарын хуваарийг шалгах хуудас

1. **Урьдчилсан нислэг (T хасах 2 долоо хоног)**

- А үе шатыг өмнөх 14 долоо хоногоос 1% -иас доош унасан ба PQ хамрах хүрээ бүс бүрт >=70% байгааг баталгаажуул (`sorafs_orchestrator_pq_candidate_ratio`).
   - Канарын цонхыг батлах засаглалын тоймыг төлөвлөх.
   - Тайзны `sorafs.gateway.rollout_phase = "ramp"`-г шинэчилж (оркестраторын JSON-г засварлаж, дахин байршуулах) болон сурталчилгааны шугамыг хуурай ажиллуул.

2. **Буухиа канар (T өдөр)**

   - Оркестр болон оролцож буй релей манифест дээр `rollout_phase = "ramp"` тохиргоог хийж нэг нэг бүсийг сурталчлаарай.
   - PQ Ratchet хяналтын самбар дээрх "Бодлогын үйл явдлуудын үр дүн" болон "Браунтын хурд"-ыг (одоо танилцуулах самбартай) TTL хамгаалалтын кэшээс хоёр дахин их хэмжээгээр хяна.
   - Аудит хадгалахын тулд ажиллуулахын өмнө болон дараа нь `sorafs_cli guard-directory fetch` агшин зуурын зургийг хайчилж ав.

3. **Клиент/SDK канар (T нэмэх 1 долоо хоног)**

   - Үйлчлүүлэгчийн тохиргоонд `rollout_phase = "ramp"`-г эргүүлэх эсвэл SDK когортуудад зориулсан `stage-b`-г хүчингүй болгох.
   - Телеметрийн ялгааг (`sorafs_orchestrator_policy_events_total` `client_id` болон `region`-р бүлэглэсэн) авч, тэдгээрийг танилцуулах ослын бүртгэлд хавсаргана.

4. **Өгөгдмөл урамшуулал (T нэмэх 3 долоо хоног)**

   - Засаглалыг унтраасны дараа найруулагч болон үйлчлүүлэгчийн тохиргоог `rollout_phase = "default"` руу шилжүүлж, гарын үсэг зурсан бэлэн байдлын хяналтын хуудсыг хувилбарын олдворууд руу эргүүлнэ үү.

## Засаглал ба нотлох баримтыг шалгах хуудас

| Фазын өөрчлөлт | Сурталчилгааны хаалга | Нотлох баримтын багц | Хяналтын самбар ба анхааруулга |
|-------------|----------------|-----------------|--------------------|
| Канар → Налуу зам *(B шатыг урьдчилан харах)* | Үе шат-А сүүлийн 14 хоногийн борлуулалтын хувь <1%, дэвшсэн бүс бүрт `sorafs_orchestrator_pq_candidate_ratio` ≥ 0.7, Argon2 тасалбар нь p95 < 50 мс-ийг баталгаажуулж, урамшууллын засаглалын зай захиалагдсан. | `cargo xtask soranet-rollout-plan` JSON/Markdown хос, хосолсон `sorafs_cli guard-directory fetch` агшин зуурын зураг (өмнө/дараа), гарын үсэг зурсан `cargo xtask soranet-rollout-capture --label canary` багц, [PQ ratchet runbook](I18005000000)-д хамаарах канарын минут. | `dashboards/grafana/soranet_pq_ratchet.json` (Бодлогын үйл явдлууд + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 буурах харьцаа), `docs/source/soranet/snnet16_telemetry_plan.md` дахь телеметрийн лавлагаа. |
| Налуу → Өгөгдмөл *(С шатлалын хэрэгжилт)* | 30 хоногийн SN16 телеметрийн шаталт таарч, `sn16_handshake_downgrade_total` үндсэн шугам дээр, `sorafs_orchestrator_brownouts_total` үйлчлүүлэгчийн канарийн үед тэг болж, прокси сэлгэх бэлтгэлийг бүртгэсэн. | `sorafs_cli proxy set-mode --mode gateway|direct` хуулбар, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` гаралт, `sorafs_cli guard-directory verify` бүртгэл, гарын үсэгтэй `cargo xtask soranet-rollout-capture --label default` багц. | `docs/source/sorafs_orchestrator_rollout.md` болон `dashboards/grafana/soranet_privacy_metrics.json`-д баримтжуулсан ижил PQ Ratchet самбар дээр нэмээд SN16 бууруулах хавтангууд. |
| Яаралтай байдлын зэрэглэлийг бууруулах / буцаах бэлэн байдал | Чанарын бууралтын тоолуур огцом өсөх, хамгаалалтын лавлах баталгаажуулалт амжилтгүй болох, эсвэл `/policy/proxy-toggle` буфер нь доошилсон үйл явдлуудыг бүртгэх үед идэвхждэг. | `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` бүртгэл, `cargo xtask soranet-rollout-capture --label rollback`, ослын тасалбар болон мэдэгдлийн загваруудын хяналтын хуудас. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` болон дохиоллын багц (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Олдвор бүрийг `artifacts/soranet_pq_rollout/<timestamp>_<label>/` дор үүсгэсэн `rollout_capture.json`-тэй хамт хадгалаарай, ингэснээр удирдлагын багцад онооны самбар, сурталчилгааны хэрэглүүрийн ул мөр, дижест агуулагдана.
- Байршуулсан нотлох баримтын SHA256 хураангуйг (минут PDF, зураг авалтын багц, хамгаалалтын агшин агшин) сурталчилгааны протоколд хавсаргаснаар парламентын зөвшөөрлийг үе шатлалын кластерт хандахгүйгээр дахин тоглуулах боломжтой.
- `docs/source/soranet/snnet16_telemetry_plan.md` нь үгсийн сан болон сэрэмжлүүлгийн босго бууруулах каноник эх сурвалж хэвээр байгааг батлахын тулд урамшууллын тасалбар дахь телеметрийн төлөвлөгөөг лавлана уу.

## Хяналтын самбар ба телеметрийн шинэчлэлтүүд

`dashboards/grafana/soranet_pq_ratchet.json` нь одоо энэ тоглоомын дэвтэртэй холбогдож, одоогийн үе шатыг харуулсан "Өөрчлөх төлөвлөгөө" гэсэн тэмдэглэгээний самбартай хамт ирдэг тул засаглалын тойм нь аль үе шат идэвхтэй байгааг батлах боломжтой. Самбарын тайлбарыг тохиргооны бариулд хийх ирээдүйн өөрчлөлтүүдтэй синхрончлоорой.

Анхааруулахын тулд одоо байгаа дүрмүүд нь `stage` шошгыг ашигласнаар канар болон өгөгдмөл үе шатууд тусдаа бодлогын босго (`dashboards/alerts/soranet_handshake_rules.yml`) өдөөгдөнө.

## Буцах дэгээ

### Өгөгдмөл → Налуу зам (С шат → Б шат)

1. Оркестраторын зэрэглэлийг `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp`-ээр бууруулж (мөн SDK тохиргоонд ижил үе шатыг тусгаснаар) В шатыг флотын хэмжээнд үргэлжлүүлнэ.
2. Үйлчлүүлэгчдийг `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`-ээр дамжуулан аюулгүй тээвэрлэлтийн профайлд оруулах, хуулбарыг авах, ингэснээр `/policy/proxy-toggle` засварын ажлын урсгалыг шалгах боломжтой хэвээр үлдээнэ үү.
3. `artifacts/soranet_pq_rollout/` доор харуулын лавлах ялгаа, promtool гаралт, хяналтын самбарын дэлгэцийн агшинг архивлахын тулд `cargo xtask soranet-rollout-capture --label rollback-default`-г ажиллуул.

### Налуу зам → Канар (Б шат → А шат)

1. `sorafs_cli guard-directory import --guard-directory guards.json`-ээр дэвшихээс өмнө авсан хамгаалалтын лавлах агшин зуурын агшинг импортлож, `sorafs_cli guard-directory verify`-г дахин ажиллуулснаар буулгах багцад хэш багтана.
2. Оркестр болон үйлчлүүлэгчийн тохиргоон дээр `rollout_phase = "canary"` (эсвэл `anonymity_policy stage-a`-ээр дарах) тохируулаад, дараа нь [PQ ratchet runbook](./pq-ratchet-runbook.md)-аас PQ ратчет өрөмдлөгийг дахин тоглуулж, бууралтын шугамыг баталгаажуулна уу.
3. Засаглалд мэдэгдэхээсээ өмнө шинэчлэгдсэн PQ Ratchet болон SN16 телеметрийн дэлгэцийн агшин болон сэрэмжлүүлгийн үр дүнг ослын бүртгэлд хавсаргана уу.

### Хамгаалалтын хашлага сануулга- Буурах бүрд `docs/source/ops/soranet_transport_rollback.md` лавлагаа авч, дараагийн ажилд зориулж түр зуурын бууралтыг `TODO:` зүйл болгон бүртгэлд бүртгүүлнэ үү.
- `dashboards/alerts/soranet_handshake_rules.yml` болон `dashboards/alerts/soranet_privacy_rules.yml`-г буцаахаас өмнө болон дараа нь `promtool test rules` хамрах хүрээний дор байлгаж, дохионы шилжилт хөдөлгөөнийг зураг авалтын багцын хажууд баримтжуулна.