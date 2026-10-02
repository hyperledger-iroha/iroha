package org.hyperledger.iroha.sdk.offline.petal

/** A detected corner finder. */
class PetalFinder(
    /** Centre `x` in pixel-edge coordinates. */
    val x: Double,
    /** Centre `y` in pixel-edge coordinates. */
    val y: Double,
    /** Apparent outer diameter in pixels. */
    val size: Double,
) {
    override fun equals(other: Any?): Boolean =
        other is PetalFinder && x == other.x && y == other.y && size == other.size

    override fun hashCode(): Int = 31 * (31 * x.hashCode() + y.hashCode()) + size.hashCode()

    override fun toString(): String = "PetalFinder(x=$x, y=$y, size=$size)"
}

/** A 4-connected component of a binarised image. */
class PetalComponent internal constructor(
    /** Pixel count. */
    val area: Int,
    /** Left-most pixel column. */
    val minX: Int,
    /** Right-most pixel column. */
    val maxX: Int,
    /** Top-most pixel row. */
    val minY: Int,
    /** Bottom-most pixel row. */
    val maxY: Int,
    internal val sumX: Double,
    internal val sumY: Double,
    internal val sumXX: Double,
    internal val sumYY: Double,
    internal val sumXY: Double,
) {
    /** Bounding-box width in pixels. */
    val width: Double get() = (maxX - minX + 1).toDouble()

    /** Bounding-box height in pixels. */
    val height: Double get() = (maxY - minY + 1).toDouble()

    /** Centroid `x` (pixel-edge coordinates). */
    val centroidX: Double get() = sumX / area.toDouble()

    /** Centroid `y` (pixel-edge coordinates). */
    val centroidY: Double get() = sumY / area.toDouble()

    /** Ratio of the smaller to the larger principal axis of the blob. */
    val axisRatio: Double get() = PetalLocator.axisRatio(area, sumX, sumY, sumXX, sumYY, sumXY)
}

/**
 * Finding the four corner finders in a camera luma plane.
 *
 * Pipeline: adaptive threshold (local mean via an integral image) → 4-connected
 * component labelling → blossom detection (a large, round, isolated blob) →
 * selection of the four finders that form a plausible, similarly sized
 * quadrilateral → intensity-weighted centre refinement. Solid blossoms survive
 * defocus that would fill in the gaps of a ring-shaped marker.
 */
object PetalLocator {
    private val SENSITIVITIES = doubleArrayOf(0.12, 0.22, 0.34)

    /** Finder candidates combined into quadrilaterals, largest first. */
    private const val MAX_COMBINED = 10

    /**
     * Marks pixels that are clearly brighter than their neighbourhood.
     *
     * [sensitivity] scales the margin above the local mean in units of the
     * image's dynamic range (≈ 0.12 for faint codes, larger values separate
     * blurred finder rings from their cores). Returns a row-major mask.
     */
    @JvmStatic
    fun adaptiveBinarize(image: PetalLuma, sensitivity: Double): BooleanArray {
        val workspace = PetalWorkspace()
        val levels = prepare(image, workspace)
        threshold(image, workspace, levels, sensitivity)
        return workspace.mask.copyOf(image.width * image.height)
    }

    /** Labels 4-connected components of a row-major [mask]; returns the components with area > 0. */
    @JvmStatic
    fun labelComponents(mask: BooleanArray, width: Int, height: Int): List<PetalComponent> {
        require(width >= 0 && height >= 0 && width.toLong() * height.toLong() == mask.size.toLong()) {
            "mask size mismatch"
        }
        val workspace = PetalWorkspace()
        workspace.ensurePixels(width, height)
        val count = label(mask, width, height, workspace)
        return List(count) { index ->
            PetalComponent(
                workspace.area[index], workspace.minX[index], workspace.maxX[index], workspace.minY[index],
                workspace.maxY[index], workspace.sumX[index], workspace.sumY[index], workspace.sumXX[index],
                workspace.sumYY[index], workspace.sumXY[index],
            )
        }
    }

    /** Detects blossom finders: large, round, isolated blobs, in component order. */
    @JvmStatic
    fun blossoms(components: List<PetalComponent>): List<PetalFinder> {
        val workspace = PetalWorkspace()
        workspace.ensureComponents(components.size)
        components.forEachIndexed { index, component ->
            workspace.area[index] = component.area
            workspace.minX[index] = component.minX
            workspace.maxX[index] = component.maxX
            workspace.minY[index] = component.minY
            workspace.maxY[index] = component.maxY
            workspace.sumX[index] = component.sumX
            workspace.sumY[index] = component.sumY
            workspace.sumXX[index] = component.sumXX
            workspace.sumYY[index] = component.sumYY
            workspace.sumXY[index] = component.sumXY
        }
        return blossoms(workspace, components.size)
    }

    /**
     * Chooses four finders that look like the corners of one code, ordered
     * clockwise (as displayed) from the one nearest the top-left.
     *
     * Lit tiles and merged dots form blob candidates too, so the largest size
     * class is tried first and at most the ten largest candidates are combined:
     * the corner finders are always the biggest isolated round blobs in view.
     */
    @JvmStatic
    fun selectQuad(finders: List<PetalFinder>): List<PetalFinder>? = selectQuadArray(finders)?.toList()

    /** Sharpens a finder centre with an intensity-weighted centroid. */
    @JvmStatic
    fun refineCenter(image: PetalLuma, finder: PetalFinder): PetalFinder {
        val radius = Math.ceil(finder.size * 0.5).toInt()
        val cx = Math.floor(finder.x).toInt()
        val cy = Math.floor(finder.y).toInt()
        val reach = finder.size * 0.5
        val width = image.width
        val height = image.height
        val pixels = image.pixels
        var floor = Double.MAX_VALUE
        var peak = 0.0
        for (dy in -radius..radius) {
            val y = cy + dy
            if (y < 0 || y >= height) continue
            for (dx in -radius..radius) {
                val x = cx + dx
                if (x < 0 || x >= width) continue
                val ex = x + 0.5 - finder.x
                val ey = y + 0.5 - finder.y
                if (Math.sqrt(ex * ex + ey * ey) <= reach) {
                    val value = (pixels[y * width + x].toInt() and 0xFF).toDouble()
                    floor = PetalNumerics.min(floor, value)
                    peak = PetalNumerics.max(peak, value)
                }
            }
        }
        if (peak - floor < 20.0) return finder
        val threshold = floor + 0.5 * (peak - floor)
        var sw = 0.0
        var sx = 0.0
        var sy = 0.0
        for (dy in -radius..radius) {
            val y = cy + dy
            if (y < 0 || y >= height) continue
            for (dx in -radius..radius) {
                val x = cx + dx
                if (x < 0 || x >= width) continue
                val px = x + 0.5
                val py = y + 0.5
                val ex = px - finder.x
                val ey = py - finder.y
                if (Math.sqrt(ex * ex + ey * ey) <= reach) {
                    val value = (pixels[y * width + x].toInt() and 0xFF).toDouble()
                    val weight = PetalNumerics.max(value - threshold, 0.0)
                    sw += weight
                    sx += weight * px
                    sy += weight * py
                }
            }
        }
        return if (sw <= 0.0) finder else PetalFinder(sx / sw, sy / sw, finder.size)
    }

    /**
     * Locates the four finders of a code, trying progressively stricter
     * thresholds so blurred rings still separate from their cores. Returns the
     * refined finders clockwise from the top-left, or `null`.
     */
    @JvmStatic
    fun locate(image: PetalLuma): List<PetalFinder>? = locate(image, PetalWorkspace())?.toList()

    internal fun locate(image: PetalLuma, workspace: PetalWorkspace): Array<PetalFinder>? {
        if (image.width == 0 || image.height == 0) return null
        val levels = prepare(image, workspace)
        for (sensitivity in SENSITIVITIES) {
            threshold(image, workspace, levels, sensitivity)
            val count = label(workspace.mask, image.width, image.height, workspace)
            val quad = selectQuadArray(blossoms(workspace, count)) ?: continue
            return Array(4) { refineCenter(image, quad[it]) }
        }
        return null
    }

    /** Ratio of the smaller to the larger principal axis of a blob's second moments. */
    internal fun axisRatio(area: Int, sumX: Double, sumY: Double, sumXX: Double, sumYY: Double, sumXY: Double): Double {
        val n = area.toDouble()
        val cx = sumX / n
        val cy = sumY / n
        val vxx = sumXX / n - cx * cx
        val vyy = sumYY / n - cy * cy
        val vxy = sumXY / n - cx * cy
        val mean = 0.5 * (vxx + vyy)
        val difference = vxx - vyy
        val spread = Math.sqrt(0.25 * (difference * difference) + vxy * vxy)
        val major = mean + spread
        val minor = PetalNumerics.max(mean - spread, 0.0)
        return if (major <= 0.0) 0.0 else Math.sqrt(minor / major)
    }

