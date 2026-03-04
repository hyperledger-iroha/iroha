---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md)-аас тохируулсан.

# SoraFS Үйлчилгээ үзүүлэгчийн элсэлт ба иргэний үнэмлэхний бодлого (SF-2b төсөл)

Энэ тэмдэглэлд **SF-2b**-д зориулсан үйл ажиллагааны үр дүнг тусгасан болно: тодорхойлох ба
элсэлтийн ажлын урсгал, биеийн байцаалтын шаардлага, баталгаажуулалтыг хэрэгжүүлэх
SoraFS хадгалах үйлчилгээ үзүүлэгчдийн даацын ачаалал. Энэ нь өндөр түвшний үйл явцыг өргөжүүлдэг
SoraFS Architecture RFC-д тодорхойлсон бөгөөд үлдсэн ажлыг дараах байдлаар хуваана.
хянах боломжтой инженерийн даалгавар.

## Бодлогын зорилго

- Зөвхөн шалгагдсан операторууд `ProviderAdvertV1` бүртгэлийг нийтлэх боломжтой эсэхийг шалгаарай.
  сүлжээ хүлээн авах болно.
- Зар сурталчилгааны түлхүүр бүрийг засаг захиргаанаас баталсан иргэний үнэмлэхтэй хавсаргах,
  баталгаажсан төгсгөлийн цэгүүд, хамгийн бага бооцооны хувь нэмэр.
- Torii, гарц, болон
  `sorafs-node` ижил шалгалтыг хэрэгжүүлнэ.
- Детерминизмыг зөрчихгүйгээр шинэчлэх, яаралтай цуцлахыг дэмжих
  багаж хэрэгслийн эргономик.

## Хувийн мэдээлэл, бооцооны шаардлага

| Шаардлага | Тодорхойлолт | Хүргэлттэй |
|-------------|-------------|-------------|
| Зар сурталчилгааны үндсэн гарал үүсэл | Үйлчилгээ үзүүлэгчид зар бүрт гарын үсэг зурдаг Ed25519 товчлуурыг бүртгүүлэх ёстой. Элсэлтийн багц нь нийтийн түлхүүрийг засаглалын гарын үсэгтэй хамт хадгалдаг. | `ProviderAdmissionProposalV1` схемийг `advert_key` (32 байт) ашиглан өргөтгөж, бүртгэлээс (`sorafs_manifest::provider_admission`) лавлана уу. |
| Гадасны заагч | Элсэлтийн хувьд идэвхтэй бооцооны цөөрөм рүү чиглэсэн тэгээс өөр `StakePointer` шаардлагатай. | `sorafs_manifest::provider_advert::StakePointer::validate()`-д баталгаажуулалт, CLI/туршилтын гадаргуугийн алдааг нэмнэ үү. |
| Харьяаллын шошго | Нийлүүлэгчид харьяаллыг тунхагладаг + хууль ёсны холбоо барих. | Саналын схемийг `jurisdiction_code` (ISO 3166-1 альфа-2) болон нэмэлт `contact_uri`-ээр сунгана уу. |
| Эцсийн цэгийн баталгаажуулалт | Сурталчилсан төгсгөлийн цэг бүр нь mTLS эсвэл QUIC гэрчилгээний тайлангаар баталгаажсан байх ёстой. | `EndpointAttestationV1` Norito ачааллыг тодорхойлж, элсэлтийн багц дотор төгсгөлийн цэг бүрт хадгална. |

## Элсэлтийн ажлын явц

