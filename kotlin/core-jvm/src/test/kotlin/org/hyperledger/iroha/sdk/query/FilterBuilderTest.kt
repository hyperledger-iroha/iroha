package org.hyperledger.iroha.sdk.query

import java.math.BigDecimal
import java.math.BigInteger
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class FilterBuilderTest {
    @Test
    fun builderMatchesTheParser() {
        val built = (field("owned_by") eq "alice") and
            (field("quantity") gte BigDecimal("10.5")) and
            (field("status").isIn("A", "B") or (field("tier") lt -1)) and
            !field("metadata.frozen").exists() and
            field("note").isNotNull()
        val parsed = Filter.parse(
            """owned_by = "alice" and quantity >= 10.5 and (status in ["A", "B"] or tier < -1)
               and not exists(metadata.frozen) and note is not null""",
        )
        assertEquals(parsed, built)
        assertEquals(
            """owned_by = "alice" and quantity >= "10.5" and (status in ["A", "B"] or tier < -1) and not exists(metadata.frozen) and note is not null""",
            built.toString(),
        )
        built.validate()
    }

    @Test
    fun literalsAreExactAndNeverDoubles() {
        assertEquals(JsonNumber.of(7L), (field("a") eq 7).value())
        assertEquals(JsonNumber.of(-7L), (field("a") eq -7L).value())
        assertEquals(JsonNumber.of(BigInteger("18446744073709551615")), (field("a") eq BigInteger("18446744073709551615")).value())
        assertEquals(JsonString("18446744073709551616"), (field("a") eq BigInteger("18446744073709551616")).value())
        assertEquals(JsonString("-9223372036854775809"), (field("a") eq BigInteger("-9223372036854775809")).value())
        assertEquals(JsonNumber.of(25L), (field("a") eq BigDecimal("25")).value())
        assertEquals(JsonString("25.0"), (field("a") eq BigDecimal("25.0")).value())
        assertEquals(JsonNumber.of(1000L), (field("a") eq BigDecimal("1E+3")).value())
        assertEquals(JsonString("0.000001"), (field("a") eq BigDecimal("1E-6")).value())
        assertEquals(JsonString("1.5"), (field("a") eq JsonNumber.parse("1.5")).value())
        assertFailsWith<IllegalArgumentException> { field("a").isIn(listOf(1.5)) }
        assertEquals(
            """amount in ["1.5", 2]""",
            field("amount").isIn(listOf(BigDecimal("1.5"), 2L)).toString(),
        )
    }

    @Test
    fun renderingParenthesizesExactlyLikeTorii() {
        val a = field("a") eq 1
        val b = field("b") eq 2
        val c = field("c") eq 3
        assertEquals("a = 1 or b = 2 or c = 3", (a or b or c).toString())
        assertEquals("(a = 1 or b = 2) and c = 3", ((a or b) and c).toString())
        assertEquals("not (a = 1 or b = 2)", (!(a or b)).toString())
        assertEquals("not (a = 1 and b = 2)", (!(a and b)).toString())
        assertEquals("a = 1 or b = 2 and c = 3", (a or (b and c)).toString())
        assertEquals("not a = 1", (!a).toString())
        assertEquals("x is not null", field("x").isNotNull().toString())
        assertEquals("not x is not null", (!field("x").isNotNull()).toString())
        assertEquals(
            "metadata.`ui-order` > 1 and `exists` = true and metadata.and = 1",
            ((field("metadata.ui-order") gt 1) and (field("exists") eq true) and (field("metadata.and") eq 1)).toString(),
        )
        assertEquals(Filter.parse((!(a or b)).toString()), !(a or b))
    }

    @Test
    fun operandsAreValidatedWhenBuilt() {
        assertFailsWith<InvalidFilterException> { field("a").isIn(*emptyArray<String>()) }
        val duplicate = assertFailsWith<InvalidFilterException> { field("a").isIn("x", "x") }
        assertEquals("invalid operand for `a`: membership list values must be unique", duplicate.message)
        assertEquals("invalid_filter", duplicate.code)
        assertFailsWith<InvalidFilterException> { field("a").isIn(listOf("x", 1L)) }
        field("metadata.mixed").isIn(listOf("x", 1L))
        assertFailsWith<InvalidFilterException> { field("a") lt Json.NULL }
        assertFailsWith<InvalidFilterException> { field("status") eq Json.array(Json.of(1)) }
        field("metadata.tags") eq Json.array(Json.of("x"))
        assertFailsWith<InvalidFilterException> { field("") }
        assertFailsWith<InvalidFilterException> { field("a..b") }
        assertFailsWith<InvalidFilterException> { field("has space") }
    }

    @Test
    fun treeLimitsMatchTorii() {
        var deep: Filter = field("a") eq 1
        repeat(10) { deep = !deep }
        deep.validate()
        val tooDeep = assertFailsWith<InvalidFilterException> { (!deep).validate() }
        assertEquals("filter exceeds the nesting depth limit of 10", tooDeep.message)
        val wide = Filter.all((0 until 1_024).map { field("f$it") eq it })!!
        val tooMany = assertFailsWith<InvalidFilterException> { wide.validate() }
        assertEquals("filter exceeds the node count limit of 1024", tooMany.message)
        val listValues = (0 until 1_024).map { it.toLong() }
        val many = Filter.all((0 until 5).map { field("f$it").isIn(*listValues.toLongArray()) })!!
        val totals = assertFailsWith<InvalidFilterException> { many.validate() }
        assertEquals("filter exceeds the total membership values limit of 4096", totals.message)
    }

    @Test
    fun jsonFormErrorsNameTheNode() {
        val unknownOp = assertFailsWith<InvalidFilterException> {
            Filter.fromJson("""{"op":"and","args":[{"op":"eq","args":["a",1]},{"op":"like","args":["b","x"]}]}""")
        }
        assertTrue(unknownOp.message!!.startsWith("unknown operator `like`"), unknownOp.message)
        assertTrue(unknownOp.message!!.endsWith("(at `args[1]`)"), unknownOp.message)
        val extra = assertFailsWith<InvalidFilterException> { Filter.fromJson("""{"op":"eq","args":["a",1],"x":1}""") }
        assertEquals("unknown member `x`; a filter node has only `op` and `args`", extra.message)
        val binary = assertFailsWith<InvalidFilterException> { Filter.fromJson("""{"op":"eq"}""") }
        assertEquals("`eq` takes [\"field\", value] (at `args`)", binary.message)
        assertEquals(Filter.parse("a = 1"), Filter.fromJson(Json.of("a = 1")))
        assertFailsWith<FilterSyntaxException> { Filter.fromJson(Json.of("a =")) }
    }

    @Test
    fun fractionalJsonNumbersAreRejectedLikeTorii() {
        for (json in listOf(
            """{"op":"eq","args":["a",1.5]}""",
            """{"op":"gte","args":["a",0.25]}""",
            """{"op":"in","args":["a",[1,2.5]]}""",
            """{"op":"eq","args":["a",1e3]}""",
            """{"op":"eq","args":["metadata.tags",[1,[2.5]]]}""",
        )) {
            val error = assertFailsWith<InvalidFilterException>(json) { Filter.fromJson(json) }
            assertTrue(error.message!!.contains("write decimals as strings"), error.message)
        }
        Filter.fromJson("""{"op":"eq","args":["a","1.5"]}""")
        Filter.fromJson("""{"op":"eq","args":["a",340282366920938463463374607431768211455]}""")
        assertFailsWith<InvalidFilterException> {
            Filter.fromJson("""{"op":"eq","args":["a",340282366920938463463374607431768211456]}""")
        }
        assertFailsWith<InvalidFilterException> {
            field("metadata.tags") eq Json.array(JsonNumber.parse("1.5"))
        }
        assertEquals("a = \"1.5\"", (field("a") eq JsonNumber.parse("1.5")).toString())
    }

    @Test
    fun structuredLiteralsTravelOnlyInTheJsonForm() {
        val tags = (field("metadata.tags") eq Json.parse("""["a","b"]""")) and (field("owned_by") eq "alice")
        val query = ListQuery.builder().filter(tags).build()
        assertEquals(
            """{"filter":{"op":"and","args":[{"op":"eq","args":["metadata.tags",["a","b"]]},""" +
                """{"op":"eq","args":["owned_by","alice"]}]}}""",
            query.toJson().toJsonString(),
        )
        val pairs = assertFailsWith<ListQueryException> { query.toQueryPairs() }
        assertEquals("invalid_filter", pairs.code)
        assertTrue(pairs.message!!.contains("JSON form"), pairs.message)
        assertFailsWith<ListQueryException> { query.toQueryString() }
        assertFailsWith<ListQueryException> {
            org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamOptions.builder().setFilter(tags)
        }
        val profile = field("metadata.profile") eq Json.parse("""{"tier":1,"score":{"value":2}}""")
        assertEquals(profile, Filter.fromJson(profile.toJson()))
        val fractional = assertFailsWith<InvalidFilterException> {
            Filter.fromJson("""{"op":"eq","args":["metadata.profile",{"tier":1,"score":{"value":2.5}}]}""")
        }
        assertTrue(fractional.message!!.contains("write decimals as strings"), fractional.message)
        assertFailsWith<InvalidFilterException> { field("metadata.profile") eq Json.parse("""{"rate":0.5}""") }
    }

    @Test
    fun combinatorsFlattenAndFold() {
        val a = field("a") eq 1
        val b = field("b") eq 2
        val c = field("c") eq 3
        val anded = (a and b) and (c and a)
        assertTrue(anded is Filter.And && anded.operands.size == 4)
        assertNull(Filter.all(emptyList()))
        assertEquals(a or b or c, Filter.any(a, b, c))
        assertEquals(a and b, Filter.all(listOf(a, b)))
    }

    @Test
    fun syntaxErrorsCarryPositionsInTheMessage() {
        val error = assertFailsWith<FilterSyntaxException> { Filter.parse("a = 1\nand b ~ 2") }
        assertEquals("unexpected character `~` (line 2, column 7)", error.message)
        assertEquals(2, error.line)
        val single = assertFailsWith<FilterSyntaxException> { Filter.parse("a == 1 && b = 2") }
        assertEquals("use the keyword `and` instead of `&` or `&&` (column 8)", single.message)
        val tooLong = assertFailsWith<FilterSyntaxException> { Filter.parse("a = \"" + "x".repeat(40_000) + "\"") }
        assertEquals("filters must not exceed 32768 bytes", tooLong.reason)
    }

    @Test
    fun sortKeysRenderWithBackticks() {
        assertEquals("-quantity", field("quantity").desc().toString())
        assertEquals("metadata.`ui-order`", SortKey.asc("metadata.ui-order").toString())
        assertEquals("desc", SortKey.asc("desc").toString())
        assertEquals("-`not`", SortKey.desc("not").toString())
        assertEquals(listOf(SortKey.desc("quantity"), SortKey.asc("id")), SortKey.parseList("-quantity, id"))
        val tooMany = assertFailsWith<FilterSyntaxException> { SortKey.parseList((1..9).joinToString(",") { "f$it" }) }
        assertEquals("sort specifications accept at most 8 keys", tooMany.reason)
        assertEquals("invalid_sort", assertFailsWith<ListQueryException> { SortKey.asc("") }.code)
    }

    private fun Filter.value(): Json = (this as Filter.Comparison).value
}
