---
lang: mn
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

Статус: Torii, CLI болон элсэлтийн үндсэн шалгалтаар (2025 оны 11-р сар) хэрэгжүүлж, хэрэгжүүлсэн.

## Тойм

- Эмхэтгэсэн IVM байт кодыг (`.to`) Torii-д илгээх эсвэл олгох замаар байрлуулна.
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` заавар
  шууд.
- Зангилаанууд `code_hash` болон каноник ABI хэшийг дотооддоо дахин тооцоолдог; таарахгүй
  тодорхойлон үгүйсгэх.
- Хадгалагдсан олдворууд нь `contract_manifests` гинжин хэлхээний дор амьдардаг ба
  `contract_code` бүртгэлүүд. Зөвхөн лавлагааны хэшийг харуулах ба жижиг хэвээр байна;
  кодын байт нь `code_hash`-ээр түлхүүрлэгдсэн байдаг.
- Хамгаалагдсан нэрийн орон зай нь өмнө нь батлагдсан засаглалын саналыг шаардаж болно a
  байршуулахыг зөвшөөрч байна. Элсэлтийн зам нь саналын ачаалал болон
  үед `(namespace, contract_id, code_hash, abi_hash)` тэгш байдлыг хэрэгжүүлдэг
  нэрийн орон зай хамгаалагдсан.

## Хадгалагдсан олдворууд ба хадгалалт

- `RegisterSmartContractCode` өгөгдсөн манифестийг оруулна/дарж бичнэ
  `code_hash`. Ижил хэш аль хэдийн байгаа бол шинээр солигдоно
  манифест.
- `RegisterSmartContractBytes` хөрвүүлсэн програмыг доор хадгалдаг
  `contract_code[code_hash]`. Хэрэв хэшийн байт аль хэдийн байгаа бол тэдгээр нь таарах ёстой
  яг; ялгаатай байт нь өөрчлөгдөөгүй зөрчлийг үүсгэдэг.
- Кодын хэмжээг `max_contract_code_bytes` захиалгат параметрээр хязгаарласан
  (өгөгдмөл 16 МБ). Өмнө нь `SetParameter(Custom)` гүйлгээгээр үүнийг хүчингүй болго
  томоохон олдворуудыг бүртгэх.
- Хадгалах нь хязгааргүй: манифестууд болон кодууд тодорхой болтол боломжтой хэвээр байна
  ирээдүйн засаглалын ажлын урсгалд хасагдсан. TTL эсвэл автомат GC байхгүй.

## Элсэлтийн хоолой

- Баталгаажуулагч нь IVM толгой хэсгийг задлан шинжилж, `version_major == 1`-г мөрдүүлж, шалгана.
  `abi_version == 1`. Үл мэдэгдэх хувилбарууд нэн даруй татгалздаг; ажиллах хугацаа байхгүй
  солих.
- `code_hash`-д манифест аль хэдийн байгаа бол баталгаажуулалт нь
  Хадгалсан `code_hash`/`abi_hash` нь ирүүлсэн мэдээллээс тооцоолсон утгатай тэнцүү байна.
  хөтөлбөр. Тохиромжгүй байдал нь `Manifest{Code,Abi}HashMismatch` алдаа үүсгэдэг.
- Хамгаалагдсан нэрийн орон зайд чиглэсэн гүйлгээ нь мета өгөгдлийн түлхүүрүүдийг агуулсан байх ёстой
  `gov_namespace` ба `gov_contract_id`. Элсэлтийн зам нь тэдгээрийг харьцуулдаг
  батлагдсан `DeployContract` саналуудын эсрэг; Хэрэв тохирох санал байхгүй бол
  гүйлгээг `NotPermitted` татгалзсан.

## Torii төгсгөлийн цэгүүд (`app_api` онцлог)- `POST /v2/contracts/deploy`
  - Хүсэлтийн үндсэн хэсэг: `DeployContractDto` (талбарын дэлгэрэнгүйг `docs/source/torii_contracts_api.md` хэсгээс үзнэ үү).
  - Torii base64-ийн ачааллыг тайлж, хоёр хэшийг тооцоолж, манифест үүсгэдэг,
    мөн `RegisterSmartContractCode` нэмэхийг илгээнэ
    нэрийн өмнөөс гарын үсэг зурсан гүйлгээнд `RegisterSmartContractBytes`
    дуудагч.
  - Хариулт: `{ ok, code_hash_hex, abi_hash_hex }`.
  - Алдаа: хүчингүй base64, дэмжигдээгүй ABI хувилбар, зөвшөөрөл дутуу
    (`CanRegisterSmartContractCode`), хэмжээ хязгаар хэтэрсэн, засаглалын гарц.
- `POST /v2/contracts/code`
  - `RegisterContractCodeDto` (эрх мэдэл, хувийн түлхүүр, манифест) хүлээн зөвшөөрч, зөвхөн илгээнэ
    `RegisterSmartContractCode`. Манифестуудыг дараахаас тусад нь хийх үед хэрэглэнэ
    байт код.
- `POST /v2/contracts/instance`
  - `DeployAndActivateInstanceDto` (эрх мэдэл, хувийн түлхүүр, нэрийн зай/гэрээний_id, `code_b64`, нэмэлт манифест хүчингүй болгох) -ийг хүлээн авч, байрлуулж + атомаар идэвхжүүлнэ.
- `POST /v2/contracts/instance/activate`
  - `ActivateInstanceDto` (эрх мэдэл, хувийн түлхүүр, нэрийн зай, contract_id, `code_hash`)-г хүлээн зөвшөөрч, зөвхөн идэвхжүүлэх зааварчилгааг илгээнэ.
- `GET /v2/contracts/code/{code_hash}`
  - `{ manifest: { code_hash, abi_hash } }`-г буцаана.
    Нэмэлт манифест талбарууд дотооддоо хадгалагдсан боловч энд орхигдуулсан
    тогтвортой API.
- `GET /v2/contracts/code-bytes/{code_hash}`
  - `{ code_b64 }`-г base64 гэж кодлогдсон `.to` зургийн хамт буцаана.

Гэрээний бүх амьдралын мөчлөгийн төгсгөлийн цэгүүд нь тусгайлан тохируулсан байршуулах хязгаарлагчийг хуваалцдаг
`torii.deploy_rate_per_origin_per_sec` (секундэд жетон) ба
`torii.deploy_burst_per_origin` (тэсрэлтийн жетон). Өгөгдмөл нь тэсрэлттэй 4 req/s байна
`X-API-Token`, алсын IP эсвэл төгсгөлийн цэгийн сануулгаас авсан токен/түлхүүр тус бүрд 8.
Итгэмжлэгдсэн операторуудын хязгаарлагчийг идэвхгүй болгохын тулд аль нэг талбарыг `null` болгож тохируулна уу. Хэзээ
хязгаарлагч гал гарч, Torii нэмэгдэнэ
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` телеметрийн тоолуур болон
HTTP 429-г буцаана; аливаа зохицуулагчийн алдааны өсөлт
Анхааруулахын тулд `torii_contract_errors_total{endpoint=…}`.

