---
lang: mn
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Тооцоолох эгнээ (SSC-1)

Тооцоолох зам нь HTTP маягийн тодорхой дуудлагыг хүлээн авч, Kotodama дээр буулгадаг.
оролтын цэгүүд, тооцоо хийх, засаглалыг шалгахад зориулж тоолуур/баримт бичдэг.
Энэхүү RFC нь манифест схем, дуудлага/хүлээн авсан дугтуй, хамгаалагдсан хязгаарлагдмал орчны хамгаалалтын хашлага,
болон анхны хувилбарын тохиргооны өгөгдмөл.

## Манифест

- Схем: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` нь `1` дээр бэхлэгдсэн; өөр хувилбартай манифестууд татгалзсан
  баталгаажуулалтын үеэр.
- Маршрут бүр нь:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama нэвтрэх цэгийн нэр)
  - кодлогчийн зөвшөөрөгдсөн жагсаалт (`codecs`)
  - TTL/хий/хүсэлт/хариултын хязгаар (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - детерминизм/гүйцэтгэх анги (`determinism`, `execution_class`)
  - SoraFS оролт/загварын тодорхойлогч (`input_limits`, нэмэлт `model`)
  - үнийн бүлэг (`price_family`) + нөөцийн танилцуулга (`resource_profile`)
  - баталгаажуулалтын бодлого (`auth`)
- Sandbox хамгаалалтын хашлага нь `sandbox` манифест блокт байрладаг бөгөөд бүгд хуваалцдаг.
  чиглүүлэлтүүд (горим/санамсаргүй байдал/хадгалах болон тодорхой бус системийн дуудлагаас татгалзах).

Жишээ нь: `fixtures/compute/manifest_compute_payments.json`.

## Дуудлага, хүсэлт, баримт

- Схем: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` in
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` нь каноник хүсэлтийн хэшийг үүсгэдэг (толгойг хадгалдаг)
  тодорхойлогч `BTreeMap`-д ачаалал нь `payload_hash` хэлбэрээр явагддаг).
- `ComputeCall` нэрийн орон зай/маршрут, кодлогч, TTL/хий/хариултын хязгаар,
  нөөцийн профайл + үнийн бүлэг, баталгаажуулалт (`Public` эсвэл UAID-д холбогдсон)
  `ComputeAuthn`), детерминизм (`Strict` vs `BestEffort`), гүйцэтгэлийн анги
  зөвлөмжүүд (CPU/GPU/TEE), зарласан SoraFS оролтын байт/хэсэг, нэмэлт ивээн тэтгэгч
  төсөв, каноник хүсэлтийн дугтуй. Хүсэлтийн хэш-г ашигладаг
  дахин тоглуулах хамгаалалт ба чиглүүлэлт.
- Маршрут нь нэмэлт SoraFS загварын лавлагаа болон оролтын хязгаарыг оруулж болно
  (дотор/бүх том том); манифест хамгаалагдсан хязгаарлагдмал орчны дүрмийн gate GPU/TEE зөвлөмжүүд.
- `ComputePriceWeights::charge_units` нь тоолуурын өгөгдлийг тооцооны тооцоолол болгон хувиргадаг
  Цикл болон гаралтын байт дээрх таазны хуваалтаар нэгжүүд.
- `ComputeOutcome` тайлан `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, эсвэл `InternalError` ба сонголтоор хариултын хэш орно/
  аудитын хэмжээ/кодек.

Жишээ нь:
- Дуудлага: `fixtures/compute/call_compute_payments.json`
- Баримт бичиг: `fixtures/compute/receipt_compute_payments.json`

## хамгаалагдсан хязгаарлагдмал орчин болон нөөцийн профайл- `ComputeSandboxRules` нь гүйцэтгэх горимыг анхдагчаар `IvmOnly` болгон түгжиж,
  Хүсэлтийн хэшийн үрийг тодорхойлох санамсаргүй байдал, зөвхөн уншихыг зөвшөөрдөг SoraFS
  хандалт хийх ба детерминистик бус системийн дуудлагыг үгүйсгэдэг. GPU/TEE-н зөвлөмжийг өөрчилсөн
  Гүйцэтгэлийг тодорхой болгохын тулд `allow_gpu_hints`/`allow_tee_hints`.
- `ComputeResourceBudget` нь цикл, шугаман санах ой, стек дээр профайл бүрийн дээд хязгаарыг тогтоодог.
  хэмжээ, IO төсөв, гаралт, мөн GPU зөвлөмжүүд болон WASI-lite туслахуудыг унтраадаг.
- Анхдагчаар хоёр профайлыг (`cpu-small`, `cpu-balanced`) илгээдэг.
  Детерминист нөөцтэй `defaults::compute::resource_profiles`.

## Үнэ болон тооцооны нэгж

- Үнийн гэр бүлүүд (`ComputePriceWeights`) зураглалын мөчлөг болон гаралтын байтыг тооцоололд оруулах
  нэгж; анхдагч төлбөр `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)`
  `unit_label = "cu"`. Гэр бүлүүд нь `price_family`-ээр манифест болон
  элсэлтийн үед хүчин төгөлдөр болно.
- Хэмжилтийн бүртгэл нь `charged_units` дээр нэмээд түүхий цикл/оролт/гарц/хугацаатай
  эвлэрүүлэх нийлбэр. Төлбөрийг гүйцэтгэлийн зэрэглэлээр нэмэгдүүлсэн ба
  детерминизмын үржүүлэгч (`ComputePriceAmplifiers`) ба хязгаартай
  `compute.economics.max_cu_per_call`; гарцыг хавчаараар тогтооно
  `compute.economics.max_amplification_ratio`-ийн холболтын хариу өсгөлт.
