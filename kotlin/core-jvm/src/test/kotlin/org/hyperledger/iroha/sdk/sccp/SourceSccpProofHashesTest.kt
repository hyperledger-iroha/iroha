package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class SourceSccpProofHashesTest {
    @Test
    fun derivesSourceAdapterVerifierVkHashesForUiTooling() {
        val vectors = mapOf(
            SccpSourceProofs.DOMAIN_ETH to "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46",
            SccpSourceProofs.DOMAIN_BSC to "0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc",
            SccpSourceProofs.DOMAIN_SOL to "0xe7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6",
            SccpSourceProofs.DOMAIN_TON to "0xf03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad",
            SccpSourceProofs.DOMAIN_TRON to "0x0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364",
            SccpSourceProofs.DOMAIN_SORA_KUSAMA to "0xf7768653132995511594e6e7edb4af22f78bba615650d9dda72f14bb18984daf",
            SccpSourceProofs.DOMAIN_SORA_POLKADOT to "0x4f8456bf8626436a16d763c40bf23dffb962232f0766c4ae33d6e594f8be1635",
            SccpSourceProofs.DOMAIN_SORA2 to "0x96bbfa08489249b28a1444d0dcb9d5b4023bd688091f31c0b435601dad48dbb4",
        )
        vectors.forEach { (sourceDomain, expectedHash) ->
            assertEquals(expectedHash, SccpSourceProofs.sourceAdapterVerifierVkHash(sourceDomain))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.sourceAdapterVerifierVkHash(
                SccpSourceProofs.DOMAIN_TON,
                SccpSourceProofs.DOMAIN_TON,
            )
        }
    }

    @Test
    fun derivesNativeDestinationBindingHashesForUiTooling() {
        val vectors = mapOf(
            SccpSourceProofs.DOMAIN_SOL to (
                "sccp:0:3:sol:solana-program-v1:2" to
                    "0x078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6"
                ),
            SccpSourceProofs.DOMAIN_TON to (
                "sccp:0:4:ton:ton-contract-v1:3" to
                    "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799"
                ),
            SccpSourceProofs.DOMAIN_SORA_KUSAMA to (
                "sccp:0:6:sora-kusama:substrate-runtime-v1:5" to
                    "0x2ee5c37634c3fab7e9086ea43af7553089fc24dc2ce27d76c46ef4c3da57bb56"
                ),
            SccpSourceProofs.DOMAIN_SORA_POLKADOT to (
                "sccp:0:7:sora-polkadot:substrate-runtime-v1:5" to
                    "0x570ec340d4fee4a84eaa7a53b19baa53c9f4f8d7f64c3c43639fde0c6b3fdef0"
                ),
            SccpSourceProofs.DOMAIN_SORA2 to (
                "sccp:0:8:sora2:substrate-runtime-v1:5" to
                    "0xda5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4"
                ),
        )
        vectors.forEach { (domain, expected) ->
            assertEquals(expected.first, SccpSourceProofs.destinationBindingKey(domain))
            assertEquals(expected.second, SccpSourceProofs.destinationBindingHash(domain))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.destinationBindingHash(SccpSourceProofs.DOMAIN_ETH)
        }
    }

    @Test
    fun derivesEvmAndTronDestinationBindingsForUiTooling() {
        val evmBinding = SccpSourceProofs.evmDestinationBinding(
            targetDomain = SccpSourceProofs.DOMAIN_ETH,
            networkId = "0x" + "33".repeat(32),
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertEquals(
            listOf(
                "evm",
                "0",
                "1",
                "33".repeat(32),
                "0x" + "11".repeat(20),
                "0x" + "22".repeat(20),
                "0x" + "bb".repeat(32),
                "0x" + "cc".repeat(32),
            ).joinToString(":"),
            evmBinding.key,
        )
        assertEquals(
            "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf",
            evmBinding.hash,
        )
        assertEquals(
            evmBinding.hash,
            SccpSourceProofs.evmDestinationBindingHash(
                targetDomain = SccpSourceProofs.DOMAIN_ETH,
                networkId = "0x" + "33".repeat(32),
                verifierAddress = "0x" + "11".repeat(20),
                bridgeAddress = "0x" + "22".repeat(20),
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
            ),
        )

        val tronAddress = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
        val tronBinding = SccpSourceProofs.tronDestinationBinding(
            networkId = "0x" + "33".repeat(32),
            verifierAddress = tronAddress,
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertEquals(
            listOf(
                "tron",
                "0",
                "5",
                "33".repeat(32),
                tronAddress,
                "0x" + "bb".repeat(32),
                "0x" + "cc".repeat(32),
            ).joinToString(":"),
            tronBinding.key,
        )
        assertEquals(
            "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f",
            tronBinding.hash,
        )
        assertEquals(
            tronBinding.hash,
            SccpSourceProofs.tronDestinationBindingHash(
                networkId = "0x" + "33".repeat(32),
                verifierAddress = tronAddress,
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
            ),
        )

        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.evmDestinationBinding(
                targetDomain = SccpSourceProofs.DOMAIN_ETH,
                networkId = "0x" + "33".repeat(32),
                verifierAddress = "0x" + "11".repeat(20),
                bridgeAddress = "0x" + "11".repeat(20),
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronDestinationBinding(
                networkId = "0x" + "33".repeat(32),
                verifierAddress = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
            )
        }
        val paddedTronAddressFailure = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronDestinationBinding(
                networkId = "0x" + "33".repeat(32),
                verifierAddress = " $tronAddress",
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
            )
        }
        assertTrue(
            paddedTronAddressFailure.message.orEmpty().contains("canonical Base58Check"),
        )
    }

    @Test
    fun derivesSourceMaterialAndDeploymentRecordHashesForUiTooling() {
        val materialVectors = mapOf(
            SccpSourceProofs.DOMAIN_ETH to "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77",
            SccpSourceProofs.DOMAIN_BSC to "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a",
            SccpSourceProofs.DOMAIN_SOL to "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331",
            SccpSourceProofs.DOMAIN_TON to "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc",
            SccpSourceProofs.DOMAIN_TRON to "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
            SccpSourceProofs.DOMAIN_SORA_KUSAMA to "0x012c66498a85190d6075c441fad30fe01816796ee1713838fe8bb97f2ad1c924",
            SccpSourceProofs.DOMAIN_SORA_POLKADOT to "0x40cd55d64e92d688b839242e170f1722485cddf2e42b4ff22e53c5e7723e570d",
            SccpSourceProofs.DOMAIN_SORA2 to "0x6fc968441106993502dd05ebeadea1dbfee0f7814680f1ad006d4584c99a8a2d",
        )
        val deploymentVectors = mapOf(
            SccpSourceProofs.DOMAIN_ETH to "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4",
            SccpSourceProofs.DOMAIN_BSC to "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d",
            SccpSourceProofs.DOMAIN_SOL to "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc",
            SccpSourceProofs.DOMAIN_TON to "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887",
            SccpSourceProofs.DOMAIN_TRON to "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
            SccpSourceProofs.DOMAIN_SORA_KUSAMA to "0xda47a31715813ef5bff0882cd0e0e8b0cc89d426e005e37e0f94a2bdba2043cd",
            SccpSourceProofs.DOMAIN_SORA_POLKADOT to "0x2a57fe4beb69e8201299f2c01259a025cafc8388bb38e2a727c2fc872893e13a",
            SccpSourceProofs.DOMAIN_SORA2 to "0xdac819bff0aa57f7596f06297dfec39027aaab63213497020b772c355a6eaecb",
        )
        materialVectors.forEach { (domain, expectedMaterialHash) ->
            assertTrue(this.sampleSourceVerifierMaterialBytes(domain).isNotEmpty())
            assertEquals(expectedMaterialHash, this.sampleSourceVerifierMaterialHash(domain))
            assertEquals(deploymentVectors[domain], this.sampleSourceAdapterDeploymentHash(domain))
        }
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                    sourceTrustAnchorHash = "0x" + "44".repeat(32),
                    consensusVerifierHash = "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = "0x" + "66".repeat(32),
                    finalityPolicyHash = "0x" + "88".repeat(32),
                    sourceStateVerifierHash = "0x" + "77".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                    sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                )
            }.message.orEmpty().contains("sourceStateVerifierHash"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_SOL,
                    sourceTrustAnchorHash = "0x" + "44".repeat(32),
                    consensusVerifierHash = "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = "0x" + "66".repeat(32),
                    finalityPolicyHash = "0x" + "88".repeat(32),
                    sourceStateVerifierHash = "0x" + "77".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                )
            }.message.orEmpty().contains("sourceBridgeEmitterAddress"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                    sourceTrustAnchorHash = "0x" + "44".repeat(32),
                    consensusVerifierHash = "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = "0x" + "66".repeat(32),
                    finalityPolicyHash = "0x" + "88".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                    sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                    networkId = "0x" + "33".repeat(32),
                    configHash = "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b",
                )
            }.message.orEmpty().contains("sourceBridgeNetworkId"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                    sourceTrustAnchorHash = "0x" + "44".repeat(32),
                    consensusVerifierHash = "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = "0x" + "66".repeat(32),
                    finalityPolicyHash = "0x" + "88".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                    sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                    networkId = SccpSourceProofs.ETH_MAINNET_NETWORK_ID,
                    ownerAddress = "0x" + "22".repeat(20),
                    configHash = "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b",
                )
            }.message.orEmpty().contains("sourceBridgeOwnerAddress"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                    sourceTrustAnchorHash = "0x" + "44".repeat(32),
                    consensusVerifierHash = "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = "0x" + "66".repeat(32),
                    finalityPolicyHash = "0x" + "88".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                    sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                    networkId = SccpSourceProofs.ETH_MAINNET_NETWORK_ID,
                    configHash = "0x" + "99".repeat(32),
                )
            }.message.orEmpty().contains("sourceBridgeConfigHash"),
        )
        val tonTemplateComponentHashes = mapOf(
            "sourceTrustAnchorHash" to "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
            "consensusVerifierHash" to "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
            "messageInclusionVerifierHash" to "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
            "sourceStateVerifierHash" to "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
            "finalityPolicyHash" to "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
        )
        tonTemplateComponentHashes.forEach { (field, templateHash) ->
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_TON,
                    sourceTrustAnchorHash = if (field == "sourceTrustAnchorHash") templateHash else "0x" + "44".repeat(32),
                    consensusVerifierHash = if (field == "consensusVerifierHash") templateHash else "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = if (field == "messageInclusionVerifierHash") templateHash else "0x" + "66".repeat(32),
                    finalityPolicyHash = if (field == "finalityPolicyHash") templateHash else "0x" + "88".repeat(32),
                    sourceStateVerifierHash = if (field == "sourceStateVerifierHash") templateHash else "0x" + "77".repeat(32),
                )
            }
        }
        val tronTemplateComponentHashes = mapOf(
            "sourceTrustAnchorHash" to "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c",
            "consensusVerifierHash" to "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea",
            "messageInclusionVerifierHash" to "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc",
            "finalityPolicyHash" to "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864",
        )
        tronTemplateComponentHashes.forEach { (field, templateHash) ->
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_TRON,
                    sourceTrustAnchorHash = if (field == "sourceTrustAnchorHash") templateHash else "0x" + "44".repeat(32),
                    consensusVerifierHash = if (field == "consensusVerifierHash") templateHash else "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = if (field == "messageInclusionVerifierHash") templateHash else "0x" + "66".repeat(32),
                    finalityPolicyHash = if (field == "finalityPolicyHash") templateHash else "0x" + "88".repeat(32),
                    bridgeAddress = "0x" + "11".repeat(20),
                    sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                    networkId = "0x" + "33".repeat(32),
                    ownerAddress = "0x" + "22".repeat(20),
                    configHash = "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d",
                )
            }
        }
        val solanaTemplateComponentHashes = mapOf(
            "sourceTrustAnchorHash" to "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
            "consensusVerifierHash" to "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
            "messageInclusionVerifierHash" to "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
            "sourceStateVerifierHash" to SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
            "finalityPolicyHash" to "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
        )
        solanaTemplateComponentHashes.forEach { (field, templateHash) ->
            val error = assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                    sourceDomain = SccpSourceProofs.DOMAIN_SOL,
                    sourceTrustAnchorHash = if (field == "sourceTrustAnchorHash") templateHash else "0x" + "44".repeat(32),
                    consensusVerifierHash = if (field == "consensusVerifierHash") templateHash else "0x" + "55".repeat(32),
                    messageInclusionVerifierHash = if (field == "messageInclusionVerifierHash") templateHash else "0x" + "66".repeat(32),
                    finalityPolicyHash = if (field == "finalityPolicyHash") templateHash else "0x" + "88".repeat(32),
                    sourceStateVerifierHash = if (field == "sourceStateVerifierHash") templateHash else "0x" + "77".repeat(32),
                )
            }
            assertTrue(
                error.message!!.contains("Solana template verifier hash") ||
                    error.message!!.contains("Solana template component hash"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_TRON,
                sourceTrustAnchorHash = "0x" + "44".repeat(32),
                consensusVerifierHash = "0x" + "55".repeat(32),
                messageInclusionVerifierHash = "0x" + "66".repeat(32),
                finalityPolicyHash = "0x" + "88".repeat(32),
                bridgeAddress = "0x" + "11".repeat(20),
                sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                networkId = "0x" + "33".repeat(32),
                ownerAddress = "0x" + "22".repeat(20),
                configHash = "0x" + "99".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                sourceTrustAnchorHash = "0x" + "44".repeat(32),
                consensusVerifierHash = "0x" + "44".repeat(32),
                messageInclusionVerifierHash = "0x" + "66".repeat(32),
                finalityPolicyHash = "0x" + "88".repeat(32),
                bridgeAddress = "0x" + "11".repeat(20),
                sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                networkId = SccpSourceProofs.ETH_MAINNET_NETWORK_ID,
                configHash = "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSourceAdapterEngineDeploymentBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                sourceTrustAnchorHash = "0x" + "44".repeat(32),
                consensusVerifierHash = "0x" + "55".repeat(32),
                messageInclusionVerifierHash = "0x" + "66".repeat(32),
                finalityPolicyHash = "0x" + "88".repeat(32),
                deploymentReceiptHash = SccpSourceProofs.sourceAdapterVerifierVkHash(
                    SccpSourceProofs.DOMAIN_ETH,
                ),
                bridgeAddress = "0x" + "11".repeat(20),
                sourceBridgeEmitterCodeHash = "0x" + "77".repeat(32),
                networkId = SccpSourceProofs.ETH_MAINNET_NETWORK_ID,
                configHash = "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b",
            )
        }
        assertEquals(
            "0x97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61",
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_SOL,
                solanaTowerReplayVerifierHash = "0x" + "bb".repeat(32),
                solanaFullAccountsdbLatticeVerifierHash = "0x" + "cc".repeat(32),
                solanaBankForkChoiceVerifierHash = "0x" + "dd".repeat(32),
            ),
        )
        assertEquals(
            "0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9",
            this.sampleSolanaFullLightClientGateHash(),
        )
        assertFailsWith<IllegalArgumentException> {
            this.sampleSolanaFullLightClientGateHash(towerReplayHash = "0x" + "00".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleSolanaFullLightClientGateHash(
                towerReplayHash = "0x" + "bb".repeat(32),
                fullAccountsdbLatticeHash = "0x" + "bb".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleSolanaFullLightClientGateHash(
                towerReplayHash = sourceStateVerifierHash(SccpSourceProofs.DOMAIN_SOL)!!,
            )
        }
        val solanaTemplateAuditError = assertFailsWith<IllegalArgumentException> {
            this.sampleSolanaFullLightClientGateHash(
                towerReplayHash =
                    "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
            )
        }
        assertTrue(solanaTemplateAuditError.message!!.contains("template material"))
        val solanaTemplateStateError = assertFailsWith<IllegalArgumentException> {
            this.sampleSolanaFullLightClientGateHash(
                sourceStateHash = SccpSolana.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
            )
        }
        assertTrue(
            solanaTemplateStateError.message!!.contains("Solana template verifier hash"),
        )
        assertFailsWith<IllegalArgumentException> {
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_SOL,
                solanaTowerReplayVerifierHash = "0x" + "bb".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_TON,
                solanaTowerReplayVerifierHash = "0x" + "bb".repeat(32),
                solanaFullAccountsdbLatticeVerifierHash = "0x" + "cc".repeat(32),
                solanaBankForkChoiceVerifierHash = "0x" + "dd".repeat(32),
            )
        }
        assertEquals(
            "0x61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07",
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_TON,
                tonMasterchainConfigVerifierHash = "0x" + "bb".repeat(32),
                tonValidatorSetTransitionVerifierHash = "0x" + "cc".repeat(32),
                tonShardAccountsDictionaryVerifierHash = "0x" + "dd".repeat(32),
            ),
        )
        assertEquals(
            "0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596",
            this.sampleTonFullLightClientGateHash(),
        )
        assertFailsWith<IllegalArgumentException> {
            this.sampleTonFullLightClientGateHash(masterchainConfigHash = "0x" + "00".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleTonFullLightClientGateHash(
                masterchainConfigHash = "0x" + "bb".repeat(32),
                validatorSetTransitionHash = "0x" + "bb".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleTonFullLightClientGateHash(
                masterchainConfigHash = sourceStateVerifierHash(SccpSourceProofs.DOMAIN_TON)!!,
            )
        }
        val tonTemplateAuditError = assertFailsWith<IllegalArgumentException> {
            this.sampleTonFullLightClientGateHash(
                masterchainConfigHash = tonTemplateComponentHashes.getValue("sourceTrustAnchorHash"),
            )
        }
        assertTrue(tonTemplateAuditError.message!!.contains("template material"))
        assertFailsWith<IllegalArgumentException> {
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_TON,
                tonMasterchainConfigVerifierHash = "0x" + "bb".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_SOL,
                tonMasterchainConfigVerifierHash = "0x" + "bb".repeat(32),
                tonValidatorSetTransitionVerifierHash = "0x" + "cc".repeat(32),
                tonShardAccountsDictionaryVerifierHash = "0x" + "dd".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            this.sampleSourceAdapterDeploymentHash(
                SccpSourceProofs.DOMAIN_ETH,
                adapterVerifierVkHash = "0x" + "99".repeat(32),
            )
        }
    }

    @Test
    fun derivesSourceProofHashesFromWitnessMaterial() {
        val sourceEventDigest = "34".repeat(32)
        val zeroSourceEventDigest = "00".repeat(32)
        val branch = listOf(ByteArray(32) { 0xee.toByte() })
        val changedBranch = listOf(ByteArray(32) { 0x12.toByte() })
        val evmReceiptRootMptValueHex =
            "f8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0" + "bb".repeat(32)
        val evmReceiptTrieProofNodes = listOf(hexBytes("f847822080b842$evmReceiptRootMptValueHex"))
        val evmReceiptsRoot = "6438aaabb78989f2803c6b0f227ee0f94beecde07cdd9c737e258e4faf581b68"

        val evmBytes = SccpSourceProofs.canonicalEvmReceiptProofBytes(
            sourceEventDigest = sourceEventDigest,
            beaconSlot = "11",
            executionBlockNumber = "12",
            executionBlockHash = "aa".repeat(32),
            executionReceiptsRoot = evmReceiptsRoot,
            beaconFinalizedRoot = "cc".repeat(32),
            syncCommitteeRoot = "dd".repeat(32),
            receiptRootIndex = "0",
            receiptTrieProofNodes = evmReceiptTrieProofNodes,
            inclusionBranch = branch,
        )
        assertEquals(306, evmBytes.size)
        val evmHash = SccpSourceProofs.evmReceiptProofHash(
            sourceEventDigest = sourceEventDigest,
            beaconSlot = "11",
            executionBlockNumber = "12",
            executionBlockHash = "aa".repeat(32),
            executionReceiptsRoot = evmReceiptsRoot,
            beaconFinalizedRoot = "cc".repeat(32),
            syncCommitteeRoot = "dd".repeat(32),
            receiptRootIndex = "0",
            receiptTrieProofNodes = evmReceiptTrieProofNodes,
            inclusionBranch = branch,
        )
        val changedEvmHash = SccpSourceProofs.evmReceiptProofHash(
            sourceEventDigest = sourceEventDigest,
            beaconSlot = "11",
            executionBlockNumber = "12",
            executionBlockHash = "aa".repeat(32),
            executionReceiptsRoot = evmReceiptsRoot,
            beaconFinalizedRoot = "cc".repeat(32),
            syncCommitteeRoot = "dd".repeat(32),
            receiptRootIndex = "0",
            receiptTrieProofNodes = evmReceiptTrieProofNodes,
            inclusionBranch = changedBranch,
        )
        assertTrue(evmHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(evmHash != changedEvmHash)
        val zeroEvmDigest = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptProofBytes(
                sourceEventDigest = zeroSourceEventDigest,
                beaconSlot = "11",
                executionBlockNumber = "12",
                executionBlockHash = "aa".repeat(32),
                executionReceiptsRoot = evmReceiptsRoot,
                beaconFinalizedRoot = "cc".repeat(32),
                syncCommitteeRoot = "dd".repeat(32),
                receiptRootIndex = "0",
                receiptTrieProofNodes = evmReceiptTrieProofNodes,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroEvmDigest.message.orEmpty().contains("sourceEventDigest must not be zero"))

        assertEquals(
            306,
            SccpSourceProofs.canonicalBscReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                validatorEpoch = "21",
                blockNumber = "22",
                blockHash = "aa".repeat(32),
                receiptsRoot = evmReceiptsRoot,
                validatorSetHash = "cc".repeat(32),
                commitSealHash = "dd".repeat(32),
                receiptRootIndex = "0",
                receiptTrieProofNodes = evmReceiptTrieProofNodes,
                inclusionBranch = branch,
            ).size,
        )
        val zeroBscDigest = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscReceiptProofBytes(
                sourceEventDigest = zeroSourceEventDigest,
                validatorEpoch = "21",
                blockNumber = "22",
                blockHash = "aa".repeat(32),
                receiptsRoot = evmReceiptsRoot,
                validatorSetHash = "cc".repeat(32),
                commitSealHash = "dd".repeat(32),
                receiptRootIndex = "0",
                receiptTrieProofNodes = evmReceiptTrieProofNodes,
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroBscDigest.message.orEmpty().contains("sourceEventDigest must not be zero"))
        assertTrue(
            SccpSourceProofs.bscReceiptProofHash(
                sourceEventDigest = sourceEventDigest,
                validatorEpoch = "21",
                blockNumber = "22",
                blockHash = "aa".repeat(32),
                receiptsRoot = evmReceiptsRoot,
                validatorSetHash = "cc".repeat(32),
                commitSealHash = "dd".repeat(32),
                receiptRootIndex = "0",
                receiptTrieProofNodes = evmReceiptTrieProofNodes,
                inclusionBranch = branch,
            ) != SccpSourceProofs.bscReceiptProofHash(
                sourceEventDigest = sourceEventDigest,
                validatorEpoch = "21",
                blockNumber = "22",
                blockHash = "aa".repeat(32),
                receiptsRoot = evmReceiptsRoot,
                validatorSetHash = "cc".repeat(32),
                commitSealHash = "dd".repeat(32),
                receiptRootIndex = "0",
                receiptTrieProofNodes = evmReceiptTrieProofNodes,
                inclusionBranch = changedBranch,
            ),
        )
        val validatorPayload = SccpSourceProofs.canonicalBscValidatorSetPayloadBytes(
            validatorAddresses = listOf("11".repeat(20), "22".repeat(20)),
            validatorPowers = listOf("1", "2"),
        )
        assertEquals(
            "0102000000${"11".repeat(20)}0100000000000000${"22".repeat(20)}0200000000000000",
            validatorPayload.joinToString("") { "%02x".format(it.toInt() and 0xff) },
        )
        assertEquals(
            "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370",
            SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
        )
        assertEquals(
            "0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370",
            SccpSourceProofs.bscValidatorSetPayloadHash(
                validatorAddresses = listOf("11".repeat(20), "22".repeat(20)),
                validatorPowers = listOf("1", "2"),
            ),
        )
        assertEquals(
            "0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762",
            SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
        )
        val bscCommitValidatorPublicKeys = listOf(
            hexBytes("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
            hexBytes("02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"),
            hexBytes("02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"),
            hexBytes("02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13"),
        )
        val bscCommitValidatorSetHash =
            "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c"
        val bscCommitMessageHash =
            "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7"
        val bscCommitSignatures = listOf(
            hexBytes("1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f00"),
            hexBytes("921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64facf6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f001"),
            hexBytes("cfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d3619600"),
        )
        assertEquals(
            117,
            SccpSourceProofs.canonicalBscCommitMessageBytes(
                validatorEpoch = "2",
                blockNumber = "401",
                blockHash = "22".repeat(32),
                receiptsRoot = "33".repeat(32),
                validatorSetHash = bscCommitValidatorSetHash,
            ).size,
        )
        assertEquals(
            bscCommitMessageHash,
            SccpSourceProofs.bscCommitMessageHash(
                validatorEpoch = "2",
                blockNumber = "401",
                blockHash = "22".repeat(32),
                receiptsRoot = "33".repeat(32),
                validatorSetHash = bscCommitValidatorSetHash,
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.bscCommitMessageHash(
                validatorEpoch = "2",
                blockNumber = "401",
                blockHash = "22".repeat(32),
                receiptsRoot = "33".repeat(32),
                validatorSetHash = bscCommitValidatorSetHash,
                sourceDomain = SccpSourceProofs.DOMAIN_ETH,
            )
        }
        val bscCommitSeal = SccpSourceProofs.BscCommitSealProof(
            totalPower = "4",
            signedPower = "3",
            commitMessageHash = bscCommitMessageHash,
            validatorPublicKeys = bscCommitValidatorPublicKeys,
            validatorPowers = listOf("1", "1", "1", "1"),
            signersBitmap = hexBytes("07"),
            signatures = bscCommitSignatures,
            validatorSetHash = bscCommitValidatorSetHash,
        )
        assertEquals(297, SccpSourceProofs.canonicalBscCommitSealBytes(bscCommitSeal).size)
        assertEquals(
            "0xcd9d87b24d8c1cf7615cb4267cde5a3fc24bbb770807134ee75d4ddaba992172",
            SccpSourceProofs.bscCommitSealHash(bscCommitSeal),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalBscCommitSealBytes(
                    bscCommitSeal.copy(
                        signedPower = "2",
                        signersBitmap = hexBytes("03"),
                        signatures = bscCommitSignatures.take(2),
                    ),
                )
            }.message!!.contains("two thirds"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalBscCommitSealBytes(bscCommitSeal.copy(signersBitmap = hexBytes("1f")))
            }.message!!.contains("padding bits"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalBscCommitSealBytes(
                    bscCommitSeal.copy(
                        signatures = listOf(
                            hexBytes("31" + bscCommitSignatures[0].hex().drop(2)),
                            bscCommitSignatures[1],
                            bscCommitSignatures[2],
                        ),
                    ),
                )
            }.message!!.contains("recover"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalBscCommitSealBytes(bscCommitSeal.copy(validatorSetHash = "aa".repeat(32)))
            }.message!!.contains("validatorSetHash"),
        )
        val storageValue = hexBytes("02")
        val storageValueHash = SccpSourceProofs.bscValidatorSetStorageValueHash(storageValue)
        val metadataProof = SccpSourceProofs.BscValidatorSetMetadataProof(
            stateRoot = "aa".repeat(32),
            nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
            validatorContractAddress = hexBytes("00".repeat(18) + "1000"),
            accountProofNodes = listOf(hexBytes("f842a0" + "11".repeat(32))),
            storageRoot = "bb".repeat(32),
            validatorSetLengthSlot = "cc".repeat(32),
            validatorSetLengthValue = storageValue,
            validatorSetLengthValueHash = storageValueHash,
            validatorSetLengthProofNodes = listOf(hexBytes("e4822080a0" + "22".repeat(32))),
            validatorStorageProofs = listOf(
                SccpSourceProofs.BscValidatorStorageProof(
                    validatorIndex = 0,
                    storageSlot = "dd".repeat(32),
                    storageValue = hexBytes("94" + "11".repeat(20)),
                    storageValueHash = SccpSourceProofs.bscValidatorSetStorageValueHash(
                        hexBytes("94" + "11".repeat(20)),
                    ),
                    storageProofNodes = listOf(hexBytes("e4822080a0" + "33".repeat(32))),
                ),
                SccpSourceProofs.BscValidatorStorageProof(
                    validatorIndex = 1,
                    storageSlot = "ee".repeat(32),
                    storageValue = hexBytes("94" + "22".repeat(20)),
                    storageValueHash = SccpSourceProofs.bscValidatorSetStorageValueHash(
                        hexBytes("94" + "22".repeat(20)),
                    ),
                    storageProofNodes = listOf(hexBytes("e4822080a0" + "44".repeat(32))),
                ),
            ),
        )
        assertEquals(560, SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(metadataProof).size)
        val metadataHash = SccpSourceProofs.bscValidatorSetMetadataProofHash(metadataProof)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(metadataProof.copy(version = 0))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(
                    validatorStorageProofs = listOf(
                        metadataProof.validatorStorageProofs.first().copy(version = 0),
                    ),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(validatorContractAddress = ByteArray(19) { 0x12.toByte() }),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(accountProofNodes = emptyList()),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(validatorSetLengthProofNodes = emptyList()),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(validatorStorageProofs = emptyList()),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(
                    validatorStorageProofs = listOf(
                        metadataProof.validatorStorageProofs.first().copy(storageProofNodes = emptyList()),
                    ),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(validatorSetLengthValueHash = "ff".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetMetadataProofBytes(
                metadataProof.copy(
                    validatorStorageProofs = listOf(
                        metadataProof.validatorStorageProofs.first().copy(storageValueHash = "ff".repeat(32)),
                    ),
                ),
            )
        }
        assertTrue(metadataHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            metadataHash != SccpSourceProofs.bscValidatorSetMetadataProofHash(
                metadataProof.copy(stateRoot = "12".repeat(32)),
            ),
        )
        assertEquals(
            189,
            SccpSourceProofs.canonicalBscValidatorSetTransitionMessageBytes(
                fromValidatorEpoch = "41",
                toValidatorEpoch = "42",
                transitionBlockNumber = "8400",
                transitionBlockHash = "aa".repeat(32),
                parentValidatorSetHash = "bb".repeat(32),
                nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                validatorSetMetadataProofHash = metadataHash,
            ).size,
        )
        assertTrue(
            SccpSourceProofs.bscValidatorSetTransitionMessageHash(
                fromValidatorEpoch = "41",
                toValidatorEpoch = "42",
                transitionBlockNumber = "8400",
                transitionBlockHash = "aa".repeat(32),
                parentValidatorSetHash = "bb".repeat(32),
                nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                validatorSetMetadataProofHash = metadataHash,
            ) != SccpSourceProofs.bscValidatorSetTransitionMessageHash(
                fromValidatorEpoch = "41",
                toValidatorEpoch = "42",
                transitionBlockNumber = "8400",
                transitionBlockHash = "aa".repeat(32),
                parentValidatorSetHash = "bb".repeat(32),
                nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                validatorSetMetadataProofHash = "12".repeat(32),
            ),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.bscValidatorSetTransitionMessageHash(
                    fromValidatorEpoch = "41",
                    toValidatorEpoch = "42",
                    transitionBlockNumber = "8401",
                    transitionBlockHash = "aa".repeat(32),
                    parentValidatorSetHash = "bb".repeat(32),
                    nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                    nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                    validatorSetMetadataProofHash = metadataHash,
                )
            }.message!!.contains("epoch-start block"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.bscValidatorSetTransitionMessageHash(
                    fromValidatorEpoch = "41",
                    toValidatorEpoch = "43",
                    transitionBlockNumber = "8400",
                    transitionBlockHash = "aa".repeat(32),
                    parentValidatorSetHash = "bb".repeat(32),
                    nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                    nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                    validatorSetMetadataProofHash = metadataHash,
                )
            }.message!!.contains("fromValidatorEpoch"),
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.bscValidatorSetTransitionMessageHash(
                    fromValidatorEpoch = "41",
                    toValidatorEpoch = "42",
                    transitionBlockNumber = "8400",
                    transitionBlockHash = "aa".repeat(32),
                    parentValidatorSetHash = "bb".repeat(32),
                    nextValidatorSetHash = SccpSourceProofs.bscValidatorSetHashFromPayload(validatorPayload),
                    nextValidatorSetPayloadHash = SccpSourceProofs.bscValidatorSetPayloadHash(validatorPayload),
                    validatorSetMetadataProofHash = metadataHash,
                    sourceDomain = 0,
                )
            }.message!!.contains("sourceDomain"),
        )
        val parliaPayload = SccpSourceProofs.canonicalBscValidatorSetPayloadBytes(
            validatorAddresses = listOf("11".repeat(20), "22".repeat(20)),
            validatorPowers = listOf("1", "1"),
        )
        val parliaExtra = sampleBscParliaExtra()
        assertEquals(parliaPayload.hex(), SccpSourceProofs.bscValidatorSetPayloadFromParliaExtra(parliaExtra).hex())
        assertEquals(
            parliaPayload.hex(),
            SccpSourceProofs.bscValidatorSetPayloadFromHeaderRlp(sampleBscParliaHeaderRlp(parliaExtra)).hex(),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.bscValidatorSetPayloadFromHeaderRlp(byteArrayOf(0x80.toByte()))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetPayloadBytes(
                validatorAddresses = listOf("11".repeat(20), "11".repeat(20)),
                validatorPowers = listOf("1", "2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetPayloadBytes(
                validatorAddresses = listOf("11".repeat(20)),
                validatorPowers = listOf("0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalBscValidatorSetPayloadBytes(
                validatorAddresses = (1..256).map { it.toString(16).padStart(40, '0') },
                validatorPowers = List(256) { "1" },
            )
        }
        val tonValidatorPublicKeys = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() })
        val tonValidatorWeights = listOf("1", "2")
        val tonValidatorSetPayload = SccpSourceProofs.canonicalTonValidatorSetPayloadBytes(
            validatorPublicKeys = tonValidatorPublicKeys,
            validatorWeights = tonValidatorWeights,
        )
        assertEquals(
            "0102000000${"11".repeat(32)}0100000000000000${"22".repeat(32)}0200000000000000",
            tonValidatorSetPayload.hex(),
        )
        assertEquals(
            "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            SccpSourceProofs.tonValidatorSetHash(tonValidatorPublicKeys, tonValidatorWeights),
        )
        assertEquals(
            "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0",
            SccpSourceProofs.tonValidatorSetPayloadHash(tonValidatorSetPayload),
        )
        assertEquals(
            365,
            SccpSourceProofs.canonicalTonMasterchainBlockMessageBytes(
                masterchainSeqno = "19",
                masterchainWorkchainId = -1,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                masterchainConfigProofHash = "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                shardProofHash = "ee".repeat(32),
            ).size,
        )
        val tonBlockMessageHash = SccpSourceProofs.tonMasterchainBlockMessageHash(
            masterchainSeqno = "19",
            masterchainWorkchainId = -1,
            masterchainShard = "9223372036854775808",
            masterchainBlockHash = "aa".repeat(32),
            masterchainFileHash = "a5".repeat(32),
            validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
            masterchainConfigProofHash = "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
            shardWorkchainId = 0,
            shardShard = "9223372036854775808",
            shardSeqno = "7",
            shardBlockHash = "bb".repeat(32),
            shardFileHash = "bc".repeat(32),
            shardStateRoot = "cc".repeat(32),
            transactionRoot = "dd".repeat(32),
            shardProofHash = "ee".repeat(32),
        )
        assertEquals(
            "0xa00389d016059db04cc59c3032047ffb214782d4aa747302568636344fa7c74f",
            tonBlockMessageHash,
        )
        val tonSignatureProof = SccpSourceProofs.TonValidatorSignatureProof(
            totalWeight = "3",
            signedWeight = "3",
            blockMessageHash = tonBlockMessageHash,
            validatorPublicKeys = tonValidatorPublicKeys,
            validatorWeights = tonValidatorWeights,
            signersBitmap = byteArrayOf(0x03),
            signatures = listOf(ByteArray(64) { 0xab.toByte() }, ByteArray(64) { 0xcd.toByte() }),
            validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
        )
        assertEquals(322, SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(tonSignatureProof).size)
        assertEquals(
            "0xc31577a0488fe754d44eb0aafae46a8e4be36b0088b0cdec4ad34f8d0a7acedd",
            SccpSourceProofs.tonMasterchainValidatorSignaturesHash(tonSignatureProof),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainBlockMessageBytes(
                masterchainSeqno = "19",
                masterchainWorkchainId = 0,
                masterchainShard = "9223372036854775808",
                masterchainBlockHash = "aa".repeat(32),
                masterchainFileHash = "a5".repeat(32),
                validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                masterchainConfigRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                masterchainConfigProofHash = "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
                shardWorkchainId = 0,
                shardShard = "9223372036854775808",
                shardSeqno = "7",
                shardBlockHash = "bb".repeat(32),
                shardFileHash = "bc".repeat(32),
                shardStateRoot = "cc".repeat(32),
                transactionRoot = "dd".repeat(32),
                shardProofHash = "ee".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(validatorSetHash = "bb".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(version = 0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(totalWeight = "4"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(signedWeight = "2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    signedWeight = "1",
                    signersBitmap = byteArrayOf(0x01),
                    signatures = listOf(ByteArray(64) { 0xab.toByte() }),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    signedWeight = "0",
                    signersBitmap = byteArrayOf(0x00),
                    signatures = emptyList(),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    signersBitmap = byteArrayOf(0x04),
                    signatures = emptyList(),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    signatures = listOf(ByteArray(64), ByteArray(64) { 0xcd.toByte() }),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    signatures = listOf(ByteArray(63) { 0xab.toByte() }, ByteArray(64) { 0xcd.toByte() }),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                tonSignatureProof.copy(
                    validatorPublicKeys = listOf(ByteArray(32), tonValidatorPublicKeys[1]),
                ),
            )
        }
        val zeroTonValidatorSetPayload =
            byteArrayOf(1, 1, 0, 0, 0) + ByteArray(32) + byteArrayOf(1, 0, 0, 0, 0, 0, 0, 0)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tonValidatorSetHashFromPayload(zeroTonValidatorSetPayload)
        }
        val parentSyncPublicKeys =
            listOf(ByteArray(48) { 0x11.toByte() }, ByteArray(48) { 0x22.toByte() })
        val parentSyncWeights = listOf("1", "2")
        val parentSyncPops =
            listOf(ByteArray(96) { 0xaa.toByte() }, ByteArray(96) { 0xbb.toByte() })
        val nextSyncPayload = SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys = listOf(ByteArray(48) { 0x33.toByte() }, ByteArray(48) { 0x44.toByte() }),
            syncCommitteeWeights = listOf("3", "4"),
            syncCommitteePops = listOf(ByteArray(96) { 0xcc.toByte() }, ByteArray(96) { 0xdd.toByte() }),
        )
        assertEquals(
            "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
            SccpSourceProofs.ethSyncCommitteeHash(parentSyncPublicKeys, parentSyncWeights, parentSyncPops),
        )
        assertEquals(
            "010200000030000000${"33".repeat(48)}030000000000000060000000${"cc".repeat(96)}" +
                "30000000${"44".repeat(48)}040000000000000060000000${"dd".repeat(96)}",
            nextSyncPayload.joinToString("") { "%02x".format(it.toInt() and 0xff) },
        )
        assertEquals(
            "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
            SccpSourceProofs.ethSyncCommitteeHashFromPayload(nextSyncPayload),
        )
        assertEquals(
            "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
            SccpSourceProofs.ethSyncCommitteePayloadHash(nextSyncPayload),
        )
        val ethTransitionMessageHash = SccpSourceProofs.ethSyncCommitteeTransitionMessageHash(
            fromSyncPeriod = "7",
            toSyncPeriod = "8",
            transitionSlot = "19",
            finalizedBeaconRoot = "aa".repeat(32),
            parentSyncCommitteeHash = "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
            nextSyncCommitteeHash = "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
            nextSyncCommitteePayloadHash = "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
            nextSyncCommitteeBranchHash = "be".repeat(32),
        )
        assertEquals(
            "0xc5cbfaf915a63e59bc142277814f13fab1e8012a0bd56db7033b18bc02637bec",
            ethTransitionMessageHash,
        )
        assertEquals(
            1068,
            SccpSourceProofs.canonicalEthSyncCommitteeTransitionSignatureBytes(
                fromSyncPeriod = "7",
                toSyncPeriod = "8",
                transitionSlot = "19",
                finalizedBeaconRoot = "aa".repeat(32),
                parentSyncCommitteeHash = "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                nextSyncCommitteeHash = "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncCommitteePayload = nextSyncPayload,
                nextSyncCommitteePayloadHash = "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                nextSyncCommitteeBranchHash = "be".repeat(32),
                transitionMessageHash = ethTransitionMessageHash,
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            ).size,
        )
        assertEquals(
            "0x2d03886e7ea307f7b5a77af00075b32536cbf016d0d8554bec2b1e424252f858",
            SccpSourceProofs.ethSyncCommitteeTransitionSignatureHash(
                fromSyncPeriod = "7",
                toSyncPeriod = "8",
                transitionSlot = "19",
                finalizedBeaconRoot = "aa".repeat(32),
                parentSyncCommitteeHash = "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                nextSyncCommitteeHash = "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncCommitteePayload = nextSyncPayload,
                nextSyncCommitteePayloadHash = "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                nextSyncCommitteeBranchHash = "be".repeat(32),
                transitionMessageHash = ethTransitionMessageHash,
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthSyncCommitteeTransitionSignatureBytes(
                fromSyncPeriod = "7",
                toSyncPeriod = "8",
                transitionSlot = "19",
                finalizedBeaconRoot = "aa".repeat(32),
                parentSyncCommitteeHash = "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                nextSyncCommitteeHash = "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncCommitteePayload = nextSyncPayload,
                nextSyncCommitteePayloadHash = "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                nextSyncCommitteeBranchHash = "be".repeat(32),
                transitionMessageHash = ethTransitionMessageHash,
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
                version = 0,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
                version = 0,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys = List(513) { ByteArray(48) { 0x11.toByte() } },
                syncCommitteeWeights = List(513) { "1" },
                syncCommitteePops = List(513) { ByteArray(96) { 0xaa.toByte() } },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys = listOf(ByteArray(47) { 0x11.toByte() }, parentSyncPublicKeys[1]),
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys = listOf(ByteArray(48), parentSyncPublicKeys[1]),
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = listOf(ByteArray(96), parentSyncPops[1]),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = ByteArray(65),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "0",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x00),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x04),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "2",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x01),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "4",
                signedWeight = "3",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "1",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x01),
                aggregateSignature = ByteArray(96) { 0xee.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                totalWeight = "3",
                signedWeight = "3",
                syncCommitteeMessageHash = ethTransitionMessageHash,
                syncCommitteePublicKeys = parentSyncPublicKeys,
                syncCommitteeWeights = parentSyncWeights,
                syncCommitteePops = parentSyncPops,
                signersBitmap = byteArrayOf(0x03),
                aggregateSignature = ByteArray(96),
            )
        }
        val witnessPayload = SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
            witnessAddresses = listOf("41" + "11".repeat(20), "41" + "22".repeat(20)),
            witnessWeights = listOf("1", "2"),
        )
        assertEquals(
            "010200000041${"11".repeat(20)}010000000000000041${"22".repeat(20)}0200000000000000",
            witnessPayload.joinToString("") { "%02x".format(it.toInt() and 0xff) },
        )
        assertEquals(
            "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429",
            SccpSourceProofs.tronWitnessSchedulePayloadHash(witnessPayload),
        )
        assertEquals(
            "0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429",
            SccpSourceProofs.tronWitnessSchedulePayloadHash(
                witnessAddresses = listOf("41" + "11".repeat(20), "41" + "22".repeat(20)),
                witnessWeights = listOf("1", "2"),
            ),
        )
        assertEquals(
            "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be",
            SccpSourceProofs.tronWitnessScheduleHashFromPayload(witnessPayload),
        )
        val zeroWitnessPayload = hexBytes("010100000041${"00".repeat(20)}0100000000000000")
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronWitnessSchedulePayloadHash(zeroWitnessPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronWitnessScheduleHashFromPayload(zeroWitnessPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
                witnessAddresses = (0 until 65).map { "41" + "11".repeat(19) + it.toString(16).padStart(2, '0') },
                witnessWeights = List(65) { "1" },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
                witnessAddresses = listOf("41" + "11".repeat(20), "41" + "11".repeat(20)),
                witnessWeights = listOf("1", "2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
                witnessAddresses = listOf("41" + "00".repeat(20)),
                witnessWeights = listOf("1"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
                witnessAddresses = listOf("41" + "11".repeat(20)),
                witnessWeights = listOf("0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSchedulePayloadBytes(
                witnessAddresses = listOf("41" + "11".repeat(20), "41" + "22".repeat(20)),
                witnessWeights = listOf("18446744073709551615", "1"),
            )
        }
        val overflowingWitnessPayload = hexBytes(
            "010200000041${"11".repeat(20)}ffffffffffffffff41${"22".repeat(20)}0100000000000000",
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronWitnessSchedulePayloadHash(overflowingWitnessPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronWitnessScheduleHashFromPayload(overflowingWitnessPayload)
        }
        val tronWitnessScheduleHash = "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be"
        val tronSolidBlockMessageHash = SccpSourceProofs.tronSolidBlockMessageHash(
            sourceDomain = SccpSourceProofs.DOMAIN_TRON,
            solidBlockNumber = "12345",
            blockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            witnessScheduleHash = tronWitnessScheduleHash,
            receiptRoot = "bb".repeat(32),
            transactionRoot = "dd".repeat(32),
            receiptProofHash = "cc".repeat(32),
        )
        assertEquals(
            "0x065173d89272a549b504258936729c5226dfdb866ccb9422757d95ec9fa6d688",
            tronSolidBlockMessageHash,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockMessageBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_ETH,
                solidBlockNumber = "12345",
                blockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                witnessScheduleHash = tronWitnessScheduleHash,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                receiptProofHash = "cc".repeat(32),
            )
        }
        val tronTestOwnerAddress = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf"
        val tronSourceEventTransactionId = "be9223cdfd6728fd2512f270a44f928fbd58df98f8e9e5fe13c4dc73503192e4"
        val tronSourceEventSignature = hexBytes(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798" +
                "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b" +
                "00",
        )
        assertEquals(
            "0x4266cf4de71c96e4fde925b686abbd50e67026f63ad90e0cf4899d4925d45849",
            SccpSourceProofs.tronWitnessSealHash(
                totalWeight = "1",
                signedWeight = "1",
                solidBlockMessageHash = "0x$tronSourceEventTransactionId",
                witnessAddresses = listOf(tronTestOwnerAddress),
                witnessWeights = listOf("1"),
                signersBitmap = byteArrayOf(0x01),
                signatures = listOf(tronSourceEventSignature),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessSealBytes(
                totalWeight = "1",
                signedWeight = "1",
                solidBlockMessageHash = "0x$tronSourceEventTransactionId",
                witnessAddresses = listOf("0x41" + "11".repeat(20)),
                witnessWeights = listOf("1"),
                signersBitmap = byteArrayOf(0x01),
                signatures = listOf(tronSourceEventSignature),
            )
        }
        val parentWitnessSchedulePayload = hexBytes(
            "0101000000417e5f4552091a69125d5dfcb7b8c2659029395bdf0100000000000000",
        )
        val parentWitnessScheduleHash = "0x87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6"
        assertEquals(parentWitnessScheduleHash, SccpSourceProofs.tronWitnessScheduleHashFromPayload(parentWitnessSchedulePayload))
        val transitionMessage = SccpSourceProofs.canonicalTronWitnessScheduleTransitionMessageBytes(
            sourceDomain = SccpSourceProofs.DOMAIN_TRON,
            fromWitnessScheduleEpoch = "7",
            toWitnessScheduleEpoch = "8",
            transitionBlockNumber = "12345",
            transitionBlockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            parentWitnessScheduleHash = parentWitnessScheduleHash,
            nextWitnessScheduleHash = tronWitnessScheduleHash,
            nextWitnessSchedulePayload = witnessPayload,
        )
        assertEquals(157, transitionMessage.size)
        assertContentEquals(
            hexBytes(
                "0105000000070000000000000008000000000000003930000000000000" +
                    "0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286" +
                    "87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6" +
                    "0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be" +
                    "d6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429",
            ),
            transitionMessage,
        )
        val transitionMessageHash = "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43"
        assertEquals(
            transitionMessageHash,
            SccpSourceProofs.tronWitnessScheduleTransitionMessageHash(
                sourceDomain = SccpSourceProofs.DOMAIN_TRON,
                fromWitnessScheduleEpoch = "7",
                toWitnessScheduleEpoch = "8",
                transitionBlockNumber = "12345",
                transitionBlockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = tronWitnessScheduleHash,
                nextWitnessSchedulePayload = witnessPayload,
            ),
        )
        val transitionSignature = hexBytes(
            "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5" +
                "65d3d639f676a837945854abb3f59c4b93355bb55a789e31a25aee261500932d01",
        )
        assertEquals(
            "0xbb3b7ef87bd3efb77d9b7f0a4dba8e7398827621d59039c694c285a7e2deacce",
            SccpSourceProofs.tronWitnessScheduleTransitionSealHash(
                sourceDomain = SccpSourceProofs.DOMAIN_TRON,
                fromWitnessScheduleEpoch = "7",
                toWitnessScheduleEpoch = "8",
                transitionBlockNumber = "12345",
                transitionBlockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = tronWitnessScheduleHash,
                nextWitnessSchedulePayload = witnessPayload,
                transitionMessageHash = transitionMessageHash,
                totalWeight = "1",
                signedWeight = "1",
                witnessAddresses = listOf(tronTestOwnerAddress),
                witnessWeights = listOf("1"),
                signersBitmap = byteArrayOf(0x01),
                signatures = listOf(transitionSignature),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronWitnessScheduleTransitionSealBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_TRON,
                fromWitnessScheduleEpoch = "7",
                toWitnessScheduleEpoch = "8",
                transitionBlockNumber = "12345",
                transitionBlockHash = "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                parentWitnessScheduleHash = parentWitnessScheduleHash,
                nextWitnessScheduleHash = tronWitnessScheduleHash,
                nextWitnessSchedulePayload = witnessPayload,
                transitionMessageHash = "0x" + "dd".repeat(32),
                totalWeight = "1",
                signedWeight = "1",
                witnessAddresses = listOf(tronTestOwnerAddress),
                witnessWeights = listOf("1"),
                signersBitmap = byteArrayOf(0x01),
                signatures = listOf(transitionSignature),
            )
        }
        val authorityPayload = SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            authorityPublicKeys = listOf("11".repeat(32), "22".repeat(32)),
            authorityWeights = listOf("1", "2"),
        )
        assertEquals(
            "0102000000${"11".repeat(32)}0100000000000000${"22".repeat(32)}0200000000000000",
            authorityPayload.joinToString("") { "%02x".format(it.toInt() and 0xff) },
        )
        assertEquals(
            "0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16",
            SccpSourceProofs.substrateAuthoritySetPayloadHash(authorityPayload),
        )
        assertEquals(
            "0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16",
            SccpSourceProofs.substrateAuthoritySetPayloadHash(
                authorityPublicKeys = listOf("11".repeat(32), "22".repeat(32)),
                authorityWeights = listOf("1", "2"),
            ),
        )
        assertEquals(
            "0xde84b8b7a5409c0f2cff1191173d6caa681d902b35e42669106ec6ea3193a117",
            SccpSourceProofs.substrateAuthoritySetHashFromPayload(authorityPayload),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                authorityPublicKeys = listOf("11".repeat(32), "11".repeat(32)),
                authorityWeights = listOf("1", "2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                authorityPublicKeys = listOf("00".repeat(32)),
                authorityWeights = listOf("1"),
            )
        }
        val zeroAuthorityPayload = ByteArray(45)
        zeroAuthorityPayload[0] = 1.toByte()
        zeroAuthorityPayload[1] = 1.toByte()
        zeroAuthorityPayload[37] = 1.toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.substrateAuthoritySetHashFromPayload(zeroAuthorityPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                authorityPublicKeys = listOf("11".repeat(32)),
                authorityWeights = listOf("0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                authorityPublicKeys = List(2049) { "11".repeat(32) },
                authorityWeights = List(2049) { "1" },
            )
        }
        val parentAuthorityKeys = listOf("11".repeat(32), "22".repeat(32), "33".repeat(32))
        val parentAuthorityWeights = listOf("5", "7", "11")
        val nextAuthorityKeys = listOf("aa".repeat(32), "bb".repeat(32), "cc".repeat(32))
        val nextAuthorityWeights = listOf("13", "17", "19")
        val parentAuthorityPayload = SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            authorityPublicKeys = parentAuthorityKeys,
            authorityWeights = parentAuthorityWeights,
        )
        val nextAuthorityPayload = SccpSourceProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            authorityPublicKeys = nextAuthorityKeys,
            authorityWeights = nextAuthorityWeights,
        )
        assertEquals(
            "0103000000${"11".repeat(32)}0500000000000000${"22".repeat(32)}0700000000000000" +
                "${"33".repeat(32)}0b00000000000000",
            parentAuthorityPayload.hex(),
        )
        assertEquals(
            "0103000000${"aa".repeat(32)}0d00000000000000${"bb".repeat(32)}1100000000000000" +
                "${"cc".repeat(32)}1300000000000000",
            nextAuthorityPayload.hex(),
        )
        val parentAuthorityHash = "0xb2efd5d86304ea728a8a9ed4013aab8f3e10c0cf862e859c9cade55e660934ef"
        val nextAuthorityHash = "0x07cdbba0d61fdd4324b571dd793965e52acbf7f4c163af328e26c92c047501b3"
        val nextAuthorityPayloadHash = "0x12ce972498ba5cd8a760aee0429fdc30d8b6447890e1bf77d8dde46f86b40d85"
        assertEquals(parentAuthorityHash, SccpSourceProofs.substrateAuthoritySetHashFromPayload(parentAuthorityPayload))
        assertEquals(nextAuthorityHash, SccpSourceProofs.substrateAuthoritySetHashFromPayload(nextAuthorityPayload))
        assertEquals(nextAuthorityPayloadHash, SccpSourceProofs.substrateAuthoritySetPayloadHash(nextAuthorityPayload))
        val substrateTransitionMessageHash = SccpSourceProofs.substrateAuthoritySetTransitionMessageHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
            fromGrandpaSetId = "41",
            toGrandpaSetId = "42",
            transitionBlockNumber = "9001",
            transitionBlockHash = "44".repeat(32),
            parentAuthoritySetHash = parentAuthorityHash,
            nextAuthoritySetHash = nextAuthorityHash,
            nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
        )
        assertEquals(
            "0x60589333bf798bf592b2642d0fbac39b4e9305576cd2ebe9dd1f448a97a0596b",
            substrateTransitionMessageHash,
        )
        assertEquals(
            157,
            SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionMessageBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
            ).size,
        )
        assertEquals(
            684,
            SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                version = 1,
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayload = nextAuthorityPayload,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                transitionMessageHash = substrateTransitionMessageHash,
                proofVersion = 1,
                totalWeight = "23",
                signedWeight = "18",
                authorityPublicKeys = parentAuthorityKeys,
                authorityWeights = parentAuthorityWeights,
                signersBitmap = byteArrayOf(0x06),
                signatures = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
            ).size,
        )
        assertEquals(
            "0x4d50a606c6858d3a4af5caf991a6dd8ac10dce717b14bd36ba70e5b0b098d302",
            SccpSourceProofs.substrateAuthoritySetTransitionJustificationHash(
                version = 1,
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayload = nextAuthorityPayload,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                transitionMessageHash = substrateTransitionMessageHash,
                proofVersion = 1,
                totalWeight = "23",
                signedWeight = "18",
                authorityPublicKeys = parentAuthorityKeys,
                authorityWeights = parentAuthorityWeights,
                signersBitmap = byteArrayOf(0x06),
                signatures = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                version = 0,
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayload = nextAuthorityPayload,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                transitionMessageHash = substrateTransitionMessageHash,
                proofVersion = 1,
                totalWeight = "23",
                signedWeight = "18",
                authorityPublicKeys = parentAuthorityKeys,
                authorityWeights = parentAuthorityWeights,
                signersBitmap = byteArrayOf(0x06),
                signatures = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                version = 1,
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayload = nextAuthorityPayload,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                transitionMessageHash = substrateTransitionMessageHash,
                proofVersion = 0,
                totalWeight = "23",
                signedWeight = "18",
                authorityPublicKeys = parentAuthorityKeys,
                authorityWeights = parentAuthorityWeights,
                signersBitmap = byteArrayOf(0x06),
                signatures = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                version = 1,
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                fromGrandpaSetId = "41",
                toGrandpaSetId = "42",
                transitionBlockNumber = "9001",
                transitionBlockHash = "44".repeat(32),
                parentAuthoritySetHash = parentAuthorityHash,
                nextAuthoritySetHash = nextAuthorityHash,
                nextAuthoritySetPayload = nextAuthorityPayload,
                nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                transitionMessageHash = substrateTransitionMessageHash,
                proofVersion = 1,
                totalWeight = "23",
                signedWeight = "18",
                authorityPublicKeys = parentAuthorityKeys,
                authorityWeights = parentAuthorityWeights,
                signersBitmap = ByteArray(257) { 0xff.toByte() },
                signatures = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
            )
        }
        fun assertBadSubstrateJustification(
            totalWeight: String = "23",
            signedWeight: String = "18",
            signersBitmap: ByteArray = byteArrayOf(0x06),
            signatures: List<ByteArray> = listOf(ByteArray(64) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
        ) {
            assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                    version = 1,
                    sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                    fromGrandpaSetId = "41",
                    toGrandpaSetId = "42",
                    transitionBlockNumber = "9001",
                    transitionBlockHash = "44".repeat(32),
                    parentAuthoritySetHash = parentAuthorityHash,
                    nextAuthoritySetHash = nextAuthorityHash,
                    nextAuthoritySetPayload = nextAuthorityPayload,
                    nextAuthoritySetPayloadHash = nextAuthorityPayloadHash,
                    transitionMessageHash = substrateTransitionMessageHash,
                    proofVersion = 1,
                    totalWeight = totalWeight,
                    signedWeight = signedWeight,
                    authorityPublicKeys = parentAuthorityKeys,
                    authorityWeights = parentAuthorityWeights,
                    signersBitmap = signersBitmap,
                    signatures = signatures,
                )
            }
        }
        assertBadSubstrateJustification(totalWeight = "22")
        assertBadSubstrateJustification(signedWeight = "17")
        assertBadSubstrateJustification(signedWeight = "12", signersBitmap = byteArrayOf(0x03))
        assertBadSubstrateJustification(signersBitmap = byteArrayOf(0x00), signatures = emptyList())
        assertBadSubstrateJustification(signersBitmap = byteArrayOf(0x08), signatures = emptyList())
        assertBadSubstrateJustification(
            signatures = listOf(ByteArray(64), ByteArray(64) { 0x88.toByte() }),
        )
        assertBadSubstrateJustification(
            signatures = listOf(ByteArray(63) { 0x77.toByte() }, ByteArray(64) { 0x88.toByte() }),
        )

        assertEquals(
            evmReceiptRootMptValueHex,
            SccpSourceProofs.canonicalEvmReceiptRootMptValue("bb".repeat(32)).hex(),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRootMptValue("1234")
        }
        val zeroHash = "00".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)
        }
        assertEquals(
            "f8419f736363703a74726f6e3a726563656970742d726f6f742d76616c75653a7631a0" + "bb".repeat(32),
            SccpSourceProofs.canonicalTronReceiptRootMptValue("bb".repeat(32)).hex(),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptRootMptValue("1234")
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptRootMptValue(zeroHash)
        }
        assertEquals(
            133,
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = branch,
            ).size,
        )
        val paddedTronDigest = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = "$sourceEventDigest ",
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = branch,
            )
        }
        assertTrue(paddedTronDigest.message.orEmpty().contains("sourceEventDigest must be canonical hex"))
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = zeroHash,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = zeroHash,
                transactionRoot = "dd".repeat(32),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = zeroHash,
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = emptyList(),
            )
        }
        assertTrue(
            SccpSourceProofs.tronReceiptProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = branch,
            ) != SccpSourceProofs.tronReceiptProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = changedBranch,
            ),
        )
        val tronReceiptStateNode = hexBytes("e4822080a0" + "bb".repeat(32))
        assertEquals(
            186,
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            ).size,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = zeroHash,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = zeroHash,
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = zeroHash,
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = emptyList(),
            )
        }
        assertEquals(
            "0x847c5ee3e6f4f83fef4d754a9aed93fae38c6677011cae03b10228c17c60b13b",
            SccpSourceProofs.tronReceiptStateProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            ),
        )
        assertTrue(
            SccpSourceProofs.tronReceiptStateProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            ) != SccpSourceProofs.tronReceiptStateProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "1",
                receiptTrieProofNodes = listOf(tronReceiptStateNode),
                inclusionBranch = branch,
            ),
        )
        val transactionSourceBytes = hexBytes(
            "0af3010a02123418b9602208565656565656565640959aef3a5acf01081f12ca" +
                "010a31747970652e676f6f676c65617069732e636f6d2f70726f746f636f6c2e" +
                "54726967676572536d617274436f6e74726163741294010a15417e5f4552091a" +
                "69125d5dfcb7b8c2659029395bdf121541454545454545454545454545454545" +
                "4545454545226406841e30000000000000000000000000000000000000000000" +
                "0000000000000000000005000000000000000000000000000000000000000000" +
                "0000000000000000000000343434343434343434343434343434343434343434" +
                "34343434343434343434347090e5ee3a900180e1eb171241cc58d7ac52c91117" +
                "92495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de3437" +
                "2c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c012a0410001801",
        )
        val transactionSourceBranch = emptyList<ByteArray>()
        val transactionSourceRoot =
            "1751c62dce36d5d642e48480b45d48ed16dd1b9b40ce216bc2f15c1b1ccf300b"
        val transactionSourceInclusionBranch = listOf(ByteArray(32) { 0xaa.toByte() })
        assertEquals(
            "06841e30${"00".repeat(31)}05${"00".repeat(32)}${"34".repeat(32)}",
            SccpSourceProofs.tronSourceMessageCallData(5, 0, sourceEventDigest).hex(),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronSourceMessageCallData(0, 0, sourceEventDigest)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronSourceMessageCallData(5, 5, sourceEventDigest)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.tronSourceMessageCallData(5, 0, "00".repeat(32))
        }
        assertEquals(
            476,
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            ).size,
        )
        assertContentEquals(
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
                sourceBridgeEmitterAddress = "45".repeat(20),
                sourceBridgeOwnerAddress = "7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            ),
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
                sourceBridgeEmitterAddress = "46".repeat(20),
                sourceBridgeOwnerAddress = "7e5f4552091a69125d5dfcb7b8c2659029395bdf",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
                sourceBridgeEmitterAddress = "45".repeat(20),
                sourceBridgeOwnerAddress = "22".repeat(20),
            )
        }
        assertEquals(
            "0xfc98a09ae9e7f63ccd383b2f3e104efce0d2c291dc7900ffd49e4f391e6016b6",
            SccpSourceProofs.tronTransactionSourceProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            ),
        )
        val omittedDefaultRetTransactionSourceBytes = hexBytes(
            transactionSourceBytes.hex().replace("2a0410001801", "2a021801"),
        )
        assertEquals(
            474,
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "62489e5ad22dd0fc7a4b8444c2b17ef28c2c885a01bd0f97fd7f63fbfb1552bd",
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = omittedDefaultRetTransactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            ).size,
        )
        assertEquals(
            "0xdb367957f5100b81ef1b074867c5c7c846c8bb3b44353668f65bf1c8ec805a18",
            SccpSourceProofs.tronTransactionSourceProofHash(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "62489e5ad22dd0fc7a4b8444c2b17ef28c2c885a01bd0f97fd7f63fbfb1552bd",
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = omittedDefaultRetTransactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            ),
        )
        val nonCanonicalTransactionSourceBytes = transactionSourceBytes.copyOf()
        nonCanonicalTransactionSourceBytes[nonCanonicalTransactionSourceBytes.size - 7] = 0x1f.toByte()
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = nonCanonicalTransactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            )
        }
        val wrongSignerTransactionSourceBytes = transactionSourceBytes.replacingFirst(
            hexBytes("cc58d7ac52c9111792495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de34372c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c01"),
            hexBytes("b50455577deef2a0d6c3c521d97de050d5b9ba46df00c8ddad014bac4ca3345173223f1d4c5940538f1b1da069bed6828a9b27794bd1eac1a35810baaef28d2101"),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = wrongSignerTransactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "1",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "cc".repeat(32),
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = transactionSourceInclusionBranch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = "e4a77765ae41dc30b8bf3f7d9847170e0646e3dd0189433d2e3c88296221c942",
                transactionIndex = "1",
                transactionCount = "3",
                transactionBytes = hexBytes("123456"),
                transactionMerkleBranch = listOf(ByteArray(32) { 0x11.toByte() }, ByteArray(32) { 0x22.toByte() }),
                inclusionBranch = transactionSourceInclusionBranch,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest = sourceEventDigest,
                receiptRoot = "bb".repeat(32),
                transactionRoot = transactionSourceRoot,
                transactionIndex = "0",
                transactionCount = "1",
                transactionBytes = transactionSourceBytes,
                transactionMerkleBranch = transactionSourceBranch,
                inclusionBranch = emptyList(),
            )
        }
        val tronParentRawHeaderHex =
            "08b8b096ffbc311220${"cc".repeat(32)}1a20${"bb".repeat(32)}38b8604a1541${"11".repeat(20)}50015a20${"aa".repeat(32)}"
        val tronRawHeaderHex =
            "08b9b096ffbc311220${"dd".repeat(32)}1a200000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d438b9604a1541${"11".repeat(20)}50015a20${"ee".repeat(32)}"
        val tronParentRawHeaderHash =
            "0x5647d462e78851c6701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4"
        val tronRawHeaderHash =
            "0x614a09275b6d0fffb6bc08fb34f737c093d9dd2adefccb04344715e2619c8286"
        val tronParentBlockId =
            "0x0000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4"
        val tronBlockId =
            "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286"
        val parentRawHeader = SccpSourceProofs.canonicalTronRawBlockHeaderBytes(
            number = "12344",
            txTrieRoot = "cc".repeat(32),
            accountStateRoot = "aa".repeat(32),
            parentBlockId = "bb".repeat(32),
            witnessAddress = "41" + "11".repeat(20),
            headerVersion = 1,
            timestampMs = "1700000012344",
        )
        val rawHeader = SccpSourceProofs.canonicalTronRawBlockHeaderBytes(
            number = "12345",
            txTrieRoot = "dd".repeat(32),
            accountStateRoot = "ee".repeat(32),
            parentBlockId = tronParentBlockId,
            witnessAddress = "41" + "11".repeat(20),
            headerVersion = 1,
            timestampMs = "1700000012345",
        )
        assertEquals(tronParentRawHeaderHex, parentRawHeader.hex())
        assertEquals(tronRawHeaderHex, rawHeader.hex())
        assertEquals(tronParentRawHeaderHash, SccpSourceProofs.tronRawBlockHeaderHash(parentRawHeader))
        assertEquals(tronRawHeaderHash, SccpSourceProofs.tronRawBlockHeaderHash(rawHeader))
        assertEquals(tronParentBlockId, SccpSourceProofs.tronBlockIdFromRawDataHash("12344", tronParentRawHeaderHash))
        assertEquals(tronBlockId, SccpSourceProofs.tronBlockIdFromRawDataHash("12345", tronRawHeaderHash))
        val paddedTxTrieRoot = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronRawBlockHeaderBytes(
                number = "12345",
                txTrieRoot = " " + "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "11".repeat(20),
                headerVersion = 1,
                timestampMs = "1700000012345",
            )
        }
        assertTrue(paddedTxTrieRoot.message.orEmpty().contains("txTrieRoot must be canonical hex"))
        listOf("012345", "0x3039", "+12345", " 12345").forEach { nonCanonicalNumber ->
            val failure = assertFailsWith<IllegalArgumentException> {
                SccpSourceProofs.tronBlockIdFromRawDataHash(nonCanonicalNumber, tronRawHeaderHash)
            }
            assertTrue(failure.message.orEmpty().contains("number must be an unsigned integer"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronRawBlockHeaderBytes(
                number = "12346",
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronBlockId,
                witnessAddress = "41" + "00".repeat(20),
                headerVersion = 1,
                timestampMs = "1700000012346",
            )
        }
        assertEquals(
            650,
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = rawHeader,
                witnessSignature = tronHeaderSignature(0),
                parentRawData = parentRawHeader,
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = tronRawHeaderHash,
                parentRawDataHash = tronParentRawHeaderHash,
                blockId = tronBlockId,
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            ).size,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = rawHeader,
                witnessSignature = tronHeaderSignature(0),
                parentRawData = parentRawHeader,
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = tronParentRawHeaderHash,
                blockId = tronBlockId,
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        val overlongKeyRawHeader = byteArrayOf(0x88.toByte(), 0x00) + rawHeader.copyOfRange(1, rawHeader.size)
        val overlongKeyRawHeaderHash = SccpSourceProofs.tronRawBlockHeaderHash(overlongKeyRawHeader)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = overlongKeyRawHeader,
                witnessSignature = tronHeaderSignature(0),
                parentRawData = parentRawHeader,
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = overlongKeyRawHeaderHash,
                parentRawDataHash = tronParentRawHeaderHash,
                blockId = SccpSourceProofs.tronBlockIdFromRawDataHash("12345", overlongKeyRawHeaderHash),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = rawHeader,
                witnessSignature = tronHeaderSignature(0),
                parentRawData = parentRawHeader,
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = tronRawHeaderHash,
                parentRawDataHash = tronParentRawHeaderHash,
                blockId = tronBlockId,
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "00".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertEquals(
            "0x25416bda5734ecef1ab9920d15f1011e962f6ff90e9c6247ff6b2ce34a5ab49f",
            SccpSourceProofs.tronSolidBlockHeaderProofHash(
                rawData = rawHeader,
                witnessSignature = tronHeaderSignature(0),
                parentRawData = parentRawHeader,
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = tronRawHeaderHash,
                parentRawDataHash = tronParentRawHeaderHash,
                blockId = tronBlockId,
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "ee".repeat(32),
                parentBlockId = tronParentBlockId,
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            ),
        )

        assertEquals(
            225,
            SccpSourceProofs.canonicalSubstrateStorageProofBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ).size,
        )
        val zeroSubstrateDigest = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateStorageProofBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = zeroHash,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            )
        }
        assertTrue(zeroSubstrateDigest.message.orEmpty().contains("sourceEventDigest must not be zero"))
        val substrateStatement = SccpSourceProofs.canonicalSubstrateRuntimeStorageVerificationStatementBytes(
            sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = "0",
            finalizedBlockNumber = "31",
            grandpaSetId = "32",
            blockHash = "aa".repeat(32),
            authoritySetHash = "cc".repeat(32),
            eventsRoot = "bb".repeat(32),
            inclusionBranch = branch,
        )
        assertEquals(
            SccpSourceProofs.canonicalSubstrateStorageProofBytes(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ).toList(),
            substrateStatement.toList(),
        )
        val runtimeStoragePublicInputsHash = SccpSourceProofs.substrateRuntimeStorageProofPublicInputsHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = "0",
            finalizedBlockNumber = "31",
            grandpaSetId = "32",
            blockHash = "aa".repeat(32),
            authoritySetHash = "cc".repeat(32),
            eventsRoot = "bb".repeat(32),
            inclusionBranch = branch,
        )
        assertTrue(runtimeStoragePublicInputsHash.matches(Regex("0x[0-9a-f]{64}")))
        val runtimeStorageColumns = SccpSourceProofs.substrateRuntimeStoragePublicInputColumns(
            sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = "0",
            finalizedBlockNumber = "31",
            grandpaSetId = "32",
            blockHash = "aa".repeat(32),
            authoritySetHash = "cc".repeat(32),
            eventsRoot = "bb".repeat(32),
            inclusionBranch = branch,
        )
        assertEquals(11, runtimeStorageColumns.size)
        assertEquals(
            listOf("0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7"),
            runtimeStorageColumns[8],
        )
        assertEquals(listOf(runtimeStoragePublicInputsHash), runtimeStorageColumns[10])
        val runtimeStorageRequest = SccpSourceProofs.buildSubstrateRuntimeStorageProofRequest(
            sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest = sourceEventDigest,
            sourceEventLeafIndex = "0",
            finalizedBlockNumber = "31",
            grandpaSetId = "32",
            blockHash = "aa".repeat(32),
            authoritySetHash = "cc".repeat(32),
            eventsRoot = "bb".repeat(32),
            sourceTrustAnchorHash = "aa".repeat(32),
            consensusVerifierHash = "bb".repeat(32),
            messageInclusionVerifierHash = "cc".repeat(32),
            finalityPolicyHash = "dd".repeat(32),
            sourceStateVerifierHash = "12".repeat(32),
            inclusionBranch = branch,
        )
        assertEquals(
            SccpSourceProofs.SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
            runtimeStorageRequest.circuitId,
        )
        assertEquals(runtimeStoragePublicInputsHash, runtimeStorageRequest.runtimeStorageProofPublicInputsHash)
        assertEquals("31", runtimeStorageRequest.fastpqPublicInputs.slot)
        assertEquals(
            listOf(
                "sccp:substrate:runtime-storage:v1:context",
                "sccp:substrate:runtime-storage:v1:statement",
                "sccp:substrate:runtime-storage:v1:storage-key",
            ),
            runtimeStorageRequest.fastpqTransitions.map { it.key },
        )
        val originalStatementBytes = runtimeStorageRequest.statementBytes
        runtimeStorageRequest.statementBytes[0] = 0
        assertContentEquals(originalStatementBytes, runtimeStorageRequest.statementBytes)
        val originalSchemaDescriptor = runtimeStorageRequest.schemaDescriptor
        runtimeStorageRequest.schemaDescriptor[0] = 0
        assertContentEquals(originalSchemaDescriptor, runtimeStorageRequest.schemaDescriptor)
        @Suppress("UNCHECKED_CAST")
        (runtimeStorageRequest.publicInputColumns as MutableList<List<String>>).clear()
        assertEquals(11, runtimeStorageRequest.publicInputColumns.size)
        @Suppress("UNCHECKED_CAST")
        (runtimeStorageRequest.fastpqTransitions as MutableList<SccpSourceProofs.SubstrateRuntimeStorageFastpqTransition>)
            .clear()
        assertEquals(3, runtimeStorageRequest.fastpqTransitions.size)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildSubstrateRuntimeStorageProofRequest(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                sourceTrustAnchorHash = "aa".repeat(32),
                consensusVerifierHash = "bb".repeat(32),
                messageInclusionVerifierHash = "cc".repeat(32),
                finalityPolicyHash = "dd".repeat(32),
                sourceStateVerifierHash = "12".repeat(32),
                inclusionBranch = branch,
                storageProofHash = "aa".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildSubstrateRuntimeStorageProofRequest(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                sourceTrustAnchorHash = "aa".repeat(32),
                consensusVerifierHash = "bb".repeat(32),
                messageInclusionVerifierHash = "cc".repeat(32),
                finalityPolicyHash = "dd".repeat(32),
                sourceStateVerifierHash = "af2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
                inclusionBranch = branch,
            )
        }
        assertTrue(
            SccpSourceProofs.substrateStorageProofHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ) != SccpSourceProofs.substrateStorageProofHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = changedBranch,
            ),
        )
        assertTrue(
            SccpSourceProofs.substrateStorageProofHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ) != SccpSourceProofs.substrateStorageProofHash(
                sourceDomain = SccpSourceProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest = sourceEventDigest,
                sourceEventLeafIndex = "1",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ),
        )
    }

    @Test
    fun derivesEthBeaconExecutionPayloadSszRootsFromWitnessMaterial() {
        val headerRlp = sampleEthExecutionHeaderRlp()
        val executionPayloadRoot = SccpSourceProofs.ethExecutionPayloadHeaderRootFromRlp(headerRlp)
        val executionPayloadBranch = listOf(
            ByteArray(32) { 0xee.toByte() },
            ByteArray(32) { 0xff.toByte() },
            ByteArray(32) { 0x11.toByte() },
            ByteArray(32) { 0x22.toByte() },
        )
        val beaconBodyRoot = SccpSourceProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
            executionPayloadRoot,
            executionPayloadBranch,
        )
        val beaconHeaderRoot = SccpSourceProofs.ethBeaconBlockHeaderRoot(
            beaconSlot = "320",
            beaconProposerIndex = "17",
            beaconParentRoot = "aa".repeat(32),
            beaconStateRoot = "bb".repeat(32),
            beaconBodyRoot = beaconBodyRoot,
        )

        assertEquals(
            "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
            executionPayloadRoot,
        )
        assertEquals(
            "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
            beaconBodyRoot,
        )
        assertEquals(
            "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
            beaconHeaderRoot,
        )
        assertTrue(
            SccpSourceProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot,
                listOf(
                    ByteArray(32) { 0xff.toByte() },
                    ByteArray(32) { 0xff.toByte() },
                    ByteArray(32) { 0x11.toByte() },
                    ByteArray(32) { 0x22.toByte() },
                ),
            ) != beaconBodyRoot,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot,
                listOf(ByteArray(32) { 0xee.toByte() }),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.ethExecutionPayloadHeaderRootFromRlp(byteArrayOf(0x80.toByte()))
        }
    }

    @Test
    fun rejectsMalformedSourceProofWitnessMaterial() {
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest = "34".repeat(32),
                receiptRoot = "bb".repeat(32),
                transactionRoot = "dd".repeat(32),
                inclusionBranch = listOf(byteArrayOf(1, 2, 3)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest = "34".repeat(32),
                receiptRoot = "bb".repeat(32),
                transactionRoot = "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                receiptRootIndex = "0",
                receiptTrieProofNodes = emptyList(),
                inclusionBranch = listOf(ByteArray(32) { 0xee.toByte() }),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronRawBlockHeaderBytes(
                number = "0",
                txTrieRoot = "bb".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "cc".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                headerVersion = 1,
                timestampMs = "1700000012345",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = byteArrayOf(1),
                witnessSignature = ByteArray(64),
                parentRawData = byteArrayOf(2),
                parentWitnessSignature = ByteArray(65),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = "bb".repeat(32),
                blockId = "cc".repeat(32),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "ee".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = ByteArray(16 * 1024 + 1) { 0xaa.toByte() },
                witnessSignature = tronHeaderSignature(0),
                parentRawData = byteArrayOf(2),
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = "bb".repeat(32),
                blockId = "cc".repeat(32),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "ee".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = byteArrayOf(1),
                witnessSignature = ByteArray(65) { 0xaa.toByte() },
                parentRawData = byteArrayOf(2),
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = "bb".repeat(32),
                blockId = "cc".repeat(32),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "ee".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = byteArrayOf(1),
                witnessSignature = tronHeaderSignature(0),
                parentRawData = byteArrayOf(2),
                parentWitnessSignature = tronHeaderSignature(4),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = "bb".repeat(32),
                blockId = "cc".repeat(32),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "ee".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        val zeroRSignature = tronHeaderSignature(0)
        zeroRSignature.fill(0, 0, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawData = byteArrayOf(1),
                witnessSignature = zeroRSignature,
                parentRawData = byteArrayOf(2),
                parentWitnessSignature = tronHeaderSignature(27),
                rawDataHash = "aa".repeat(32),
                parentRawDataHash = "bb".repeat(32),
                blockId = "cc".repeat(32),
                txTrieRoot = "dd".repeat(32),
                accountStateRoot = "aa".repeat(32),
                parentBlockId = "ee".repeat(32),
                witnessAddress = "41" + "11".repeat(20),
                timestampMs = "1700000012345",
                headerVersion = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalSubstrateStorageProofBytes(
                sourceDomain = -1,
                sourceEventDigest = "34".repeat(32),
                sourceEventLeafIndex = "0",
                finalizedBlockNumber = "31",
                grandpaSetId = "32",
                blockHash = "aa".repeat(32),
                authoritySetHash = "cc".repeat(32),
                eventsRoot = "bb".repeat(32),
                inclusionBranch = emptyList(),
            )
        }
    }

    private fun tronHeaderSignature(recoveryId: Int): ByteArray =
        ByteArray(65) { index ->
            when {
                index < 32 -> 0xaa.toByte()
                index < 64 -> 0x01
                else -> recoveryId
            }.toByte()
        }

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0)
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun ByteArray.replacingFirst(needle: ByteArray, replacement: ByteArray): ByteArray {
        require(needle.size == replacement.size)
        val offset = (0..(size - needle.size)).firstOrNull { index ->
            needle.indices.all { needleIndex -> this[index + needleIndex] == needle[needleIndex] }
        } ?: error("needle not found")
        return copyOf().also { replacement.copyInto(it, offset) }
    }

    private fun minimalBeLengthBytes(length: Int): ByteArray {
        var working = length
        val bytes = ArrayList<Byte>()
        do {
            bytes.add(0, (working and 0xff).toByte())
            working = working ushr 8
        } while (working != 0)
        return bytes.toByteArray()
    }

    private fun rlpString(value: ByteArray): ByteArray {
        if (value.size == 1 && (value[0].toInt() and 0xff) < 0x80) return value
        if (value.size < 56) return byteArrayOf((0x80 + value.size).toByte()) + value
        val lengthBytes = minimalBeLengthBytes(value.size)
        return byteArrayOf((0xb7 + lengthBytes.size).toByte()) + lengthBytes + value
    }

    private fun rlpList(fields: List<ByteArray>): ByteArray {
        var payloadSize = 0
        for (field in fields) {
            payloadSize += field.size
        }
        val payload = ByteArray(payloadSize)
        var offset = 0
        for (field in fields) {
            System.arraycopy(field, 0, payload, offset, field.size)
            offset += field.size
        }
        if (payload.size < 56) return byteArrayOf((0xc0 + payload.size).toByte()) + payload
        val lengthBytes = minimalBeLengthBytes(payload.size)
        return byteArrayOf((0xf7 + lengthBytes.size).toByte()) + lengthBytes + payload
    }

    private fun sampleBscParliaExtra(): ByteArray =
        ByteArray(32) { 0x11.toByte() } +
            byteArrayOf(2) +
            ByteArray(20) { 0x11.toByte() } +
            ByteArray(48) { 0x01.toByte() } +
            ByteArray(20) { 0x22.toByte() } +
            ByteArray(48) { 0x02.toByte() } +
            ByteArray(65) { 0x99.toByte() }

    private fun sampleBscParliaHeaderRlp(extraData: ByteArray): ByteArray =
        rlpList(
            listOf(
                rlpString(ByteArray(32) { 0x10.toByte() }),
                rlpString(ByteArray(32) { 0x11.toByte() }),
                rlpString(ByteArray(20) { 0x12.toByte() }),
                rlpString(ByteArray(32) { 0x13.toByte() }),
                rlpString(ByteArray(32) { 0x14.toByte() }),
                rlpString(ByteArray(32) { 0x15.toByte() }),
                rlpString(ByteArray(256) { 0x00.toByte() }),
                rlpString(byteArrayOf(2)),
                rlpString(byteArrayOf(1)),
                rlpString(byteArrayOf(1)),
                rlpString(byteArrayOf(1)),
                rlpString(byteArrayOf(1)),
                rlpString(extraData),
                rlpString(ByteArray(32) { 0x00.toByte() }),
                rlpString(ByteArray(8) { 0x00.toByte() }),
            ),
        )

    private fun sampleEthExecutionHeaderRlp(
        receiptsRoot: ByteArray = ByteArray(32) { 0x15.toByte() },
    ): ByteArray =
        rlpList(
            listOf(
                rlpString(ByteArray(32) { 0x10.toByte() }),
                rlpString(ByteArray(32) { 0x11.toByte() }),
                rlpString(ByteArray(20) { 0x12.toByte() }),
                rlpString(ByteArray(32) { 0x13.toByte() }),
                rlpString(ByteArray(32) { 0x14.toByte() }),
                rlpString(receiptsRoot),
                rlpString(ByteArray(256) { 0x00.toByte() }),
                rlpString(ByteArray(0)),
                rlpString(byteArrayOf(0x2a)),
                rlpString(byteArrayOf(0x01, 0xc9.toByte(), 0xc3.toByte(), 0x80.toByte())),
                rlpString(byteArrayOf(0x52, 0x08)),
                rlpString(byteArrayOf(0x65, 0x53, 0xf1.toByte(), 0x00)),
                rlpString("iroha-sccp-test".toByteArray(Charsets.UTF_8)),
                rlpString(ByteArray(32) { 0x16.toByte() }),
                rlpString(ByteArray(8) { 0x00.toByte() }),
                rlpString(byteArrayOf(0x3b, 0x9a.toByte(), 0xca.toByte(), 0x00)),
                rlpString(ByteArray(32) { 0x17.toByte() }),
                rlpString(ByteArray(0)),
                rlpString(ByteArray(0)),
                rlpString(ByteArray(32) { 0x18.toByte() }),
            ),
        )

    private fun sampleSourceVerifierMaterialBytes(domain: Int): ByteArray =
        SccpSourceProofs.canonicalSourceVerifierMaterialBytes(
            sourceDomain = domain,
            sourceTrustAnchorHash = "0x" + "44".repeat(32),
            consensusVerifierHash = "0x" + "55".repeat(32),
            messageInclusionVerifierHash = "0x" + "66".repeat(32),
            finalityPolicyHash = "0x" + "88".repeat(32),
            sourceStateVerifierHash = sourceStateVerifierHash(domain),
            bridgeAddress = bridgeAddress(domain),
            sourceBridgeEmitterCodeHash = sourceBridgeCodeHash(domain),
            networkId = networkId(domain),
            ownerAddress = ownerAddress(domain),
            configHash = configHash(domain),
        )

    private fun sampleSourceVerifierMaterialHash(domain: Int): String =
        SccpSourceProofs.sourceVerifierMaterialHash(
            sourceDomain = domain,
            sourceTrustAnchorHash = "0x" + "44".repeat(32),
            consensusVerifierHash = "0x" + "55".repeat(32),
            messageInclusionVerifierHash = "0x" + "66".repeat(32),
            finalityPolicyHash = "0x" + "88".repeat(32),
            sourceStateVerifierHash = sourceStateVerifierHash(domain),
            bridgeAddress = bridgeAddress(domain),
            sourceBridgeEmitterCodeHash = sourceBridgeCodeHash(domain),
            networkId = networkId(domain),
            ownerAddress = ownerAddress(domain),
            configHash = configHash(domain),
        )

    private fun sampleSourceAdapterDeploymentHash(
        domain: Int,
        adapterVerifierVkHash: String? = null,
        solanaTowerReplayVerifierHash: String? = null,
        solanaFullAccountsdbLatticeVerifierHash: String? = null,
        solanaBankForkChoiceVerifierHash: String? = null,
        tonMasterchainConfigVerifierHash: String? = null,
        tonValidatorSetTransitionVerifierHash: String? = null,
        tonShardAccountsDictionaryVerifierHash: String? = null,
    ): String =
        SccpSourceProofs.sourceAdapterEngineDeploymentHash(
            sourceDomain = domain,
            sourceTrustAnchorHash = "0x" + "44".repeat(32),
            consensusVerifierHash = "0x" + "55".repeat(32),
            messageInclusionVerifierHash = "0x" + "66".repeat(32),
            finalityPolicyHash = "0x" + "88".repeat(32),
            deploymentReceiptHash = "0x" + "aa".repeat(32),
            adapterVerifierVkHash = adapterVerifierVkHash,
            sourceStateVerifierHash = sourceStateVerifierHash(domain),
            bridgeAddress = bridgeAddress(domain),
            sourceBridgeEmitterCodeHash = sourceBridgeCodeHash(domain),
            networkId = networkId(domain),
            ownerAddress = ownerAddress(domain),
            configHash = configHash(domain),
            solanaTowerReplayVerifierHash = solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash = solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash = solanaBankForkChoiceVerifierHash,
            tonMasterchainConfigVerifierHash = tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash = tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash = tonShardAccountsDictionaryVerifierHash,
        )

    private fun sampleSolanaFullLightClientGateHash(
        towerReplayHash: String = "0x" + "bb".repeat(32),
        fullAccountsdbLatticeHash: String = "0x" + "cc".repeat(32),
        bankForkChoiceHash: String = "0x" + "dd".repeat(32),
        sourceStateHash: String? = sourceStateVerifierHash(SccpSourceProofs.DOMAIN_SOL),
    ): String =
        SccpSourceProofs.solanaFullLightClientGateHash(
            sourceDomain = SccpSourceProofs.DOMAIN_SOL,
            sourceTrustAnchorHash = "0x" + "44".repeat(32),
            consensusVerifierHash = "0x" + "55".repeat(32),
            messageInclusionVerifierHash = "0x" + "66".repeat(32),
            finalityPolicyHash = "0x" + "88".repeat(32),
            deploymentReceiptHash = "0x" + "aa".repeat(32),
            solanaTowerReplayVerifierHash = towerReplayHash,
            solanaFullAccountsdbLatticeVerifierHash = fullAccountsdbLatticeHash,
            solanaBankForkChoiceVerifierHash = bankForkChoiceHash,
            sourceStateVerifierHash = sourceStateHash,
        )

    private fun sampleTonFullLightClientGateHash(
        masterchainConfigHash: String = "0x" + "bb".repeat(32),
        validatorSetTransitionHash: String = "0x" + "cc".repeat(32),
        shardAccountsDictionaryHash: String = "0x" + "dd".repeat(32),
    ): String =
        SccpSourceProofs.tonFullLightClientGateHash(
            sourceDomain = SccpSourceProofs.DOMAIN_TON,
            sourceTrustAnchorHash = "0x" + "44".repeat(32),
            consensusVerifierHash = "0x" + "55".repeat(32),
            messageInclusionVerifierHash = "0x" + "66".repeat(32),
            finalityPolicyHash = "0x" + "88".repeat(32),
            deploymentReceiptHash = "0x" + "aa".repeat(32),
            tonMasterchainConfigVerifierHash = masterchainConfigHash,
            tonValidatorSetTransitionVerifierHash = validatorSetTransitionHash,
            tonShardAccountsDictionaryVerifierHash = shardAccountsDictionaryHash,
            sourceStateVerifierHash = sourceStateVerifierHash(SccpSourceProofs.DOMAIN_TON),
        )

    private fun sourceStateVerifierHash(domain: Int): String? =
        if (
            domain == SccpSourceProofs.DOMAIN_SOL ||
                domain == SccpSourceProofs.DOMAIN_TON ||
                domain == SccpSourceProofs.DOMAIN_SORA_KUSAMA ||
                domain == SccpSourceProofs.DOMAIN_SORA_POLKADOT ||
                domain == SccpSourceProofs.DOMAIN_SORA2
        ) {
            "0x" + "77".repeat(32)
        } else {
            null
        }

    private fun bridgeAddress(domain: Int): String? =
        if (domain == SccpSourceProofs.DOMAIN_ETH ||
            domain == SccpSourceProofs.DOMAIN_BSC ||
            domain == SccpSourceProofs.DOMAIN_TRON
        ) {
            "0x" + "11".repeat(20)
        } else {
            null
        }

    private fun sourceBridgeCodeHash(domain: Int): String? =
        if (bridgeAddress(domain) != null) "0x" + "77".repeat(32) else null

    private fun networkId(domain: Int): String? =
        when (domain) {
            SccpSourceProofs.DOMAIN_ETH -> SccpSourceProofs.ETH_MAINNET_NETWORK_ID
            SccpSourceProofs.DOMAIN_TRON -> "0x" + "33".repeat(32)
            else -> null
        }

    private fun ownerAddress(domain: Int): String? =
        if (domain == SccpSourceProofs.DOMAIN_TRON) "0x" + "22".repeat(20) else null

    private fun configHash(domain: Int): String? =
        when (domain) {
            SccpSourceProofs.DOMAIN_ETH ->
                "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
            SccpSourceProofs.DOMAIN_TRON ->
                "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"
            else -> null
        }
}