1. **Санал үүсгэх**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` нэмнэ үү
     `ProviderAdmissionProposalV1` + баталгаажуулалтын багцыг үйлдвэрлэж байна.
   - Баталгаажуулалт: шаардлагатай талбаруудыг баталгаажуулах, гадас > 0, `profile_id` дахь каноник chunker бариул.
2. **Засаглалын баталгаа**
   - Зөвлөл одоо байгаа зүйлийг ашиглан `blake3("sorafs-provider-admission-v1" || canonical_bytes)` гарын үсэг зурна
     дугтуйны хэрэгсэл (`sorafs_manifest::governance` модуль).
   - Дугтуй `governance/providers/<provider_id>/admission.json` хүртэл хадгалагдана.
3. **Бүртгэлийн нэвтрэлт**
   - Хуваалцсан баталгаажуулагчийг хэрэгжүүлэх (`sorafs_manifest::provider_admission::validate_envelope`)
     Torii/гарцууд/CLI дахин ашиглах.
   - Torii элсэлтийн замыг шинэчилнэ үү.
4. **Сэргээх, хүчингүй болгох**
   - `ProviderAdmissionRenewalV1`-г нэмэлт төгсгөлийн цэг/гадасны шинэчлэлтүүдээр нэмнэ үү.
   - Хүчингүй болгох шалтгааныг бүртгэж, засаглалын үйл явдлыг түлхэж буй `--revoke` CLI замыг ил гарга.

## Хэрэгжүүлэх даалгавар

| Талбай | Даалгавар | Эзэмшигч(үүд) | Статус |
|------|------|----------|--------|
| Схем | `crates/sorafs_manifest/src/provider_admission.rs` доор `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) гэж тодорхойл. `sorafs_manifest::provider_admission`-д баталгаажуулалтын туслахуудаар хэрэгжүүлсэн.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хадгалах / Засаглал | ✅ Дууссан |
| CLI хэрэгсэл | `sorafs_manifest_stub`-г дараах дэд командуудаар сунгана: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Багажны WG | ✅ |

CLI урсгал одоо завсрын гэрчилгээний багцуудыг (`--endpoint-attestation-intermediate`) хүлээн авч, ялгаруулдаг
каноник санал/дугтуйны байт, мөн `sign`/`verify` үед зөвлөлийн гарын үсгийг баталгаажуулдаг. Операторууд чадна
Зар сурталчилгааны үндсэн хэсгийг шууд өгөх, эсвэл гарын үсэг зурсан зарыг дахин ашиглах, гарын үсгийн файлуудыг хослуулах замаар нийлүүлж болно.
`--council-signature-public-key` нь `--council-signature-file`-тэй, автоматжуулалтад ээлтэй.

### CLI лавлагаа

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …`-ээр команд бүрийг ажиллуул.

- `proposal`
  - Шаардлагатай тугнууд: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, дор хаяж нэг `--endpoint=<kind:host>`.
  - Төгсгөлийн цэгийн баталгаажуулалт нь `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, дамжуулан гэрчилгээ
    `--endpoint-attestation-leaf=<path>` (нэмэх нэмэлт `--endpoint-attestation-intermediate=<path>`)
    гинжин элемент тус бүрийн хувьд) болон тохиролцсон ALPN ID
    (`--endpoint-attestation-alpn=<token>`). QUIC төгсгөлийн цэгүүд нь тээврийн тайлангуудыг нийлүүлж болно
    `--endpoint-attestation-report[-hex]=…`.
  - Гаралт: каноник Norito саналын байт (`--proposal-out`) ба JSON хураангуй
    (өгөгдмөл stdout эсвэл `--json-out`).
- `sign`
  - Оруулга: санал (`--proposal`), гарын үсэг зурсан зар (`--advert`), нэмэлт зар сурталчилгааны үндсэн хэсэг
    (`--advert-body`), хадгалах хугацаа, дор хаяж нэг зөвлөлийн гарын үсэг. Гарын үсэг өгөх боломжтой
    inline (`--council-signature=<signer_hex:signature_hex>`) эсвэл файлуудаар дамжуулан нэгтгэх
    `--council-signature-public-key`, `--council-signature-file=<path>`.
  - Баталгаажсан дугтуй (`--envelope-out`) ба JSON тайланг гаргаж, нэгтгэсэн холболтыг харуулсан,
    гарын үсэг зурсан хүний тоо, оролтын зам.