- Ивээн тэтгэгчийн төсөв (`ComputeCall::sponsor_budget_cu`) эсрэг мөрдөгдөж байна
  дуудлага тутамд/өдөр тутмын хязгаар; тооцооны нэгж нь зарласан ивээн тэтгэгчийн төсвөөс хэтрэхгүй байх ёстой.
-Засаглалын үнийн шинэчлэлт нь эрсдэлийн ангиллын хязгаарыг ашигладаг
  `compute.economics.price_bounds` ба үндсэн гэр бүлүүд
  `compute.economics.price_family_baseline`; ашиглах
  `ComputeEconomics::apply_price_update` шинэчлэгдэхээс өмнө дельтануудыг баталгаажуулна
  идэвхтэй гэр бүлийн газрын зураг. Torii тохиргооны шинэчлэлтүүдийг ашигладаг
  `ConfigUpdate::ComputePricing` ба kiso нь үүнийг ижил хязгаартайгаар хэрэгжүүлдэг
  засаглалын засваруудыг детерминистик байлгах.

## Тохиргоо

Тооцооллын шинэ тохиргоо `crates/iroha_config/src/parameters`-д амьдардаг:

- Хэрэглэгчийн харагдац: `Compute` (`user.rs`) env дарагдсан:
  - `COMPUTE_ENABLED` (өгөгдмөл `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Үнэ/эдийн засаг: `compute.economics` зураг авалт
  `max_cu_per_call`/`max_amplification_ratio`, хураамж хуваах, ивээн тэтгэгчийн дээд хязгаар
  (дуудлагын болон өдөр тутмын CU), үнийн гэр бүлийн суурь үзүүлэлт + эрсдэлийн ангилал/хязгаарлалт
  засаглалын шинэчлэл, гүйцэтгэлийн зэрэглэлийн үржүүлэгч (GPU/TEE/best-fort).
- Бодит/өгөгдмөл: `actual.rs` / `defaults.rs::compute` задлан шинжилсэн
  `Compute` тохиргоо (нэрийн орон зай, профайл, үнийн бүлгүүд, хамгаалагдсан хязгаарлагдмал орчин).
- Буруу тохиргоо (хоосон нэрийн зай, өгөгдмөл профайл/гэр бүл дутуу, TTL хязгаар
  урвуу) нь задлан шинжлэх явцад `InvalidComputeConfig` хэлбэрээр гарч ирнэ.

## Туршилт ба бэхэлгээ

- Тодорхойлогч туслахууд (`request_hash`, үнэ) болон бэхэлгээний хоёр талын аялалууд
  `crates/iroha_data_model/src/compute/mod.rs` (`fixtures_round_trip`, үзнэ үү.
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON бэхэлгээ нь `fixtures/compute/`-д амьдардаг бөгөөд өгөгдлийн загвараар хэрэгждэг
  регрессийн хамрах хүрээг шалгах тестүүд.

## SLO хэрэгсэл ба төсөв- `compute.slo.*` тохиргоо нь гарцын SLO товчлууруудыг (нислэгийн дараалал) ил гаргадаг
  гүн, RPS хязгаар, хоцрогдлын зорилтууд) дотор
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Defaults: 32
  Нислэгийн үеэр, нэг чиглэлд 512 дараалал, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- SLO-ийн хураангуй болон хүсэлт/гарцыг авахын тулд хөнгөн жингийн вандан морийг ажиллуул
  хормын хувилбар: `cargo run -p xtask --bin compute_gateway -- вандан [манифест_зам]
  [давталт] [давсарлага] [гарах_дир]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 давталт, зэрэгцээ 16, гаралт дор
  `artifacts/compute_gateway/bench_summary.{json,md}`). Вандан сандал ашигладаг
  тодорхойлогч ачаалал (`fixtures/compute/payload_compute_payments.json`) ба
  Дасгал хийх явцад дахин тоглуулахаас зайлсхийхийн тулд хүсэлтийн толгой
  `echo`/`uppercase`/`sha3` нэвтрэх цэгүүд.

## SDK/CLI хосолсон тохируулгууд

- Каноник бэхэлгээ нь `fixtures/compute/`-ийн дагуу ажилладаг: манифест, дуудлага, ачаалал, болон
  гарц маягийн хариу/баримтын зохион байгуулалт. Ачааны хэш нь дуудлагатай тохирч байх ёстой
  `request.payload_hash`; туслагчийн ачаалал амьдардаг
  `fixtures/compute/payload_compute_payments.json`.
- CLI нь `iroha compute simulate` болон `iroha compute invoke`-ийг нийлүүлдэг:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` амьдардаг
  `javascript/iroha_js/src/compute.js` доор регрессийн тесттэй
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` нь ижил бэхэлгээг ачаалж, ачааллын хэшийг баталгаажуулдаг,
  тестээр нэвтрэх цэгүүдийг дуурайлган хийдэг
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift туслахууд бүгд ижил Norito төхөөрөмжийг хуваалцдаг тул SDK-ууд
  a-г дарахгүйгээр хүсэлтийн бүтэц болон хэшийг офлайнаар зохицуулах
  ажиллаж байгаа гарц.