## Засаглалын нэгдэл ба хамгаалагдсан нэрийн орон зай- `gov_protected_namespaces` (нэрийн зайны JSON массив) захиалгат параметрийг тохируулах
  strings) элсэлтийн гарцыг идэвхжүүлэх. Torii туслагчдыг доор харуулав
  `/v2/gov/protected-namespaces` ба CLI нь тэдгээрийг дамжуулан тусгадаг
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- `ProposeDeployContract` (эсвэл Torii) ашиглан үүсгэсэн саналууд
  `/v2/gov/proposals/deploy-contract` төгсгөлийн цэг) авах
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- Ард нийтийн санал асуулга баталсны дараа `EnactReferendum` саналыг баталж, баталж байна.
  элсэлт нь тохирох мета өгөгдөл болон код агуулсан байршуулалтыг хүлээн авна.
- Гүйлгээнд `gov_namespace=a namespace` болон мета өгөгдлийн хосыг агуулсан байх ёстой
  `gov_contract_id=an identifier` (мөн `contract_namespace` тохируулах ёстой /
  `contract_id`, дуудлагын цагийг холбох). CLI туслахууд эдгээрийг дүүргэдэг
  автоматаар та `--namespace`/`--contract-id` дамжих үед.
- Хамгаалагдсан нэрийн орон зайг идэвхжүүлсэн үед дараалалд орох оролдлого татгалздаг
  одоо байгаа `contract_id`-г өөр нэрийн талбарт дахин холбох; батлагдсаныг ашиглах
  санал болгох эсвэл өөр газар байрлуулахаас өмнө өмнөх хүчин зүйлийг цуцлах.
