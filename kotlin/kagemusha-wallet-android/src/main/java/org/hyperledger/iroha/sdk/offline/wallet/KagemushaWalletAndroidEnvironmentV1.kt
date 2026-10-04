// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.SystemClock
import android.os.UserManager
import java.io.File
import java.io.FileInputStream

/**
 * Narrow Android process and storage facts used by [KagemushaWalletAndroidPlatformV1], so the
 * adapter's decisions run in JVM unit tests against fakes. Methods report errors by throwing.
 */
internal interface KagemushaWalletAndroidEnvironmentV1 {
    /** `Build.VERSION.SDK_INT`. */
    val apiLevel: Int

    /** `ApplicationInfo.flags` of the host application. */
    fun applicationFlags(): Int

    /** `ApplicationInfo.backupAgentName`, or null when the app declares no backup agent. */
    fun backupAgentName(): String?

    /** Whether the context is a device-protected (direct-boot) storage context. */
    fun isDeviceProtectedStorage(): Boolean

    /** API 28+ and `PackageManager.FEATURE_STRONGBOX_KEYSTORE`. */
    fun hasStrongBox(): Boolean

    /** `UserManager.isUserUnlocked()`: credential-encrypted storage is available. */
    fun isUserUnlocked(): Boolean

    /** `Context.getNoBackupFilesDir()` of the credential-encrypted application context. */
    fun noBackupFilesDir(): File

    /** `SystemClock.elapsedRealtime()`: `CLOCK_BOOTTIME` milliseconds, including deep sleep. */
    fun elapsedRealtimeMillis(): Long

    /** Raw text of `/proc/sys/kernel/random/boot_id`, at most [KAGEMUSHA_WALLET_ANDROID_BOOT_ID_READ_LIMIT_V1] bytes. */
    fun readBootId(): String
}

/** Upper bound of the boot-id read; the file holds a 36-character UUID and a newline. */
internal const val KAGEMUSHA_WALLET_ANDROID_BOOT_ID_READ_LIMIT_V1: Int = 64

/** Production environment over the application context. */
internal class KagemushaWalletAndroidSystemEnvironmentV1(context: Context) : KagemushaWalletAndroidEnvironmentV1 {
    private val context: Context = context.applicationContext ?: context

    override val apiLevel: Int get() = Build.VERSION.SDK_INT

    override fun applicationFlags(): Int = context.applicationInfo.flags

    override fun backupAgentName(): String? = context.applicationInfo.backupAgentName

    override fun isDeviceProtectedStorage(): Boolean = context.isDeviceProtectedStorage

    override fun hasStrongBox(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.P &&
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)

    override fun isUserUnlocked(): Boolean {
        val users = context.getSystemService(Context.USER_SERVICE) as? UserManager
            ?: throw IllegalStateException("UserManager is unavailable")
        return users.isUserUnlocked
    }

    override fun noBackupFilesDir(): File = context.noBackupFilesDir

    override fun elapsedRealtimeMillis(): Long = SystemClock.elapsedRealtime()

    override fun readBootId(): String = FileInputStream(BOOT_ID_PATH).use { input ->
        val buffer = ByteArray(KAGEMUSHA_WALLET_ANDROID_BOOT_ID_READ_LIMIT_V1 + 1)
        var length = 0
        while (length < buffer.size) {
            val read = input.read(buffer, length, buffer.size - length)
            if (read < 0) break
            length += read
        }
        check(length <= KAGEMUSHA_WALLET_ANDROID_BOOT_ID_READ_LIMIT_V1) { "boot_id is oversized" }
        String(buffer, 0, length, Charsets.US_ASCII)
    }

    private companion object {
        const val BOOT_ID_PATH = "/proc/sys/kernel/random/boot_id"
    }
}