- `verify`
  - Одоо байгаа дугтуйг (`--envelope`) баталгаажуулж, тохирох саналыг шалгах,
    зар сурталчилгаа эсвэл зар сурталчилгааны үндсэн хэсэг. JSON тайлан нь хураангуй утгууд, гарын үсгийн баталгаажуулалтын төлөв,
    ямар нэмэлт олдвор таарч байна.
- `renewal`
  - Шинээр батлагдсан дугтуйг өмнө нь соёрхон баталсан тоймтой холбоно. шаарддаг
    `--previous-envelope=<path>` ба залгамжлагч `--envelope=<path>` (хоёулаа Norito ачаалал).
    CLI нь профайлын нэр, чадвар, зар сурталчилгааны түлхүүрүүд өөрчлөгдөхгүй хэвээр байгааг баталгаажуулдаг
    бооцоо, төгсгөлийн цэг, мета өгөгдлийн шинэчлэлтийг зөвшөөрөх. Каноникийг гаргана
    `ProviderAdmissionRenewalV1` байт (`--renewal-out`) дээр JSON хураангуй.
- `revoke`
  - Дугтуй нь заавал байх ёстой үйлчилгээ үзүүлэгчийн яаралтай тусламжийн `ProviderAdmissionRevocationV1` багцыг гаргадаг.
    эргүүлэн татах. `--envelope=<path>`, `--reason=<text>`, дор хаяж нэг шаардлагатай
    `--council-signature`, нэмэлт `--revoked-at`/`--notes`. CLI нь гарын үсэг зурж баталгаажуулдаг
    хүчингүй болгох тойм, Norito ачааллыг `--revocation-out`-ээр дамжуулан бичиж, JSON тайлан хэвлэдэг
    тойм болон гарын үсгийн тоог авах.
