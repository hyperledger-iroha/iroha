---
lang: mn
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4`-аас иш татсан нууц хөрөнгийн аудит ба үйл ажиллагааны ном.

# Нууц хөрөнгийн аудит ба үйл ажиллагааны дэвтэр

Энэхүү гарын авлага нь аудиторууд болон операторуудын найддаг нотлох баримтуудыг нэгтгэдэг
нууц хөрөнгийн урсгалыг баталгаажуулах үед. Энэ нь эргэлтийн тоглоомын номыг нөхөж өгдөг
(`docs/source/confidential_assets_rotation.md`) болон шалгалт тохируулгын дэвтэр
(`docs/source/confidential_assets_calibration.md`).

## 1. Сонгомол тодруулга, үйл явдлын мэдээ

- Нууц заавар бүр `ConfidentialEvent` бүтэцтэй ачааллыг ялгаруулдаг.
  (`Shielded`, `Transferred`, `Unshielded`)
  `crates/iroha_data_model/src/events/data/events.rs:198` ба сериалчилсан
  гүйцэтгэгчид (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Регрессийн иж бүрдэл нь аудиторууд найдаж болохуйц бетоны ачааллыг хэрэгжүүлдэг
  тодорхойлогч JSON бүдүүвчүүд (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii эдгээр үйл явдлыг стандарт SSE/WebSocket дамжуулах шугамаар илчилдэг; аудиторууд
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) ашиглан бүртгүүлэх,
  сонголтоор нэг хөрөнгийн тодорхойлолтыг хамрах хүрээг хамарна. CLI жишээ:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- Бодлогын мета өгөгдөл болон хүлээгдэж буй шилжилтийг дамжуулан авах боломжтой
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), Swift SDK-ээр тусгагдсан
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) болон баримтжуулсан
  нууц хөрөнгийн дизайн болон SDK гарын авлага хоёулаа
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Телеметр, хяналтын самбар, шалгалт тохируулгын нотолгоо

- Ажиллах цагийн хэмжүүр модны гүн, амлалт/хилийн түүх, үндсийг нүүлгэх
  тоолуур, шалгагч-кэшийн цохилтын харьцаа
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana хяналтын самбарууд
  `dashboards/grafana/confidential_assets.json` холбогдох хавтангууд болон
  дохиолол, ажлын урсгалыг `docs/source/confidential_assets.md:401`-д баримтжуулсан.
- Шалгалт тохируулгын гүйлтүүд (NS/op, gas/op, ns/gas) гарын үсэгтэй бүртгэлтэй
  `docs/source/confidential_assets_calibration.md`. Хамгийн сүүлийн үеийн Apple Silicon
  NEON run-г архивласан
  `docs/source/confidential_assets_calibration_neon_20260428.log`, мөн адил
  дэвтэр нь SIMD-төвийг сахисан болон AVX2 профайлуудын түр чөлөөлөлтийг хүртэл бүртгэдэг
  x86 хостууд онлайн болно.

## 3. Ослын хариу арга хэмжээ & Операторын даалгавар

- Эргүүлэх/шинэчлэх журам нь дотор байдаг
  `docs/source/confidential_assets_rotation.md`, шинийг хэрхэн тайзны талаар өгүүлэх
  параметрийн багц, бодлогын шинэчлэлийг төлөвлөх, түрийвч/аудиторт мэдэгдэх. The
  tracker (`docs/source/project_tracker/confidential_assets_phase_c.md`) жагсаалтууд
  runbook эзэмшигчид болон бэлтгэлийн хүлээлт.
- Үйлдвэрлэлийн бэлтгэл сургуулилт эсвэл яаралтай тусламжийн цонхонд операторууд нотлох баримт хавсаргана
  `status.md` бичилтүүд (жишээ нь, олон эгнээтэй давталтын бүртгэл) бөгөөд үүнд:
  `curl` бодлогын шилжилтийн баталгаа, Grafana агшин зуурын зураг болон холбогдох үйл явдал
  аудиторууд гаа → шилжүүлэг → цагийн хуваарийг илчлэх боломжтой болгодог.

## 4. Хөндлөнгийн тойм Cadence

- Аюулгүй байдлын хяналтын хамрах хүрээ: нууц хэлхээ, параметрийн бүртгэл, бодлого
  шилжилт, телеметр. Энэхүү баримт бичиг болон шалгалт тохируулгын дэвтэр маягтууд
  борлуулагчдад илгээсэн нотлох баримтын багц; хянан шалгах хуваарийг дамжуулан хянадаг
  `docs/source/project_tracker/confidential_assets_phase_c.md` дахь M4.
- Операторууд `status.md`-г худалдагчийн олж авсан мэдээлэл эсвэл дагаж мөрдөх мэдээллээр шинэчилж байх ёстой.
  үйлдлийн зүйлүүд. Гадны хяналт дуусах хүртэл энэ runbook-ийн үүрэг гүйцэтгэнэ
  үйл ажиллагааны суурь аудиторууд тест хийж болно.