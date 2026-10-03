package org.hyperledger.iroha.sdk.offline.petal

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.Base64
import java.util.zip.Inflater
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/** Shared helpers for the Petal Stream tests. */
internal object PetalTestSupport {
    /** Walks up from the working directory to the repository-root `fixtures/petal/<name>`. */
    fun fixturePath(name: String): Path {
        var current: Path? = Paths.get("").toAbsolutePath().normalize()
        while (current != null) {
            val candidate = current.resolve("fixtures/petal/$name")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent
        }
        error("fixtures/petal/$name was not found above the test working directory")
    }

    fun loadFixture(name: String): JsonObject =
        Json.parseToJsonElement(String(Files.readAllBytes(fixturePath(name)), StandardCharsets.UTF_8)).jsonObject

    fun JsonObject.obj(key: String): JsonObject = getValue(key).jsonObject

    fun JsonObject.array(key: String): JsonArray = getValue(key).jsonArray

    fun JsonObject.text(key: String): String = getValue(key).jsonPrimitive.content

    fun JsonObject.number(key: String): Long = getValue(key).jsonPrimitive.long

    fun JsonObject.hexBytes(key: String): ByteArray = hex(text(key))

    fun JsonArray.longs(): LongArray = LongArray(size) { this[it].jsonPrimitive.long }

    fun JsonElement.longs(): LongArray = jsonArray.longs()

    fun hex(text: String): ByteArray {
        require(text.length % 2 == 0) { "odd hex length" }
        return ByteArray(text.length / 2) { index ->
            ((Character.digit(text[2 * index], 16) shl 4) or Character.digit(text[2 * index + 1], 16)).toByte()
        }
    }

    /** Decodes base64 and inflates the zlib stream of a fixture luma plane. */
    fun inflateBase64(text: String): ByteArray {
        val compressed = Base64.getDecoder().decode(text)
        val inflater = Inflater()
        inflater.setInput(compressed)
        val out = ByteArrayOutputStream(compressed.size * 4)
        val buffer = ByteArray(64 * 1024)
        while (!inflater.finished()) {
            val count = inflater.inflate(buffer)
            if (count == 0 && (inflater.needsInput() || inflater.needsDictionary())) error("truncated zlib stream")
            out.write(buffer, 0, count)
        }
        inflater.end()
        return out.toByteArray()
    }

    /** `len` bytes from xorshift32 seeded with [seed] (the reference test payloads). */
    fun payload(len: Int, seed: Int): ByteArray {
        val rng = PetalXorshift32(seed)
        return ByteArray(len) { rng.nextByte().toByte() }
    }

    /** Unsigned 32-bit value of a raw [Int]. */
    fun u32(value: Int): Long = value.toLong() and 0xFFFF_FFFFL

    /** Renders frame [frame] of [encoder] and converts it to luma. */
    fun renderLuma(encoder: PetalStreamEncoder, frame: Int, size: Int, supersample: Int): PetalLuma =
        PetalRenderer.render(encoder.cells(frame), PetalRenderOptions(size, supersample)).toLuma()

    /** Pushes the clean lanes of [frame] into [assembler] as a decoder would. */
    fun feedFrame(assembler: PetalStreamAssembler, encoder: PetalStreamEncoder, frame: Int, lanes: List<PetalLane>) {
        val words = encoder.words(frame)
        for (lane in lanes) {
            val data = PetalLanes.decodeLane(lane, words.lane(lane))
            when (lane) {
                PetalLane.D -> assembler.pushDLane(checkNotNull(PetalStream.parseDLane(data)))
                else -> assembler.pushAtoms(checkNotNull(PetalStream.parseAtomLane(lane, data)))
            }
        }
    }

    /** The reference tests' tiny LCG (Knuth MMIX constants). */
    class Lcg(private var state: Long) {
        fun next(): Int {
            state = state * 6_364_136_223_846_793_005L + 1_442_695_040_888_963_407L
            return (state ushr 33).toInt()
        }

        fun byte(): Int = next() and 0xFF

        fun below(bound: Int): Int = ((next().toLong() and 0xFFFF_FFFFL) % bound).toInt()
    }
}
