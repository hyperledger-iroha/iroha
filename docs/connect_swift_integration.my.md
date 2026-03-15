---
lang: my
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit ကို Xcode iOS ပရောဂျက်တစ်ခုတွင် ပေါင်းစပ်ခြင်း။

ဤလမ်းညွှန်ချက်တွင် Rust Norito တံတား (XCFramework) နှင့် Swift wrappers များကို iOS အက်ပ်တစ်ခုသို့ ပေါင်းစည်းရန်၊ ထို့နောက် Iroha Connect frames များကို Rust host အဖြစ် တူညီသော Norito ကိုအသုံးပြု၍ WebSocket ပေါ်ရှိ ဖရိန်များကို လဲလှယ်ပုံကို ပြသထားသည်။

လိုအပ်ချက်များ
- NoritoBridge.xcframework zip (CI workflow ဖြင့်တည်ဆောက်ထားသော) နှင့် Swift helper `NoritoBridgeKit.swift` (သရုပ်ပြပရောဂျက်ကို တိုက်ရိုက်မသုံးစွဲပါက `examples/ios/NoritoDemo/Sources` အောက်တွင် ဗားရှင်းကို ကူးယူပါ။
- Xcode 15+၊ iOS 13+ ပစ်မှတ်။

ရွေးချယ်စရာ A- Swift Package Manager (အကြံပြုထားသည်)
1) `crates/connect_norito_bridge/` တွင် `Package.swift.template` ကို အသုံးပြု၍ Binary SPM ထုတ်ဝေပါ (URL ဖြည့်ပြီး CI မှ checksum)။
2) Xcode တွင်- File → Add Packages… → SPM repo URL ကို ရိုက်ထည့်ပါ → `NoritoBridge` ထုတ်ကုန်ကို သင့်ပစ်မှတ်သို့ ထည့်ပါ။
3) `NoritoBridgeKit.swift` ကို သင့်အက်ပ်ပစ်မှတ်သို့ ပေါင်းထည့်ပါ (သင့်ပရောဂျက်သို့ ဆွဲယူပါ၊ “လိုအပ်ပါက ကော်ပီ” ကို အမှတ်ခြစ်ထားကြောင်း သေချာပါစေ။

ရွေးချယ်မှု B- CocoaPods
1) `NoritoBridge.podspec.template` မှ Podspec တစ်ခုဖန်တီးပါ (`s.source` ဇစ် URL ကိုဖြည့်ပါ)။
2) `pod trunk push NoritoBridge.podspec`။
3) သင်၏ Podfile တွင်- `pod 'NoritoBridge'` → `pod install`။
4) `NoritoBridgeKit.swift` ကို သင့်အက်ပ်ပစ်မှတ်သို့ ထည့်ပါ။

သွင်းကုန်
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Connect session တစ်ခုကို Bootstrapping လုပ်ခြင်း။

`ConnectClient` သည် WebSocket ကို ကိုင်တွယ်ပြီး `ConnectSession` က ကြိုးကိုင်ပေးသည် ။
ဘောင်များနှင့် စာဝှက်စာသား စာအိတ်များ။ အောက်ပါအတိုအထွာသည် dApp သည် စက်ရှင်တစ်ခုအား မည်သို့ဖွင့်မည်ကို ပြသသည်၊
ချိတ်ဆက်ကီးများကို ရယူပြီး အတည်ပြုချက် တုံ့ပြန်ချက်ကို စောင့်ပါ။

