---
lang: my
direction: ltr
source: docs/portal/docs/sdks/recipes/java-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29669312b7e43caa320fb6710b78202069b8c9fb04a083f90af4a7d9468d4a51
source_last_modified: "2026-01-30T18:06:01.648526+00:00"
translation_last_reviewed: 2026-02-07
title: Java ledger flow recipe
description: Drive the register → mint → transfer demo using the IrohaAndroid JVM library.
slug: /sdks/recipes/java-ledger-flow
translator: machine-google-reviewed
---

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤစာရွက်သည် `java/iroha_android` module ၏ desktop/JVM မူကွဲကို ပစ်မှတ်ထားသည်။
၎င်းသည် ပိုင်ဆိုင်မှုအဓိပ္ပါယ်ဖွင့်ဆိုချက်တစ်ခုကို မှတ်ပုံတင်ခြင်းဖြင့် CLI ၏ လမ်းညွှန်ချက်အား ထင်ဟပ်စေသည်။
စီမံခန့်ခွဲသူအကောင့်၊ ဒုတိယအကောင့်တစ်ခုသို့ လွှဲပြောင်းကာ ရလဒ်ကို ပုံနှိပ်ခြင်း။
လက်ကျန်။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/java/src/main/java/ledger/LedgerFlow.java"
  filename="src/main/java/ledger/LedgerFlow.java"
  description="သင်၏ IDE သို့မဟုတ် ပရောဂျက် နမူနာထဲသို့ ထည့်သွင်းရန် Java နမူနာအပြည့်အစုံကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

## 1. ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် (CLI) ကို မှတ်ပုံတင်ပါ။

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. အထောက်အထားများကို ထုတ်ယူပါ။

```bash
# raw 32-byte Ed25519 private key in hex (without multicodec prefix)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

> သင်ဆိုလျှင် `iroha_cli tools crypto private-key export --raw --private-key <multihash>` ကိုသုံးပါ။
> default config မှ multihash prefix ကိုဖယ်ရှားရန်လိုအပ်သည်။

## 3. မှီခိုမှုထည့်ပါ။

```kts title="build.gradle.kts"
dependencies {
    implementation(project(":java:iroha_android"))
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1") // Ed25519 signer helper
}
```

## 4. ဥပမာ ပရိုဂရမ်

```java title="src/main/java/ledger/LedgerFlow.java"
package ledger;

import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.HexFormat;
import java.util.List;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.HttpClientTransport;
import org.hyperledger.iroha.android.client.RetryPolicy;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import java.util.Base64;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

public final class LedgerFlow {
  private LedgerFlow() {}

  public static void main(String[] args) throws Exception {
    final var env = System.getenv();
    final String adminAccount = env.get("ADMIN_ACCOUNT");
    final String receiverAccount = env.get("RECEIVER_ACCOUNT");
    final String privateKeyHex = env.get("ADMIN_PRIVATE_KEY_RAW");
    if (adminAccount == null || receiverAccount == null || privateKeyHex == null) {
      throw new IllegalStateException("Set ADMIN_ACCOUNT, RECEIVER_ACCOUNT, ADMIN_PRIVATE_KEY_RAW");
    }

    final Signer signer = signerFromHex(privateKeyHex);
    // Replace placeholders with Norito wire payload bytes for each instruction.
    final List<InstructionBox> instructions =
        List.of(
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_REGISTER_ASSET_DEFINITION>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_REGISTER_ASSET_DEFINITION>")),
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_MINT_ASSET>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_MINT_ASSET>")),
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_TRANSFER_ASSET>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_TRANSFER_ASSET>")));

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000000-0000-0000-0000-000000000000")
            .setAuthority(adminAccount)
            .setCreationTimeMs(System.currentTimeMillis())
            .setInstructions(instructions)
            .build();

    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(), IrohaKeyManager.withSoftwareFallback());
    final var signed = builder.encodeAndSign(payload, signer);

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://127.0.0.1:8080"))
            .setRequestTimeout(Duration.ofSeconds(10))
            .setRetryPolicy(RetryPolicy.none())
            .build();
    final var transport = new HttpClientTransport(config);
    final var response = transport.submitTransaction(signed).join();
    System.out.println("Submitted tx hash: " + response.hashHex().orElse("(pending)"));

    // Fetch the receiver’s balances via the Torii JSON API.
    final HttpClient http = HttpClient.newHttpClient();
    final String encodedAccount = URLEncoder.encode(receiverAccount, StandardCharsets.UTF_8);
    final HttpRequest req =
        HttpRequest.newBuilder()
            .uri(URI.create("http://127.0.0.1:8080/v1/accounts/" + encodedAccount + "/assets"))
            .GET()
            .build();
    final HttpResponse<String> assets =
        http.send(req, HttpResponse.BodyHandlers.ofString());
    System.out.println("Receiver balances: " + assets.body());
  }

  private static Signer signerFromHex(String privateHex) {
    final byte[] seed = HexFormat.of().parseHex(privateHex);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seed, 0);
    final Ed25519PublicKeyParameters publicKey = privateKey.generatePublicKey();
    return new Signer() {
      @Override
      public byte[] sign(byte[] message) {
        final Ed25519Signer signer = new Ed25519Signer();
        signer.init(true, privateKey);
        signer.update(message, 0, message.length);
        return signer.generateSignature();
      }

      @Override
      public byte[] publicKey() {
        return publicKey.getEncoded();
      }

      @Override
      public String algorithm() {
        return "Ed25519";
      }
    };
  }

  private static String assetId(String assetDefinition, String accountId) {
    final int idx = assetDefinition.indexOf('#');
    if (idx < 0) {
      return assetDefinition + "#" + accountId;
    }
    final String defName = assetDefinition.substring(0, idx);
    final String defDomain = assetDefinition.substring(idx + 1);
    return defName + "#" + defDomain + "#" + accountId;
  }
}
```

သင်နှစ်သက်သော တည်ဆောက်ရေးကိရိယာ (`./gradlew :java:iroha_android:assemble && ./gradlew run`) ဖြင့် စုစည်းပါ။

## 5. တန်းတူညီမျှမှုကို အတည်ပြုပါ။

- `iroha --config defaults/client.toml transaction get --hash <hash>` မှတစ်ဆင့် ငွေပေးငွေယူ hashe များကို စစ်ဆေးပါ။
- CLI (`asset list filter '{"id":"norito:4e52543000000002"}'`) ဖြင့် လက်ကျန်ငွေများကို ရယူပါ။
- SDKတိုင်းသည် ဒီမိုစီးဆင်းမှုအတွက် ထပ်တူကျသော Norito payloads ထုတ်ပေးကြောင်းသေချာစေရန် Rust/Python/JavaScript/Swift ချက်ပြုတ်နည်းများနှင့် ရလဒ်များကို နှိုင်းယှဉ်ပါ။