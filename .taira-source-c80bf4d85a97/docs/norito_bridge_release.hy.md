---
lang: hy
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge Release Packaging

Այս ուղեցույցը նախանշում է `NoritoBridge` Swift կապերը հրապարակելու համար անհրաժեշտ քայլերը որպես
XCFramework, որը կարելի է օգտագործել Swift Package Manager-ից և CocoaPods-ից: Այն
Աշխատանքային ընթացքը Swift-ի արտեֆակտները պահում է կողպեքի մեջ՝ Rust crate-ը թողարկում է այդ նավը
Iroha-ի Norito կոդեկ: Հրապարակված սպառման վերաբերյալ վերջնական հրահանգների համար
արտեֆակտներ հավելվածի ներսում (Xcode նախագծի միացում, ChaChaPoly-ի օգտագործում և այլն), տես
`docs/connect_swift_integration.md`.

> **Նշում․
> Apple-ի գործիքակազմը հասանելի է առցանց (հետագծվում է Release Engineering macOS շինարարների հետքաղքում):
> Մինչ այդ ստորև նշված քայլերը պետք է ձեռքով կատարվեն մշակող Mac-ում:

## Նախադրյալներ

- MacOS հոսթ՝ տեղադրված վերջին կայուն Xcode հրամանի տող գործիքներով:
- Rust գործիքների շղթա, որը համապատասխանում է `rust-toolchain.toml` աշխատանքային տարածքին:
- Swift Toolchain 5.7 կամ ավելի նոր տարբերակ:
- CocoaPods (Ruby gems-ի միջոցով), եթե հրապարակվում է կենտրոնական ակնոցների պահեստում:
- Մուտք գործեք Hyperledger Iroha թողարկման ստորագրման բանալիներ՝ Swift արտեֆակտները պիտակավորելու համար:

## Տարբերակման մոդել

1. Որոշեք Rust crate տարբերակը Norito կոդեկի համար (`crates/norito/Cargo.toml`):
2. Նշեք աշխատանքային տարածքը թողարկման նույնացուցիչով (օրինակ՝ `v2.1.0`):
3. Օգտագործեք նույն իմաստային տարբերակը Swift փաթեթի և CocoaPods podspec-ի համար:
4. Երբ Rust crate-ն ավելացնում է իր տարբերակը, կրկնեք գործընթացը և հրապարակեք համապատասխանությունը
   Swift artifact. Փորձարկման ընթացքում տարբերակները կարող են ներառել մետատվյալների վերջածանցներ (օրինակ՝ `-alpha.1`):

## Կառուցեք քայլեր

1. Պահեստի արմատից կանչեք օգնական սկրիպտը՝ XCFramework-ը հավաքելու համար.

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Սցենարը կազմում է Rust bridge գրադարանը iOS-ի և macOS-ի թիրախների համար և փաթեթավորում է դրանք
   ստացված ստատիկ գրադարաններ մեկ XCFramework գրացուցակի տակ:
   Այն նաև արտանետում է `dist/NoritoBridge.artifacts.json`՝ գրավելով կամուրջի տարբերակը և
   մեկ հարթակի համար SHA-256 հեշեր (շրջանցեք տարբերակը `--bridge-version <version>`-ով, եթե
   անհրաժեշտ է):

2. Տեղադրեք XCFramework-ը բաշխման համար.

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Թարմացրեք Swift փաթեթի մանիֆեստը (`IrohaSwift/Package.swift`)՝ մատնանշելու նորը
   տարբերակ և ստուգիչ գումար.

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Երկուական թիրախը որոշելիս գրանցեք ստուգման գումարը `Package.swift`-ում:

4. Թարմացրեք `IrohaSwift/IrohaSwift.podspec`-ը նոր տարբերակով, ստուգիչ գումարով և արխիվով
   URL.

5. **Վերականգնել վերնագրերը, եթե կամուրջը նոր արտահանումներ է ձեռք բերել:** Swift կամուրջն այժմ բացահայտում է
   `connect_norito_set_acceleration_config`, որպեսզի `AccelerationSettings`-ը կարողանա փոխարկել Metal /
   GPU backends. Ապահովել `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   համընկնում է `crates/connect_norito_bridge/include/connect_norito_bridge.h`-ից առաջ zipping-ը:

6. Գործարկեք Swift վավերացման փաթեթը՝ նախքան պիտակավորելը.

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   Առաջին հրամանը ապահովում է Swift փաթեթը (ներառյալ `AccelerationSettings`) մնալը
   կանաչ; երկրորդը հաստատում է հարմարանքների հավասարությունը, ներկայացնում է հավասարության/CI վահանակները և
   իրականացնում է նույն հեռաչափական ստուգումները, որոնք կիրառվում են Buildkite-ում (ներառյալ
   `ci/xcframework-smoke:<lane>:device_tag` մետատվյալների պահանջ):

7. Ստեղծեք արտեֆակտները թողարկվող ճյուղում և նշեք commit-ը:

## Հրատարակչություն

### Swift փաթեթի կառավարիչ

- Հրել պիտակը հանրային Git պահոց:
- Համոզվեք, որ պիտակը հասանելի է փաթեթի ինդեքսով (Apple կամ համայնքի հայելի):
- Սպառողները այժմ կարող են կախված լինել `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`-ից:

### CocoaPods

1. Վավերացրեք պատիճը տեղական մակարդակում՝

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Հպեք թարմացված podspec-ը.

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Հաստատեք, որ նոր տարբերակը հայտնվում է CocoaPods ինդեքսում:

## CI նկատառումներ

- Ստեղծեք macOS աշխատանք, որը գործարկում է փաթեթավորման սցենարը, արխիվացնում է արտեֆակտները և վերբեռնում
  ստեղծվել է ստուգիչ գումար՝ որպես աշխատանքային հոսքի արդյունք:
- Դարպասը թողարկվում է Swift-ի ցուցադրական հավելվածի շենքի վրա՝ ընդդեմ նոր արտադրված շրջանակի:
- Պահպանեք շինարարական տեղեկամատյանները՝ ձախողումների ախտորոշմանը օգնելու համար:

## Լրացուցիչ ավտոմատացման գաղափարներ

- Օգտագործեք `xcodebuild -create-xcframework` անմիջապես, երբ բոլոր անհրաժեշտ թիրախները բացահայտվեն:
- Ինտեգրել ստորագրումը/նոտարական վավերացումը ծրագրավորող մեքենաներից դուրս բաշխման համար:
- Պահպանեք ինտեգրման թեստերը փաթեթավորված տարբերակի հետ կողպեքում՝ ամրացնելով SPM-ը
  կախվածություն թողարկման պիտակից: