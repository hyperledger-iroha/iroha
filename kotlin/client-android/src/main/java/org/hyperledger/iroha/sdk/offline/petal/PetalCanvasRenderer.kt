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
 * petals, then the lit ring dots. Each colour is one path draw: the finder and
 * backdrop paths are built once, every field cell's notched petal is built the
 * first time it lights up, and a frame only gathers the lit petals and dots
 * into reused paths, so once the petals are built drawing allocates nothing.
 * Uses API 19 or older graphics calls (`Path.op` for the clipped backdrop and
 * the petal notch). Not thread-safe: draw from the UI (or one render) thread.
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
    private val unitPetal = Path()

    /** The petal shape of every field cell, and the petal it was built from. */
    private val petalShapes = arrayOfNulls<Path>(PetalLayout.FIELD_CELLS)
    private val petalOwners = arrayOfNulls<PetalDrawPetal>(PetalLayout.FIELD_CELLS)
    private val scratchPetal = Path()
    private val placement = Matrix()

    private val lightPetals = Path()
    private val pinkPetals = Path()
    private val dots = Path()

    init {
        val halfLength = PetalDrawList.PETAL_HALF_LENGTH.toFloat()
        val halfWidth = PetalDrawList.PETAL_HALF_WIDTH.toFloat()
        val oval = Path().apply { addOval(RectF(-halfLength, -halfWidth, halfLength, halfWidth), Path.Direction.CW) }
        val notch = Path().apply {
            addCircle(
                PetalDrawList.PETAL_NOTCH_DISTANCE.toFloat(),
                0f,
                PetalDrawList.PETAL_NOTCH_RADIUS.toFloat(),
                Path.Direction.CW,
            )
        }
        // the notch is cosmetic: a plain ellipse is a valid petal when the boolean operation fails
        if (!unitPetal.op(oval, notch, Path.Op.DIFFERENCE)) unitPetal.set(oval)
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
        for (petal in drawList.petals) (if (petal.light) lightPetals else pinkPetals).addPath(petalShape(petal))
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

    /** The shape of [petal]: cached per field cell, rebuilt only for a petal object it was not built from. */
    private fun petalShape(petal: PetalDrawPetal): Path {
        val cell = petal.cell
        if (cell !in 0 until PetalLayout.FIELD_CELLS) {
            place(petal, scratchPetal)
            return scratchPetal
        }
        if (petalOwners[cell] === petal) return petalShapes[cell]!!
        val shape = petalShapes[cell] ?: Path().also { petalShapes[cell] = it }
        place(petal, shape)
        petalOwners[cell] = petal
        return shape
    }

    /** Writes the unit petal turned to [petal]'s outward direction and moved to its centre into [into]. */
    private fun place(petal: PetalDrawPetal, into: Path) {
        placement.setRotate(Math.toDegrees(petal.angle).toFloat())
        placement.postTranslate(petal.centerX.toFloat(), petal.centerY.toFloat())
        unitPetal.transform(placement, into)
    }

    private companion object {
        fun opaque(rgb: Int): Int = rgb or 0xFF00_0000.toInt()
    }
}