```swift
let connectURL = URL(string: "wss://node.example/v2/connect/ws?sid=\(sidB64)&role=app")!
var connectRequest = URLRequest(url: connectURL)
connectRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
let connectClient = ConnectClient(request: connectRequest)
let sessionID = Data(base64Encoded: sidB64)!

Task {
    await connectClient.start()

    let keyPair = try ConnectCrypto.generateKeyPair()
    var connectSession = ConnectSession(sessionID: sessionID, client: connectClient)

    let open = ConnectOpen(
        appPublicKey: keyPair.publicKey,
        appMetadata: ConnectAppMetadata(name: "Demo dApp", iconURL: nil, description: nil),
        constraints: ConnectConstraints(chainID: "00000000-0000-0000-0000-000000000000"),
        permissions: ConnectPermissions(methods: ["sign"], events: [])
    )
    try await connectSession.sendOpen(open: open)

    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(localPrivateKey: keyPair.privateKey,
                                                                  peerPublicKey: approval.walletPublicKey,
                                                                  sessionID: sessionID)
        connectSession.setDirectionKeys(directionKeys)
        // Ready to decrypt ciphertext envelopes
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### စာဝှက်စာသားဘောင်များ ပေးပို့ခြင်း (လက်မှတ်ထိုးတောင်းဆိုမှုများ၊ စသည်ဖြင့်)

dApp သည် လက်မှတ်တစ်ခုတောင်းဆိုရန် လိုအပ်သောအခါတွင် ၎င်းသည် ကုဒ်ဝှက်ရန် Norito တံတားအကူများကို အသုံးပြုသည်။
စာအိတ်တစ်ခုသည် ChaChaPoly ဖြင့် payload ကို စာဝှက်ပြီး `ConnectFrame` ဖြင့် ထုပ်ပိုးထားသည်။

```swift
let bridge = NoritoBridgeKit()
let seq = nextSequence()
let txBytes = Data([0x01, 0x02, 0x03])
let envelope = try bridge.encodeEnvelopeSignRequestTx(sequence: seq, txBytes: txBytes)

let aad = ConnectAEAD.header(sessionID: sessionID, direction: .appToWallet, sequence: seq)
let nonce = ConnectAEAD.nonce(sequence: seq)
let ciphertext = try ChaChaPoly.seal(envelope,
                                     using: SymmetricKey(data: directionKeys.appToWallet),
                                     nonce: nonce,
                                     authenticating: aad).combined

let frame = ConnectFrame(sessionID: sessionID,
                         direction: .appToWallet,
                         sequence: seq,
                         kind: .ciphertext(ConnectCiphertext(payload: ciphertext)))