- Хэрэв эгнээний манифест нь нэгээс дээш баталгаажуулагч чуулга тогтоосон бол оруулна уу
  `gov_manifest_approvers` (JSON баталгаажуулагч дансны ID массив) тул дарааллыг тоолох боломжтой
  гүйлгээний эрх бүхий байгууллагын хамт нэмэлт зөвшөөрөл. Lanes мөн татгалздаг
  манифестэд байхгүй нэрийн орон зайд хамаарах мета өгөгдөл
  `protected_namespaces` багц.

## CLI туслахууд

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  Torii байршуулах хүсэлтийг илгээдэг (хэшийг шууд тооцоолох).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  манифест бүтээх (өгөгдсөн түлхүүрээр гарын үсэг зурсан), байт + манифестыг бүртгэх,
  мөн нэг гүйлгээнд `(namespace, contract_id)` холболтыг идэвхжүүлдэг. Ашиглах
  `--dry-run` нь тооцоолсон хэш болон зааврын тоог хэвлэхгүйгээр
  илгээж, гарын үсэг зурсан манифест JSON-г хадгалахын тулд `--manifest-out`.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` тооцоолно
  `code_hash`/`abi_hash` эмхэтгэсэн `.to` ба сонголтоор манифестт гарын үсэг зурна,
  JSON хэвлэх эсвэл `--out` руу бичих.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  офлайн VM дамжуулалтыг ажиллуулж, ABI/хэш метадата болон дараалалд байгаа ISI-г мэдээлдэг
  (тоо ба зааврын ids) сүлжээнд хүрэлгүйгээр. Хавсаргах
  `--namespace/--contract-id` дуудлагын цагийн мета өгөгдлийг тусгах.
- `iroha_cli app contracts manifest get --code-hash <hex>` нь Torii-ээр дамжуулан манифест татаж авдаг
  мөн сонголтоор диск рүү бичнэ.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` татагдсан
  хадгалагдсан `.to` дүрс.
- `iroha_cli app contracts instances --namespace <ns> [--table]` жагсаалт идэвхжсэн
  гэрээний тохиолдлууд (манифест + мета өгөгдөлд тулгуурласан).
- Засаглалын туслахууд (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get`) хамгаалагдсан нэрийн орон зайн ажлын урсгалыг зохион байгуулж,
  JSON олдворуудыг аудит хийх.

## Туршилт ба хамрах хүрээ

- `crates/iroha_core/tests/contract_code_bytes.rs` кодын дагуу нэгжийн туршилт
  хадгалах, idempotency, болон хэмжээ хязгаар.
- `crates/iroha_core/tests/gov_enact_deploy.rs` манифест оруулгыг дамжуулан баталгаажуулдаг
  хууль тогтоомж, `crates/iroha_core/tests/gov_protected_gate.rs` дасгалууд
  хамгаалагдсан нэрийн орон зайг төгсгөлөөс нь хүртэл хүлээн авах.
- Torii чиглүүлэлтүүд нь хүсэлт/хариу нэгжийн тестүүдийг багтаасан бөгөөд CLI командууд нь
  JSON-ийн хоёр талын аяллыг тогтвортой байлгахын тулд интеграцийн туршилтууд.

Ард нийтийн санал асуулгын дэлгэрэнгүй ачааллыг `docs/source/governance_api.md` болон
саналын хуудасны ажлын урсгал.