    /** Dynamic-range levels shared by every sensitivity pass. */
    private class Levels(val low: Double, val high: Double)

    /** Builds the integral image into [workspace] and measures the 2 % / 99.5 % percentiles. */
    private fun prepare(image: PetalLuma, workspace: PetalWorkspace): Levels {
        val w = image.width
        val h = image.height
        require((w + 1).toLong() * (h + 1).toLong() <= Int.MAX_VALUE) { "image is too large" }
        workspace.ensurePixels(w, h)
        val integral = workspace.integral
        val pixels = image.pixels
        val stride = w + 1
        // Sums wrap modulo 2^32; every box sum is far below 2^31, so the
        // inclusion–exclusion differences stay exact.
        for (x in 0..w) integral[x] = 0
        val histogram = IntArray(256)
        for (y in 0 until h) {
            var row = 0
            val source = y * w
            val target = (y + 1) * stride
            val previous = y * stride
            integral[target] = 0
            for (x in 0 until w) {
                val value = pixels[source + x].toInt() and 0xFF
                histogram[value] += 1
                row += value
                integral[target + x + 1] = integral[previous + x + 1] + row
            }
        }
        val total = (w.toLong() * h.toLong()).toDouble()
        return Levels(percentile(histogram, total, 0.02), percentile(histogram, total, 0.995))
    }

    private fun percentile(histogram: IntArray, total: Double, fraction: Double): Double {
        val target = total * fraction
        var seen = 0.0
        for (level in 0 until 256) {
            seen += histogram[level].toDouble()
            if (seen >= target) return level.toDouble()
        }
        return 255.0
    }

    /** Thresholds the prepared image into `workspace.mask`. */
    private fun threshold(image: PetalLuma, workspace: PetalWorkspace, levels: Levels, sensitivity: Double) {
        val w = image.width
        val h = image.height
        val range = PetalNumerics.max(levels.high - levels.low, 8.0)
        val radius = (minOf(w, h) / 8).coerceIn(12, 64)
        val margin = PetalNumerics.max(sensitivity * range, 5.0)
        val floor = levels.low + 0.2 * range
        val integral = workspace.integral
        val mask = workspace.mask
        val pixels = image.pixels
        val stride = w + 1
        for (y in 0 until h) {
            val y0 = if (y > radius) y - radius else 0
            val y1 = minOf(y + radius + 1, h)
            val top = y0 * stride
            val bottom = y1 * stride
            val rows = y1 - y0
            val base = y * w
            for (x in 0 until w) {
                val x0 = if (x > radius) x - radius else 0
                val x1 = minOf(x + radius + 1, w)
                val sum = integral[bottom + x1] + integral[top + x0] - integral[top + x1] - integral[bottom + x0]
                val mean = sum.toDouble() / ((x1 - x0) * rows).toDouble()
                val value = (pixels[base + x].toInt() and 0xFF).toDouble()
                mask[base + x] = value > mean + margin && value > floor
            }
        }
    }

