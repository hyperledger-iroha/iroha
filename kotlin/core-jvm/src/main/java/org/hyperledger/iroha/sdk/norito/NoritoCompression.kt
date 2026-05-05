// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import com.github.luben.zstd.Zstd

internal object NoritoCompression {

    @JvmStatic
    fun compressZstd(payload: ByteArray, level: Int): ByteArray =
        Zstd.compress(payload, level)

    @JvmStatic
    fun decompressZstd(payload: ByteArray, targetSize: Int): ByteArray =
        Zstd.decompress(payload, targetSize)
}
