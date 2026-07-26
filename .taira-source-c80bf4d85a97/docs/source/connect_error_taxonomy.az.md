---
lang: az
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Connect Xətası Taksonomiyası (Swift bazası)

Bu qeyd IOS-CONNECT-010-u izləyir və paylaşılan səhv taksonomiyasını sənədləşdirir
Nexus SDK-ları birləşdirin. Swift indi kanonik `ConnectError` sarğısını ixrac edir,
bütün Connect ilə əlaqəli uğursuzluqları altı kateqoriyadan birinə uyğunlaşdıran telemetriya,
idarə panelləri və UX surəti platformalar arasında uyğunlaşdırılır.

> Son yenilənmə: 2026-01-15  
> Sahib: Swift SDK Rəhbəri (taksonomiya stüardı)  
> Vəziyyət: Swift + Android + JavaScript tətbiqləri **endi**; növbə inteqrasiyası gözlənilir.

## Kateqoriyalar

| Kateqoriya | Məqsəd | Tipik mənbələr |
|----------|---------|-----------------|
| `transport` | Sessiyanı dayandıran WebSocket/HTTP nəqliyyat xətaları. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Çərçivələri kodlaşdırarkən/deşifrə edərkən seriyalaşdırma/körpü uğursuzluqları. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | İstifadəçi və ya operatorun düzəldilməsini tələb edən TLS/attestasiya/siyasət xətaları. | `URLError(.secureConnectionFailed)`, Torii 4xx cavabları |
| `timeout` | Boş/oflayn müddətlər və gözətçilər (TTL növbəsi, sorğu vaxtı). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO arxa təzyiq siqnalları verir ki, proqramlar zərif şəkildə yüklənsin. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Qalan hər şey: SDK-dan sui-istifadə, itkin Norito körpüsü, zədələnmiş jurnallar. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Hər bir SDK taksonomiyaya uyğun gələn və ifşa edən xəta növünü dərc edir
strukturlaşdırılmış telemetriya atributları: `category`, `code`, `fatal` və isteğe bağlı
metadata (`http_status`, `underlying`).

## Sürətli xəritəçəkmə

Swift `ConnectError`, `ConnectErrorCategory` və köməkçi protokolları ixrac edir
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Bütün ictimai Bağlantı xətası
növlər `ConnectErrorConvertible`-ə uyğundur, beləliklə proqramlar `error.asConnectError()`-ə zəng edə bilər
və nəticəni telemetriya/logging təbəqələrinə yönləndirin.| Swift xətası | Kateqoriya | Kod | Qeydlər |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | İkiqat `start()` göstərir; developer səhvi. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Bağlandıqdan sonra göndərmə/qəbul edərkən yüksəldi. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket binar gözləyərkən mətn yükünü çatdırdı. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Qarşı tərəf gözlənilmədən axını bağladı. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Proqram simmetrik düymələri konfiqurasiya etməyi unutdu. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito faydalı yükdə tələb olunan sahələr yoxdur. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Köhnə SDK tərəfindən görülən gələcək faydalı yük. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito körpüsü çatışmır və ya çərçivə baytlarını kodlaşdırmaq/deşifrə etmək alınmadı. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Körpü əlçatan deyil və ya açar uzunluqları uyğun gəlmir. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Oflayn növbə uzunluğu konfiqurasiya edilmiş həddi keçdi. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` ilə örtülmüşdür. |
| `URLError` TLS halları | `authorization` | `network.tls_failure` | ATS/TLS danışıqlarında uğursuzluqlar. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | SDK-nın başqa yerində JSON deşifrəsi/kodlanması uğursuz oldu; mesaj Swift dekoder kontekstindən istifadə edir. |
| Hər hansı digər `Error` | `internal` | `unknown_error` | Zəmanətli tutma; mesaj güzgüləri `LocalizedError`. |

Vahid testləri (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) kilidlənir
Xəritəçəkmə beləliklə gələcək refaktorlar kateqoriyaları və ya kodları səssizcə dəyişə bilməz.

### İstifadə nümunəsi

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Telemetriya və idarə panelləri

Swift SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)` təmin edir
kanonik atribut xəritəsini qaytarır. İxracatçılar bunları çatdırmalıdırlar
Əlavə əlavələrlə `connect.error` OTEL tədbirlərinə atributlar:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Tablolar `connect.error` sayğaclarını növbə dərinliyi ilə əlaqələndirir (`connect.queue_depth`)
və qeydləri qeyd etmədən reqressiyaları aşkar etmək üçün histoqramları yenidən birləşdirin.

## Android xəritələşdirilməsiAndroid SDK ixrac edir `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` və köməkçi kommunal xidmətlər
`org.hyperledger.iroha.android.connect.error`. Qurucu tipli köməkçilər istənilən `Throwable`-i çevirirlər
taksonomiyaya uyğun faydalı yükə daxil olmaq, nəqliyyat/TLS/kodek istisnalarından kateqoriyalar çıxarmaq,
və deterministik telemetriya atributlarını ifşa edin ki, OpenTelemetry/sempling stacks istehlak edə bilsin.
sifarişsiz nəticə adapterlər.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.jav a:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` artıq `ConnectErrorConvertible`-i həyata keçirir, növbəOverflow/tamaaşını verir
daşqın/keçmə şərtləri üçün kateqoriyalar, beləliklə oflayn növbə alətləri eyni axına qoşula bilər.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README indi taksonomiyaya istinad edir və nəqliyyat istisnalarının necə bağlanacağını göstərir
telemetriya yaymazdan əvvəl dApp təlimatını Swift bazası ilə uyğunlaşdırın.【java/iroha_android/README.md:167】

## JavaScript xəritəsi

Node.js/brauzer müştəriləri `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` və
`connectErrorFrom()`, `@iroha/iroha-js`. Paylaşılan köməkçi HTTP status kodlarını yoxlayır,
Node xəta kodları (socket, TLS, timeout), `DOMException` adları və kodek xətaları
bu qeyddə sənədləşdirilmiş eyni kateqoriyalar/kodlar, TypeScript tərifləri isə telemetriyanı modelləşdirir
atribut ləğv edir, beləliklə alətlər əl ilə yayımlamadan OTEL hadisələrini yaysın.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README iş prosesini sənədləşdirir və bu taksonomiyaya yenidən keçid verir ki, proqram qrupları bunu bacarsın
alət parçalarını hərfi kopyalayın.【javascript/iroha_js/README.md:1387】

## Növbəti addımlar (SDK arası)

- **Növbə inteqrasiyası:** oflayn növbə göndərildikdən sonra sıradan çıxma/düşmə məntiqini təmin edin
  səthlər `ConnectQueueError` dəyərləri belə daşqın Telemetriya etibarlı olaraq qalır.