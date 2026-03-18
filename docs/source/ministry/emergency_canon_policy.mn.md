---
lang: mn
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Онцгой байдлын Canon & TTL бодлого (MINFO-6a)

Замын зургийн лавлагаа: **MINFO-6a — Онцгой байдлын canon & TTL бодлого**.

Энэхүү баримт бичиг нь одоо Torii болон CLI-д нийлүүлэгдэж буй жагсаалтаас хасах дүрэм, TTL хэрэгжилт болон засаглалын үүргийг тодорхойлдог. Операторууд шинэ оруулгуудыг нийтлэх эсвэл яаралтай тусламжийг дуудахын өмнө эдгээр дүрмийг дагаж мөрдөх ёстой.

## Түвшингийн тодорхойлолт

| Түвшин | Өгөгдмөл TTL | Хяналтын цонх | Тавигдах шаардлага |
|------|-------------|---------------|--------------|
| Стандарт | 180 хоног (`torii.sorafs_gateway.denylist.standard_ttl`) | байхгүй | `issued_at` заасан байх ёстой. `expires_at` орхигдсон байж болно; Torii нь анхдагчаар `issued_at + standard_ttl` бөгөөд урт цонхноос татгалздаг. |
| Яаралтай | 30 хоног (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 хоног (`torii.sorafs_gateway.denylist.emergency_review_window`) | Урьдчилан батлагдсан каноныг (жишээ нь, `csam-hotline`) лавласан хоосон биш `emergency_canon` шошго шаардлагатай. `issued_at` + `expires_at` 30 хоногийн цонхонд буух ёстой бөгөөд хянан шалгах нотлох баримт нь автоматаар үүсгэгдсэн эцсийн хугацааг (`issued_at + review_window`) иш татсан байх ёстой. |
| Байнгын | Хугацаа дуусахгүй | байхгүй | Олонхийн засаглалын шийдвэр гаргахад зориулагдсан. Оролтууд нь хоосон биш `governance_reference` (саналын дугаар, тунхаглалын хэш гэх мэт) иш татсан байх ёстой. `expires_at` татгалзсан. |

Анхдагч тохиргоог `torii.sorafs_gateway.denylist.*`-ээр тохируулах боломжтой хэвээр байгаа бөгөөд `iroha_cli` нь Torii файлыг дахин ачаалахаас өмнө хүчингүй оруулгуудыг барих хязгаарлалтыг тусгадаг.

## Ажлын урсгал

1. **Метадата бэлтгэх:**-д `policy_tier`, `issued_at`, `expires_at` (боломжтой бол) болон `emergency_canon`/`governance_reference` (JSON8070 оролт бүр дотор) орно.
2. **Дотоодоор баталгаажуулах:** `iroha app sorafs gateway lint-denylist --path <denylist.json>`-г ажиллуулснаар CLI нь файлыг оруулах эсвэл бэхлэхээс өмнө түвшний тусгай TTL болон шаардлагатай талбаруудыг мөрддөг.
3. **Нотлох баримтыг нийтлэх:** Аудиторууд шийдвэрийг хянах боломжтой болохын тулд GAR хэргийн багцын (хэлэлцэх асуудлын багц, бүх нийтийн санал асуулгын тэмдэглэл гэх мэт) хэсэгт дурдсан канон id буюу засаглалын лавлагааг хавсаргана уу.
4. **Яаралтай тусламжийн бүртгэлийг шалгах:** яаралтай тусламжийн канонуудын хугацаа 30 хоногийн дотор автоматаар дуусна. Операторууд 7 хоногийн цонхны дотор фактын дараах хяналтыг хийж, үр дүнг яамны tracker/SoraFS нотлох баримтын санд бүртгэх ёстой.
5. **Torii:** дахин ачаалсны дараа баталгаажуулсны дараа `torii.sorafs_gateway.denylist.path`-ээр дамжуулан хасах жагсаалтын замыг байрлуулж, Torii-г дахин эхлүүлэх/дахин ачаалах; Ажиллах хугацаа нь оруулгуудыг зөвшөөрөхөөс өмнө ижил хязгаарлалтыг мөрддөг.

## Багаж хэрэгсэл ба лавлагаа

- Ажиллах үеийн бодлогын хэрэгжилт нь `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) дээр ажилладаг бөгөөд дуудагч одоо `torii.sorafs_gateway.denylist.*` оролтыг задлан шинжлэх үед түвшний мета өгөгдлийг ашигладаг.
- CLI баталгаажуулалт нь `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) доторх ажиллах цагийн семантикийг тусгадаг. TTL-ууд тохируулсан цонхноос хэтэрсэн эсвэл заавал канон/засаглалын лавлагаа байхгүй үед линтер амжилтгүй болно.
- Тохиргооны товчлуурууд нь `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) доор тодорхойлогддог тул хэрэв засаглал өөр хязгаарыг баталвал операторууд TTL-ийг өөрчлөх/хянах хугацааг өөрчлөх боломжтой.
- Нийтийн түүвэр үгүйсгэх жагсаалт (`docs/source/sorafs_gateway_denylist_sample.json`) одоо бүх гурван шатлалыг харуулсан бөгөөд шинэ оруулгуудад каноник загвар болгон ашиглах ёстой.Эдгээр хашлага нь онцгой байдлын дүрмийн жагсаалтыг кодчилох, хязгааргүй TTL-ээс урьдчилан сэргийлэх, байнгын блокуудын засаглалын тодорхой нотолгоог албадах замаар замын зураглалын **MINFO-6a** зүйлд нийцдэг.

## Бүртгэлийн автоматжуулалт ба нотлох баримтын экспорт

Яаралтай тусламжийн канон зөвшөөрөл нь тодорхойлогч бүртгэлийн агшин зуурын зургийг гаргах ёстой бөгөөд a
Torii татгалзсан жагсаалтыг ачаалахаас өмнө diff багц. Багаж хэрэгсэл доор байна
`xtask/src/sorafs.rs` ба CI бэхэлгээ `ci/check_sorafs_gateway_denylist.sh`
ажлын явцыг бүхэлд нь хамарна.

### Каноник багц үүсгэх

1. Түүхий бичилтүүдийг (ихэвчлэн засаглалын хянадаг файл) ажлын хэсэгт үе шат
   лавлах.
2. JSON-г дараах аргаар каноник болгож, битүүмжлэх:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Энэ тушаал нь гарын үсэг зурсан `.json`, Norito `.to` багцыг ялгаруулдаг.
   дугтуй болон засаглалын тоймчдын хүлээгдэж буй Merkle-root текст файл.
   Лавлахыг `artifacts/ministry/denylist_registry/` (эсвэл таны
   сонгосон нотолгооны хувин) тиймээс `scripts/ministry/transparency_release.py` боломжтой
   дараа `--artifact denylist_bundle=<path>`-р аваарай.
3. Үүсгэсэн `checksums.sha256`-г түлхэхээсээ өмнө багцын хажууд байлга.
   SoraFS/GAR хүртэл. CI-ийн `ci/check_sorafs_gateway_denylist.sh` нь адилхан дасгал хийдэг
   `pack` багаж хэрэгслийн ажлыг баталгаажуулахын тулд дээжийн жагсаалтын эсрэг туслах.
   хувилбар бүр.

### Ялгаа + аудитын багц

1. -г ашиглан шинэ багцыг өмнөх үйлдвэрлэлийн хормын хувилбартай харьцуул
   xtask ялгаа туслагч:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON тайланд бүх нэмэлт, хасалтыг жагсааж, нотлох баримтыг тусгасан болно
   `MinistryDenylistChangeV1`-ийн хэрэглэсэн бүтэц (лав
   `docs/source/sorafs_gateway_self_cert.md` ба дагаж мөрдөх төлөвлөгөө).
2. Каноны хүсэлт болгонд `denylist_diff.json`-г хавсаргана уу (энэ нь хэр их болохыг нотлох болно.
   оруулгуудыг хөндөж, аль шатлал өөрчлөгдсөн, ямар нотлох хэш газрын зураг
   каноник багц).
3. Ялгааг автоматаар үүсгэх үед (CI эсвэл хувилбарын дамжуулах хоолой) -г экспортлоорой
   `denylist_diff.json` зам `--artifact denylist_diff=<path>`-ээр дамжих тул
   ил тод байдлын манифест нь үүнийг ариутгасан хэмжүүрүүдийн хамт бүртгэдэг. Үүнтэй ижил CI
   Туслагч нь CLI хураангуй алхамыг ажиллуулдаг `--evidence-out <path>`-г хүлээн авдаг ба
   үүссэн JSON-г дараа нь нийтлэхийн тулд хүссэн байршилд хуулна.

### Нийтлэл ба ил тод байдал1. Улирал тутмын ил тод байдлын лавлах руу багц + ялгаатай олдворуудыг буулгана уу
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). Ил тод байдал
   суллах туслах нь дараа нь тэдгээрийг багтааж болно:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Үүсгэсэн багц/ялгааг улирлын тайланд иш татна
   (`docs/source/ministry/reports/<YYYY-Q>.md`) ба ижил замыг хавсаргана
   GAR саналын багц, ингэснээр аудиторууд нотлох баримтыг нэвтрэхгүйгээр дахин тоглуулах боломжтой
   дотоод CI. `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   artefacts/ministry/denylist_registry//denylist_evidence.json` одоо
   pack/diff/evidence хуурай ажиллуулах (`iroha_cli програмын sorafs гарц руу залгах) гүйцэтгэдэг
   prove` under the cap) тиймээс автоматжуулалт нь товчлолын хажуугаар үргэлжлүүлэх боломжтой
   каноник багцууд.
