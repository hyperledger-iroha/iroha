package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class HttpClientTransportTest {
    @Test
    fun issueIdentifierClaimReceiptForwardsAccountAliasPathLiteral() {
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        transport.issueIdentifierClaimReceipt(
            "alice@wonderland.dataspace",
            IdentifierResolveRequest.encrypted("phone#retail", "abcd"),
        ).join()

        assertEquals(
            "https://torii.example/api/v1/accounts/alice%40wonderland.dataspace/identifiers/claim-receipt",
            executor.lastRequest.uri.toString(),
        )
    }

    @Test
    fun deployContractPostsAliasFirstPayloadAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "bundle_name": "single-contract-deploy",
                  "bundle_digest": "mock-bundle-digest",
                  "chain_fingerprint": "mock-chain@height-0",
                  "dry_run": false,
                  "completed_stages": ["plan", "deploy"],
                  "failure_point": null,
                  "contracts": [
                    {
                      "name": "router::universal",
                      "contract_alias": "router::universal",
                      "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                      "previous_contract_address": null,
                      "upgraded": false,
                      "dataspace": "router",
                      "deploy_nonce": 7,
                      "tx_hash_hex": "${"11".repeat(32)}",
                      "code_hash_hex": "${"22".repeat(32)}",
                      "abi_hash_hex": "${"33".repeat(32)}",
                      "status": "submitted"
                    }
                  ],
                  "init_calls": [],
                  "assertions": []
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.deployContract(
            authority = "alice",
            privateKey = "privkey",
            codeB64 = "AQID",
            contractAlias = "router::universal",
        ).join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertTrue(parsed.ok)
        assertEquals("mock-bundle-digest", parsed.bundleDigest)
        assertEquals("router::universal", parsed.contracts.first().contractAlias)
        assertEquals("router", parsed.contracts.first().dataspace)
        assertEquals(7L, parsed.contracts.first().deployNonce)
        assertEquals("11".repeat(32), parsed.contracts.first().txHashHex)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/contracts/deploy", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice", payload["authority"])
        assertEquals("privkey", payload["private_key"])
        assertEquals("AQID", payload["code_b64"])
        assertEquals("router::universal", payload["contract_alias"])
        assertFalse(payload.containsKey("lease_expiry_ms"))
    }

    @Test
    fun callContractPostsSelectorPayloadAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "submitted": true,
                  "dataspace": "router",
                  "code_hash_hex": "${"44".repeat(32)}",
                  "abi_hash_hex": "${"55".repeat(32)}",
                  "creation_time_ms": 1712345678901,
                  "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                  "tx_hash_hex": "${"66".repeat(32)}",
                  "entrypoint": "contribute",
                  "transaction_scaffold_b64": "AQID",
                  "signed_transaction_b64": "BAUG",
                  "signing_message_b64": "BwgJ"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.callContract(
            authority = "alice",
            privateKey = "privkey",
            gasLimit = 5_000L,
            contractAlias = "router::universal",
            entrypoint = "contribute",
            payload = linkedMapOf("buyer" to "alice", "payment_amount" to 1L),
            gasAssetId = "xor#sora",
        ).join()

        assertTrue(response.ok)
        assertTrue(response.submitted)
        assertEquals("router", response.dataspace)
        assertEquals("contribute", response.entrypoint)
        assertEquals("AQID", response.transactionScaffoldB64)
        assertEquals("BAUG", response.signedTransactionB64)
        assertEquals("BwgJ", response.signingMessageB64)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/contracts/call", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice", payload["authority"])
        assertEquals("privkey", payload["private_key"])
        assertEquals("router::universal", payload["contract_alias"])
        assertFalse(payload.containsKey("contract_address"))
        assertEquals(5000L, (payload["gas_limit"] as Number).toLong())
        assertEquals("contribute", payload["entrypoint"])
        assertEquals("xor#sora", payload["gas_asset_id"])
        @Suppress("UNCHECKED_CAST")
        val args = payload["payload"] as Map<String, Any?>
        assertEquals("alice", args["buyer"])
        assertEquals(1L, (args["payment_amount"] as Number).toLong())
    }

    @Test
    fun callContractRejectsAmbiguousSelector() {
        val transport = HttpClientTransport.withExecutor(
            executor = CapturingExecutor(),
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<IllegalArgumentException> {
            transport.callContract(
                authority = "alice",
                privateKey = "privkey",
                gasLimit = 5_000L,
                contractAddress = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                contractAlias = "router::universal",
            )
        }

        assertTrue(error.message?.contains("Exactly one") == true)
    }

    @Test
    fun getGovernanceContractParsesResponse() {
        val contractAddress = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "found": true,
                  "contract_address": "$contractAddress",
                  "dataspace": "router",
                  "code_hash_hex": "${"77".repeat(32)}"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.getGovernanceContract(contractAddress).join()

        assertTrue(response.found)
        assertEquals(contractAddress, response.contractAddress)
        assertEquals("router", response.dataspace)
        assertEquals("77".repeat(32), response.codeHashHex)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("GET", request.method)
        assertEquals(
            "https://torii.example/api/v1/gov/contracts/$contractAddress",
            request.uri.toString(),
        )
        assertEquals(0, request.body.size)
    }

    @Test
    fun listRamLfeProgramPoliciesParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "total": 1,
                  "items": [
                    {
                      "program_id": "identifier_lookup_retail",
                      "owner": "sorau1NpOwner",
                      "active": true,
                      "resolver_public_key": "ed25519:resolver-key",
                      "backend": "bfv-programmed-sha3-256-v1",
                      "verification_mode": "signed",
                      "input_encryption": "bfv-v1",
                      "input_encryption_public_parameters": "ABCD",
                      "input_encryption_public_parameters_decoded": {
                        "parameters": {
                          "polynomial_degree": 64,
                          "plaintext_modulus": 257,
                          "ciphertext_modulus": 1099511627776,
                          "decomposition_base_log": 12
                        },
                        "public_key": {
                          "b": [1, 2, 3],
                          "a": [4, 5, 6]
                        },
                        "max_input_bytes": 32
                      },
                      "note": "retail programmed policy",
                      "proof_verifier": {
                        "proof_backend": "halo2-ipa",
                        "circuit_id": "ram-lfe-v1",
                        "public_inputs_schema_hash": "${"44".repeat(32)}",
                        "verifying_key_bytes_b64": "AQID"
                      }
                    }
                  ]
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val response = transport.listRamLfeProgramPolicies().join()

        assertEquals(1L, response.total)
        assertEquals(1, response.items.size)
        val item = response.items.first()
        assertEquals("identifier_lookup_retail", item.programId)
        assertEquals("sorau1NpOwner", item.owner)
        assertTrue(item.active)
        assertEquals("signed", item.verificationMode)
        assertEquals("bfv-v1", item.inputEncryption)
        val decodedParameters = assertNotNull(item.inputEncryptionPublicParametersDecoded)
        assertEquals(64L, decodedParameters.parameters.polynomialDegree)
        val proofVerifier = assertNotNull(item.proofVerifier)
        assertEquals("halo2-ipa", proofVerifier.proofBackend)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("GET", request.method)
        assertEquals("https://torii.example/v1/ram-lfe/program-policies", request.uri.toString())
        assertTrue(request.headers["Accept"]?.contains("application/json") == true)
    }

    @Test
    fun executeRamLfeProgramParsesResponseAndPostsPlaintextHex() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "program_id": "identifier_lookup_retail",
                  "opaque_hash": "opaque-hash-literal",
                  "receipt_hash": "receipt-hash-literal",
                  "output_hash": "output-hash-literal",
                  "associated_data_hash": "associated-data-hash-literal",
                  "executed_at_ms": 42,
                  "expires_at_ms": 142,
                  "backend": "bfv-programmed-sha3-256-v1",
                  "verification_mode": "signed",
                  "receipt": {
                    "payload": {
                      "program_id": {"name": "identifier_lookup_retail"},
                      "program_digest": "hash:${"11".repeat(32).uppercase()}#ABCD",
                      "backend": "bfv-programmed-sha3-256-v1",
                      "verification_mode": {"mode": "Signed", "value": null},
                      "output_hash": "hash:${"22".repeat(32).uppercase()}#BCDE",
                      "associated_data_hash": "hash:${"33".repeat(32).uppercase()}#CDEF",
                      "executed_at_ms": 42,
                      "expires_at_ms": 142
                    },
                    "signature": "${"aa".repeat(64)}"
                  }
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val response = transport.executeRamLfeProgram("identifier_lookup_retail", "0xABCD", null).join()

        assertTrue(response.isPresent)
        val execute = response.get()
        assertEquals("identifier_lookup_retail", execute.programId)
        assertEquals("output-hash-literal", execute.outputHash)
        assertEquals("signed", execute.verificationMode)
        assertTrue(execute.receipt.containsKey("payload"))

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals(
            "https://torii.example/v1/ram-lfe/programs/identifier_lookup_retail/execute",
            request.uri.toString(),
        )
        assertEquals("""{"input_hex":"abcd"}""", readBody(request))
    }

    @Test
    fun executeRamLfeProgramReturnsEmptyOnNotFoundAndPostsEncryptedHex() {
        val executor = StubResponseExecutor(
            statusCode = 404,
            body = byteArrayOf(),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val response = transport.executeRamLfeProgram("identifier_lookup_retail", null, "ABCD").join()

        assertFalse(response.isPresent)
        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("""{"encrypted_input":"abcd"}""", readBody(request))
    }

    @Test
    fun verifyRamLfeReceiptPostsRawReceiptAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "valid": true,
                  "program_id": "identifier_lookup_retail",
                  "backend": "bfv-programmed-sha3-256-v1",
                  "verification_mode": "signed",
                  "output_hash": "output-hash-literal",
                  "associated_data_hash": "associated-data-hash-literal",
                  "output_hash_matches": true
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )
        val receipt = linkedMapOf<String, Any>(
            "payload" to linkedMapOf<String, Any?>(
                "program_id" to mapOf("name" to "identifier_lookup_retail"),
                "backend" to "bfv-programmed-sha3-256-v1",
                "verification_mode" to mapOf("mode" to "Signed", "value" to null),
                "program_digest" to "hash:${"11".repeat(32).uppercase()}#ABCD",
                "output_hash" to "hash:${"22".repeat(32).uppercase()}#BCDE",
                "associated_data_hash" to "hash:${"33".repeat(32).uppercase()}#CDEF",
                "executed_at_ms" to 42L,
                "expires_at_ms" to 142L,
            ),
            "signature" to "aa".repeat(64),
        )

        val response = transport.verifyRamLfeReceipt(receipt, "C0FFEE").join()

        assertTrue(response.valid)
        assertEquals("identifier_lookup_retail", response.programId)
        assertEquals(true, response.outputHashMatches)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("https://torii.example/api/v1/ram-lfe/receipts/verify", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("c0ffee", payload["output_hex"])
        assertTrue(payload["receipt"] is Map<*, *>)
    }

    @Test
    fun getVpnProfileDeserializesNativeLeaseFields() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "available": true,
                  "relay_endpoint": "/dns/relay.example/udp/9443/quic",
                  "supported_exit_classes": ["standard", "low-latency"],
                  "default_exit_class": "standard",
                  "lease_secs": 600,
                  "dns_push_interval_secs": 60,
                  "meter_family": "soranet.vpn.standard",
                  "route_pushes": ["0.0.0.0/0"],
                  "excluded_routes": ["10.0.0.0/8"],
                  "dns_servers": ["1.1.1.1"],
                  "tunnel_addresses": ["10.208.0.2/32"],
                  "mtu_bytes": 1024,
                  "display_billing_label": "standard XOR",
                  "fee_asset_id": "xor#universal.universal",
                  "escrow_account_id": "sorauEscrow",
                  "operator_account_id": "sorauOperator",
                  "lease_fee_nanos": 1000000,
                  "settlement_grace_secs": 120,
                  "flow_label_bits": 24,
                  "padding_budget_ms": 15,
                  "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val profile = transport.getVpnProfile().join()

        assertTrue(profile.available)
        assertEquals("xor#universal.universal", profile.feeAssetId)
        assertEquals("sorauEscrow", profile.escrowAccountId)
        assertEquals("sorauOperator", profile.operatorAccountId)
        assertEquals(1_000_000L, profile.leaseFeeNanos)
        assertEquals(120L, profile.settlementGraceSecs)
        assertEquals("ab".repeat(32), profile.relayTlsSpkiSha256Hex)
        assertEquals("GET", executor.lastRequest.method)
        assertEquals("https://torii.example/v1/vpn/profile", executor.lastRequest.uri.toString())
    }

    @Test
    fun createVpnQuoteSignsCanonicalBodyAndParsesOpenLeaseInstruction() {
        val quoteId = "11".repeat(32)
        val meteringKey = "22".repeat(32)
        val executor = StubResponseExecutor(
            statusCode = 201,
            body = vpnQuoteJson(quoteId, meteringKey).toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice", keyPair.private, 1_700_000_000_000L, "vpn-nonce-1")
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val quote = transport.createVpnQuote(
            VpnQuoteCreateRequest("low-latency", "0x$meteringKey"),
            auth,
        ).join()

        assertEquals(quoteId, quote.quoteId)
        assertEquals(quoteId, quote.leaseIdHex)
        assertEquals(meteringKey, quote.meteringPublicKeyHex)
        assertEquals("iroha_data_model::isi::vpn::OpenVpnLeaseEscrow", quote.openLeaseInstruction?.wireId)
        assertEquals(1, quote.txInstructions.size)
        assertEquals(quote.openLeaseInstruction?.payloadHex, quote.txInstructions.first().payloadHex)

        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/vpn/quotes", request.uri.toString())
        assertEquals("""{"exit_class":"low-latency","metering_public_key_hex":"$meteringKey"}""", readBody(request))
        assertEquals("alice", request.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        assertEquals("1700000000000", request.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS]?.first())
        assertEquals("vpn-nonce-1", request.headers[CanonicalRequestSigner.HEADER_NONCE]?.first())
        assertCanonicalSignature(request, keyPair.public, 1_700_000_000_000L, "vpn-nonce-1")
    }

    @Test
    fun pushDeviceRegisterAndUnregisterSignCanonicalBody() {
        val executor = QueueResponseExecutor(listOf(202 to "", 202 to ""))
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )
        val requestBody = PushDeviceRequest(" alice ", "FCM", " token-1 ", listOf(" activity "))

        transport.registerPushDevice(
            requestBody,
            ToriiCanonicalRequestAuth("alice", keyPair.private, 1_700_000_000_010L, "push-nonce-1"),
        ).join()
        transport.unregisterPushDevice(
            requestBody,
            ToriiCanonicalRequestAuth("alice", keyPair.private, 1_700_000_000_011L, "push-nonce-2"),
        ).join()

        val register = executor.requests[0]
        assertEquals("POST", register.method)
        assertEquals("https://torii.example/v1/notify/devices", register.uri.toString())
        assertEquals("""{"account_id":"alice","platform":"FCM","token":"token-1","topics":["activity"]}""", readBody(register))
        assertEquals("alice", register.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        assertCanonicalSignature(register, keyPair.public, 1_700_000_000_010L, "push-nonce-1")

        val unregister = executor.requests[1]
        assertEquals("DELETE", unregister.method)
        assertEquals("https://torii.example/v1/notify/devices", unregister.uri.toString())
        assertEquals(readBody(register), readBody(unregister))
        assertCanonicalSignature(unregister, keyPair.public, 1_700_000_000_011L, "push-nonce-2")
    }

    @Test
    fun vpnSessionAndReceiptMethodsUseNativeLeaseDtos() {
        val sessionId = "33".repeat(32)
        val paymentTxHash = "44".repeat(32)
        val meteringKey = "55".repeat(32)
        val receiptJson = vpnReceiptJson(sessionId, paymentTxHash, settled = true)
        val executor = QueueResponseExecutor(
            listOf(
                201 to vpnSessionJson(sessionId, paymentTxHash),
                200 to vpnSessionJson(sessionId, paymentTxHash),
                200 to vpnReceiptJson(sessionId, paymentTxHash, settled = false),
                201 to receiptJson,
                200 to """{"items":[$receiptJson],"total":1}""",
            )
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice", keyPair.private, 1_700_000_000_001L, "vpn-nonce-2")
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val session = transport.createVpnSession(
            VpnSessionCreateRequest("standard", sessionId, "0x$paymentTxHash", meteringKey),
            auth,
        ).join()
        val fetched = transport.getVpnSession(sessionId, auth).join()
        val deleted = transport.deleteVpnSession("0x$sessionId", auth).join()
        val submitted = transport.submitVpnReceipt(
            VpnReceiptSubmitRequest("0xCAFE", "BEEF", "0x$sessionId"),
            auth,
        ).join()
        val receipts = transport.listVpnReceipts(auth).join()

        assertEquals(sessionId, session.sessionId)
        assertTrue(fetched.isPresent)
        assertEquals(sessionId, fetched.get().quoteId)
        assertTrue(deleted.isPresent)
        assertEquals("disconnected", deleted.get().status)
        assertEquals("settled", submitted.status)
        assertEquals(750_000L, submitted.earnedFeeNanos)
        assertEquals(250_000L, submitted.refundedFeeNanos)
        assertEquals("iroha_data_model::isi::vpn::SettleVpnLease", submitted.settleLeaseInstruction?.wireId)
        assertEquals(1L, receipts.total)
        assertEquals(sessionId, receipts.items.first().leaseIdHex)

        assertEquals("""{"exit_class":"standard","metering_public_key_hex":"$meteringKey","payment_tx_hash":"$paymentTxHash","quote_id":"$sessionId"}""", readBody(executor.requests[0]))
        assertEquals("GET", executor.requests[1].method)
        assertEquals("https://torii.example/v1/vpn/sessions/$sessionId", executor.requests[1].uri.toString())
        assertEquals("DELETE", executor.requests[2].method)
        assertEquals("""{"client_voucher_hex":"beef","lease_id_hex":"$sessionId","relay_receipt_hex":"cafe"}""", readBody(executor.requests[3]))
        assertEquals("https://torii.example/v1/vpn/receipts", executor.requests[4].uri.toString())
    }

    @Test
    fun resolveAccountAliasParsesSuccessfulResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "alice@universal",
                  "account_id": "aid:alice-123",
                  "index": 42,
                  "source": "directory"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("alice@universal").join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertEquals("alice@universal", parsed.alias)
        assertEquals("aid:alice-123", parsed.accountId)
        assertEquals(42L, parsed.index)
        assertEquals("directory", parsed.source)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/aliases/resolve", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice@universal", payload["alias"])
        assertEquals(1, payload.size)
    }

    @Test
    fun resolveAccountAliasParsesSuccessfulResponseWithoutIndex() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "banking@centralbank.universal",
                  "account_id": "aid:banking-123",
                  "source": "rekey_record"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("banking@centralbank.universal").join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertEquals("banking@centralbank.universal", parsed.alias)
        assertEquals("aid:banking-123", parsed.accountId)
        assertNull(parsed.index)
        assertEquals("rekey_record", parsed.source)
    }

    @Test
    fun resolveAccountAliasReturnsEmptyOnNotFound() {
        val executor = StubResponseExecutor(
            statusCode = 404,
            body = byteArrayOf(),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("missing@universal").join()

        assertFalse(response.isPresent)
        assertNull(response.orElse(null))
    }

    @Test
    fun resolveAccountAliasRejectsNonIntegerIndex() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "alice@universal",
                  "account_id": "aid:alice-123",
                  "index": 3.5
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<java.util.concurrent.ExecutionException> {
            transport.resolveAccountAlias("alice@universal").get()
        }
        assertNotNull(error.cause)
    }

    @Test
    fun resolveAccountAliasPropagatesMalformedJson() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = "not a json object".toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<java.util.concurrent.ExecutionException> {
            transport.resolveAccountAlias("alice@universal").get()
        }
        assertNotNull(error.cause)
    }

    @Test
    fun submitTransactionPrefersAuthoritativeReceiptHashHeaderForPolling() {
        val transaction = sampleTransaction(0x11)
        val localHash = SignedTransactionHasher.hashHex(transaction)
        val authoritativeHash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        val executor = TrackingExecutor(
            expectedHash = authoritativeHash,
            submitHeaderHash = authoritativeHash.uppercase(),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build(),
        )

        val response = transport.submitTransaction(transaction).join()

        assertFalse(localHash == response.hashHex())
        assertEquals(authoritativeHash, response.hashHex())

        val payload = transport
            .waitForTransactionStatus(response.hashHex()!!, PipelineStatusOptions(intervalMillis = 0L))
            .join()

        assertEquals("Committed", PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        assertTrue(executor.observedExpectedHash)
    }

    private fun readBody(request: TransportRequest): String =
        String(request.body, StandardCharsets.UTF_8)

    private fun assertCanonicalSignature(
        request: TransportRequest,
        publicKey: java.security.PublicKey,
        timestampMs: Long,
        nonce: String,
    ) {
        val encodedSignature = assertNotNull(request.headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.first())
        val signature = Base64.getDecoder().decode(encodedSignature)
        val message = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            request.method,
            request.uri,
            request.body,
            timestampMs,
            nonce,
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(publicKey)
        verifier.update(message)
        assertTrue(verifier.verify(signature))
    }

    private fun vpnQuoteJson(quoteId: String, meteringKey: String): String =
        """
            {
              "quote_id": "$quoteId",
              "lease_id_hex": "$quoteId",
              "session_id_hex": "${"aa".repeat(16)}",
              "payment_reference": "$quoteId",
              "account_id": "alice",
              "exit_class": "low-latency",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "lease_secs": 600,
              "quote_expires_at_ms": 1700000600000,
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauEscrow",
              "operator_account_id": "sorauOperator",
              "lease_fee_nanos": 1000000,
              "route_pushes": ["0.0.0.0/0"],
              "excluded_routes": [],
              "dns_servers": ["1.1.1.1"],
              "tunnel_addresses": ["10.208.0.2/32"],
              "mtu_bytes": 1024,
              "meter_family": "soranet.vpn.standard",
              "flow_label_bits": 24,
              "padding_budget_ms": 15,
              "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}",
              "metering_public_key_hex": "$meteringKey",
              "open_lease_instruction": {
                "wire_id": "iroha_data_model::isi::vpn::OpenVpnLeaseEscrow",
                "payload_hex": "cafe"
              },
              "tx_instructions": [
                {
                  "wire_id": "iroha_data_model::isi::vpn::OpenVpnLeaseEscrow",
                  "payload_hex": "cafe"
                }
              ]
            }
        """.trimIndent()

    private fun vpnSessionJson(sessionId: String, paymentTxHash: String): String =
        """
            {
              "session_id": "$sessionId",
              "account_id": "alice",
              "exit_class": "standard",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "lease_secs": 600,
              "expires_at_ms": 1700000600000,
              "connected_at_ms": 1700000000000,
              "meter_family": "soranet.vpn.standard",
              "quote_id": "$sessionId",
              "payment_reference": "$sessionId",
              "payment_tx_hash": "$paymentTxHash",
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauEscrow",
              "operator_account_id": "sorauOperator",
              "lease_fee_nanos": 1000000,
              "flow_label_bits": 24,
              "padding_budget_ms": 15,
              "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}",
              "route_pushes": ["0.0.0.0/0"],
              "excluded_routes": [],
              "dns_servers": ["1.1.1.1"],
              "tunnel_addresses": ["10.208.0.2/32"],
              "mtu_bytes": 1024,
              "helper_ticket_hex": "cafe",
              "bytes_in": 0,
              "bytes_out": 0,
              "status": "active"
            }
        """.trimIndent()

    private fun vpnReceiptJson(sessionId: String, paymentTxHash: String, settled: Boolean): String {
        val status = if (settled) "settled" else "disconnected"
        val source = if (settled) "relay" else "torii"
        val earned = if (settled) 750_000L else 0L
        val refunded = if (settled) 250_000L else 1_000_000L
        val settle = if (settled) {
            """,
              "settle_lease_instruction": {
                "wire_id": "iroha_data_model::isi::vpn::SettleVpnLease",
                "payload_hex": "f00d"
              },
              "tx_instructions": [
                {
                  "wire_id": "iroha_data_model::isi::vpn::SettleVpnLease",
                  "payload_hex": "f00d"
                }
              ]"""
        } else {
            """,
              "settle_lease_instruction": null,
              "tx_instructions": []"""
        }
        return """
            {
              "session_id": "$sessionId",
              "account_id": "alice",
              "exit_class": "standard",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "meter_family": "soranet.vpn.standard",
              "connected_at_ms": 1700000000000,
              "disconnected_at_ms": 1700000010000,
              "duration_ms": 10000,
              "bytes_in": 1024,
              "bytes_out": 2048,
              "status": "$status",
              "receipt_source": "$source",
              "quote_id": "$sessionId",
              "payment_tx_hash": "$paymentTxHash",
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauEscrow",
              "operator_account_id": "sorauOperator",
              "lease_fee_nanos": 1000000,
              "earned_fee_nanos": $earned,
              "refunded_fee_nanos": $refunded,
              "lease_id_hex": "$sessionId"$settle
            }
        """.trimIndent()
    }

    private fun sampleTransaction(seed: Int): SignedTransaction {
        val codec = NoritoJavaCodecAdapter()
        val encoded = codec.encodeTransaction(
            TransactionPayload(
                chainId = String.format("%08x", seed),
                creationTimeMs = 1_700_000_000_000L + seed,
                timeToLiveMs = 5_000L,
                nonce = seed + 1,
                metadata = mapOf("note" to "tx-$seed"),
            ),
        )
        val signature = ByteArray(64) { (seed + 1).toByte() }
        val publicKey = ByteArray(32) { (seed + 2).toByte() }
        return SignedTransaction(
            encoded,
            signature,
            publicKey,
            codec.schemaName(),
        )
    }

    private open class CapturingExecutor : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(404).setBody(byteArrayOf()).build(),
            )
        }
    }

    private class StubResponseExecutor(
        private val statusCode: Int,
        private val body: ByteArray,
    ) : CapturingExecutor() {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(statusCode).setBody(body).build(),
            )
        }
    }

    private class QueueResponseExecutor(
        responses: List<Pair<Int, String>>,
    ) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()
        private val responses = java.util.ArrayDeque(responses)

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val (statusCode, body) = responses.removeFirst()
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(statusCode)
                    .setBody(body.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    private class TrackingExecutor(
        private val expectedHash: String,
        private val submitHeaderHash: String?,
    ) : HttpTransportExecutor {
        var observedExpectedHash = false
            private set
        private var pollCount = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            if (request.method == "POST") {
                val builder = TransportResponse.builder()
                    .setStatusCode(202)
                    .setBody(byteArrayOf())
                if (submitHeaderHash != null) {
                    builder.addHeader("x-iroha-transaction-hash", submitHeaderHash)
                }
                return CompletableFuture.completedFuture(builder.build())
            }
            if (request.method == "GET") {
                if (request.uri.query?.contains("hash=$expectedHash") == true) {
                    observedExpectedHash = true
                }
                val kind = if (pollCount++ == 0) "Pending" else "Committed"
                return CompletableFuture.completedFuture(
                    TransportResponse.builder()
                        .setStatusCode(200)
                        .setBody(
                            """{"kind":"Transaction","content":{"status":{"kind":"$kind"}}}"""
                                .toByteArray(StandardCharsets.UTF_8),
                        )
                        .build(),
                )
            }
            throw IllegalStateException("Unexpected HTTP method ${request.method}")
        }
    }
}
