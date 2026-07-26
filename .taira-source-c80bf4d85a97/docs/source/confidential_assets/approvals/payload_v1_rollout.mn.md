---
lang: mn
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Ачаа ачааллын v1-ийг нэвтрүүлэх зөвшөөрөл (SDK зөвлөл, 2026-04-28).
//!
//! `roadmap.md:M1`-д шаардлагатай SDK Зөвлөлийн шийдвэрийн санамж бичгийг авч,
//! шифрлэгдсэн ачааллын v1 хувилбар нь аудит хийх боломжтой бичлэгтэй (хүргэх боломжтой M1.4).

# Ачаалал ихтэй v1 гаргах шийдвэр (2026-04-28)

- **Дарга:** SDK Зөвлөлийн Тэргүүлэгч (М. Такемия)
- **Санал өгөх гишүүд:** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Ажиглагчид:** Хөтөлбөрийн удирдлага, Телеметрийн үйл ажиллагаа

## Оролтоо хянасан

1. **Swift bindings & submitters** — `ShieldRequest`/`UnshieldRequest`, синхрончлолгүй илгээгч болон Tx бүтээгчийн туслахууд паритет тест болон docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ergonomics** — `iroha app zk envelope` туслагч нь замын зураглалын эргономикийн шаардлагад нийцүүлэн кодчилох/шалгах ажлын урсгал болон алдаа оношилгоог хамардаг.【crates/iroha_cli/src/zk.rs:1256】
3. **Deterministic fixtures & parity suites** — Norito байт/алдааны гадаргууг хадгалахын тулд хуваалцсан бэхэлгээ + Rust/Swift баталгаажуулалт зэрэгцүүлсэн.【засвар/нууц/шифрлэгдсэн_ашиглалтын_v1.json:1】【крат/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Шийдвэр

- **SDK болон CLI-д зориулсан ачааллын v1 хувилбарыг** зөвшөөрч, Swift түрийвчийг захиалгаар сантехникгүйгээр нууц дугтуй гаргах боломжийг олгоно.
- **Нөхцөл:** 
  - Паритын тохируулгуудыг CI шилжилтийн дохиоллын доор байлга (`scripts/check_norito_bindings_sync.py`-тэй холбогдсон).
  - Үйл ажиллагааны дэвтрийг `docs/source/confidential_assets.md` (Swift SDK PR-аар аль хэдийн шинэчилсэн) дээр баримтжуулна уу.
  - Аливаа үйлдвэрлэлийн далбааг эргүүлэхээс өмнө шалгалт тохируулга + телеметрийн нотолгоог бүртгээрэй (M2 дор хянадаг).

## Үйлдлийн зүйлс

| Эзэмшигч | Зүйл | Хугацаа |
|-------|------|-----|
| Swift Lead | GA ашиглах боломжтой + README хэсгүүдийг зарлах | 2026-05-01 |
| CLI засварлагч | `iroha app zk envelope --from-fixture` туслах (заавал биш) | Хоцрогдол (хоригдохгүй) |
| DevRel WG | Ачаа ачааллын v1 зааварчилгааны тусламжтайгаар түрийвчний шуурхай эхлэлийг шинэчилнэ үү 2026-05-05 |

> **Тэмдэглэл:** Энэхүү санамж нь `roadmap.md:2426` дээрх "зөвлөлийн зөвшөөрлийг хүлээж байгаа" гэсэн түр зуурын мэдэгдлийг орлож, мөрдөгч M1.4 зүйлд нийцнэ. Дараах үйлдлийн зүйлүүд хаагдах бүрт `status.md`-г шинэчил.