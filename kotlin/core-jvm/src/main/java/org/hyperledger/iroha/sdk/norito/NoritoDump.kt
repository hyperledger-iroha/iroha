// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/** Simple CLI to print Norito header information. */
object NoritoDump {

    @JvmStatic
    fun main(args: Array<String>) {
        if (args.size != 1) {
            System.err.println("Usage: NoritoDump <path>")
            System.exit(1)
        }
        val data = Files.readAllBytes(Paths.get(args[0]))
        val result = NoritoHeader.decode(data, null)
        val header = result.header
        println("Norito header:")
        println("  version: ${NoritoHeader.MAJOR_VERSION}.${NoritoHeader.MINOR_VERSION}")
        println("  schema hash: ${bytesToHex(header.schemaHash)}")
        println("  compression: ${header.compression}")
        println("  payload length: ${header.payloadLength}")
        println("  checksum: 0x${"%016x".format(header.checksum)}")
        println("  flags: 0x${"%02x".format(header.flags)}")
    }

    private fun bytesToHex(bytes: ByteArray): String =
        bytes.joinToString("") { "%02x".format(it) }
}