3. Нийтлэгдсэний дараа засаглалын ачааллыг дамжуулан
   `cargo xtask ministry-transparency anchor` (автоматаар дууддаг
   `transparency_release.py` `--governance-dir` өгөгдсөн үед) тиймээс
   denylist registry digest нь ил тод байдлын нэгэн адил DAG модонд харагдана
   суллах.

Энэ үйл явцын дараа "бүртгэлийн автоматжуулалт ба нотлох баримтын экспорт" хаагдана.
цоорхойг `roadmap.md:450`-д дуудаж, яаралтай тусламжийн бүх каноныг баталгаажуулдаг
Шийдвэр нь хуулбарлах боломжтой олдворууд, JSON ялгаа, ил тод байдлын бүртгэлтэй ирдэг
оруулгууд.

### TTL & Canon нотлох туслах

Багц/ялгаа хос үүсгэсний дараа CLI нотлох туслах програмыг ажиллуул
Засаглалд шаардлагатай TTL-ийн хураангуй болон яаралтай хянан шалгах эцсийн хугацаа:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Энэ тушаал нь JSON-ийн эх сурвалжийг хэш болгож, оруулга бүрийг баталгаажуулж, компакт ялгаруулдаг
агуулсан хураангуй:

- `kind` болон бодлогын түвшний хамгийн эртний/сүүлийн үеийн нийт оруулгууд
  цаг тэмдэг ажиглагдсан.
