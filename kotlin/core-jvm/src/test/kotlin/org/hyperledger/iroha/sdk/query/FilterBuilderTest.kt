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
    fun fieldPathsMustNotContainBackticks() {
        val message = "invalid field `metadata.a`b`: field paths must not contain backticks"
        assertEquals(message, assertFailsWith<InvalidFilterException> { field("metadata.a`b") }.message)
        assertEquals(message, assertFailsWith<IllegalArgumentException> { FieldPath.of("metadata.a`b") }.message)
        for (json in listOf(
            """{"op":"eq","args":["metadata.a`b",1]}""",
            """{"op":"in","args":["metadata.a`b",[1]]}""",
            """{"op":"exists","args":["metadata.a`b"]}""",
        )) {
            assertEquals(message, assertFailsWith<InvalidFilterException>(json) { Filter.fromJson(json) }.message)
        }
        assertEquals("invalid_sort", assertFailsWith<ListQueryException> { SortKey.asc("a`b") }.code)
        assertEquals("invalid_select", assertFailsWith<ListQueryException> { ListQuery.builder().select("a`b") }.code)
        val select = assertFailsWith<ListQueryException> { ListQuery.fromJson(Json.parse("""{"select":["a`b"]}""")) }
        assertEquals("invalid_select", select.code)
        // A backtick in the text form always opens or closes a quoted segment.
        assertEquals(field("a-b.c") eq 1, Filter.parse("`a-b`.c = 1"))
    }

    @Test
    fun stringLiteralsKeepDelAndC1ButRejectC0Controls() {
        val raw = "x\u007fy\u0080\u0085\u009fz"
        val parsed = Filter.parse("a = \"$raw\"")
        assertEquals(field("a") eq raw, parsed)
        assertEquals("a = \"$raw\"", parsed.toString())
        assertEquals(parsed, Filter.parse("a = '$raw'"))
        for (control in listOf("\u0000", "\u0001", "\t", "\n", "\u001f")) {
            val error = assertFailsWith<FilterSyntaxException> { Filter.parse("a = \"x${control}y\"") }
            assertEquals("control characters must be escaped inside string literals", error.reason)
            assertEquals(7, error.column)
        }
        // Only `"`, `\` and U+0000..U+001F are escaped, with JSON's short forms.
        val value = "q\"b\\s\b\u000C\n\r\t\u0000\u001f\u007f\u0085/'"
        val rendered = (field("a") eq value).toString()
        assertEquals("a = \"q\\\"b\\\\s\\b\\f\\n\\r\\t\\u0000\\u001f\u007f\u0085/'\"", rendered)
        assertEquals(field("a") eq value, Filter.parse(rendered))
    }

    @Test
    fun singleOperandConnectivesDecodeToTheirOperand() {
        for (op in listOf("and", "or")) {
            val decoded = Filter.fromJson("""{"op":"$op","args":[{"op":"eq","args":["a",1]}]}""")
            assertEquals(field("a") eq 1, decoded)
            assertEquals("a = 1", decoded.toString())
        }
        val nested = Filter.fromJson(
            """{"op":"and","args":[{"op":"or","args":[{"op":"and","args":[{"op":"eq","args":["a",1]},""" +
                """{"op":"is_null","args":["b"]}]}]},{"op":"or","args":[{"op":"eq","args":["a",1]}]}]}""",
        )
        assertEquals("(a = 1 and b is null) and a = 1", nested.toString())
        assertTrue(nested is Filter.And && nested.operands.size == 2 && nested.operands[0] is Filter.And)
        // The collapsed connective still counts toward the depth limit.
        var deep = """{"op":"eq","args":["a",1]}"""
        repeat(11) { deep = """{"op":"or","args":[$deep]}""" }
        val tooDeep = assertFailsWith<InvalidFilterException> { Filter.fromJson(deep) }
        assertEquals("filter exceeds the nesting depth limit of 10", tooDeep.message)
    }

    @Test
    fun aggregatesAreBoundedAndTheirPathsValidated() {
        fun spec(groups: Int, metrics: Int): AggregateSpec.Builder {
            val builder = AggregateSpec.builder().groupBy(*Array(groups) { "metadata.k$it" })
            repeat(metrics) { builder.metric(AggregateMetric.count("m$it")) }
            return builder
        }
        val widest = spec(AggregateSpec.MAX_GROUP_BY, AggregateSpec.MAX_METRICS).build()
        val body = ListQuery.builder().aggregate(widest).build().toJson()
        assertEquals(widest, ListQuery.fromJson(body).aggregate)
        val groups = assertFailsWith<ListQueryException> { spec(AggregateSpec.MAX_GROUP_BY + 1, 1).build() }
        assertEquals("`group_by` lists at most 8 fields", groups.message)
        assertEquals("invalid_aggregate", groups.code)
        val metrics = assertFailsWith<ListQueryException> { spec(0, AggregateSpec.MAX_METRICS + 1).build() }
        assertEquals("`metrics` lists at most 16 metrics", metrics.message)
        assertEquals("aggregate", metrics.parameter)
        for (json in listOf(
            """{"aggregate":{"groupby":["a"],"metrics":[{"alias":"n","fn":"count"}]}}""",
            """{"aggregate":{"metrics":[{"alias":"n","fn":"count","feild":"a"}]}}""",
            """{"aggregate":{"group_by":["a","b","c","d","e","f","g","h","i"],"metrics":[{"alias":"n","fn":"count"}]}}""",
            """{"aggregate":{"metrics":""" + (0..16).joinToString(",", "[", "]") { """{"alias":"m$it","fn":"count"}""" } + "}}",
            """{"aggregate":{"group_by":["a..b"],"metrics":[{"alias":"n","fn":"count"}]}}""",
            """{"aggregate":{"group_by":["a`b"],"metrics":[{"alias":"n","fn":"count"}]}}""",
            """{"aggregate":{"metrics":[{"alias":"s","fn":"sum","field":"a b"}]}}""",
        )) {
            val error = assertFailsWith<ListQueryException>(json) { ListQuery.fromJson(Json.parse(json)) }
            assertEquals("invalid_aggregate", error.code, json)
            assertEquals("aggregate", error.parameter, json)
        }
        val backtick = assertFailsWith<ListQueryException> { AggregateSpec.builder().groupBy("a`b") }
        assertEquals("invalid field `a`b`: field paths must not contain backticks", backtick.message)
        assertFailsWith<ListQueryException> { AggregateMetric.sum("s", "a`b") }
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
