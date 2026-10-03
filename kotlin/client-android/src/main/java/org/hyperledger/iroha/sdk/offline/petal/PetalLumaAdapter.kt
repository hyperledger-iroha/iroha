package org.hyperledger.iroha.sdk.offline.petal

import android.graphics.ImageFormat
import android.media.Image
import java.nio.ByteBuffer

/**
 * Turns camera luma (Y) planes into [PetalLuma] images for [PetalScanSession].
 *
 * Dependency-free: the SDK does not ship CameraX, so apps call this from
 * whichever camera API they use. Petal decoding is rotation and mirror
 * invariant, so pass `rotationDegrees = 0` (the default) unless an upright
 * image is needed for something else; rotating costs a second copy.
 *
 * Camera setup (`specs/petal_stream.md` §8, "Scanner guidance"):
 * - Resolution: request about 1280×720 analysis frames wherever the device
 *   sustains at least 5 decoded frames per second, and fall back to 640×480
 *   only when it cannot. At 720p (about 15 px per tile) all three lanes read;
 *   at 480p (tiles 6.7–9.1 px) only lane `D` and, in nominal light, lane `P`
 *   do and lane `K` never does, which makes a 7.5 KB payment roughly four
 *   times slower.
 * - Exposure: automatic exposure over-exposes a mostly black screen. Set the
 *   exposure compensation to about −1 EV (about −2 EV on old cameras) where the
 *   camera API offers it, or lock exposure once the code is seen. The decoder
 *   tolerates over-exposure and veiling light (its normalised tile read keeps
 *   lanes `P` and `K` alive), but it should not be asked to.
 *
 * CameraX `ImageAnalysis` (YUV_420_888, requested at about 1280×720, see the
 * camera setup above):
 * ```kotlin
 * val session = PetalScanSession()
 * imageAnalysis.setAnalyzer(executor) { proxy ->
 *     try {
 *         val y = proxy.planes[0]
 *         val luma = PetalLumaAdapter.fromYPlane(y.buffer, y.rowStride, y.pixelStride, proxy.width, proxy.height)
 *         val outcome = session.push(luma, SystemClock.elapsedRealtime())
 *         outcome.completed?.let { done -> deliver(done.payload) }
 *     } finally {
 *         proxy.close()
 *     }
 * }
 * ```
 *
 * Camera1 `PreviewCallback.onPreviewFrame(data, camera)` with the default NV21
 * format, whose first `width * height` bytes are the Y plane:
 * ```kotlin
 * val size = camera.parameters.previewSize
 * val luma = PetalLumaAdapter.fromNv21(data, size.width, size.height)
 * val outcome = session.push(luma, SystemClock.elapsedRealtime())
 * camera.addCallbackBuffer(data)
 * ```
 *
 * Camera2 `ImageReader` images in `YUV_420_888` go through [fromImage].
 * Decode on a background thread. The reference decoder needs about 10 ms for a
 * 1280×720 frame; measure the target device and fall back to 640×480 only when
 * it decodes fewer than 5 frames per second at 720p.
 *
 * Push every analysed frame into the same [PetalScanSession]: after a frame
 * decodes, the session tracks the code from its pose (about a quarter of the
 * work of a full search) as long as frames keep arriving within
 * [PetalScanSession.TRACK_WINDOW_MILLIS]. A code with one corner blossom
 * covered by a thumb or cut off by the frame edge still reads; when a push
 * raises `stats().inferred`, a hint such as "one corner blossom is hidden"
 * helps the user uncover it.
 */
object PetalLumaAdapter {
    /**
     * Copies the Y plane of a `YUV_420_888` [image] (Camera2 `ImageReader`, or
     * CameraX `ImageProxy.image`). The image is not closed.
     */
    @JvmStatic
    @JvmOverloads
    fun fromImage(image: Image, rotationDegrees: Int = 0): PetalLuma {
        require(image.format == ImageFormat.YUV_420_888) { "Petal scanning needs YUV_420_888 camera images" }
        val plane = image.planes[0]
        return PetalLuma.fromPlane(
            plane.buffer,
            image.width,
            image.height,
            plane.rowStride,
            plane.pixelStride,
            rotationDegrees,
        )
    }

    /**
     * Copies a luma plane given as a [buffer] starting at its position, with
     * [rowStride] bytes between rows and [pixelStride] bytes between samples,
     * rotating it clockwise by [rotationDegrees] (`0`, `90`, `180` or `270`).
     */
    @JvmStatic
    @JvmOverloads
    fun fromYPlane(
        buffer: ByteBuffer,
        rowStride: Int,
        pixelStride: Int,
        width: Int,
        height: Int,
        rotationDegrees: Int = 0,
    ): PetalLuma = PetalLuma.fromPlane(buffer, width, height, rowStride, pixelStride, rotationDegrees)

    /** Copies the Y plane (the first `width * height` bytes) of an NV21 or NV12 preview frame. */
    @JvmStatic
    @JvmOverloads
    fun fromNv21(data: ByteArray, width: Int, height: Int, rotationDegrees: Int = 0): PetalLuma {
        require(width > 0 && height > 0 && width.toLong() * height.toLong() <= data.size.toLong()) {
            "preview frame is smaller than its Y plane"
        }
        return PetalLuma.fromPlane(ByteBuffer.wrap(data, 0, width * height), width, height, width, 1, rotationDegrees)
    }
}