- `emergency_reviews[]` жагсаалт нь яаралтай тусламжийн канон бүрийг өөрийнх нь хамт жагсаав
  тодорхойлогч, хүчинтэй дуусах хугацаа, зөвшөөрөгдөх дээд хэмжээ TTL, тооцоолсон
  `review_due_by` эцсийн хугацаа.

`denylist_evidence.json`-г багцалсан багц/ялгааны хажууд хавсаргаснаар аудиторууд
CLI-г дахин ажиллуулахгүйгээр TTL нийцэж байгаа эсэхийг баталгаажуулна уу. Аль хэдийн үүсгэсэн CI ажлын байр
багцууд нь туслахыг дуудаж, нотлох баримтыг нийтлэх боломжтой (жишээ нь
дуудаж `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`), хангах
Каноны хүсэлт бүр тогтвортой хураангуйтай ирдэг.

### Merkle бүртгэлийн нотлох баримт

MINFO-6-д нэвтрүүлсэн Merkle бүртгэл нь операторуудаас нийтлэхийг шаарддаг
TTL хураангуйн хажууд үндэс ба оруулга бүрийн нотолгоо. Гүйсний дараа шууд
нотлох туслах, Мерклийн олдворуудыг барьж ав:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```JSON агшин зуурын зураг нь BLAKE3 Merkle үндэс, навчны тоо болон бүрийг бүртгэдэг
тодорхойлогч/хэш хос, ингэснээр GAR саналууд хэш хийсэн модыг яг таг лавлах боломжтой.
`--norito-out` нийлүүлэх нь JSON-ийн хажууд `.to` олдворыг хадгалах боломжийг олгодог.
гарцууд нь Norito-ээр дамжуулан бүртгэлийн оруулгуудыг хусахгүйгээр шууд залгидаг.
stdout. `merkle proof` нь дурын битийн чиглэлийн битүүд болон ах дүү хэшүүдийг ялгаруулдаг.
тэг дээр суурилсан оруулгын индекс нь тус бүрийг оруулах нотлох баримтыг хавсаргахад хялбар болгодог
GAR санамж бичигт дурдсан яаралтай тусламжийн канон—заавал биш Norito хуулбар нь нотлох баримтыг хадгалдаг.
дэвтэр дээр түгээхэд бэлэн. Дараа нь JSON болон Norito олдворуудыг хадгална уу
TTL хураангуй болон ялгавартай багцад оруулснаар ил тод байдал болон засаглал
зангуу нь ижил үндэсийг иш татдаг.