try await connectClient.send(frame: frame)
```

`ConnectAEAD.header` / `ConnectAEAD.nonce` များသည် အဆင်ပြေစေရန် ကူညီသူများဖြစ်သည် (အတိုအထွာကို တွင်ကြည့်ပါ
`docs/connect_swift_ios.md`) မျှဝေထားသော `connect:v1` ခေါင်းစီးအဓိပ္ပါယ်မှ ဖန်တီးထားသည်။ သူတို
အခြား utility ကိုမထည့်လိုပါက inline လုပ်ရန်လွယ်ကူသည်-

```swift
enum ConnectAEAD {
    static func header(sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> Data {
        var buffer = Data()
        buffer.append("connect:v1".data(using: .utf8)!)
        buffer.append(sessionID)
        buffer.append(direction == .appToWallet ? 0 : 1)
        var seq = sequence.littleEndian
        withUnsafeBytes(of: &seq) { buffer.append(contentsOf: $0) }
        buffer.append(1) // Ciphertext kind
        return buffer
    }

    static func nonce(sequence: UInt64) -> ChaChaPoly.Nonce {
        var bytes = Data(count: 12)
        var seq = sequence.littleEndian
        bytes.replaceSubrange(4..<12, with: withUnsafeBytes(of: &seq) { Data($0) })
        return try! ChaChaPoly.Nonce(data: bytes)
    }
}
```

### ဘောင်များကို လက်ခံခြင်း / စာဝှက်ခြင်း။

`ConnectSession` သည် `nextEnvelope()` ကို ဖော်ထုတ်ပြီးဖြစ်သည်၊
သော့များကို စီစဉ်သတ်မှတ်ထားသည်။ သင်ကိုယ်တိုင်ဝင်ရောက်ခွင့် လိုအပ်ပါက (ဥပမာ၊ ရှိပြီးသား ဒီကုဒ်ဒါနှင့် ကိုက်ညီရန်
ပိုက်လိုင်း)၊ အောက်ခြေအဆင့် အကူအညီကို ခေါ်ဆိုနိုင်သည်-

```swift
func decryptFrame(_ frame: ConnectFrame,
                  symmetricKey: SymmetricKey,
                  sessionID: Data) throws -> ConnectEnvelope {
    guard case .ciphertext(let payload) = frame.kind else {
        throw ConnectEnvelopeError.unsupportedFrameKind
    }
    let aad = ConnectAEAD.header(sessionID: sessionID,
                                 direction: frame.direction,
                                 sequence: frame.sequence)
    let nonce = ConnectAEAD.nonce(sequence: frame.sequence)
    let box = try ChaChaPoly.SealedBox(combined: payload.payload)
    let plaintext = try ChaChaPoly.open(box, using: symmetricKey, authenticating: aad)
    return try ConnectEnvelope.decode(jsonData: plaintext)
}
```

`NoritoBridgeKit` သည် `decodeCiphertextFrame`၊ `decodeEnvelopeJson`၊
အမှားရှာပြင်ခြင်း သို့မဟုတ် အပြန်အလှန်လုပ်ဆောင်နိုင်စွမ်းစမ်းသပ်ခြင်းအတွက် `decodeSignResultAlgorithm`။ ထုတ်လုပ်မှုအတွက်
အက်ပ်များ၊ `ConnectSession` နှင့် `ConnectEnvelope` ကို အားကိုးပါ၊ ထို့ကြောင့် အပြုအမူသည် Rust နှင့် ကိုက်ညီပါသည်။
Android SDKs အတိအကျ။

## CI အတည်ပြုခြင်း။

- အပ်ဒိတ်လုပ်ထားသော တံတားအရုပ်များကို မထုတ်ဝေမီ သို့မဟုတ် Connect ပေါင်းစည်းမှုများကို တွန်းအားပေးခြင်းမပြုမီ၊ လုပ်ဆောင်ပါ။

  ```bash
  make swift-ci
  ```

  ပစ်မှတ်သည် fixture parity ကိုအတည်ပြုပြီး dashboard feeds ကိုစစ်ဆေးပြီး CLI ကိုပြန်ဆိုသည်
  ဒေသအလိုက် အကျဉ်းချုပ်များ။ Buildkite တွင် တူညီသော အလုပ်အသွားအလာသည် ထိုကဲ့သို့သော မက်တာဒေတာကီးများပေါ်တွင် မူတည်သည်။
  `ci/xcframework-smoke:<lane>:device_tag`; တည်းဖြတ်ပြီးနောက် မက်တာဒေတာ ရှိနေကြောင်း အတည်ပြုပါ။
  ပိုက်လိုင်းများ သို့မဟုတ် အေးဂျင့်တက်ဂ်များ ဒိုင်ခွက်များသည် ရလဒ်များကို မှန်ကန်သော simulator သို့မဟုတ် မှန်ကန်သော အမှတ်အသားပြုနိုင်စေရန်
  StrongBox လမ်းသွား။
- အမိန့်မချပါက၊ parity playbook (`docs/source/swift_parity_triage.md`) ကို လိုက်နာပါ။
  မည်သည့်လမ်းကြောကို ပြန်လည်ပြုပြင်ရန် လိုအပ်သည်ကို ခွဲခြားသိရှိနိုင်ရန် ပြန်ဆိုထားသော `mobile_ci` ထုတ်ပေးမှုကို စစ်ဆေးပါ။
  သို့မဟုတ် ထပ်မစမ်းမီ အဖြစ်အပျက် နောက်ဆက်တွဲ။