| Баталгаажуулалт | Torii, гарцууд болон `sorafs-node` ашигладаг хуваалцсан баталгаажуулагчийг хэрэгжүүлнэ үү. Нэгж + CLI интеграцийн тестээр хангах.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сүлжээний TL / Хадгалах | ✅ Дууссан |
| Torii интеграци | Баталгаажуулагчийг Torii зар сурталчилгаа руу оруулах, бодлогогүй зар сурталчилгаанаас татгалзах, телеметрийг цацах. | Сүлжээний TL | ✅ Дууссан | Torii одоо засаглалын дугтуйг (`torii.sorafs.admission_envelopes_dir`) ачаалж, залгих явцад хураангуй/гарын үсэг тохирч байгааг шалгаж, хүлээн авалтыг илрүүлдэг. телеметр.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.
| Шинэчлэл | Шинэчлэх / хүчингүй болгох схем + CLI туслахуудыг нэмж, амьдралын мөчлөгийн удирдамжийг баримт бичигт нийтлэх (доорх runbook болон CLI тушаалуудыг үзнэ үү. `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md: |120 Хадгалах / Засаглал | ✅ Дууссан |
| Телеметрийн | `provider_admission` хяналтын самбар ба сэрэмжлүүлгийг (сунгалт байхгүй, дугтуйны хугацаа дууссан) тодорхойлно уу. | Ажиглалт | 🟠 Явж байна | `torii_sorafs_admission_total{result,reason}` тоолуур байгаа; хяналтын самбар/сэрэмжлүүлэг хүлээгдэж байна.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Шинэчлэх, хүчингүй болгох Runbook

#### Хуваарьт сунгалт (гадасны/топологийн шинэчлэл)
1. `provider-admission proposal` болон `provider-admission sign`-ээр залгамжлагч санал/зар сурталчилгааны хослолыг бүтээж, `--retention-epoch`-ийг нэмэгдүүлж, бооцоо/төгсгөлийн цэгүүдийг шаардлагатай бол шинэчилнэ үү.
2. Гүйцэтгэх  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Тус тушаал нь өөрчлөгдөөгүй чадвар/профайлын талбаруудыг дамжуулан баталгаажуулдаг
   `AdmissionRecord::apply_renewal`, `ProviderAdmissionRenewalV1` ялгаруулж, дижестийг хэвлэдэг.
   засаглалын бүртгэл.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Өмнөх дугтуйг `torii.sorafs.admission_envelopes_dir`-д сольж, Norito/JSON шинэчлэлтийг засаглалын репозиторт хийж, `docs/source/sorafs/migration_ledger.md`-д шинэчлэх хэш + хадгалах үеийг хавсаргана уу.
4. Операторуудад шинэ дугтуй ажиллаж байгааг мэдэгдэж, залгисан эсэхийг баталгаажуулахын тулд `torii_sorafs_admission_total{result="accepted",reason="stored"}`-д хяналт тавина.
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`-ээр дамжуулан каноник бэхэлгээг сэргээн засварлах; CI (`ci/check_sorafs_fixtures.sh`) нь Norito гаралтыг тогтвортой байлгахыг баталгаажуулдаг.

#### Яаралтай хүчингүй болгох
1. Эвдэрсэн дугтуйг тодорхойлж, хүчингүй болгох:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI нь `ProviderAdmissionRevocationV1`-д гарын үсэг зурж, гарын үсгийн багцыг баталгаажуулна.
   `verify_revocation_signatures`, мөн хүчингүй болгох тоймыг мэдээлдэг.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#48
2. `torii.sorafs.admission_envelopes_dir`-аас дугтуйг авч, Norito/JSON-г хүчингүй болгосныг хүлээн авах кэш рүү тарааж, шалтгааны хэшийг удирдлагын протоколд тэмдэглэнэ үү.
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`-г үзэхийн тулд кэш нь хүчингүй болсон зарыг устгаж байгааг баталгаажуулах; хүчингүй болгох олдворуудыг тохиолдлын эргэн тойронд байлгах.

## Туршилт ба телеметр- Элсэлтийн саналд зориулсан алтан бэхэлгээ, дугтуйг доор нэмнэ үү
  `fixtures/sorafs_manifest/provider_admission/`.
- Саналыг сэргээх, дугтуйг шалгахын тулд CI (`ci/check_sorafs_fixtures.sh`) сунгана.
- Үүсгэсэн бэхэлгээнд `metadata.json` каноник боловсруулалт орно; доод туршилтууд баталж байна
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Интеграцийн тестүүдийг өгөх:
  - Torii дугтуй дутуу эсвэл хугацаа нь дууссан зар сурталчилгаанаас татгалздаг.
  - CLI нь санал → дугтуй → баталгаажуулалт.
  - Засаглалын шинэчлэл нь үйлчилгээ үзүүлэгчийн ID-г өөрчлөхгүйгээр эцсийн цэгийн гэрчилгээг эргүүлнэ.
- Телеметрийн шаардлага:
  - `provider_admission_envelope_{accepted,rejected}` тоолуурыг Torii дээр ялгаруулна. ✅ `torii_sorafs_admission_total{result,reason}` одоо хүлээн зөвшөөрөгдсөн/татгалзсан үр дүнг харуулж байна.
  - Ажиглалтын хяналтын самбарт хугацаа дуусах тухай анхааруулгыг нэмэх (сунгалтыг 7 хоногийн дотор хийх).

## Дараагийн алхамууд

1. ✅ Norito схемийн өөрчлөлтийг дуусгаж, баталгаажуулалтын туслахуудыг
   `sorafs_manifest::provider_admission`. Онцлогын туг шаардлагагүй.
2. ✅ CLI ажлын урсгалыг (`proposal`, `sign`, `verify`, `renewal`, `revoke`) нэгтгэх туршилтаар баримтжуулж, хэрэгжүүлдэг; засаглалын скриптүүдийг runbook-тэй синхрон байлгах.
3. ✅ Torii элсэлт/нээлт нь дугтуйг залгиж, телеметрийн тоолуурыг хүлээн авах/татгалзах зорилгоор ил гаргана.
4. Ажиглалтад анхаарлаа хандуулаарай: элсэлтийн хяналтын самбар/сануулгыг дуусгахын тулд долоо хоногийн дотор сунгалт хийх нь анхааруулга (`torii_sorafs_admission_total`, хугацаа дуусах хэмжигч) болно.