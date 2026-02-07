---
lang: ka
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge გამოშვების შეფუთვა

ეს გზამკვლევი ასახავს `NoritoBridge` Swift საკინძების გამოსაქვეყნებლად საჭირო ნაბიჯებს, როგორც
XCFramework, რომლის მოხმარება შესაძლებელია Swift Package Manager-ისა და CocoaPods-ისგან. The
სამუშაო პროცესი ინარჩუნებს Swift-ის არტეფაქტებს დაბლოკვის ეტაპში Rust crate-ის გამოშვებით, რომ გემი
Iroha-ის Norito კოდეკი. გამოქვეყნებულის მოხმარების შესახებ ბოლომდე ინსტრუქციებისთვის
არტეფაქტები აპის შიგნით (Xcode პროექტის გაყვანილობა, ChaChaPoly-ის გამოყენება და ა.შ.), იხ
`docs/connect_swift_integration.md`.

> **შენიშვნა:** CI ავტომატიზაცია ამ ნაკადისთვის დაეშვება მას შემდეგ, რაც macOS შემქმნელები საჭიროებენ
> Apple tooling შემოდის ონლაინ (თვალს ადევნებთ Release Engineering macOS Builder-ის ჩამორჩენას).
> მანამდე ქვემოთ მოცემული ნაბიჯები ხელით უნდა შესრულდეს განვითარების Mac-ზე.

## წინაპირობები

- macOS ჰოსტი უახლესი სტაბილური Xcode ბრძანების ხაზის ხელსაწყოებით დაინსტალირებული.
- Rust ხელსაწყოების ჯაჭვი, რომელიც შეესაბამება სამუშაო სივრცეს `rust-toolchain.toml`.
- Swift toolchain 5.7 ან უფრო ახალი.
- CocoaPods (Ruby ძვირფასი ქვების საშუალებით) თუ გამოქვეყნდება ტექნიკური მახასიათებლების ცენტრალურ საცავში.
- წვდომა Hyperledger Iroha გამოშვების ხელმოწერის გასაღებებზე Swift-ის არტეფაქტების მონიშვნისთვის.

## ვერსიის მოდელი

1. განსაზღვრეთ Rust crate ვერსია Norito კოდეკისთვის (`crates/norito/Cargo.toml`).
2. მონიშნეთ სამუშაო ადგილი გამოშვების იდენტიფიკატორით (მაგ. `v2.1.0`).
3. გამოიყენეთ იგივე სემანტიკური ვერსია Swift პაკეტისთვის და CocoaPods podspec-ისთვის.
4. როდესაც Rust crate გაზრდის თავის ვერსიას, გაიმეორეთ პროცესი და გამოაქვეყნეთ შესატყვისი
   სვიფტის არტეფაქტი. ვერსიები შეიძლება შეიცავდეს მეტამონაცემების სუფიქსებს (მაგ. `-alpha.1`) ტესტირებისას.

## ნაბიჯების აშენება

1. საცავის ფესვიდან, გამოიძახეთ დამხმარე სკრიპტი XCFramework-ის ასაწყობად:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   სკრიპტი აგროვებს Rust bridge ბიბლიოთეკას iOS და macOS მიზნებისთვის და აერთიანებს მათ
   შედეგად მიღებული სტატიკური ბიბლიოთეკები ერთი XCFramework დირექტორიაში.
   ის ასევე ასხივებს `dist/NoritoBridge.artifacts.json`, იჭერს ხიდის ვერსიას და
   SHA-256 ჰეშების თითო პლატფორმაზე (გადალახეთ ვერსია `NORITO_BRIDGE_VERSION`-ით, თუ
   საჭიროა).

2. ჩაწერეთ XCFramework განაწილებისთვის:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. განაახლეთ Swift პაკეტის მანიფესტი (`IrohaSwift/Package.swift`), რათა მიუთითოთ ახალი
   ვერსია და საკონტროლო ჯამი:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   ჩაწერეთ საკონტროლო ჯამი `Package.swift`-ში ბინარული სამიზნის განსაზღვრისას.

4. განაახლეთ `IrohaSwift/IrohaSwift.podspec` ახალი ვერსიით, გამშვები ჯამით და არქივით
   URL.

5. ** სათაურების რეგენერაცია, თუ ხიდმა მოიპოვა ახალი ექსპორტი. ** Swift ხიდი ახლა ამჟღავნებს
   `connect_norito_set_acceleration_config` ასე რომ, `AccelerationSettings`-ს შეუძლია გადართოს Metal /
   GPU backends. დარწმუნდით `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   ემთხვევა `crates/connect_norito_bridge/include/connect_norito_bridge.h` zipping-მდე.

6. გაუშვით Swift-ის ვალიდაციის კომპლექტი მონიშვნამდე:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   პირველი ბრძანება უზრუნველყოფს Swift პაკეტის (მათ შორის `AccelerationSettings`) დარჩენას
   მწვანე; მეორე ამოწმებს მოწყობილობების პარიტეტს, ასახავს პარიტეტის/CI დაფებს და
   ახორციელებს იგივე ტელემეტრიის შემოწმებებს Buildkite-ში (მათ შორის
   `ci/xcframework-smoke:<lane>:device_tag` მეტამონაცემების მოთხოვნა).

7. გამოუშვით გენერირებული არტეფაქტები გამოშვების ფილიალში და მონიშნეთ commit.

## გამომცემლობა

### Swift პაკეტის მენეჯერი

- მიიტანეთ ტეგი საჯარო Git საცავში.
- დარწმუნდით, რომ ტეგი ხელმისაწვდომია პაკეტის ინდექსით (Apple ან Community Mirror).
- მომხმარებელს ახლა შეუძლია დამოკიდებული იყოს `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`-ზე.

### კაკაოს ფხვნილები

1. დაადასტურეთ პოდი ლოკალურად:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. დააჭირეთ განახლებულ პოდსპეკტს:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. დაადასტურეთ, რომ ახალი ვერსია გამოჩნდება CocoaPods ინდექსში.

## CI მოსაზრებები

- შექმენით macOS სამუშაო, რომელიც აწარმოებს შეფუთვის სკრიპტს, დაარქივებს არტეფაქტებს და ატვირთავს
  გენერირებული საკონტროლო ჯამი, როგორც სამუშაო პროცესის გამომავალი.
- Gate გამოშვებულია Swift-ის დემო აპლიკაციის შენობაში ახლად წარმოებული ჩარჩოს წინააღმდეგ.
- შეინახეთ Build ჟურნალები შეცდომების დიაგნოსტიკაში დასახმარებლად.

## დამატებითი ავტომატიზაციის იდეები

- გამოიყენეთ `xcodebuild -create-xcframework` პირდაპირ მას შემდეგ, რაც ყველა საჭირო სამიზნე გამოაშკარავდება.
- ხელმოწერის/ნოტარიზაციის ინტეგრირება დეველოპერის მანქანების გარეთ გავრცელებისთვის.
- შეინახეთ ინტეგრაციის ტესტები დაბლოკვის ეტაპად შეფუთულ ვერსიასთან SPM-ის ჩამაგრებით
  დამოკიდებულება გამოშვების ტეგზე.