package org.hyperledger.iroha.sdk.offline.petal

import android.graphics.Canvas
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF

/**
 * Draws a [PetalDrawList] onto an [android.graphics.Canvas].
 *
 * The frame is painted into the square `(left, top, left + size, top + size)`
 * in the order [PetalDrawList] documents: background, finder blossoms (discs
 * then petal notches), the faint `天` backdrop clipped to its disc, the lit
 * petals, then the lit ring dots. Every petal is drawn antialiased with its
 * notch, along its axis (its outward direction plus its turn): the notch tells a
 * petal from one turned by half a turn, and the turn reader's templates include
 * it. Each colour is one path draw: the finder and backdrop paths and the
 * notched unit petal are built once, and a frame only gathers the lit petals
 * (the unit petal turned and moved into place) and dots into reused paths, so
 * drawing allocates nothing. Uses API 19 or older graphics calls (`Path.op` for
 * the clipped backdrop). Not thread-safe: draw from the UI (or one render) thread.
 */
class PetalCanvasRenderer {
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }

    private val finderDiscs = Path()
    private val finderNotches = Path()
    private val backdrop = Path()
    private val backdropClip = Path()
    private var backdropClipped = false
    private var discSource: List<PetalCircle>? = null
    private var notchSource: List<PetalCircle>? = null
    private var backdropSource: List<PetalRect>? = null

    /** One petal pointing along `+x` and centred on the origin, minus its notch. */
    private val unitPetal = notchedPetal()
    private val placement = Matrix()

    private val lightPetals = Path()
    private val pinkPetals = Path()
    private val dots = Path()

    init {
        val center = PetalDrawList.CANVAS.toFloat() / 2f
        backdropClip.addCircle(center, center, PetalDrawList.BACKDROP_RADIUS.toFloat(), Path.Direction.CW)
    }

    /** Paints [drawList] into the square of side [size] pixels at `(left, top)`. */
    fun draw(canvas: Canvas, drawList: PetalDrawList, left: Float, top: Float, size: Float) {
        prepareShapes(drawList)
        val palette = drawList.palette
        val outer = canvas.save()
        canvas.translate(left, top)
        val scale = size / PetalDrawList.CANVAS.toFloat()
        canvas.scale(scale, scale)
        val side = PetalDrawList.CANVAS.toFloat()
        fill.color = opaque(palette.background)
        canvas.drawRect(0f, 0f, side, side, fill)

        fill.color = opaque(palette.light)
        canvas.drawPath(finderDiscs, fill)
        fill.color = opaque(palette.background)
        canvas.drawPath(finderNotches, fill)

        fill.color = opaque(palette.backdrop)
        if (backdropClipped) {
            canvas.drawPath(backdrop, fill)
        } else {
            val clip = canvas.save()
            canvas.clipPath(backdropClip)
            canvas.drawPath(backdrop, fill)
            canvas.restoreToCount(clip)
        }

        lightPetals.rewind()
        pinkPetals.rewind()
        for (petal in drawList.petals) {
            // the unit petal turned to the petal's axis, then moved to its centre
            placement.setRotate(Math.toDegrees(petal.angle).toFloat())
            placement.postTranslate(petal.centerX.toFloat(), petal.centerY.toFloat())
            (if (petal.light) lightPetals else pinkPetals).addPath(unitPetal, placement)
        }
        fill.color = opaque(palette.light)
        canvas.drawPath(lightPetals, fill)
        fill.color = opaque(palette.pink)
        canvas.drawPath(pinkPetals, fill)

        dots.rewind()
        for (dot in drawList.dots) dots.addCircle(dot.x.toFloat(), dot.y.toFloat(), dot.radius.toFloat(), Path.Direction.CW)
        canvas.drawPath(dots, fill)
        canvas.restoreToCount(outer)
    }

    /** Builds the finder and backdrop paths the first time (draw lists share them between frames). */
    private fun prepareShapes(drawList: PetalDrawList) {
        if (drawList.finderDiscs !== discSource || drawList.finderNotches !== notchSource) {
            finderDiscs.rewind()
            for (disc in drawList.finderDiscs) {
                finderDiscs.addCircle(disc.x.toFloat(), disc.y.toFloat(), disc.radius.toFloat(), Path.Direction.CW)
            }
            finderNotches.rewind()
            for (notch in drawList.finderNotches) {
                finderNotches.addCircle(notch.x.toFloat(), notch.y.toFloat(), notch.radius.toFloat(), Path.Direction.CW)
            }
            discSource = drawList.finderDiscs
            notchSource = drawList.finderNotches
        }
        if (drawList.backdropCells !== backdropSource) {
            backdrop.rewind()
            for (cell in drawList.backdropCells) {
                backdrop.addRect(cell.left.toFloat(), cell.top.toFloat(), cell.right.toFloat(), cell.bottom.toFloat(), Path.Direction.CW)
            }
            // clip once; should the boolean operation fail, every frame clips with the circle instead
            backdropClipped = backdrop.op(backdropClip, Path.Op.INTERSECT)
            backdropSource = drawList.backdropCells
        }
    }

    private companion object {
        fun opaque(rgb: Int): Int = rgb or 0xFF00_0000.toInt()

        /**
         * The petal of `specs/petal_stream.md` §2.3 in its own frame, built from two arcs so that
         * no boolean path operation can lose its notch: the ellipse of half-length
         * [PetalDrawList.PETAL_HALF_LENGTH] along `+x` and half-width
         * [PetalDrawList.PETAL_HALF_WIDTH], the long way round from one point where the notch (a
         * disc of radius [PetalDrawList.PETAL_NOTCH_RADIUS] centred
         * [PetalDrawList.PETAL_NOTCH_DISTANCE] along `+x`) meets it to the other, then the notch's
         * arc back inside the ellipse. Arc angles of an oval are parametric: the point at angle `t`
         * is `(a cos t, b sin t)`.
         */
        fun notchedPetal(): Path {
            val a = PetalDrawList.PETAL_HALF_LENGTH
            val b = PetalDrawList.PETAL_HALF_WIDTH
            val d = PetalDrawList.PETAL_NOTCH_DISTANCE
            val r = PetalDrawList.PETAL_NOTCH_RADIUS
            // the notch circle meets the ellipse where x²(1 − b²/a²) − 2dx + d² + b² − r² = 0
            val qa = 1.0 - b * b / (a * a)
            val qc = d * d + b * b - r * r
            val x = (2.0 * d - Math.sqrt(4.0 * d * d - 4.0 * qa * qc)) / (2.0 * qa)
            val y = b * Math.sqrt(1.0 - x * x / (a * a))
            val ellipseStart = Math.toDegrees(Math.atan2(y / b, x / a)).toFloat()
            val notchHalf = Math.toDegrees(Math.atan2(y, d - x)).toFloat()
            return Path().apply {
                arcTo(RectF(-a.toFloat(), -b.toFloat(), a.toFloat(), b.toFloat()), ellipseStart, 360f - 2f * ellipseStart, true)
                arcTo(RectF((d - r).toFloat(), -r.toFloat(), (d + r).toFloat(), r.toFloat()), 180f + notchHalf, -2f * notchHalf, false)
                close()
            }
        }
    }
}
