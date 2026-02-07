---
lang: mn
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Итгэмжлэгдсэн үе тэнгийнхний # Genesis Bootstrap

Орон нутгийн `genesis.file`-гүй Iroha үе тэнгийнхэн итгэмжлэгдсэн нөхдөөс гарын үсэг зурсан генезисийн блокыг татаж авах боломжтой.
Norito кодлогдсон ачаалах протоколыг ашиглан.

- **Протокол:** үе тэнгийнхэн нь `GenesisRequest` (мета өгөгдөлд `Preflight`, ачааллын хувьд `Fetch`) болон
  `request_id`-р түлхүүрлэгдсэн `GenesisResponse` хүрээ. Хариулагчдын дунд гинжин дугаар, гарын үсэг зурсан pubkey,
  хэш болон нэмэлт хэмжээтэй зөвлөмж; Ачаа ачааллыг зөвхөн `Fetch` дээр буцаадаг ба давхардсан хүсэлтийн ID-ууд
  `DuplicateRequest` хүлээн авах.
- **Харуул:** хариулагч нар зөвшөөрөгдсөн жагсаалтыг (`genesis.bootstrap_allowlist` эсвэл итгэмжлэгдсэн үе тэнгийнхэн) хэрэгжүүлдэг.
  багц), гинжин дугаар/pubkey/хэш тааруулах, хурдны хязгаар (`genesis.bootstrap_response_throttle`) болон
  хэмжээсийн таг (`genesis.bootstrap_max_bytes`). Зөвшөөрөгдсөн жагсаалтаас гадуурх хүсэлтүүд `NotAllowed` хүлээн авах ба
  буруу түлхүүрээр гарын үсэг зурсан ачаалал нь `MismatchedPubkey` хүлээн авдаг.
- **Хүсэлтийн урсгал:** хадгалах сан хоосон, `genesis.file` тохируулаагүй үед (болон
  `genesis.bootstrap_enabled=true`), зангилаа нь нэмэлтээр итгэмжлэгдсэн үе тэнгийнхний өмнө нислэг үйлддэг.
  `genesis.expected_hash`, дараа нь ачааллыг татаж, гарын үсгийг `validate_genesis_block`-ээр баталгаажуулж,
  мөн блок хэрэглэхээс өмнө Кура-тай хамт `genesis.bootstrap.nrt` хэвээр байна. Bootstrap дахин оролдоно
  хүндэт `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval`, болон
  `genesis.bootstrap_max_attempts`.
- **Бүтэлгүйтлийн горимууд:** зөвшөөрөгдсөн жагсаалтын алдаа, хэлхээ/pubkey/хэш таарахгүй, хэмжээ зэргээс шалтгаалан хүсэлтийг татгалзсан.
  дээд хязгаарын зөрчил, тарифын хязгаар, орон нутгийн генезис дутуу, эсвэл давхардсан хүсэлтийн ID. Зөрчилтэй хэшүүд
  үе тэнгийнхэн нь дуудлагыг цуцлах; Орон нутгийн тохиргоонд хариулагч/цаг тасрахгүй.
- **Операторын алхмууд:** хамгийн багадаа нэг итгэмжлэгдсэн үе тэнгийнхэн хүчинтэй генезитэй холбогдох боломжтой эсэхийг баталгаажуулж, тохируулна уу.
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` болон дахин оролдох товчлуурууд ба
  тохирохгүй ачааллыг хүлээн авахаас зайлсхийхийн тулд `expected_hash` зүүг тохируулна уу. Тогтвортой ачаалал байж болно
  `genesis.file`-г `genesis.bootstrap.nrt` руу зааж, дараагийн гутал дээр дахин ашигласан.