    /**
     * Labels the 4-connected components of [mask] into the workspace component
     * arrays (in order of each component's first pixel) and returns their count.
     */
    private fun label(mask: BooleanArray, w: Int, h: Int, workspace: PetalWorkspace): Int {
        val labels = workspace.labels
        var parent = workspace.parent
        parent[0] = 0
        var next = 1
        for (y in 0 until h) {
            val base = y * w
            for (x in 0 until w) {
                val i = base + x
                if (!mask[i]) {
                    labels[i] = 0
                    continue
                }
                val left = if (x > 0) labels[i - 1] else 0
                val up = if (y > 0) labels[i - w] else 0
                labels[i] = if (left == 0 && up == 0) {
                    if (next == parent.size) {
                        workspace.growParent(next + 1)
                        parent = workspace.parent
                    }
                    parent[next] = next
                    next += 1
                    next - 1
                } else if (up == 0) {
                    left
                } else if (left == 0) {
                    up
                } else {
                    val a = findRoot(parent, left)
                    val b = findRoot(parent, up)
                    val keep = if (a < b) a else b
                    val drop = if (a < b) b else a
                    parent[drop] = keep
                    keep
                }
            }
        }
        workspace.ensureComponents(next)
        val area = workspace.area
        val minX = workspace.minX
        val maxX = workspace.maxX
        val minY = workspace.minY
        val maxY = workspace.maxY
        val sumX = workspace.sumX
        val sumY = workspace.sumY
        val sumXX = workspace.sumXX
        val sumYY = workspace.sumYY
        val sumXY = workspace.sumXY
        java.util.Arrays.fill(area, 0, next, 0)
        java.util.Arrays.fill(sumX, 0, next, 0.0)
        java.util.Arrays.fill(sumY, 0, next, 0.0)
        java.util.Arrays.fill(sumXX, 0, next, 0.0)
        java.util.Arrays.fill(sumYY, 0, next, 0.0)
        java.util.Arrays.fill(sumXY, 0, next, 0.0)
        for (y in 0 until h) {
            val base = y * w
            val py = y + 0.5
            for (x in 0 until w) {
                val labelValue = labels[base + x]
                if (labelValue == 0) continue
                val root = findRoot(parent, labelValue)
                if (area[root] == 0) {
                    minX[root] = x
                    maxX[root] = x
                    minY[root] = y
                    maxY[root] = y
                }
                area[root] += 1
                if (x < minX[root]) minX[root] = x
                if (x > maxX[root]) maxX[root] = x
                if (y < minY[root]) minY[root] = y
                if (y > maxY[root]) maxY[root] = y
                val px = x + 0.5
                sumX[root] += px
                sumY[root] += py
                sumXX[root] += px * px
                sumYY[root] += py * py
                sumXY[root] += px * py
            }
        }
        // Keep the components with area > 0, in root-label order (in place: target <= source).
        var count = 0
        for (root in 0 until next) {
            if (area[root] == 0) continue
            if (count != root) {
                area[count] = area[root]
                minX[count] = minX[root]
                maxX[count] = maxX[root]
                minY[count] = minY[root]
                maxY[count] = maxY[root]
                sumX[count] = sumX[root]
                sumY[count] = sumY[root]
                sumXX[count] = sumXX[root]
                sumYY[count] = sumYY[root]
                sumXY[count] = sumXY[root]
            }
            count += 1
        }
        return count
    }

    private fun findRoot(parent: IntArray, start: Int): Int {
        var label = start
        while (parent[label] != label) {
            parent[label] = parent[parent[label]]
            label = parent[label]
        }
        return label
    }

    /** Detects blossoms among the first [count] workspace components. */
    private fun blossoms(workspace: PetalWorkspace, count: Int): ArrayList<PetalFinder> {
        val area = workspace.area
        val centroidX = workspace.centroidX
        val centroidY = workspace.centroidY
        for (index in 0 until count) {
            centroidX[index] = workspace.sumX[index] / area[index].toDouble()
            centroidY[index] = workspace.sumY[index] / area[index].toDouble()
        }
        val found = ArrayList<PetalFinder>()
        for (index in 0 until count) {
            val blobArea = area[index]
            if (blobArea < 100) continue
            val width = (workspace.maxX[index] - workspace.minX[index] + 1).toDouble()
            val height = (workspace.maxY[index] - workspace.minY[index] + 1).toDouble()
            val size = PetalNumerics.max(width, height)
            val fill = blobArea.toDouble() / (width * height)
            if (size < 14.0 || !(fill >= 0.45 && fill <= 0.9)) continue
            val ratio = axisRatio(
                blobArea, workspace.sumX[index], workspace.sumY[index],
                workspace.sumXX[index], workspace.sumYY[index], workspace.sumXY[index],
            )
            if (ratio < 0.5) continue
            val x = centroidX[index]
            val y = centroidY[index]
            // isolation: nothing else of substance close by
            val minimumArea = 0.015 * blobArea.toDouble()
            val reach = 0.8 * size
            var crowded = false
            for (other in 0 until count) {
                val otherArea = area[other]
                if (other == index || otherArea < 8 || otherArea.toDouble() < minimumArea) continue
                val dx = centroidX[other] - x
                val dy = centroidY[other] - y
                if (Math.sqrt(dx * dx + dy * dy) < reach) {
                    crowded = true
                    break
                }
            }
            if (!crowded) found += PetalFinder(x, y, size)
        }
        return found
    }

    private fun selectQuadArray(finders: List<PetalFinder>): Array<PetalFinder>? {
        var largest = 0.0
        for (finder in finders) largest = PetalNumerics.max(largest, finder.size)
        val strong = finders.filter { it.size >= 0.55 * largest }
        return selectQuadFrom(strong) ?: selectQuadFrom(finders)
    }

