package org.hyperledger.iroha.sdk.offline.petal

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF

/**
 * Draws a [PetalDrawList] onto an [android.graphics.Canvas].
 *
 * The frame is painted into the square `(left, top, left + size, top + size)`
 * in the order [PetalDrawList] documents: background, finder blossoms (discs
 * then petal notches), rounded tiles with their katakana strokes (round caps
 * and joins, clipped to the glyph box), then the lit ring dots. The sixteen
 * glyph paths are built once per renderer. Uses only API 1 graphics calls, so
 * it runs on every supported Android version. Not thread-safe: draw from the
 * UI (or one render) thread.
 */
class PetalCanvasRenderer {
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }

    private val glyphStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
        // Glyph paths are drawn in the 32-unit glyph grid, so the width is in grid units too.
        strokeWidth = PetalGlyphs.STROKE_WIDTH.toFloat()
    }

    private val glyphPaths: Array<Path> = Array(PetalGlyphs.GLYPH_COUNT) { glyph ->
        Path().apply {
            for (polyline in PetalGlyphs.strokes(glyph)) {
                moveTo(polyline[0].toFloat(), polyline[1].toFloat())
                var point = 2
                while (point + 1 < polyline.size) {
                    lineTo(polyline[point].toFloat(), polyline[point + 1].toFloat())
                    point += 2
                }
            }
        }
    }

    private val tileRect = RectF()

    /** Paints [drawList] into the square of side [size] pixels at `(left, top)`. */
    fun draw(canvas: Canvas, drawList: PetalDrawList, left: Float, top: Float, size: Float) {
        val palette = drawList.palette
        val background = opaque(palette.background)
        val light = opaque(palette.light)
        val pink = opaque(palette.pink)
        val ink = opaque(palette.ink)
        val outer = canvas.save()
        canvas.translate(left, top)
        val scale = size / PetalLayout.CANVAS.toFloat()
        canvas.scale(scale, scale)
        val side = PetalLayout.CANVAS.toFloat()
        fill.color = background
        canvas.drawRect(0f, 0f, side, side, fill)

        fill.color = light
        for (disc in drawList.finderDiscs) {
            canvas.drawCircle(disc.x.toFloat(), disc.y.toFloat(), disc.radius.toFloat(), fill)
        }
        fill.color = background
        for (notch in drawList.finderNotches) {
            canvas.drawCircle(notch.x.toFloat(), notch.y.toFloat(), notch.radius.toFloat(), fill)
        }

        val half = (PetalLayout.TILE_SIZE / 2.0).toFloat()
        val corner = PetalLayout.TILE_CORNER_RADIUS.toFloat()
        val glyphScale = PetalDrawList.GLYPH_SCALE.toFloat()
        val grid = PetalGlyphs.GLYPH_GRID.toFloat()
        fill.color = light
        for (tile in drawList.tiles) {
            val cx = tile.centerX.toFloat()
            val cy = tile.centerY.toFloat()
            if (tile.light) {
                tileRect.set(cx - half, cy - half, cx + half, cy + half)
                canvas.drawRoundRect(tileRect, corner, corner, fill)
            }
            glyphStroke.color = if (tile.light) ink else pink
            val glyphLayer = canvas.save()
            canvas.translate(tile.glyphLeft.toFloat(), tile.glyphTop.toFloat())
            canvas.scale(glyphScale, glyphScale)
            canvas.clipRect(0f, 0f, grid, grid)
            canvas.drawPath(glyphPaths[tile.glyph], glyphStroke)
            canvas.restoreToCount(glyphLayer)
        }

        fill.color = pink
        for (dot in drawList.dots) {
            canvas.drawCircle(dot.x.toFloat(), dot.y.toFloat(), dot.radius.toFloat(), fill)
        }
        canvas.restoreToCount(outer)
    }

    private companion object {
        fun opaque(rgb: Int): Int = rgb or 0xFF00_0000.toInt()
    }
}
