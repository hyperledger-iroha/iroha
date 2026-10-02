package org.hyperledger.iroha.sdk.offline.petal

import android.content.Context
import android.graphics.Canvas
import android.util.AttributeSet
import android.view.View

/**
 * A square [View] that plays a Petal stream: frame `0, 1, 2, …` of a
 * [PetalStreamEncoder] at [framesPerSecond], wrapping after frame 65535.
 *
 * It is framework-agnostic: use it from XML layouts, from code, or from
 * Jetpack Compose through `AndroidView`. Call [setStream], then [start]; the
 * view keeps the screen on while it plays and pauses its timer while it is
 * detached from a window. Frames are encoded on the UI thread, which costs well
 * under a millisecond even for a 10 KB payload. Use from the UI thread only.
 */
class PetalStreamView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0,
) : View(context, attrs, defStyleAttr) {
    private val renderer = PetalCanvasRenderer()
    private var encoder: PetalStreamEncoder? = null
    private var drawList: PetalDrawList? = null
    private var running = false

    /** Colours of the frames; changing them redraws the current frame. */
    var palette: PetalPalette = PetalPalette.DEFAULT
        set(value) {
            field = value
            encoder?.let { showFrame(it, frame) }
        }

    /** Display rate in frames per second (`0 < fps <= 60`, default 8). */
    var framesPerSecond: Double = DEFAULT_FRAMES_PER_SECOND
        set(value) {
            require(value > 0.0 && value <= 60.0) { "frames per second must be in (0, 60]" }
            field = value
        }

    /** The frame number currently shown. */
    var frame: Int = 0
        private set

    /** Whether the stream is advancing. */
    val isRunning: Boolean get() = running

    private val advance = object : Runnable {
        override fun run() {
            val current = encoder
            if (!running || current == null) return
            showFrame(current, (frame + 1) and PetalStream.MAX_FRAME)
            postOnAnimationDelayed(this, frameIntervalMillis())
        }
    }

    /** Shows [encoder] from frame 0, or clears the view when `null`. */
    fun setStream(encoder: PetalStreamEncoder?) {
        this.encoder = encoder
        if (encoder == null) {
            frame = 0
            drawList = null
            invalidate()
        } else {
            showFrame(encoder, 0)
        }
        reschedule()
    }

    /** Starts advancing frames. */
    fun start() {
        running = true
        keepScreenOn = true
        reschedule()
    }

    /** Stops on the current frame. */
    fun stop() {
        running = false
        keepScreenOn = false
        removeCallbacks(advance)
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        reschedule()
    }

    override fun onDetachedFromWindow() {
        removeCallbacks(advance)
        super.onDetachedFromWindow()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = getDefaultSize(suggestedMinimumWidth, widthMeasureSpec)
        val height = getDefaultSize(suggestedMinimumHeight, heightMeasureSpec)
        val side = minOf(width, height)
        setMeasuredDimension(side, side)
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val list = drawList ?: return
        val availableWidth = width - paddingLeft - paddingRight
        val availableHeight = height - paddingTop - paddingBottom
        val side = minOf(availableWidth, availableHeight)
        if (side <= 0) return
        renderer.draw(
            canvas,
            list,
            paddingLeft + (availableWidth - side) / 2f,
            paddingTop + (availableHeight - side) / 2f,
            side.toFloat(),
        )
    }

    private fun showFrame(encoder: PetalStreamEncoder, number: Int) {
        frame = number
        drawList = PetalDrawList.of(encoder.cells(number), palette)
        invalidate()
    }

    private fun reschedule() {
        removeCallbacks(advance)
        if (running && encoder != null && isAttachedToWindow) postOnAnimationDelayed(advance, frameIntervalMillis())
    }

    private fun frameIntervalMillis(): Long = Math.max(1L, Math.round(1000.0 / framesPerSecond))

    private companion object {
        const val DEFAULT_FRAMES_PER_SECOND = 8.0
    }
}
