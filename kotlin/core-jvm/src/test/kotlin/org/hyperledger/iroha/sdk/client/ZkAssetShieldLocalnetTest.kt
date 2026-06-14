package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.SecureRandom
import java.time.Duration
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.core.model.instructions.ConfidentialEncryptedPayload
import org.hyperledger.iroha.sdk.core.model.instructions.RegisterZkAssetInstruction
import org.hyperledger.iroha.sdk.core.model.instructions.ShieldInstruction
import org.hyperledger.iroha.sdk.core.model.instructions.ZkAssetMode
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable

private const val PRIVATE_KEY_MULTIHASH_PREFIX = "802620"
private const val DEFAULT_LOCALNET_DIR = "/tmp/2iroha-localnet"
private const val GAS_LIMIT = 1_000L

/**
 * End-to-end integration test: register a ZK-capable asset and shield public funds into it through
 * the SDK production path (typed instruction -> [NativeSignerBridge] gas-bearing encoder ->
 * [HttpClientTransport] submission -> on-chain commit), then assert the Shield anchored a new
 * shielded root readable via `POST /v1/zk/roots`.
 *
 * Opt-in: requires a running confidential-enabled localnet and is gated on `IROHA_LOCALNET_TEST=1`
 * so the default `:core-jvm:test` run stays node-free. The host JNI bridge
 * (`libconnect_norito_bridge`) must be built and on `java.library.path` (wired by the test task).
 * The localnet directory defaults to [DEFAULT_LOCALNET_DIR] and can be overridden with
 * `IROHA_LOCALNET_DIR`.
 *
 * Repeatable: it (re)registers the asset and retries the Shield until the chain accepts it, so it
 * passes whether the target asset is freshly deployed or already ZK-registered from a prior run.
 */
@EnabledIfEnvironmentVariable(named = "IROHA_LOCALNET_TEST", matches = "1")
class ZkAssetShieldLocalnetTest {

    @Test
    fun `register zk asset and shield anchor a new root on chain`() {
        val localnet = localnetDir()
        val clientToml = readSimpleToml(localnet.resolve("client.toml"))
        val gasAssetId = gasAssetId(localnet)
        val gasLimit = GAS_LIMIT

        val chainId = clientToml.getValue("chain")
        val toriiUrl = clientToml.getValue("torii_url")
        val privateKey = decodePrivateKeySeed(clientToml.getValue("private_key"))

        val instructions = genesisInstructions(localnet.resolve("genesis.json"))
        val authority = aliceAccountId(instructions)
        val asset = numericAssetId(instructions, name = "sample")

        val executor = PlatformHttpTransportExecutor.createDefault()
        val transport = HttpClientTransport(executor, ClientConfig.builder().setBaseUri(URI.create(toriiUrl)).build())
        val confidential = ConfidentialAssetToriiClient.builder().baseUri(URI.create(toriiUrl)).build()

        val rootBefore = confidential.getLatestZkAssetRoot(asset).get()

        // Ensure the asset is ZK-registered. On a fresh localnet this commits; on a reused localnet
        // it is already registered. The Shield below is the authoritative assertion: it only succeeds
        // against a registered ZK asset with shield permitted, so it transitively proves registration.
        val registerInstruction = RegisterZkAssetInstruction.builder()
            .setAsset(asset)
            .setMode(ZkAssetMode.HYBRID)
            .setAllowShield(true)
            .setAllowUnshield(true)
            .build()
        val registerTx = NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
            algorithm = SigningAlgorithm.ED25519,
            chainId = chainId,
            authority = authority,
            creationTimeMs = System.currentTimeMillis(),
            ttlMs = null,
            instruction = registerInstruction,
            privateKey = privateKey,
            gasAssetId = gasAssetId,
            gasLimit = gasLimit,
        )
        val registerStatus = submitVersionedTransaction(executor, toriiUrl, registerTx.versionedSignedTransaction)
        assertTrue(registerStatus in 200..299, "RegisterZkAsset submission rejected with HTTP $registerStatus")

