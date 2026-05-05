package org.hyperledger.iroha.sdk.crypto.export

import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException
import java.util.Properties

/** File-backed key export store using a `Properties` map. */
class FileKeyExportStore(private val file: File) : KeyExportStore {

    private val lock = Any()

    override fun load(alias: String): String? {
        if (alias.isBlank()) return null
        synchronized(lock) {
            val properties = readProperties()
            return properties.getProperty(alias)
        }
    }

    override fun store(alias: String, bundleBase64: String) {
        if (alias.isBlank()) throw KeyExportException("alias must be provided")
        if (bundleBase64.isBlank()) throw KeyExportException("bundleBase64 must be provided")
        synchronized(lock) {
            val properties = readProperties()
            properties.setProperty(alias, bundleBase64)
            writeProperties(properties)
        }
    }

    override fun delete(alias: String) {
        if (alias.isBlank()) return
        synchronized(lock) {
            val properties = readProperties()
            if (properties.remove(alias) != null) {
                writeProperties(properties)
            }
        }
    }

    private fun readProperties(): Properties {
        val properties = Properties()
        if (!file.exists()) return properties
        try {
            FileInputStream(file).use { input -> properties.load(input) }
            return properties
        } catch (ex: IOException) {
            throw KeyExportException("Failed to read key export store", ex)
        }
    }

    private fun writeProperties(properties: Properties) {
        val parent = file.parentFile
        if (parent != null && !parent.exists() && !parent.mkdirs()) {
            throw KeyExportException("Failed to create key export store directory")
        }
        try {
            FileOutputStream(file).use { output ->
                properties.store(output, "Iroha Android key exports")
            }
        } catch (ex: IOException) {
            throw KeyExportException("Failed to write key export store", ex)
        }
    }
}
