---
lang: az
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Bu səhifə qısaldılmış tərcümə xülasəsidir, tam tərcümə deyil. İdarəetmə,
> API, sübut semantikası və buraxılış tələbləri üçün dəqiq normativ mənbə
> [ingiliscə əsas səhifədir](bridge_proofs.md).

# SCCP V1 körpü sübutları — qısa xülasə

## İlk buraxılışın sərhədi

SCCP V1 ilk buraxılış üçün qapalı protokoldur. Yalnız `ethereum-mainnet`,
`bsc-mainnet` və `tron-mainnet` xarici mənbələri dəstəklənir; yeganə SORA
təyinatı `sora-taira`dır. Solana, TON, xüsusi şəbəkələr və başqa SORA təyinatları
dəstəklənmir və təhlükəsiz şəkildə rədd edilir.

Bu buraxılışda `SubmitBridgeProof` yalnız tipli `NativeProtocol` və
`SccpDestination` sübutlarını qəbul edir. Ümumi `Ics` və `TransparentZk`
təqdimatı mövcud deyil və səlahiyyətli on-chain verifier yaradılanadək rədd
olunur.

## Tipli reyestr və replay müdafiəsi

`SccpRegistryV1` lane-ə bağlanmış, tipli və yalnız əlavə olunan (append-only)
reyestrdir. Hər lane ən çox 64 saxlanılan route revision və 4,096 saxlanılan
native trust anchor tutur. Tarixi qeydlər gizli şəkildə silinmir; həddə çatdıqda
növbəti əlavə vəziyyəti dəyişmədən atomik olaraq rədd edilir.

Anchor intervalı autentifikasiya olunmuş consensus irəliləyişi ilə ölçülür:
Ethereum finalized beacon slot-dan, BSC və TRON isə finalized native block
height-dan istifadə edir. Köhnə anchor varisinin checkpoint-i də daxil olmaqla
qüvvədə qalır; son cari anchor açıq sonludur. Terminal route-un finality cutoff-u
tarixi anchor-un varis checkpoint-i ilə dəqiq eyni olmalıdır.

Davamlı inbound qeydi həm event/source finality height-i, həm də təsdiqlənmiş
`anchor_interval_height`-ı saxlayır. Lane və anchor hash ilə açarlanan davamlı
high-water indeksi əvvəl qəbul edilmiş koordinatdan aşağı varis checkpoint
seçilməsinə imkan vermir. Snapshot hydration indeksi davamlı qeydlərdən yenidən
hesablayır və dəqiq bərabərlik tələb edir; çatışmayan, köhnəlmiş, zədələnmiş və ya
əsassız indeks rədd edilir. İstifadə edilmiş message id-lər replay-i dayandırmaq
üçün davamlı saxlanılır.

TRON mənbə route-u dəqiq
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI-sindən istifadə edir.
Uğurlu icra üçün `expectedNonce == transferNonce` olmalıdır; sonra storage
artırılmamışdan əvvəl həmin dəyər canonical payload-a yazılır. Native admission
tam ABI çağırışını payload recipient-i, miqyaslanmış məbləğ və nonce əsasında
yenidən qurur; buna görə köhnə iki-argumentli selector, stale və ya future nonce,
həmçinin tükənmiş `uint64` nonce təhlükəsiz şəkildə rədd edilir.

## Tək keçidli yoxlama və iş hədləri

Destination və native sübutları bir dəfə strukturlaşdırılır, bir dəfə bağlanır
və bahalı kriptoqrafiyadan əvvəl deterministik iş ehtiyatı ayrılır. Destination
yolu BN254 pairing-product və lokal BLS finality-ni hərəsini yalnız bir dəfə
yoxlayır. Native yollar canonical shortest-prefix tələb edir: BSC üçün ən çox
1,004, TRON üçün ən çox 54 header.

`[zk.sccp]` proof sayı/bytes, native headers/bytes, Ethereum light-client
updates, secp256k1 recoveries, BLS aggregate checks/key contributions və BN254
pairing checks üçün sıfırdan böyük transaction və block limitləri qoyur. Bu
qəbul limitləri consensus-bound-dur, bütün validatorlarda eyni config-file
dəyərləri olmalıdır və environment-variable alternativləri yoxdur.

İlk buraxılışın standart limitləri:

| İş ölçüsü | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

Bir proof ən çox 8 MiB canonical bytes daşıya bilər. Yarımçıq saxlanmış və ya
rədd edilmiş transaction üçün ayrılmış iş block-a sızmır.

## Outbound öhdəliyi, saxlanma və discovery

Hər uğurlu outbound message block execution order üzrə sıx `commitment_index`
(`0..=511`) alır. V1 üçün dəyişməz hədlər hər block-da 512 message və hər message-da
4,096 canonical payload byte-dır. `[zk.sccp]` pending payload state-i həm
`max_pending_outbound_messages` (default `65536`), həm də
`max_pending_outbound_payload_bytes` (default `268435456`) ilə məhdudlaşdırır.

Kura finality yayımlanmadan və ya block body silinmədən əvvəl dəqiq canonical header-i
və root-authenticated SCCP archive-i immutable saxlayır. Proof, bundle, proof request
və recent history bərpası tarixi block body-yə və ya mutable WSV payload nüsxəsinə
ehtiyac duymur. Destination proof qəbul ediləndə pending payload və onun charge-ı
atomically silinir, fixed terminal descriptor isə locator/index ilə qalır. Pending
state məhduddur; terminal records və immutable Kura history daimi replay müdafiəsi
üçün qəsdən artır. `GET /v1/sccp/messages/recent` mürəkkəb
`{ from, after_index }` cursor-dan istifadə edir. Immutable evidence total/operator
disk usage-a daxildir, lakin evictable-body budget-dən çıxarılıb.

## Torii və HTTP hədləri

Torii hər SCCP endpoint-i üçün JSON body həddini body oxunmadan, yaddaş
ayrılmadan və kriptoqrafik yoxlamadan əvvəl tətbiq edir. Böyük `Content-Length`
və ya chunked body HTTP `413` ilə rədd edilir. Müştəri açılmış HTTP cavabını da
sabit hədd daxilində oxuyur; çatışmayan və ya saxta `Content-Length` həddi keçə
bilməz.

JSON, base64 və Norito girişləri canonical olmalıdır. Naməlum fields, təkrar
keys, uyğun gəlməyən network/route/anchor, replay, iş kvotasının aşılması və ya
yoxlama xətası heç bir qismən vəziyyət dəyişikliyi etmədən rədd edilir.