        val shieldInstruction = ShieldInstruction.builder()
            .setAsset(asset)
            .setFrom(authority)
            .setAmount("100")
            .setNoteCommitment(freshNoteCommitment())
            .setEncryptedPayload(freshEncryptedPayload())
            .build()
        val shieldTx = NativeSignerBridge.encodeShieldSignedTransaction(
            algorithm = SigningAlgorithm.ED25519,
            chainId = chainId,
            authority = authority,
            creationTimeMs = System.currentTimeMillis(),
            ttlMs = null,
            instruction = shieldInstruction,
            privateKey = privateKey,
            gasAssetId = gasAssetId,
            gasLimit = gasLimit,
        )
        val shieldStatus = submitUntilAccepted(executor, toriiUrl, shieldTx.versionedSignedTransaction)
        assertTrue(shieldStatus in 200..299, "Shield was not accepted (last HTTP $shieldStatus); RegisterZkAsset may not have committed")
        transport.waitForTransactionStatus(toHex(shieldTx.transactionHash), null).get()

        val rootAfter = confidential.getLatestZkAssetRoot(asset).get()
        assertNotNull(rootAfter, "shield must produce a readable shielded root")
        val advanced = rootBefore == null || !rootAfter.contentEquals(rootBefore)
        assertTrue(advanced, "shield must anchor a new shielded root distinct from the prior root")
    }

    private fun localnetDir(): Path =
        Paths.get(System.getenv("IROHA_LOCALNET_DIR") ?: DEFAULT_LOCALNET_DIR)

    // Posts native-encoder output (a versioned SignedTransaction in binary Norito) to the canonical
    // Torii ingress, matching what HttpClientTransport does for typed transactions.
    private fun submitVersionedTransaction(
        executor: HttpTransportExecutor,
        toriiUrl: String,
        versionedBytes: ByteArray,
    ): Int {
        val request = TransportRequest.builder()
            .setUri(URI.create(toriiUrl).resolve("v1/pipeline/transactions"))
            .setMethod("POST")
            .addHeader("Content-Type", "application/x-norito")
            .addHeader("Accept", "application/json")
            .setBody(versionedBytes)
            .setTimeout(Duration.ofSeconds(10))
            .build()
        return executor.execute(request).get().statusCode
    }

    // RegisterZkAsset commits asynchronously and Shield is rejected (HTTP 403) until it lands. Retry
    // the identical Shield submission until the chain accepts it, tolerating block latency / cold start.
    private fun submitUntilAccepted(
        executor: HttpTransportExecutor,
        toriiUrl: String,
        versionedBytes: ByteArray,
    ): Int {
        var status = 0
        repeat(30) {
            status = submitVersionedTransaction(executor, toriiUrl, versionedBytes)
            status.takeIf { code -> code in 200..299 }?.let { return it }
            Thread.sleep(1_000)
        }
        return status
    }

    private fun freshNoteCommitment(): ByteArray {
        val commitment = ByteArray(32)
        SecureRandom().nextBytes(commitment)
        commitment[0] = (commitment[0].toInt() or 0x01).toByte()
        return commitment
    }

    private fun freshEncryptedPayload(): ConfidentialEncryptedPayload {
        val random = SecureRandom()
        val ephemeral = X25519PrivateKeyParameters(random).generatePublicKey().encoded
        val nonce = ByteArray(24).also { random.nextBytes(it) }
        val ciphertext = ByteArray(48).also { random.nextBytes(it) }
        return ConfidentialEncryptedPayload(
            ephemeralPublicKey = ephemeral,
            nonce = nonce,
            ciphertext = ciphertext,
        )
    }

    private fun decodePrivateKeySeed(multihash: String): ByteArray {
        require(multihash.startsWith(PRIVATE_KEY_MULTIHASH_PREFIX)) {
            "private_key must be an Iroha Ed25519 multihash"
        }
        return hexToBytes(multihash.substring(PRIVATE_KEY_MULTIHASH_PREFIX.length))
    }

    private fun readSimpleToml(path: Path): Map<String, String> {
        val result = LinkedHashMap<String, String>()
        for (raw in Files.readAllLines(path, StandardCharsets.UTF_8)) {
            val line = raw.trim()
            val separator = line.indexOf('=')
            val parseable = line.isNotEmpty() && !line.startsWith("#") && !line.startsWith("[") && separator > 0
            parseable.takeIf { it }?.let {
                val key = line.substring(0, separator).trim()
                val value = line.substring(separator + 1).trim().trim('"')
                result[key] = value
            }
        }
        return result
    }

    // The node's gas asset comes from `[pipeline.gas] accepted_assets` in the peer config
    // (deploy_localnet.sh-patched localnets) or the genesis `ivm_gas_accepted_assets` custom
    // parameter (raw kagami localnets). Support both so the test is independent of the deploy path.
    private fun gasAssetId(localnet: Path): String =
        gasAssetFromPeerConfig(localnet)
            ?: gasAssetFromGenesis(localnet)
            ?: error("no gas asset in peer0.toml accepted_assets or genesis ivm_gas_accepted_assets")

    private fun gasAssetFromPeerConfig(localnet: Path): String? {
        val line = Files.readAllLines(localnet.resolve("peer0.toml"), StandardCharsets.UTF_8)
            .firstOrNull { it.trim().startsWith("accepted_assets") } ?: return null
        return Regex("\"([^\"]+)\"").find(line)?.groupValues?.get(1)
    }

    @Suppress("UNCHECKED_CAST")
    private fun gasAssetFromGenesis(localnet: Path): String? {
        val root = JsonParser.parse(
            String(Files.readAllBytes(localnet.resolve("genesis.json")), StandardCharsets.UTF_8),
        ) as Map<String, Any?>
        return (root.getValue("transactions") as List<Any?>).firstNotNullOfOrNull { tx ->
            val custom = ((tx as? Map<String, Any?>)?.get("parameters") as? Map<String, Any?>)
                ?.get("custom") as? Map<String, Any?>
            val payload = (custom?.get("ivm_gas_accepted_assets") as? Map<String, Any?>)?.get("payload")
            (payload as? List<Any?>)?.firstOrNull() as? String
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun genesisInstructions(path: Path): List<Any?> {
        val root = JsonParser.parse(String(Files.readAllBytes(path), StandardCharsets.UTF_8)) as Map<String, Any?>
        val transactions = root.getValue("transactions") as List<Any?>
        return transactions.flatMap { tx ->
            ((tx as? Map<String, Any?>)?.get("instructions") as? List<Any?>) ?: emptyList()
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun aliceAccountId(instructions: List<Any?>): String {
        val account = instructions.firstNotNullOf { isi ->
            ((isi as? Map<String, Any?>)?.get("Register") as? Map<String, Any?>)?.get("Account") as? Map<String, Any?>
        }
        return account.getValue("id") as String
    }

    @Suppress("UNCHECKED_CAST")
    private fun numericAssetId(instructions: List<Any?>, name: String): String {
        val definition = instructions.firstNotNullOf { isi ->
            (((isi as? Map<String, Any?>)?.get("Register") as? Map<String, Any?>)?.get("AssetDefinition") as? Map<String, Any?>)
                ?.takeIf { it["name"] == name }
        }
        return definition.getValue("id") as String
    }

    private fun hexToBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0) { "hex string must have even length" }
        return ByteArray(hex.length / 2) { index ->
            hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun toHex(bytes: ByteArray): String =
        buildString(bytes.size * 2) {
            for (byte in bytes) {
                val value = byte.toInt() and 0xff
                append("0123456789abcdef"[value ushr 4])
                append("0123456789abcdef"[value and 0x0f])
            }
        }
}
