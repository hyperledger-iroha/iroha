---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/java-ledger-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/java-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 578d37adbfce87db512f8bd847f35500545c6a1809bedec1d52764edecc6f313
source_last_modified: "2025-11-11T10:23:57.788821+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Recette de flux du registre Java
description: Exécutez la démo enregistrer → frapper → transférer avec la bibliothèque JVM IrohaAndroid.
slug: /sdks/recipes/java-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

Cette recette cible la variante desktop/JVM du module `java/iroha_android`. Elle reflète le walkthrough CLI en enregistrant une définition d’actif, en frappant sur le compte admin, en transférant vers un second compte et en affichant le solde obtenu.

<SampleDownload
  href="/sdk-recipes/java/src/main/java/ledger/LedgerFlow.java"
  filename="src/main/java/ledger/LedgerFlow.java"
  description="Téléchargez l'exemple Java complet pour l'importer dans votre IDE ou modèle de projet."
/>

## 1. Enregistrez la définition d’actif (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id coffee#wonderland
```

## 2. Exportez les identifiants

```bash
# raw 32-byte Ed25519 private key in hex (without multicodec prefix)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

> Utilisez `iroha_cli tools crypto private-key export --raw --private-key <multihash>` si vous devez retirer le préfixe multihash de la configuration par défaut.

## 3. Ajoutez les dépendances

```kts title="build.gradle.kts"
dependencies {
    implementation(project(":java:iroha_android"))
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1") // Ed25519 signer helper
}
```

## 4. Programme d’exemple

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

Compilez avec votre outil préféré (`./gradlew :java:iroha_android:assemble && ./gradlew run`).

## 5. Vérifier la parité

- Inspectez les hashes de transaction via `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Récupérez les soldes via la CLI (`asset list filter '{"id":"norito:4e52543000000002"}'`).
- Comparez les résultats avec les recettes Rust/Python/JavaScript/Swift pour garantir que chaque SDK produit des payloads Norito identiques pour le flux de démo.