    private fun selectQuadFrom(finders: List<PetalFinder>): Array<PetalFinder>? {
        if (finders.size < 4) return null
        // Largest first (ties keep discovery order) so that clutter in a busy scene
        // cannot push the real finders out of the ten candidates that are combined.
        val ranked = arrayOfNulls<PetalFinder>(minOf(finders.size, MAX_COMBINED))
        var n = 0
        for (candidate in finders) {
            var at = n
            while (at > 0 && PetalNumerics.totalCompare(candidate.size, ranked[at - 1]!!.size) > 0) at -= 1
            if (at >= ranked.size) continue
            for (k in minOf(n, ranked.size - 1) downTo at + 1) ranked[k] = ranked[k - 1]
            ranked[at] = candidate
            if (n < ranked.size) n += 1
        }
        var bestScore = 0.0
        var best: Array<PetalFinder>? = null
        val set = arrayOfNulls<PetalFinder>(4)
        val sides = DoubleArray(4)
        for (a in 0 until n) {
            for (b in a + 1 until n) {
                for (c in b + 1 until n) {
                    for (d in c + 1 until n) {
                        set[0] = ranked[a]
                        set[1] = ranked[b]
                        set[2] = ranked[c]
                        set[3] = ranked[d]
                        var smin = Double.MAX_VALUE
                        var smax = 0.0
                        var sizeSum = -0.0
                        for (finder in set) {
                            smin = PetalNumerics.min(smin, finder!!.size)
                            smax = PetalNumerics.max(smax, finder.size)
                            sizeSum += finder.size
                        }
                        if (smax / smin > 1.9) continue
                        val quad = orderClockwise(set) ?: continue
                        var lmin = Double.MAX_VALUE
                        var lmax = 0.0
                        var sideSum = -0.0
                        for (i in 0 until 4) {
                            val p = quad[i]
                            val q = quad[(i + 1) % 4]
                            val dx = p.x - q.x
                            val dy = p.y - q.y
                            sides[i] = Math.sqrt(dx * dx + dy * dy)
                        }
                        for (side in sides) {
                            lmin = PetalNumerics.min(lmin, side)
                            lmax = PetalNumerics.max(lmax, side)
                            sideSum += side
                        }
                        val meanSize = sizeSum / 4.0
                        // canvas geometry: side / finder diameter = 880 / 120
                        val ratio = (sideSum / 4.0) / meanSize
                        if (lmax / lmin > 2.6 || !(ratio >= 4.8 && ratio <= 10.5)) continue
                        val score = (smax / smin - 1.0) + (lmax / lmin - 1.0) + Math.abs((ratio - 7.33) / 7.33)
                        if (best == null || score < bestScore) {
                            bestScore = score
                            best = quad
                        }
                    }
                }
            }
        }
        return best
    }

    /**
     * Orders four finders clockwise (as displayed, `y` down) starting from the
     * one nearest the top-left of the quadrilateral's bounding box; `null` when
     * they do not form a convex quadrilateral.
     */
    private fun orderClockwise(set: Array<PetalFinder?>): Array<PetalFinder>? {
        val quad = Array(4) { set[it]!! }
        val cx = (quad[0].x + quad[1].x + quad[2].x + quad[3].x) / 4.0
        val cy = (quad[0].y + quad[1].y + quad[2].y + quad[3].y) / 4.0
        val angles = DoubleArray(4) { StrictMath.atan2(quad[it].y - cy, quad[it].x - cx) }
        // Stable insertion sort by angle (atan2 grows clockwise on screen because y points down).
        for (i in 1 until 4) {
            val finder = quad[i]
            val angle = angles[i]
            var j = i - 1
            while (j >= 0 && PetalNumerics.totalCompare(angles[j], angle) > 0) {
                quad[j + 1] = quad[j]
                angles[j + 1] = angles[j]
                j -= 1
            }
            quad[j + 1] = finder
            angles[j + 1] = angle
        }
        for (i in 0 until 4) {
            val o = quad[i]
            val a = quad[(i + 1) % 4]
            val b = quad[(i + 2) % 4]
            if ((a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x) <= 0.0) return null
        }
        var start = 0
        for (i in 1 until 4) {
            if (PetalNumerics.totalCompare(quad[i].x + quad[i].y, quad[start].x + quad[start].y) < 0) start = i
        }
        return Array(4) { quad[(start + it) % 4] }
    }
}
