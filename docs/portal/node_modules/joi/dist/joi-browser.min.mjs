//#region \0rolldown/runtime.js
var e = Object.create, t = Object.defineProperty, n = Object.getOwnPropertyDescriptor, r = Object.getOwnPropertyNames, i = Object.getPrototypeOf, a = Object.prototype.hasOwnProperty, o = (e, t) => () => (e && (t = e(e = 0)), t), s = (e, t) => () => (t || (e((t = { exports: {} }).exports, t), e = null), t.exports), c = (e, n) => {
	let r = {};
	for (var i in e) t(r, i, {
		get: e[i],
		enumerable: !0
	});
	return n || t(r, Symbol.toStringTag, { value: "Module" }), r;
}, l = (e, i, o, s) => {
	if (i && typeof i == "object" || typeof i == "function") for (var c = r(i), l = 0, u = c.length, d; l < u; l++) d = c[l], !a.call(e, d) && d !== o && t(e, d, {
		get: ((e) => i[e]).bind(null, d),
		enumerable: !(s = n(i, d)) || s.enumerable
	});
	return e;
}, u = (n, r, a) => (a = n == null ? {} : e(i(n)), l(r || !n || !n.__esModule ? t(a, "default", {
	value: n,
	enumerable: !0
}) : a, n)), d = (e) => a.call(e, "module.exports") ? e["module.exports"] : l(t({}, "__esModule", { value: !0 }), e), f = /* @__PURE__ */ s(((e, t) => {
	t.exports = class extends Error {
		name = "AssertError";
		constructor(e, t) {
			super(e || "Unknown error"), typeof Error.captureStackTrace == "function" && Error.captureStackTrace(this, t);
		}
	};
})), p = /* @__PURE__ */ s(((e, t) => {
	t.exports = function(...e) {
		try {
			return JSON.stringify(...e);
		} catch (e) {
			return "[Cannot display object: " + e.message + "]";
		}
	};
})), m = /* @__PURE__ */ s(((e, t) => {
	var n = f(), r = p(), i = t.exports = function(e, ...t) {
		if (!e) throw t.length === 1 && t[0] instanceof Error ? t[0] : new n(t.filter((e) => e !== "").map((e) => typeof e == "string" ? e : e instanceof Error ? e.message : r(e)).join(" "), i);
	};
})), h = /* @__PURE__ */ s(((e, t) => {
	var n = m(), r = {};
	t.exports = function(e, t, i) {
		if (t === !1 || t == null) return e;
		i ||= {}, typeof i == "string" && (i = { separator: i });
		let a = Array.isArray(t);
		n(!a || !i.separator, "Separator option is not valid for array-based chain");
		let o = a ? t : t.split(i.separator || "."), s = e;
		for (let e = 0; e < o.length; ++e) {
			let a = o[e], c = i.iterables && r.iterables(s);
			if (Array.isArray(s) || c === "set") {
				let e = Number(a);
				Number.isInteger(e) && (a = e < 0 ? s.length + e : e);
			}
			if (!s || typeof s == "function" && i.functions === !1 || !c && s[a] === void 0) {
				n(!i.strict || e + 1 === o.length, "Missing segment", a, "in reach path ", t), n(typeof s == "object" || i.functions === !0 || typeof s != "function", "Invalid segment", a, "in reach path ", t), s = i.default;
				break;
			}
			s = c ? c === "set" ? [...s][a] : s.get(a) : s[a];
		}
		return s;
	}, r.iterables = function(e) {
		if (e instanceof Set) return "set";
		if (e instanceof Map) return "map";
	};
})), g = /* @__PURE__ */ s(((e, t) => {
	var n = {};
	e = t.exports = {
		array: Array.prototype,
		buffer: !1,
		date: Date.prototype,
		error: Error.prototype,
		generic: Object.prototype,
		map: Map.prototype,
		promise: Promise.prototype,
		regex: RegExp.prototype,
		set: Set.prototype,
		url: URL.prototype,
		weakMap: WeakMap.prototype,
		weakSet: WeakSet.prototype
	}, n.typeMap = new Map([
		["[object Error]", e.error],
		["[object Map]", e.map],
		["[object Promise]", e.promise],
		["[object Set]", e.set],
		["[object URL]", e.url],
		["[object WeakMap]", e.weakMap],
		["[object WeakSet]", e.weakSet]
	]), e.getInternalProto = function(t) {
		if (Array.isArray(t)) return e.array;
		if (t instanceof Date) return e.date;
		if (t instanceof RegExp) return e.regex;
		if (t instanceof Error) return e.error;
		let r = Object.prototype.toString.call(t);
		return n.typeMap.get(r) || e.generic;
	};
})), _ = /* @__PURE__ */ s(((e) => {
	e.keys = function(e, t = {}) {
		return t.symbols === !1 ? Object.getOwnPropertyNames(e) : Reflect.ownKeys(e);
	};
})), v = /* @__PURE__ */ s(((e, t) => {
	var n = h(), r = g(), i = _(), a = {
		needsProtoHack: new Set([
			r.set,
			r.map,
			r.weakSet,
			r.weakMap
		]),
		structuredCloneExists: typeof structuredClone == "function"
	};
	t.exports = a.clone = function(e, t = {}, n = null) {
		if (typeof e != "object" || !e) return e;
		let o = a.clone, s = n;
		if (t.shallow) {
			if (t.shallow !== !0) return a.cloneWithShallow(e, t);
			o = (e) => e;
		} else if (s) {
			let t = s.get(e);
			if (t) return t;
		} else s = /* @__PURE__ */ new Map();
		let c = r.getInternalProto(e);
		switch (c) {
			case r.buffer: return (!1)?.from(e);
			case r.date: return new Date(e.getTime());
			case r.regex:
			case r.url: return new c.constructor(e);
		}
		let l = a.base(e, c, t);
		if (l === e) return e;
		if (s && s.set(e, l), c === r.set) for (let n of e) l.add(o(n, t, s));
		else if (c === r.map) for (let [n, r] of e) l.set(n, o(r, t, s));
		let u = i.keys(e, t);
		for (let n of u) {
			if (n === "__proto__") continue;
			if (c === r.array && n === "length") {
				l.length = e.length;
				continue;
			}
			if (a.structuredCloneExists && c === r.error && n === "stack") continue;
			let i = Object.getOwnPropertyDescriptor(e, n);
			i ? i.get || i.set ? Object.defineProperty(l, n, i) : i.enumerable ? l[n] = o(e[n], t, s) : Object.defineProperty(l, n, {
				enumerable: !1,
				writable: !0,
				configurable: !0,
				value: o(e[n], t, s)
			}) : Object.defineProperty(l, n, {
				enumerable: !0,
				writable: !0,
				configurable: !0,
				value: o(e[n], t, s)
			});
		}
		return l;
	}, a.cloneWithShallow = function(e, t) {
		let r = t.shallow;
		t = Object.assign({}, t), t.shallow = !1;
		let i = /* @__PURE__ */ new Map();
		for (let t of r) {
			let r = n(e, t);
			(typeof r == "object" || typeof r == "function") && i.set(r, r);
		}
		return a.clone(e, t, i);
	}, a.base = function(e, t, n) {
		if (n.prototype === !1) return a.needsProtoHack.has(t) ? new t.constructor() : t === r.array ? [] : {};
		let i = Object.getPrototypeOf(e);
		if (i && i.isImmutable) return e;
		if (t === r.array) {
			let e = [];
			return i !== t && Object.setPrototypeOf(e, i), e;
		} else if (t === r.error && a.structuredCloneExists && (i === t || Error.isPrototypeOf(i.constructor))) {
			let t = structuredClone(e);
			return Object.getPrototypeOf(t) !== i && Object.setPrototypeOf(t, i), t;
		}
		if (a.needsProtoHack.has(t)) {
			let e = new i.constructor();
			return i !== t && Object.setPrototypeOf(e, i), e;
		}
		return Object.create(i);
	};
})), y = /* @__PURE__ */ s(((e, t) => {
	var n = m(), r = v(), i = _(), a = {};
	t.exports = a.merge = function(e, t, o) {
		if (n(e && typeof e == "object", "Invalid target value: must be an object"), n(t == null || typeof t == "object", "Invalid source value: must be null, undefined, or an object"), !t) return e;
		if (o = Object.assign({
			nullOverride: !0,
			mergeArrays: !0
		}, o), Array.isArray(t)) {
			n(Array.isArray(e), "Cannot merge array onto an object"), o.mergeArrays || (e.length = 0);
			for (let n = 0; n < t.length; ++n) e.push(r(t[n], { symbols: o.symbols }));
			return e;
		}
		let s = i.keys(t, o);
		for (let n = 0; n < s.length; ++n) {
			let i = s[n];
			if (i === "__proto__" || !Object.prototype.propertyIsEnumerable.call(t, i)) continue;
			let c = t[i];
			if (c && typeof c == "object") {
				if (e[i] === c) continue;
				!e[i] || typeof e[i] != "object" || Array.isArray(e[i]) !== Array.isArray(c) || c instanceof Date || c instanceof RegExp ? e[i] = r(c, { symbols: o.symbols }) : a.merge(e[i], c, o);
			} else c == null ? o.nullOverride && (e[i] = c) : e[i] = c;
		}
		return e;
	};
})), b = /* @__PURE__ */ s(((e, t) => {
	var n = m(), r = v(), i = y(), a = h(), o = {};
	t.exports = function(e, t, a = {}) {
		if (n(e && typeof e == "object", "Invalid defaults value: must be an object"), n(!t || t === !0 || typeof t == "object", "Invalid source value: must be true, falsy or an object"), n(typeof a == "object", "Invalid options: must be an object"), !t) return null;
		if (a.shallow) return o.applyToDefaultsWithShallow(e, t, a);
		let s = r(e);
		return t === !0 ? s : i(s, t, {
			nullOverride: a.nullOverride === void 0 ? !1 : a.nullOverride,
			mergeArrays: !1
		});
	}, o.applyToDefaultsWithShallow = function(e, t, s) {
		let c = s.shallow;
		n(Array.isArray(c), "Invalid keys");
		let l = /* @__PURE__ */ new Map(), u = t === !0 ? null : /* @__PURE__ */ new Set();
		for (let n of c) {
			n = Array.isArray(n) ? n : n.split(".");
			let r = a(e, n);
			r && typeof r == "object" ? l.set(r, u && a(t, n) || r) : u && u.add(n);
		}
		let d = r(e, {}, l);
		if (!u) return d;
		for (let e of u) o.reachCopy(d, t, e);
		return i(d, t, {
			nullOverride: s.nullOverride === void 0 ? !1 : s.nullOverride,
			mergeArrays: !1
		});
	}, o.reachCopy = function(e, t, n) {
		for (let e of n) {
			if (!(e in t)) return;
			let n = t[e];
			if (typeof n != "object" || !n) return;
			t = n;
		}
		let r = t, i = e;
		for (let e = 0; e < n.length - 1; ++e) {
			let t = n[e];
			typeof i[t] != "object" && (i[t] = {}), i = i[t];
		}
		i[n[n.length - 1]] = r;
	};
})), x = /* @__PURE__ */ s(((e, t) => {
	t.exports = {};
})), S = /* @__PURE__ */ s(((e, t) => {
	var n = g(), r = { mismatched: null };
	t.exports = function(e, t, n) {
		return n = Object.assign({ prototype: !0 }, n), !!r.isDeepEqual(e, t, n, []);
	}, r.isDeepEqual = function(e, t, i, a) {
		if (e === t) return e !== 0 || 1 / e == 1 / t;
		let o = typeof e;
		if (o !== typeof t || e === null || t === null) return !1;
		if (o === "function") {
			if (!i.deepFunction || e.toString() !== t.toString()) return !1;
		} else if (o !== "object") return e !== e && t !== t;
		let s = r.getSharedType(e, t, !!i.prototype);
		switch (s) {
			case n.buffer: return !1;
			case n.promise: return e === t;
			case n.regex:
			case n.url: return e.toString() === t.toString();
			case r.mismatched: return !1;
		}
		for (let n = a.length - 1; n >= 0; --n) if (a[n].isSame(e, t)) return !0;
		a.push(new r.SeenEntry(e, t));
		try {
			return !!r.isDeepEqualObj(s, e, t, i, a);
		} finally {
			a.pop();
		}
	}, r.getSharedType = function(e, t, i) {
		if (i) return Object.getPrototypeOf(e) === Object.getPrototypeOf(t) ? n.getInternalProto(e) : r.mismatched;
		let a = n.getInternalProto(e);
		return a === n.getInternalProto(t) ? a : r.mismatched;
	}, r.valueOf = function(e) {
		let t = e.valueOf;
		if (t === void 0) return e;
		try {
			return t.call(e);
		} catch (e) {
			return e;
		}
	}, r.hasOwnEnumerableProperty = function(e, t) {
		return Object.prototype.propertyIsEnumerable.call(e, t);
	}, r.isSetSimpleEqual = function(e, t) {
		for (let n of Set.prototype.values.call(e)) if (!Set.prototype.has.call(t, n)) return !1;
		return !0;
	}, r.isDeepEqualObj = function(e, t, i, a, o) {
		let { isDeepEqual: s, valueOf: c, hasOwnEnumerableProperty: l } = r, { keys: u, getOwnPropertySymbols: d } = Object;
		if (e === n.array) if (a.part) {
			for (let e of t) for (let t of i) if (s(e, t, a, o)) return !0;
		} else {
			if (t.length !== i.length) return !1;
			for (let e = 0; e < t.length; ++e) if (!s(t[e], i[e], a, o)) return !1;
			return !0;
		}
		else if (e === n.set) {
			if (t.size !== i.size) return !1;
			if (!r.isSetSimpleEqual(t, i)) {
				let e = new Set(Set.prototype.values.call(i));
				for (let n of Set.prototype.values.call(t)) {
					if (e.delete(n)) continue;
					let t = !1;
					for (let r of e) if (s(n, r, a, o)) {
						e.delete(r), t = !0;
						break;
					}
					if (!t) return !1;
				}
			}
		} else if (e === n.map) {
			if (t.size !== i.size) return !1;
			for (let [e, n] of Map.prototype.entries.call(t)) if (n === void 0 && !Map.prototype.has.call(i, e) || !s(n, Map.prototype.get.call(i, e), a, o)) return !1;
		} else if (e === n.error && (t.name !== i.name || t.message !== i.message)) return !1;
		let f = c(t), p = c(i);
		if ((t !== f || i !== p) && !s(f, p, a, o)) return !1;
		let m = u(t);
		if (!a.part && m.length !== u(i).length && !a.skip) return !1;
		let h = 0;
		for (let e of m) {
			if (a.skip && a.skip.includes(e)) {
				i[e] === void 0 && ++h;
				continue;
			}
			if (!l(i, e) || !s(t[e], i[e], a, o)) return !1;
		}
		if (!a.part && m.length - h !== u(i).length) return !1;
		if (a.symbols !== !1) {
			let e = d(t), n = new Set(d(i));
			for (let r of e) {
				if (!a.skip?.includes(r)) {
					if (l(t, r)) {
						if (!l(i, r) || !s(t[r], i[r], a, o)) return !1;
					} else if (l(i, r)) return !1;
				}
				n.delete(r);
			}
			for (let e of n) if (l(i, e)) return !1;
		}
		return !0;
	}, r.SeenEntry = class {
		constructor(e, t) {
			this.obj = e, this.ref = t;
		}
		isSame(e, t) {
			return this.obj === e && this.ref === t;
		}
	};
})), C = /* @__PURE__ */ s(((e, t) => {
	var n = {};
	t.exports = function(e) {
		if (!e) return "";
		let t = "";
		for (let r = 0; r < e.length; ++r) {
			let i = e.charCodeAt(r);
			n.isSafe(i) ? t += e[r] : t += n.escapeHtmlChar(i);
		}
		return t;
	}, n.escapeHtmlChar = function(e) {
		return n.namedHtml.get(e) || (e >= 256 ? "&#" + e + ";" : `&#x${e.toString(16).padStart(2, "0")};`);
	}, n.isSafe = function(e) {
		return n.safeCharCodes.has(e);
	}, n.namedHtml = new Map([
		[38, "&amp;"],
		[60, "&lt;"],
		[62, "&gt;"],
		[34, "&quot;"],
		[160, "&nbsp;"],
		[162, "&cent;"],
		[163, "&pound;"],
		[164, "&curren;"],
		[169, "&copy;"],
		[174, "&reg;"]
	]), n.safeCharCodes = function() {
		let e = /* @__PURE__ */ new Set();
		for (let t = 32; t < 123; ++t) (t >= 97 || t >= 65 && t <= 90 || t >= 48 && t <= 57 || t === 32 || t === 46 || t === 44 || t === 45 || t === 58 || t === 95) && e.add(t);
		return e;
	}();
})), w = /* @__PURE__ */ s(((e, t) => {
	t.exports = function(e) {
		return e.replace(/[\^\$\.\*\+\-\?\=\!\:\|\\\/\(\)\[\]\{\}\,]/g, "\\$&");
	};
})), T = /* @__PURE__ */ s(((e, t) => {
	t.exports = function() {};
})), E = /* @__PURE__ */ s(((e) => {
	e.applyToDefaults = b(), e.assert = m(), e.AssertError = f(), e.Bench = x(), e.block = x(), e.clone = v(), e.contain = x(), e.deepEqual = S(), e.escapeHeaderAttribute = x(), e.escapeHtml = C(), e.escapeJson = x(), e.escapeRegex = w(), e.flatten = x(), e.ignore = T(), e.intersect = x(), e.isPromise = x(), e.merge = y(), e.once = x(), e.reach = h(), e.reachTemplate = x(), e.stringify = p(), e.wait = x();
})), D = /* @__PURE__ */ c({
	default: () => k,
	version: () => O
}), O, k, ee = o((() => {
	O = "18.2.1", k = { version: "18.2.1" };
})), te = /* @__PURE__ */ s(((e) => {
	var t = $(), n = {};
	n.wrap = t.string().min(1).max(2).allow(!1), e.preferences = t.object({
		allowUnknown: t.boolean(),
		abortEarly: t.boolean(),
		artifacts: t.boolean(),
		cache: t.boolean(),
		context: t.object(),
		convert: t.boolean(),
		dateFormat: t.valid("date", "iso", "string", "time", "utc"),
		debug: t.boolean(),
		errors: {
			escapeHtml: t.boolean(),
			label: t.valid("path", "key", !1),
			language: [t.string(), t.object().ref()],
			render: t.boolean(),
			stack: t.boolean(),
			wrap: {
				label: n.wrap,
				array: n.wrap,
				string: n.wrap
			}
		},
		externals: t.boolean(),
		messages: t.object(),
		noDefaults: t.boolean(),
		nonEnumerables: t.boolean(),
		presence: t.valid("required", "optional", "forbidden"),
		skipFunctions: t.boolean(),
		stripUnknown: t.object({
			arrays: t.boolean(),
			objects: t.boolean()
		}).or("arrays", "objects").allow(!0, !1),
		warnings: t.boolean()
	}).strict(), n.nameRx = /^[a-zA-Z0-9]\w*$/, n.rule = t.object({
		alias: t.array().items(t.string().pattern(n.nameRx)).single(),
		args: t.array().items(t.string(), t.object({
			name: t.string().pattern(n.nameRx).required(),
			ref: t.boolean(),
			assert: t.alternatives([t.function(), t.object().schema()]).conditional("ref", {
				is: !0,
				then: t.required()
			}),
			normalize: t.function(),
			message: t.string().when("assert", {
				is: t.function(),
				then: t.required()
			})
		})),
		convert: t.boolean(),
		manifest: t.boolean(),
		method: t.function().allow(!1),
		multi: t.boolean(),
		validate: t.function(),
		jsonSchema: t.function()
	}), e.extension = t.object({
		type: t.alternatives([t.string(), t.object().regex()]).required(),
		args: t.function(),
		cast: t.object().pattern(n.nameRx, t.object({
			from: t.function().maxArity(1).required(),
			to: t.function().minArity(1).maxArity(2).required()
		})),
		base: t.object().schema().when("type", {
			is: t.object().regex(),
			then: t.forbidden()
		}),
		coerce: [t.function().maxArity(3), t.object({
			method: t.function().maxArity(3).required(),
			from: t.array().items(t.string()).single()
		})],
		flags: t.object().pattern(n.nameRx, t.object({
			setter: t.string(),
			default: t.any()
		})),
		manifest: { build: t.function().arity(2) },
		messages: [t.object(), t.string()],
		modifiers: t.object().pattern(n.nameRx, t.function().minArity(1).maxArity(2)),
		overrides: t.object().pattern(n.nameRx, t.function()),
		prepare: t.function().maxArity(3),
		rebuild: t.function().arity(1),
		rules: t.object().pattern(n.nameRx, n.rule),
		jsonSchema: t.function(),
		terms: t.object().pattern(n.nameRx, t.object({
			init: t.array().allow(null).required(),
			manifest: t.object().pattern(/.+/, [t.valid("schema", "single"), t.object({ mapped: t.object({
				from: t.string().required(),
				to: t.string().required()
			}).required() })])
		})),
		validate: t.function().maxArity(3)
	}).strict(), e.extensions = t.array().items(t.object(), t.function().arity(1)).strict(), n.desc = {
		buffer: t.object({ buffer: t.string() }),
		func: t.object({
			function: t.function().required(),
			options: { literal: !0 }
		}),
		override: t.object({ override: !0 }),
		ref: t.object({ ref: t.object({
			type: t.valid("value", "global", "local"),
			path: t.array().required(),
			separator: t.string().length(1).allow(!1),
			ancestor: t.number().min(0).integer().allow("root"),
			map: t.array().items(t.array().length(2)).min(1),
			adjust: t.function(),
			iterables: t.boolean(),
			in: t.boolean(),
			render: t.boolean()
		}).required() }),
		regex: t.object({ regex: t.string().min(3) }),
		special: t.object({ special: t.valid("deep").required() }),
		template: t.object({
			template: t.string().required(),
			options: t.object()
		}),
		value: t.object({ value: t.alternatives([t.object(), t.array()]).required() })
	}, n.desc.entity = t.alternatives([
		t.array().items(t.link("...")),
		t.boolean(),
		t.function(),
		t.number(),
		t.string(),
		n.desc.buffer,
		n.desc.func,
		n.desc.ref,
		n.desc.regex,
		n.desc.special,
		n.desc.template,
		n.desc.value,
		t.link("/")
	]), n.desc.values = t.array().items(null, t.boolean(), t.function(), t.number().allow(Infinity, -Infinity, NaN), t.string().allow(""), t.symbol(), n.desc.buffer, n.desc.func, n.desc.override, n.desc.ref, n.desc.regex, n.desc.template, n.desc.value), n.desc.messages = t.object().pattern(/.+/, [
		t.string(),
		n.desc.template,
		t.object().pattern(/.+/, [t.string(), n.desc.template])
	]), e.description = t.object({
		type: t.string().required(),
		flags: t.object({
			cast: t.string(),
			default: t.any(),
			description: t.string(),
			empty: t.link("/"),
			failover: n.desc.entity,
			id: t.string(),
			label: t.string(),
			only: !0,
			presence: [
				"optional",
				"required",
				"forbidden"
			],
			result: ["raw", "strip"],
			strip: t.boolean(),
			unit: t.string()
		}).unknown(),
		preferences: {
			allowUnknown: t.boolean(),
			abortEarly: t.boolean(),
			artifacts: t.boolean(),
			cache: t.boolean(),
			convert: t.boolean(),
			dateFormat: [
				"date",
				"iso",
				"string",
				"time",
				"utc"
			],
			errors: {
				escapeHtml: t.boolean(),
				label: ["path", "key"],
				language: [t.string(), n.desc.ref],
				wrap: {
					label: n.wrap,
					array: n.wrap
				}
			},
			externals: t.boolean(),
			messages: n.desc.messages,
			noDefaults: t.boolean(),
			nonEnumerables: t.boolean(),
			presence: [
				"required",
				"optional",
				"forbidden"
			],
			skipFunctions: t.boolean(),
			stripUnknown: t.object({
				arrays: t.boolean(),
				objects: t.boolean()
			}).or("arrays", "objects").allow(!0, !1),
			warnings: t.boolean()
		},
		allow: n.desc.values,
		invalid: n.desc.values,
		rules: t.array().min(1).items({
			name: t.string().required(),
			args: t.object().min(1),
			keep: t.boolean(),
			message: [t.string(), n.desc.messages],
			warn: t.boolean()
		}),
		keys: t.object().pattern(/.*/, t.link("/")),
		link: n.desc.ref
	}).pattern(/^[a-z]\w*$/, t.any());
})), ne = /* @__PURE__ */ s(((e) => {
	var t = {
		operators: [
			"!",
			"^",
			"*",
			"/",
			"%",
			"+",
			"-",
			"<",
			"<=",
			">",
			">=",
			"==",
			"!=",
			"&&",
			"||",
			"??"
		],
		operatorCharacters: [
			"!",
			"^",
			"*",
			"/",
			"%",
			"+",
			"-",
			"<",
			"=",
			">",
			"&",
			"|",
			"?"
		],
		operatorsOrder: [
			["^"],
			[
				"*",
				"/",
				"%"
			],
			["+", "-"],
			[
				"<",
				"<=",
				">",
				">="
			],
			["==", "!="],
			["&&"],
			["||", "??"]
		],
		operatorsPrefix: ["!", "n"],
		literals: {
			"\"": "\"",
			"`": "`",
			"'": "'",
			"[": "]"
		},
		numberRx: /^(?:[0-9]*(\.[0-9]*)?){1}$/,
		tokenRx: /^[\w\$\#\.\@\:\{\}]+$/,
		symbol: Symbol("formula"),
		settings: Symbol("settings")
	};
	e.Parser = class {
		constructor(e, n = {}) {
			if (!n[t.settings] && n.constants) for (let e in n.constants) {
				let t = n.constants[e];
				if (t !== null && ![
					"boolean",
					"number",
					"string"
				].includes(typeof t)) throw Error(`Formula constant ${e} contains invalid ${typeof t} value type`);
			}
			this.settings = n[t.settings] ? n : Object.assign({
				[t.settings]: !0,
				constants: {},
				functions: {}
			}, n), this.single = null, this._parts = null, this._parse(e);
		}
		_parse(n) {
			let r = [], i = "", a = 0, o = !1, s = (n) => {
				if (a) throw Error("Formula missing closing parenthesis");
				let s = r.length ? r[r.length - 1] : null;
				if (!(!o && !i && !n)) {
					if (s && s.type === "reference" && n === ")") {
						s.type = "function", s.value = this._subFormula(i, s.value), i = "";
						return;
					}
					if (n === ")") {
						let t = new e.Parser(i, this.settings);
						r.push({
							type: "segment",
							value: t
						});
					} else if (o) {
						if (o === "]") {
							r.push({
								type: "reference",
								value: i
							}), i = "";
							return;
						}
						r.push({
							type: "literal",
							value: i
						});
					} else if (t.operatorCharacters.includes(i)) s && s.type === "operator" && t.operators.includes(s.value + i) ? s.value += i : r.push({
						type: "operator",
						value: i
					});
					else if (i.match(t.numberRx)) r.push({
						type: "constant",
						value: parseFloat(i)
					});
					else if (this.settings.constants[i] !== void 0) r.push({
						type: "constant",
						value: this.settings.constants[i]
					});
					else {
						if (!i.match(t.tokenRx)) throw Error(`Formula contains invalid token: ${i}`);
						r.push({
							type: "reference",
							value: i
						});
					}
					i = "";
				}
			};
			for (let e of n) o ? e === o ? (s(), o = !1) : i += e : a ? e === "(" ? (i += e, ++a) : e === ")" ? (--a, a ? i += e : s(e)) : i += e : e in t.literals ? o = t.literals[e] : e === "(" ? (s(), ++a) : t.operatorCharacters.includes(e) ? (s(), i = e, s()) : e === " " ? s() : i += e;
			s(), r = r.map((e, t) => e.type !== "operator" || e.value !== "-" || t && r[t - 1].type !== "operator" ? e : {
				type: "operator",
				value: "n"
			});
			let c = !1;
			for (let e of r) {
				if (e.type === "operator") {
					if (t.operatorsPrefix.includes(e.value)) continue;
					if (!c) throw Error("Formula contains an operator in invalid position");
					if (!t.operators.includes(e.value)) throw Error(`Formula contains an unknown operator ${e.value}`);
				} else if (c) throw Error("Formula missing expected operator");
				c = !c;
			}
			if (!c) throw Error("Formula contains invalid trailing operator");
			r.length === 1 && [
				"reference",
				"literal",
				"constant"
			].includes(r[0].type) && (this.single = {
				type: r[0].type === "reference" ? "reference" : "value",
				value: r[0].value
			}), this._parts = r.map((e) => {
				if (e.type === "operator") return t.operatorsPrefix.includes(e.value) ? e : e.value;
				if (e.type !== "reference") return e.value;
				if (this.settings.tokenRx && !this.settings.tokenRx.test(e.value)) throw Error(`Formula contains invalid reference ${e.value}`);
				return this.settings.reference ? this.settings.reference(e.value) : t.reference(e.value);
			});
		}
		_subFormula(n, r) {
			let i = this.settings.functions[r];
			if (typeof i != "function") throw Error(`Formula contains unknown function ${r}`);
			let a = [];
			if (n) {
				let e = "", i = 0, o = !1, s = () => {
					if (!e) throw Error(`Formula contains function ${r} with invalid arguments ${n}`);
					a.push(e), e = "";
				};
				for (let r = 0; r < n.length; ++r) {
					let a = n[r];
					o ? (e += a, a === o && (o = !1)) : a in t.literals && !i ? (e += a, o = t.literals[a]) : a === "," && !i ? s() : (e += a, a === "(" ? ++i : a === ")" && --i);
				}
				s();
			}
			return a = a.map((t) => new e.Parser(t, this.settings)), function(e) {
				let t = [];
				for (let n of a) t.push(n.evaluate(e));
				return i.call(e, ...t);
			};
		}
		evaluate(e) {
			let n = this._parts.slice();
			for (let r = n.length - 2; r >= 0; --r) {
				let i = n[r];
				if (i && i.type === "operator") {
					let a = n[r + 1];
					n.splice(r + 1, 1);
					let o = t.evaluate(a, e);
					n[r] = t.single(i.value, o);
				}
			}
			return t.operatorsOrder.forEach((r) => {
				for (let i = 1; i < n.length - 1;) if (r.includes(n[i])) {
					let r = n[i], a = t.evaluate(n[i - 1], e), o = t.evaluate(n[i + 1], e);
					n.splice(i, 2);
					let s = t.calculate(r, a, o);
					n[i - 1] = s === 0 ? 0 : s;
				} else i += 2;
			}), t.evaluate(n[0], e);
		}
	}, e.Parser.prototype[t.symbol] = !0, t.reference = function(e) {
		return function(t) {
			return t && t[e] !== void 0 ? t[e] : null;
		};
	}, t.evaluate = function(e, n) {
		return e === null ? null : typeof e == "function" ? e(n) : e[t.symbol] ? e.evaluate(n) : e;
	}, t.single = function(e, t) {
		if (e === "!") return !t;
		let n = -t;
		return n === 0 ? 0 : n;
	}, t.calculate = function(e, n, r) {
		if (e === "??") return t.exists(n) ? n : r;
		if (typeof n == "string" || typeof r == "string") {
			if (e === "+") return n = t.exists(n) ? n : "", r = t.exists(r) ? r : "", n + r;
		} else switch (e) {
			case "^": return n ** +r;
			case "*": return n * r;
			case "/": return n / r;
			case "%": return n % r;
			case "+": return n + r;
			case "-": return n - r;
		}
		switch (e) {
			case "<": return n < r;
			case "<=": return n <= r;
			case ">": return n > r;
			case ">=": return n >= r;
			case "==": return n === r;
			case "!=": return n !== r;
			case "&&": return n && r;
			case "||": return n || r;
		}
		return null;
	}, t.exists = function(e) {
		return e != null;
	};
})), A = /* @__PURE__ */ s(((e) => {
	var t = x(), n = P(), r = M();
	e.Report = class {
		constructor(t, n, r, i, a, o, s) {
			if (this.code = t, this.flags = i, this.messages = a, this.path = o.path, this.prefs = s, this.state = o, this.value = n, this.message = null, this.template = null, this.local = r || {}, this.local.label = e.label(this.flags, this.state, this.prefs, this.messages), this.value !== void 0 && !this.local.hasOwnProperty("value") && (this.local.value = this.value), this.path.length) {
				let e = this.path[this.path.length - 1];
				typeof e != "object" && (this.local.key = e);
			}
		}
		_setTemplate(e) {
			if (this.template = e, !this.flags.label && this.path.length === 0) {
				let e = this._template(this.template, "root");
				e && (this.local.label = e);
			}
		}
		toString() {
			if (this.message) return this.message;
			let e = this.code;
			if (!this.prefs.errors.render) return this.code;
			let t = this._template(this.template) || this._template(this.prefs.messages) || this._template(this.messages);
			return t === void 0 ? `Error code "${e}" is not defined, your custom type is missing the correct messages definition` : (this.message = t.render(this.value, this.state, this.prefs, this.local, {
				errors: this.prefs.errors,
				messages: [this.prefs.messages, this.messages]
			}), this.prefs.errors.label || (this.message = this.message.replace(/^"" /, "").trim()), this.message);
		}
		_template(t, n) {
			return e.template(this.value, t, n || this.code, this.state, this.prefs);
		}
	}, e.path = function(e) {
		let t = "";
		for (let n of e) typeof n != "object" && (typeof n == "string" ? (t && (t += "."), t += n) : t += `[${n}]`);
		return t;
	}, e.template = function(e, t, i, a, o) {
		if (!t) return;
		if (r.isTemplate(t)) return i === "root" ? null : t;
		let s = o.errors.language;
		if (n.isResolvable(s) && (s = s.resolve(e, a, o)), s && t[s]) {
			if (t[s][i] !== void 0) return t[s][i];
			if (t[s]["*"] !== void 0) return t[s]["*"];
		}
		return t[i] ? t[i] : t["*"];
	}, e.label = function(t, n, r, i) {
		if (!r.errors.label) return "";
		if (t.label) return t.label;
		let a = n.path;
		return r.errors.label === "key" && n.path.length > 1 && (a = n.path.slice(-1)), e.path(a) || e.template(null, r.messages, "root", n, r) || i && e.template(null, i, "root", n, r) || "value";
	}, e.process = function(t, n, r) {
		if (!t) return null;
		let { override: i, message: a, details: o } = e.details(t);
		if (i) return i;
		if (r.errors.stack) return new e.ValidationError(a, o, n);
		let s = Error.stackTraceLimit;
		Error.stackTraceLimit = 0;
		let c = new e.ValidationError(a, o, n);
		return Error.stackTraceLimit = s, c;
	}, e.details = function(e, t = {}) {
		let n = [], r = [];
		for (let i of e) {
			if (i instanceof Error) {
				if (t.override !== !1) return { override: i };
				let e = i.toString();
				n.push(e), r.push({
					message: e,
					type: "override",
					context: { error: i }
				});
				continue;
			}
			let e = i.toString();
			n.push(e), r.push({
				message: e,
				path: i.path.filter((e) => typeof e != "object"),
				type: i.code,
				context: i.local
			});
		}
		return n.length > 1 && (n = [...new Set(n)]), {
			message: n.join(". "),
			details: r
		};
	}, e.ValidationError = class extends Error {
		constructor(e, t, n) {
			super(e), this._original = n, this.details = t;
		}
		static isError(t) {
			return t instanceof e.ValidationError;
		}
	}, e.ValidationError.prototype.isJoi = !0, e.ValidationError.prototype.name = "ValidationError", e.ValidationError.prototype.annotate = t.error;
})), j = /* @__PURE__ */ s(((e) => {
	var { assert: t, clone: n, reach: r } = E(), i = P(), a, o = {
		symbol: Symbol("ref"),
		defaults: {
			adjust: null,
			in: !1,
			iterables: null,
			map: null,
			separator: ".",
			type: "value"
		}
	};
	e.create = function(e, n = {}) {
		t(typeof e == "string", "Invalid reference key:", e), i.assertOptions(n, [
			"adjust",
			"ancestor",
			"in",
			"iterables",
			"map",
			"prefix",
			"render",
			"separator"
		]), t(!n.prefix || typeof n.prefix == "object", "options.prefix must be of type object");
		let r = Object.assign({}, o.defaults, n);
		delete r.prefix;
		let a = r.separator, s = o.context(e, a, n.prefix);
		if (r.type = s.type, e = s.key, r.type === "value") if (s.root && (t(!a || e[0] !== a, "Cannot specify relative path with root prefix"), r.ancestor = "root", e ||= null), a && a === e) e = null, r.ancestor = 0;
		else if (r.ancestor !== void 0) t(!a || !e || e[0] !== a, "Cannot combine prefix with ancestor option");
		else {
			let [t, n] = o.ancestor(e, a);
			n && (e = e.slice(n), e === "" && (e = null)), r.ancestor = t;
		}
		return r.path = a ? e === null ? [] : e.split(a) : [e], new o.Ref(r);
	}, e.in = function(t, n = {}) {
		return e.create(t, {
			...n,
			in: !0
		});
	}, e.isRef = function(e) {
		return e ? !!e[i.symbols.ref] : !1;
	}, o.Ref = class {
		constructor(e) {
			t(typeof e == "object", "Invalid reference construction"), i.assertOptions(e, [
				"adjust",
				"ancestor",
				"in",
				"iterables",
				"map",
				"path",
				"render",
				"separator",
				"type",
				"depth",
				"key",
				"root",
				"display"
			]), t([!1, void 0].includes(e.separator) || typeof e.separator == "string" && e.separator.length === 1, "Invalid separator"), t(!e.adjust || typeof e.adjust == "function", "options.adjust must be a function"), t(!e.map || Array.isArray(e.map), "options.map must be an array"), t(!e.map || !e.adjust, "Cannot set both map and adjust options"), Object.assign(this, o.defaults, e), t(this.type === "value" || this.ancestor === void 0, "Non-value references cannot reference ancestors"), Array.isArray(this.map) && (this.map = new Map(this.map)), this.depth = this.path.length, this.key = this.path.length ? this.path.join(this.separator) : null, this.root = this.path[0], this.updateDisplay();
		}
		resolve(e, n, r, i, a = {}) {
			return t(!this.in || a.in, "Invalid in() reference usage"), this.type === "global" ? this._resolve(r.context, n, a) : this.type === "local" ? this._resolve(i, n, a) : this.ancestor ? this.ancestor === "root" ? this._resolve(n.ancestors[n.ancestors.length - 1], n, a) : (t(this.ancestor <= n.ancestors.length, "Invalid reference exceeds the schema root:", this.display), this._resolve(n.ancestors[this.ancestor - 1], n, a)) : this._resolve(e, n, a);
		}
		_resolve(e, t, n) {
			let i;
			if (this.type === "value" && t.mainstay.shadow && n.shadow !== !1 && (i = t.mainstay.shadow.get(this.absolute(t))), i === void 0 && (i = r(e, this.path, {
				iterables: this.iterables,
				functions: !0
			})), this.adjust && (i = this.adjust(i)), this.map) {
				let e = this.map.get(i);
				e !== void 0 && (i = e);
			}
			return t.mainstay && t.mainstay.tracer.resolve(t, this, i), i;
		}
		toString() {
			return this.display;
		}
		absolute(e) {
			return [...e.path.slice(0, -this.ancestor), ...this.path];
		}
		clone() {
			return new o.Ref(this);
		}
		describe() {
			let e = { path: this.path };
			this.type !== "value" && (e.type = this.type), this.separator !== "." && (e.separator = this.separator), this.type === "value" && this.ancestor !== 1 && (e.ancestor = this.ancestor), this.map && (e.map = [...this.map]);
			for (let t of [
				"adjust",
				"iterables",
				"render"
			]) this[t] !== null && this[t] !== void 0 && (e[t] = this[t]);
			return this.in !== !1 && (e.in = !0), { ref: e };
		}
		updateDisplay() {
			let e = this.key === null ? "" : this.key;
			if (this.type !== "value") {
				this.display = `ref:${this.type}:${e}`;
				return;
			}
			if (!this.separator) {
				this.display = `ref:${e}`;
				return;
			}
			if (!this.ancestor) {
				this.display = `ref:${this.separator}${e}`;
				return;
			}
			if (this.ancestor === "root") {
				this.display = `ref:root:${e}`;
				return;
			}
			if (this.ancestor === 1) {
				this.display = `ref:${e || ".."}`;
				return;
			}
			let t = Array(this.ancestor + 1).fill(this.separator).join("");
			this.display = `ref:${t}${e || ""}`;
		}
	}, o.Ref.prototype[i.symbols.ref] = !0, e.build = function(e) {
		return e = Object.assign({}, o.defaults, e), e.type === "value" && e.ancestor === void 0 && (e.ancestor = 1), new o.Ref(e);
	}, o.context = function(e, t, n = {}) {
		if (e = e.trim(), n) {
			let r = n.global === void 0 ? "$" : n.global;
			if (r !== t && e.startsWith(r)) return {
				key: e.slice(r.length),
				type: "global"
			};
			let i = n.local === void 0 ? "#" : n.local;
			if (i !== t && e.startsWith(i)) return {
				key: e.slice(i.length),
				type: "local"
			};
			let a = n.root === void 0 ? "/" : n.root;
			if (a !== t && e.startsWith(a)) return {
				key: e.slice(a.length),
				type: "value",
				root: !0
			};
		}
		return {
			key: e,
			type: "value"
		};
	}, o.ancestor = function(e, t) {
		if (!t || e[0] !== t) return [1, 0];
		if (e[1] !== t) return [0, 1];
		let n = 2;
		for (; e[n] === t;) ++n;
		return [n - 1, n];
	}, e.toSibling = 0, e.toParent = 1, e.Manager = class {
		constructor() {
			this.refs = [];
		}
		register(t, n) {
			if (t) {
				if (n = n === void 0 ? e.toParent : n, Array.isArray(t)) {
					for (let e of t) this.register(e, n);
					return;
				}
				if (i.isSchema(t)) {
					for (let e of t._refs.refs) e.ancestor - n >= 0 && this.refs.push({
						ancestor: e.ancestor - n,
						root: e.root
					});
					return;
				}
				e.isRef(t) && t.type === "value" && t.ancestor - n >= 0 && this.refs.push({
					ancestor: t.ancestor - n,
					root: t.root
				}), a ||= M(), a.isTemplate(t) && this.register(t.refs(), n);
			}
		}
		get length() {
			return this.refs.length;
		}
		clone() {
			let t = new e.Manager();
			return t.refs = n(this.refs), t;
		}
		reset() {
			this.refs = [];
		}
		roots() {
			return this.refs.filter((e) => !e.ancestor).map((e) => e.root);
		}
	};
})), M = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, clone: r, escapeHtml: i } = E(), a = ne(), o = P(), s = A(), c = j(), l = {
		symbol: Symbol("template"),
		opens: Array(1e3).join("\0"),
		closes: Array(1e3).join(""),
		dateFormat: {
			date: Date.prototype.toDateString,
			iso: Date.prototype.toISOString,
			string: Date.prototype.toString,
			time: Date.prototype.toTimeString,
			utc: Date.prototype.toUTCString
		}
	};
	t.exports = e = l.Template = class {
		constructor(e, t) {
			if (n(typeof e == "string", "Template source must be a string"), n(!e.includes("\0") && !e.includes(""), "Template source cannot contain reserved control characters"), this.source = e, this.rendered = e, this._template = null, t) {
				let { functions: e, ...i } = t;
				this._settings = Object.keys(i).length ? r(i) : void 0, this._functions = e, this._functions && (n(Object.keys(this._functions).every((e) => typeof e == "string"), "Functions keys must be strings"), n(Object.values(this._functions).every((e) => typeof e == "function"), "Functions values must be functions"));
			} else this._settings = void 0, this._functions = void 0;
			this._parse();
		}
		_parse() {
			if (!this.source.includes("{")) return;
			let e = l.encode(this.source), t = l.split(e), n = !1, r = [], i = t.shift();
			i && r.push(l.decode(i));
			for (let e of t) {
				let t = e[0] !== "{", i = t ? "}" : "}}", a = e.indexOf(i);
				if (a === -1 || e[1] === "{") {
					r.push(`{${l.decode(e)}`);
					continue;
				}
				let o = e.slice(+!t, a), s = o[0] === ":";
				s && (o = o.slice(1));
				let c = this._ref(l.decode(o), {
					raw: t,
					wrapped: s
				});
				r.push(c), typeof c != "string" && (n = !0);
				let u = e.slice(a + i.length);
				u && r.push(l.decode(u));
			}
			if (!n) {
				this.rendered = r.join("");
				return;
			}
			this._template = r;
		}
		static date(e, t) {
			return l.dateFormat[t.dateFormat].call(e);
		}
		describe(e = {}) {
			if (!this._settings && e.compact) return this.source;
			let t = { template: this.source };
			return this._settings && (t.options = this._settings), this._functions && (t.functions = this._functions), t;
		}
		static build(e) {
			return new l.Template(e.template, e.options || e.functions ? {
				...e.options,
				functions: e.functions
			} : void 0);
		}
		isDynamic() {
			return !!this._template;
		}
		static isTemplate(e) {
			return e ? !!e[o.symbols.template] : !1;
		}
		refs() {
			if (!this._template) return;
			let e = [];
			for (let t of this._template) typeof t != "string" && e.push(...t.refs);
			return e;
		}
		resolve(e, t, n, r) {
			return this._template && this._template.length === 1 ? this._part(this._template[0], e, t, n, r, {}) : this.render(e, t, n, r);
		}
		_part(e, ...t) {
			return e.ref ? e.ref.resolve(...t) : e.formula.evaluate(t);
		}
		render(e, t, n, r, a = {}) {
			if (!this.isDynamic()) return this.rendered;
			let o = [];
			for (let s of this._template) if (typeof s == "string") o.push(s);
			else {
				let c = this._part(s, e, t, n, r, a), u = l.stringify(c, e, t, n, r, a);
				if (u !== void 0) {
					let e = s.raw || (a.errors && a.errors.escapeHtml) === !1 ? u : i(u);
					o.push(l.wrap(e, s.wrapped && n.errors.wrap.label));
				}
			}
			return o.join("");
		}
		_ref(e, { raw: t, wrapped: n }) {
			let r = [], i = (e) => {
				let t = c.create(e, this._settings);
				return r.push(t), (e) => {
					let n = t.resolve(...e);
					return n === void 0 ? null : n;
				};
			};
			try {
				let t = this._functions ? {
					...l.functions,
					...this._functions
				} : l.functions;
				var o = new a.Parser(e, {
					reference: i,
					functions: t,
					constants: l.constants
				});
			} catch (t) {
				throw t.message = `Invalid template variable "${e}" fails due to: ${t.message}`, t;
			}
			if (o.single) {
				if (o.single.type === "reference") {
					let e = r[0];
					return {
						ref: e,
						raw: t,
						refs: r,
						wrapped: n || e.type === "local" && e.key === "label"
					};
				}
				return l.stringify(o.single.value);
			}
			return {
				formula: o,
				raw: t,
				refs: r
			};
		}
		toString() {
			return this.source;
		}
	}, l.Template.prototype[o.symbols.template] = !0, l.Template.prototype.isImmutable = !0, l.encode = function(e) {
		return e.replace(/\\(\{+)/g, (e, t) => l.opens.slice(0, t.length)).replace(/\\(\}+)/g, (e, t) => l.closes.slice(0, t.length));
	}, l.decode = function(e) {
		return e.replace(/\u0000/g, "{").replace(/\u0001/g, "}");
	}, l.split = function(e) {
		let t = [], n = "";
		for (let r = 0; r < e.length; ++r) {
			let i = e[r];
			if (i === "{") {
				let i = "";
				for (; r + 1 < e.length && e[r + 1] === "{";) i += "{", ++r;
				t.push(n), n = i;
			} else n += i;
		}
		return t.push(n), t;
	}, l.wrap = function(e, t) {
		return t ? t.length === 1 ? `${t}${e}${t}` : `${t[0]}${e}${t[1]}` : e;
	}, l.stringify = function(e, t, n, r, i, a = {}) {
		let o = typeof e, s = r && r.errors && r.errors.wrap || {}, u = !1;
		if (c.isRef(e) && e.render && (u = e.in, e = e.resolve(t, n, r, i, {
			in: e.in,
			...a
		})), e === null) return "null";
		if (o === "string") return l.wrap(e, a.arrayItems && s.string);
		if (o === "number" || o === "function" || o === "symbol") return e.toString();
		if (o !== "object") return JSON.stringify(e);
		if (e instanceof Date) return l.Template.date(e, r);
		if (e instanceof Map) {
			let t = [];
			for (let [n, r] of e.entries()) t.push(`${n.toString()} -> ${r.toString()}`);
			e = t;
		}
		if (!Array.isArray(e)) return e.toString();
		let d = [];
		for (let o of e) d.push(l.stringify(o, t, n, r, i, {
			arrayItems: !0,
			...a
		}));
		return l.wrap(d.join(", "), !u && s.array);
	}, l.constants = {
		true: !0,
		false: !1,
		null: null,
		second: 1e3,
		minute: 60 * 1e3,
		hour: 3600 * 1e3,
		day: 1440 * 60 * 1e3
	}, l.functions = {
		if(e, t, n) {
			return e ? t : n;
		},
		length(e) {
			return typeof e == "string" ? e.length : !e || typeof e != "object" ? null : Array.isArray(e) ? e.length : Object.keys(e).length;
		},
		msg(e) {
			let [t, n, r, i, a] = this, o = a.messages;
			if (!o) return "";
			let c = s.template(t, o[0], e, n, r) || s.template(t, o[1], e, n, r);
			return c ? c.render(t, n, r, i, a) : "";
		},
		number(e) {
			return typeof e == "number" ? e : typeof e == "string" ? parseFloat(e) : typeof e == "boolean" ? +!!e : e instanceof Date ? e.getTime() : null;
		}
	};
})), N = /* @__PURE__ */ s(((e) => {
	var { assert: t, clone: n } = E(), r = M();
	e.compile = function(e, i) {
		if (typeof e == "string") return t(!i, "Cannot set single message string"), new r(e);
		if (r.isTemplate(e)) return t(!i, "Cannot set single message template"), e;
		t(typeof e == "object" && !Array.isArray(e), "Invalid message options"), i = i ? n(i) : {};
		for (let n in e) {
			let a = e[n];
			if (n === "root" || r.isTemplate(a)) {
				i[n] = a;
				continue;
			}
			if (typeof a == "string") {
				i[n] = new r(a);
				continue;
			}
			t(typeof a == "object" && !Array.isArray(a), "Invalid message for", n);
			let o = n;
			for (n in i[o] = i[o] || {}, a) {
				let e = a[n];
				if (n === "root" || r.isTemplate(e)) {
					i[o][n] = e;
					continue;
				}
				t(typeof e == "string", "Invalid message for", n, "in", o), i[o][n] = new r(e);
			}
		}
		return i;
	}, e.decompile = function(e) {
		let t = {};
		for (let n in e) {
			let i = e[n];
			if (n === "root") {
				t.root = i;
				continue;
			}
			if (r.isTemplate(i)) {
				t[n] = i.describe({ compact: !0 });
				continue;
			}
			let a = n;
			for (n in t[a] = {}, i) {
				let e = i[n];
				if (n === "root") {
					t[a].root = e;
					continue;
				}
				t[a][n] = e.describe({ compact: !0 });
			}
		}
		return t;
	}, e.merge = function(i, a) {
		if (!i) return e.compile(a);
		if (!a) return i;
		if (typeof a == "string") return new r(a);
		if (r.isTemplate(a)) return a;
		let o = n(i);
		for (let e in a) {
			let n = a[e];
			if (e === "root" || r.isTemplate(n)) {
				o[e] = n;
				continue;
			}
			if (typeof n == "string") {
				o[e] = new r(n);
				continue;
			}
			t(typeof n == "object" && !Array.isArray(n), "Invalid message for", e);
			let i = e;
			for (e in o[i] = o[i] || {}, n) {
				let a = n[e];
				if (e === "root" || r.isTemplate(a)) {
					o[i][e] = a;
					continue;
				}
				t(typeof a == "string", "Invalid message for", e, "in", i), o[i][e] = new r(a);
			}
		}
		return o;
	};
})), P = /* @__PURE__ */ s(((e) => {
	var { assert: t, AssertError: n } = E(), r = (ee(), d(D)), i, a, o = { isoDate: /^(?:[-+]\d{2})?(?:\d{4}(?!\d{2}\b))(?:(-?)(?:(?:0[1-9]|1[0-2])(?:\1(?:[12]\d|0[1-9]|3[01]))?|W(?:[0-4]\d|5[0-2])(?:-?[1-7])?|(?:00[1-9]|0[1-9]\d|[12]\d{2}|3(?:[0-5]\d|6[1-6])))(?![T]$|[T][\d]+Z$)(?:[T\s](?:(?:(?:[01]\d|2[0-3])(?:(:?)[0-5]\d)?|24\:?00)(?:[.,]\d+(?!:))?)(?:\2[0-5]\d(?:[.,]\d+)?)?(?:[Z]|(?:[+-])(?:[01]\d|2[0-3])(?::?[0-5]\d)?)?)?)?$/ };
	e.version = r.version, e.defaults = {
		abortEarly: !0,
		allowUnknown: !1,
		artifacts: !1,
		cache: !0,
		context: null,
		convert: !0,
		dateFormat: "iso",
		errors: {
			escapeHtml: !1,
			label: "path",
			language: null,
			render: !0,
			stack: !1,
			wrap: {
				label: "\"",
				array: "[]"
			}
		},
		externals: !0,
		messages: {},
		nonEnumerables: !1,
		noDefaults: !1,
		presence: "optional",
		skipFunctions: !1,
		stripUnknown: !1,
		warnings: !1
	}, e.symbols = {
		any: Symbol.for("@hapi/joi/schema"),
		arraySingle: Symbol("arraySingle"),
		deepDefault: Symbol("deepDefault"),
		errors: Symbol("errors"),
		literal: Symbol("literal"),
		override: Symbol("override"),
		parent: Symbol("parent"),
		prefs: Symbol("prefs"),
		ref: Symbol("ref"),
		template: Symbol("template"),
		values: Symbol("values")
	}, e.assertOptions = function(e, n, r = "Options") {
		t(e && typeof e == "object" && !Array.isArray(e), "Options must be of type object");
		let i = Object.keys(e).filter((e) => !n.includes(e));
		t(i.length === 0, `${r} contain unknown keys: ${i}`);
	}, e.checkPreferences = function(e) {
		a ||= te();
		let t = a.preferences.validate(e);
		if (t.error) throw new n([t.error.details[0].message]);
	}, e.compare = function(e, t, n) {
		switch (n) {
			case "=": return e === t;
			case ">": return e > t;
			case "<": return e < t;
			case ">=": return e >= t;
			case "<=": return e <= t;
		}
	}, e.default = function(e, t) {
		return e === void 0 ? t : e;
	}, e.intersect = function(e, t) {
		if (typeof e.intersection == "function") return e.intersection(t);
		let n = /* @__PURE__ */ new Set();
		for (let r of e) t.has(r) && n.add(r);
		return n;
	}, e.isIsoDate = function(e) {
		return o.isoDate.test(e);
	}, e.isNumber = function(e) {
		return typeof e == "number" && !isNaN(e);
	}, e.isResolvable = function(t) {
		return t ? t[e.symbols.ref] || t[e.symbols.template] : !1;
	}, e.isSchema = function(n, r = {}) {
		let i = n && n[e.symbols.any];
		return i ? (t(r.legacy || i.version === e.version, "Cannot mix different versions of joi schemas"), !0) : !1;
	}, e.isValues = function(t) {
		return t[e.symbols.values];
	}, e.limit = function(e) {
		return Number.isSafeInteger(e) && e >= 0;
	}, e.preferences = function(t, n) {
		i ||= N(), t ||= {}, n ||= {};
		let r = Object.assign({}, t, n);
		return n.errors && t.errors && (r.errors = Object.assign({}, t.errors, n.errors), r.errors.wrap = Object.assign({}, t.errors.wrap, n.errors.wrap)), n.messages && (r.messages = i.compile(n.messages, t.messages)), delete r[e.symbols.prefs], r;
	}, e.tryWithPath = function(e, t, n = {}) {
		try {
			return e();
		} catch (e) {
			throw e.path === void 0 ? e.path = t : e.path = t + "." + e.path, n.append && (e.message = `${e.message} (${e.path})`), e;
		}
	}, e.validateArg = function(t, n, { assert: r, message: i }) {
		if (e.isSchema(r)) {
			let e = r.validate(t);
			return e.error ? e.error.message : void 0;
		} else if (!r(t)) return n ? `${n} ${i}` : i;
	}, e.verifyFlat = function(e, n) {
		for (let r of e) t(!Array.isArray(r), "Method no longer accepts array arguments:", n);
	};
})), F = /* @__PURE__ */ s(((e) => {
	var { assert: t, clone: n } = E(), r = P(), i = {
		max: 1e3,
		supported: new Set([
			"undefined",
			"boolean",
			"number",
			"string"
		])
	};
	e.provider = { provision(e) {
		return new i.Cache(e);
	} }, i.Cache = class {
		constructor(e = {}) {
			r.assertOptions(e, ["max"]), t(e.max === void 0 || e.max && e.max > 0 && isFinite(e.max), "Invalid max cache size"), this._max = e.max || i.max, this._map = /* @__PURE__ */ new Map(), this._list = new i.List();
		}
		get length() {
			return this._map.size;
		}
		set(e, t) {
			if (e !== null && !i.supported.has(typeof e)) return;
			let n = this._map.get(e);
			if (n) {
				n.value = t, this._list.first(n);
				return;
			}
			n = this._list.unshift({
				key: e,
				value: t
			}), this._map.set(e, n), this._compact();
		}
		get(e) {
			let t = this._map.get(e);
			if (t) return this._list.first(t), n(t.value);
		}
		_compact() {
			if (this._map.size > this._max) {
				let e = this._list.pop();
				this._map.delete(e.key);
			}
		}
	}, i.List = class {
		constructor() {
			this.tail = null, this.head = null;
		}
		unshift(e) {
			return e.next = null, e.prev = this.head, this.head && (this.head.next = e), this.head = e, this.tail ||= e, e;
		}
		first(e) {
			e !== this.head && (this._remove(e), this.unshift(e));
		}
		pop() {
			return this._remove(this.tail);
		}
		_remove(e) {
			let { next: t, prev: n } = e;
			return t.prev = n, n && (n.next = t), e === this.tail && (this.tail = t), e.prev = null, e.next = null, e;
		}
	};
})), I = /* @__PURE__ */ s(((e) => {
	var { assert: t } = E(), n = P(), r = j(), i = {};
	e.schema = function(e, t, r = {}) {
		n.assertOptions(r, ["appendPath", "override"]);
		try {
			return i.schema(e, t, r);
		} catch (e) {
			throw r.appendPath && e.path !== void 0 && (e.message = `${e.message} (${e.path})`), e;
		}
	}, i.schema = function(e, r, a) {
		t(r !== void 0, "Invalid undefined schema"), Array.isArray(r) && (t(r.length, "Invalid empty array schema"), r.length === 1 && (r = r[0]));
		let o = (t, ...n) => a.override === !1 ? t.valid(...n) : t.valid(e.override, ...n);
		if (i.simple(r)) return o(e, r);
		if (typeof r == "function") return e.custom(r);
		if (t(typeof r == "object", "Invalid schema content:", typeof r), n.isResolvable(r)) return o(e, r);
		if (n.isSchema(r)) return r;
		if (Array.isArray(r)) {
			for (let t of r) if (!i.simple(t)) return e.alternatives().try(...r);
			return o(e, ...r);
		}
		return r instanceof RegExp ? e.string().regex(r) : r instanceof Date ? o(e.date(), r) : (t(Object.getPrototypeOf(r) === Object.getPrototypeOf({}), "Schema can only contain plain objects"), e.object().keys(r));
	}, e.ref = function(e, t) {
		return r.isRef(e) ? e : r.create(e, t);
	}, e.compile = function(r, a, o = {}) {
		n.assertOptions(o, ["legacy"]);
		let s = a && a[n.symbols.any];
		if (s) return t(o.legacy || s.version === n.version, "Cannot mix different versions of joi schemas:", s.version, n.version), a;
		if (typeof a != "object" || !o.legacy) return e.schema(r, a, { appendPath: !0 });
		let c = i.walk(a);
		return c ? c.compile(c.root, a) : e.schema(r, a, { appendPath: !0 });
	}, i.walk = function(e) {
		if (typeof e != "object") return null;
		if (Array.isArray(e)) {
			for (let t of e) {
				let e = i.walk(t);
				if (e) return e;
			}
			return null;
		}
		let r = e[n.symbols.any];
		if (r) return {
			root: e[r.root],
			compile: r.compile
		};
		t(Object.getPrototypeOf(e) === Object.getPrototypeOf({}), "Schema can only contain plain objects");
		for (let t in e) {
			let n = i.walk(e[t]);
			if (n) return n;
		}
		return null;
	}, i.simple = function(e) {
		return e === null || [
			"boolean",
			"string",
			"number"
		].includes(typeof e);
	}, e.when = function(a, o, s) {
		if (s === void 0 && (t(o && typeof o == "object", "Missing options"), s = o, o = r.create(".")), Array.isArray(s) && (s = { switch: s }), n.assertOptions(s, [
			"is",
			"not",
			"then",
			"otherwise",
			"switch",
			"break"
		]), n.isSchema(o)) return t(s.is === void 0, "\"is\" can not be used with a schema condition"), t(s.not === void 0, "\"not\" can not be used with a schema condition"), t(s.switch === void 0, "\"switch\" can not be used with a schema condition"), i.condition(a, {
			is: o,
			then: s.then,
			otherwise: s.otherwise,
			break: s.break
		});
		if (t(r.isRef(o) || typeof o == "string", "Invalid condition:", o), t(s.not === void 0 || s.is === void 0, "Cannot combine \"is\" with \"not\""), s.switch === void 0) {
			let c = s;
			s.not !== void 0 && (c = {
				is: s.not,
				then: s.otherwise,
				otherwise: s.then,
				break: s.break
			});
			let l = c.is === void 0 ? a.$_root.invalid(null, !1, 0, "").required() : a.$_compile(c.is);
			return t(c.then !== void 0 || c.otherwise !== void 0, "options must have at least one of \"then\", \"otherwise\", or \"switch\""), t(c.break === void 0 || c.then === void 0 || c.otherwise === void 0, "Cannot specify then, otherwise, and break all together"), s.is !== void 0 && !r.isRef(s.is) && !n.isSchema(s.is) && (l = l.required()), i.condition(a, {
				ref: e.ref(o),
				is: l,
				then: c.then,
				otherwise: c.otherwise,
				break: c.break
			});
		}
		t(Array.isArray(s.switch), "\"switch\" must be an array"), t(s.is === void 0, "Cannot combine \"switch\" with \"is\""), t(s.not === void 0, "Cannot combine \"switch\" with \"not\""), t(s.then === void 0, "Cannot combine \"switch\" with \"then\"");
		let c = {
			ref: e.ref(o),
			switch: [],
			break: s.break
		};
		for (let e = 0; e < s.switch.length; ++e) {
			let i = s.switch[e], o = e === s.switch.length - 1;
			n.assertOptions(i, o ? [
				"is",
				"then",
				"otherwise"
			] : ["is", "then"]), t(i.is !== void 0, "Switch statement missing \"is\""), t(i.then !== void 0, "Switch statement missing \"then\"");
			let l = {
				is: a.$_compile(i.is),
				then: a.$_compile(i.then)
			};
			if (!r.isRef(i.is) && !n.isSchema(i.is) && (l.is = l.is.required()), o) {
				t(s.otherwise === void 0 || i.otherwise === void 0, "Cannot specify \"otherwise\" inside and outside a \"switch\"");
				let e = s.otherwise === void 0 ? i.otherwise : s.otherwise;
				e !== void 0 && (t(c.break === void 0, "Cannot specify both otherwise and break"), l.otherwise = a.$_compile(e));
			}
			c.switch.push(l);
		}
		return c;
	}, i.condition = function(e, t) {
		for (let n of ["then", "otherwise"]) t[n] === void 0 ? delete t[n] : t[n] = e.$_compile(t[n]);
		return t;
	};
})), L = /* @__PURE__ */ s(((e) => {
	var { assert: t, clone: n } = E(), r = P(), i = N(), a = {};
	e.type = function(e, o) {
		let s = Object.getPrototypeOf(e), c = n(s), l = e._assign(Object.create(c)), u = Object.assign({}, o);
		delete u.base, c._definition = u;
		let d = s._definition || {};
		u.messages = i.merge(d.messages, u.messages), u.properties = Object.assign({}, d.properties, u.properties), l.type = u.type, u.flags = Object.assign({}, d.flags, u.flags);
		let f = Object.assign({}, d.terms);
		if (u.terms) for (let e in u.terms) {
			let n = u.terms[e];
			t(l.$_terms[e] === void 0, "Invalid term override for", u.type, e), l.$_terms[e] = n.init, f[e] = n;
		}
		u.terms = f, u.args ||= d.args, u.prepare = a.prepare(u.prepare, d.prepare), u.coerce && (typeof u.coerce == "function" && (u.coerce = { method: u.coerce }), u.coerce.from && !Array.isArray(u.coerce.from) && (u.coerce = {
			method: u.coerce.method,
			from: [].concat(u.coerce.from)
		})), u.coerce = a.coerce(u.coerce, d.coerce), u.validate = a.validate(u.validate, d.validate);
		let p = Object.assign({}, d.rules);
		if (u.rules) for (let e in u.rules) {
			let n = u.rules[e];
			t(typeof n == "object", "Invalid rule definition for", u.type, e);
			let i = n.method;
			if (i === void 0 && (i = function() {
				return this.$_addRule(e);
			}), i && (t(!c[e], "Rule conflict in", u.type, e), c[e] = i), t(!p[e], "Rule conflict in", u.type, e), p[e] = n, n.alias) {
				let e = [].concat(n.alias);
				for (let t of e) c[t] = n.method;
			}
			n.args &&= (n.argsByName = /* @__PURE__ */ new Map(), n.args.map((e) => (typeof e == "string" && (e = { name: e }), t(!n.argsByName.has(e.name), "Duplicated argument name", e.name), r.isSchema(e.assert) && (e.assert = e.assert.strict().label(e.name)), n.argsByName.set(e.name, e), e)));
		}
		u.rules = p, u.jsonSchema ||= d.jsonSchema;
		let m = Object.assign({}, d.modifiers);
		if (u.modifiers) for (let e in u.modifiers) {
			t(!c[e], "Rule conflict in", u.type, e);
			let n = u.modifiers[e];
			t(typeof n == "function", "Invalid modifier definition for", u.type, e), c[e] = function(t) {
				return this.rule({ [e]: t });
			}, m[e] = n;
		}
		if (u.modifiers = m, u.overrides) {
			c._super = s, l.$_super = {};
			for (let e in u.overrides) t(s[e], "Cannot override missing", e), u.overrides[e][r.symbols.parent] = s[e], l.$_super[e] = s[e].bind(l);
			Object.assign(c, u.overrides);
		}
		u.cast = Object.assign({}, d.cast, u.cast);
		let h = Object.assign({}, d.manifest, u.manifest);
		return h.build = a.build(u.manifest && u.manifest.build, d.manifest && d.manifest.build), u.manifest = h, u.rebuild = a.rebuild(u.rebuild, d.rebuild), l;
	}, a.build = function(e, t) {
		return !e || !t ? e || t : function(n, r) {
			return t(e(n, r), r);
		};
	}, a.coerce = function(e, t) {
		return !e || !t ? e || t : {
			from: e.from && t.from ? [...new Set([...e.from, ...t.from])] : null,
			method(n, r) {
				let i;
				if ((!t.from || t.from.includes(typeof n)) && (i = t.method(n, r), i)) {
					if (i.errors || i.value === void 0) return i;
					n = i.value;
				}
				if (!e.from || e.from.includes(typeof n)) {
					let t = e.method(n, r);
					if (t) return t;
				}
				return i;
			}
		};
	}, a.prepare = function(e, t) {
		return !e || !t ? e || t : function(n, r) {
			let i = e(n, r);
			if (i) {
				if (i.errors || i.value === void 0) return i;
				n = i.value;
			}
			return t(n, r) || i;
		};
	}, a.rebuild = function(e, t) {
		return !e || !t ? e || t : function(n) {
			t(n), e(n);
		};
	}, a.validate = function(e, t) {
		return !e || !t ? e || t : function(n, r) {
			let i = t(n, r);
			if (i) {
				if (i.errors && (!Array.isArray(i.errors) || i.errors.length)) return i;
				n = i.value;
			}
			return e(n, r) || i;
		};
	};
})), re = /* @__PURE__ */ s(((e) => {
	var { assert: t } = E(), n = P(), r = j(), i = {};
	e.Ids = i.Ids = class {
		constructor() {
			this._byId = /* @__PURE__ */ new Map(), this._byKey = /* @__PURE__ */ new Map(), this._schemaChain = !1;
		}
		clone() {
			let e = new i.Ids();
			return e._byId = new Map(this._byId), e._byKey = new Map(this._byKey), e._schemaChain = this._schemaChain, e;
		}
		concat(e) {
			e._schemaChain && (this._schemaChain = !0);
			for (let [n, r] of e._byId.entries()) t(!this._byKey.has(n), "Schema id conflicts with existing key:", n), this._byId.set(n, r);
			for (let [n, r] of e._byKey.entries()) t(!this._byId.has(n), "Schema key conflicts with existing id:", n), this._byKey.set(n, r);
		}
		fork(e, r, a) {
			let o = this._collect(e);
			o.push({ schema: a });
			let s = o.shift(), c = {
				id: s.id,
				schema: r(s.schema)
			};
			t(n.isSchema(c.schema), "adjuster function failed to return a joi schema type");
			for (let e of o) c = {
				id: e.id,
				schema: i.fork(e.schema, c.id, c.schema)
			};
			return c.schema;
		}
		labels(e, t = []) {
			let n = e[0], r = this._get(n);
			if (!r) return [...t, ...e].join(".");
			let i = e.slice(1);
			return t = [...t, r.schema._flags.label || n], i.length ? r.schema._ids.labels(i, t) : t.join(".");
		}
		reach(e, n = []) {
			let r = e[0], i = this._get(r);
			t(i, "Schema does not contain path", [...n, ...e].join("."));
			let a = e.slice(1);
			return a.length ? i.schema._ids.reach(a, [...n, r]) : i.schema;
		}
		register(e, { key: r } = {}) {
			if (!e || !n.isSchema(e)) return;
			(e.$_property("schemaChain") || e._ids._schemaChain) && (this._schemaChain = !0);
			let i = e._flags.id;
			if (i) {
				let n = this._byId.get(i);
				t(!n || n.schema === e, "Cannot add different schemas with the same id:", i), t(!this._byKey.has(i), "Schema id conflicts with existing key:", i), this._byId.set(i, {
					schema: e,
					id: i
				});
			}
			r && (t(!this._byKey.has(r), "Schema already contains key:", r), t(!this._byId.has(r), "Schema key conflicts with existing id:", r), this._byKey.set(r, {
				schema: e,
				id: r
			}));
		}
		reset() {
			this._byId = /* @__PURE__ */ new Map(), this._byKey = /* @__PURE__ */ new Map(), this._schemaChain = !1;
		}
		_collect(e, n = [], r = []) {
			let i = e[0], a = this._get(i);
			t(a, "Schema does not contain path", [...n, ...e].join(".")), r = [a, ...r];
			let o = e.slice(1);
			return o.length ? a.schema._ids._collect(o, [...n, i], r) : r;
		}
		_get(e) {
			return this._byId.get(e) || this._byKey.get(e);
		}
	}, i.fork = function(t, n, r) {
		let i = e.schema(t, {
			each: (e, { key: t }) => {
				if (n === (e._flags.id || t)) return r;
			},
			ref: !1
		});
		return i ? i.$_mutateRebuild() : t;
	}, e.schema = function(e, t) {
		let n;
		for (let r in e._flags) {
			if (r[0] === "_") continue;
			let a = i.scan(e._flags[r], {
				source: "flags",
				name: r
			}, t);
			a !== void 0 && (n ||= e.clone(), n._flags[r] = a);
		}
		for (let r = 0; r < e._rules.length; ++r) {
			let a = e._rules[r], o = i.scan(a.args, {
				source: "rules",
				name: a.name
			}, t);
			if (o !== void 0) {
				n ||= e.clone();
				let t = Object.assign({}, a);
				t.args = o, n._rules[r] = t, n._singleRules.get(a.name) === a && n._singleRules.set(a.name, t);
			}
		}
		for (let r in e.$_terms) {
			if (r[0] === "_") continue;
			let a = i.scan(e.$_terms[r], {
				source: "terms",
				name: r
			}, t);
			a !== void 0 && (n ||= e.clone(), n.$_terms[r] = a);
		}
		return n;
	}, i.scan = function(e, t, a, o, s) {
		let c = o || [];
		if (typeof e != "object" || !e) return;
		let l;
		if (Array.isArray(e)) {
			for (let n = 0; n < e.length; ++n) {
				let r = t.source === "terms" && t.name === "keys" && e[n].key, o = i.scan(e[n], t, a, [n, ...c], r);
				o !== void 0 && (l ||= e.slice(), l[n] = o);
			}
			return l;
		}
		if (a.schema !== !1 && n.isSchema(e) || a.ref !== !1 && r.isRef(e)) {
			let n = a.each(e, {
				...t,
				path: c,
				key: s
			});
			return n === e ? void 0 : n;
		}
		for (let n in e) {
			if (n[0] === "_") continue;
			let r = i.scan(e[n], t, a, [n, ...c], s);
			r !== void 0 && (l ||= Object.assign({}, e), l[n] = r);
		}
		return l;
	};
})), ie = /* @__PURE__ */ s(((e, t) => {
	var { clone: n, reach: r } = E(), i = P(), a = { value: Symbol("value") };
	t.exports = a.State = class {
		constructor(e, t, n) {
			this.path = e, this.ancestors = t, this.mainstay = n.mainstay, this.schemas = n.schemas, this.debug = null;
		}
		localize(e, t = null, n = null) {
			let r = new a.State(e, t, this);
			return n && r.schemas && (r.schemas = [a.schemas(n), ...r.schemas]), r;
		}
		nest(e, t) {
			let n = new a.State(this.path, this.ancestors, this);
			return n.schemas = n.schemas && [a.schemas(e), ...n.schemas], n.debug = t, n;
		}
		shadow(e, t) {
			this.mainstay.shadow = this.mainstay.shadow || new a.Shadow(), this.mainstay.shadow.set(this.path, e, t);
		}
		snapshot() {
			this.mainstay.shadow && (this._snapshot = n(this.mainstay.shadow.node(this.path))), this.mainstay.snapshot();
		}
		restore() {
			this.mainstay.shadow && (this.mainstay.shadow.override(this.path, this._snapshot), this._snapshot = void 0), this.mainstay.restore();
		}
		commit() {
			this.mainstay.shadow && (this.mainstay.shadow.override(this.path, this._snapshot), this._snapshot = void 0), this.mainstay.commit();
		}
	}, a.schemas = function(e) {
		return i.isSchema(e) ? { schema: e } : e;
	}, a.Shadow = class {
		constructor() {
			this._values = null;
		}
		set(e, t, n) {
			if (!e.length || n === "strip" && typeof e[e.length - 1] == "number") return;
			this._values = this._values || /* @__PURE__ */ new Map();
			let r = this._values;
			for (let t = 0; t < e.length; ++t) {
				let n = e[t], i = r.get(n);
				i || (i = /* @__PURE__ */ new Map(), r.set(n, i)), r = i;
			}
			r[a.value] = t;
		}
		get(e) {
			let t = this.node(e);
			if (t) return t[a.value];
		}
		node(e) {
			if (this._values) return r(this._values, e, { iterables: !0 });
		}
		override(e, t) {
			if (!this._values) return;
			let n = e.slice(0, -1), i = e[e.length - 1], a = r(this._values, n, { iterables: !0 });
			if (t) {
				a.set(i, t);
				return;
			}
			a && a.delete(i);
		}
	};
})), ae = /* @__PURE__ */ s(((e) => {
	var { assert: t, clone: n, ignore: r, reach: i } = E(), a = P(), o = A(), s = ie(), c = { result: Symbol("result") };
	e.entry = function(e, n, r) {
		let i = a.defaults;
		r && (t(r.warnings === void 0, "Cannot override warnings preference in synchronous validation"), t(r.artifacts === void 0, "Cannot override artifacts preference in synchronous validation"), i = a.preferences(a.defaults, r));
		let s = c.entry(e, n, i);
		t(!s.mainstay.externals.length, "Schema with external rules must use validateAsync()");
		let l = { value: s.value };
		return s.error && (l.error = s.error), s.mainstay.warnings.length && (l.warning = o.details(s.mainstay.warnings)), s.mainstay.debug && (l.debug = s.mainstay.debug), s.mainstay.artifacts && (l.artifacts = s.mainstay.artifacts), l;
	}, e.entryAsync = async function(e, t, n) {
		let r = a.defaults;
		n && (r = a.preferences(a.defaults, n));
		let s = c.entry(e, t, r), l = s.mainstay;
		if (s.error) throw l.debug && (s.error.debug = l.debug), s.error;
		if (l.externals.length) {
			let t = s.value, u = [];
			for (let s of l.externals) {
				let d = s.state.path, f = s.schema.type === "link" ? l.links.get(s.schema) : null, p = t, m, h, g = d.length ? [t] : [], _ = d.length ? i(e, d) : e;
				if (d.length) {
					m = d[d.length - 1];
					let e = t;
					for (let t of d.slice(0, -1)) e = e[t], g.unshift(e);
					h = g[0], p = h[m];
				}
				try {
					let e = await s.method(p, {
						schema: s.schema,
						linked: f,
						state: s.state,
						prefs: n,
						original: _,
						error: (e, t) => (f || s.schema).$_createError(e, p, t, s.state, r),
						errorsArray: c.errorsArray,
						warn: (e, t) => l.warnings.push((f || s.schema).$_createError(e, p, t, s.state, r)),
						message: (e, t) => (f || s.schema).$_createError("external", p, t, s.state, r, { messages: e })
					});
					if (e === void 0 || e === p) continue;
					if (e instanceof o.Report) {
						if (l.tracer.log(s.schema, s.state, "rule", "external", "error"), u.push(e), r.abortEarly) break;
						continue;
					}
					if (Array.isArray(e) && e[a.symbols.errors]) {
						if (l.tracer.log(s.schema, s.state, "rule", "external", "error"), u.push(...e), r.abortEarly) break;
						continue;
					}
					h ? (l.tracer.value(s.state, "rule", p, e, "external"), h[m] = e) : (l.tracer.value(s.state, "rule", t, e, "external"), t = e);
				} catch (e) {
					throw r.errors.label && (e.message += ` (${s.label})`), e;
				}
			}
			if (s.value = t, u.length) throw s.error = o.process(u, e, r), l.debug && (s.error.debug = l.debug), s.error;
		}
		if (!r.warnings && !r.debug && !r.artifacts) return s.value;
		let u = { value: s.value };
		return l.warnings.length && (u.warning = o.details(l.warnings)), l.debug && (u.debug = l.debug), l.artifacts && (u.artifacts = l.artifacts), u;
	}, e.standard = function(t, n, r) {
		let i = r?.libraryOptions;
		return n.isAsync() ? e.entryAsync(t, n, i) : e.entry(t, n, i);
	}, c.Mainstay = class {
		constructor(e, t, n) {
			this.externals = [], this.warnings = [], this.tracer = e, this.debug = t, this.links = n, this.shadow = null, this.artifacts = null, this._snapshots = [];
		}
		snapshot() {
			this._snapshots.push({
				externals: this.externals.slice(),
				warnings: this.warnings.slice()
			});
		}
		restore() {
			let e = this._snapshots.pop();
			this.externals = e.externals, this.warnings = e.warnings;
		}
		commit() {
			this._snapshots.pop();
		}
	}, c.entry = function(t, n, r) {
		let { tracer: i, cleanup: a } = c.tracer(n, r), l = r.debug ? [] : null, u = n._ids._schemaChain ? /* @__PURE__ */ new Map() : null, d = new c.Mainstay(i, l, u), f = new s([], [], {
			mainstay: d,
			schemas: n._ids._schemaChain ? [{ schema: n }] : null
		}), p = e.validate(t, n, f, r);
		a && n.$_root.untrace();
		let m = o.process(p.errors, t, r);
		return {
			value: p.value,
			error: m,
			mainstay: d
		};
	}, c.tracer = function(e, n) {
		return e.$_root._tracer ? { tracer: e.$_root._tracer._register(e) } : n.debug ? (t(e.$_root.trace, "Debug mode not supported"), {
			tracer: e.$_root.trace()._register(e),
			cleanup: !0
		}) : { tracer: c.ignore };
	}, e.validate = function(e, t, n, r, i = {}) {
		if (t.$_terms.whens && (t = t._generate(e, n, r).schema), t._preferences && (r = c.prefs(t, r)), t._cache && r.cache) {
			let r = t._cache.get(e);
			if (n.mainstay.tracer.debug(n, "validate", "cached", !!r), r) return r;
		}
		let o = (i, a, o) => t.$_createError(i, e, a, o || n, r), s = {
			original: e,
			prefs: r,
			schema: t,
			state: n,
			error: o,
			errorsArray: c.errorsArray,
			warn: (e, t, r) => n.mainstay.warnings.push(o(e, t, r)),
			message: (i, a) => t.$_createError("custom", e, a, n, r, { messages: i })
		};
		n.mainstay.tracer.entry(t, n);
		let l = t._definition;
		if (l.prepare && e !== void 0 && r.convert) {
			let t = l.prepare(e, s);
			if (t) {
				if (n.mainstay.tracer.value(n, "prepare", e, t.value), t.errors) return c.finalize(t.value, [].concat(t.errors), s);
				e = t.value;
			}
		}
		if (l.coerce && e !== void 0 && r.convert && (!l.coerce.from || l.coerce.from.includes(typeof e))) {
			let t = l.coerce.method(e, s);
			if (t) {
				if (n.mainstay.tracer.value(n, "coerced", e, t.value), t.errors) return c.finalize(t.value, [].concat(t.errors), s);
				e = t.value;
			}
		}
		let u = t._flags.empty;
		u && u.$_match(c.trim(e, t), n.nest(u), a.defaults) && (n.mainstay.tracer.value(n, "empty", e, void 0), e = void 0);
		let d = i.presence || t._flags.presence || (t._flags._endedSwitch ? null : r.presence);
		if (e === void 0) {
			if (d === "forbidden") return c.finalize(e, null, s);
			if (d === "required") return c.finalize(e, [t.$_createError("any.required", e, null, n, r)], s);
			if (d === "optional") {
				if (t._flags.default !== a.symbols.deepDefault) return c.finalize(e, null, s);
				n.mainstay.tracer.value(n, "default", e, {}), e = {};
			}
		} else if (d === "forbidden") return c.finalize(e, [t.$_createError("any.unknown", e, null, n, r)], s);
		let f = [];
		if (t._valids) {
			let i = t._valids.get(e, n, r, t._flags.insensitive);
			if (i) return r.convert && (n.mainstay.tracer.value(n, "valids", e, i.value), e = i.value), n.mainstay.tracer.filter(t, n, "valid", i), c.finalize(e, null, s);
			if (t._flags.only) {
				let i = t.$_createError("any.only", e, { valids: t._valids.values({ display: !0 }) }, n, r);
				if (r.abortEarly) return c.finalize(e, [i], s);
				f.push(i);
			}
		}
		if (t._invalids) {
			let i = t._invalids.get(e, n, r, t._flags.insensitive);
			if (i) {
				n.mainstay.tracer.filter(t, n, "invalid", i);
				let a = t.$_createError("any.invalid", e, { invalids: t._invalids.values({ display: !0 }) }, n, r);
				if (r.abortEarly) return c.finalize(e, [a], s);
				f.push(a);
			}
		}
		if (l.validate) {
			let t = l.validate(e, s);
			if (t && (n.mainstay.tracer.value(n, "base", e, t.value), e = t.value, t.errors)) {
				if (!Array.isArray(t.errors)) return f.push(t.errors), c.finalize(e, f, s);
				if (t.errors.length) return f.push(...t.errors), c.finalize(e, f, s);
			}
		}
		return t._rules.length ? c.rules(e, f, s) : c.finalize(e, f, s);
	}, c.rules = function(e, t, n) {
		let { schema: r, state: i, prefs: o } = n;
		for (let s of r._rules) {
			let l = r._definition.rules[s.method];
			if (l.convert && o.convert) {
				i.mainstay.tracer.log(r, i, "rule", s.name, "full");
				continue;
			}
			let u, d = s.args;
			if (s._resolve.length) {
				d = Object.assign({}, d);
				for (let t of s._resolve) {
					let n = l.argsByName.get(t), s = d[t].resolve(e, i, o), c = n.normalize ? n.normalize(s) : s, f = a.validateArg(c, null, n);
					if (f) {
						u = r.$_createError("any.ref", s, {
							arg: t,
							ref: d[t],
							reason: f
						}, i, o);
						break;
					}
					d[t] = c;
				}
			}
			u ||= l.validate(e, n, d, s);
			let f = c.rule(u, s);
			if (f.errors) {
				if (i.mainstay.tracer.log(r, i, "rule", s.name, "error"), s.warn) {
					i.mainstay.warnings.push(...f.errors);
					continue;
				}
				if (o.abortEarly) return c.finalize(e, f.errors, n);
				t.push(...f.errors);
			} else i.mainstay.tracer.log(r, i, "rule", s.name, "pass"), i.mainstay.tracer.value(i, "rule", e, f.value, s.name), e = f.value;
		}
		return c.finalize(e, t, n);
	}, c.rule = function(e, t) {
		return e instanceof o.Report ? (c.error(e, t), {
			errors: [e],
			value: null
		}) : Array.isArray(e) && e[a.symbols.errors] ? (e.forEach((e) => c.error(e, t)), {
			errors: e,
			value: null
		}) : {
			errors: null,
			value: e
		};
	}, c.error = function(e, t) {
		return t.message && e._setTemplate(t.message), e;
	}, c.finalize = function(e, n, r) {
		n ||= [];
		let { schema: i, state: a, prefs: s } = r;
		if (n.length) {
			let t = c.default("failover", void 0, n, r);
			t !== void 0 && (a.mainstay.tracer.value(a, "failover", e, t), e = t, n = []);
		}
		if (n.length && i._flags.error) if (typeof i._flags.error == "function") {
			n = i._flags.error(n), Array.isArray(n) || (n = [n]);
			for (let e of n) t(e instanceof Error || e instanceof o.Report, "error() must return an Error object");
		} else n = [i._flags.error];
		if (e === void 0) {
			let t = c.default("default", e, n, r);
			a.mainstay.tracer.value(a, "default", e, t), e = t;
		}
		if (i._flags.cast && e !== void 0) {
			let t = i._definition.cast[i._flags.cast];
			if (t.from(e)) {
				let n = t.to(e, r);
				a.mainstay.tracer.value(a, "cast", e, n, i._flags.cast), e = n;
			}
		}
		if (i.$_terms.externals && s.externals && s._externals !== !1) for (let { method: e } of i.$_terms.externals) a.mainstay.externals.push({
			method: e,
			schema: i,
			state: a,
			label: o.label(i._flags, a, s)
		});
		let l = {
			value: e,
			errors: n.length ? n : null
		};
		return i._flags.result && (l.value = i._flags.result === "strip" ? void 0 : r.original, a.mainstay.tracer.value(a, i._flags.result, e, l.value), a.shadow(e, i._flags.result)), i._cache && s.cache !== !1 && !i._refs.length && i._cache.set(r.original, l), e !== void 0 && !l.errors && i._flags.artifact !== void 0 && (a.mainstay.artifacts = a.mainstay.artifacts || /* @__PURE__ */ new Map(), a.mainstay.artifacts.has(i._flags.artifact) || a.mainstay.artifacts.set(i._flags.artifact, []), a.mainstay.artifacts.get(i._flags.artifact).push(a.path)), l;
	}, c.prefs = function(e, t) {
		let n = t === a.defaults;
		return n && e._preferences[a.symbols.prefs] ? e._preferences[a.symbols.prefs] : (t = a.preferences(t, e._preferences), n && (e._preferences[a.symbols.prefs] = t), t);
	}, c.default = function(e, t, r, i) {
		let { schema: o, state: s, prefs: c } = i, l = o._flags[e];
		if (c.noDefaults || l === void 0) return t;
		if (s.mainstay.tracer.log(o, s, "rule", e, "full"), !l) return l;
		if (typeof l == "function") {
			let t = l.length ? [n(s.ancestors[0]), i] : [];
			try {
				return l(...t);
			} catch (t) {
				r.push(o.$_createError(`any.${e}`, null, { error: t }, s, c));
				return;
			}
		}
		return typeof l == "object" ? l[a.symbols.literal] ? l.literal : a.isResolvable(l) ? l.resolve(t, s, c) : n(l) : l;
	}, c.trim = function(e, t) {
		if (typeof e != "string") return e;
		let n = t.$_getRule("trim");
		return !n || !n.args.enabled ? e : e.trim();
	}, c.ignore = {
		active: !1,
		debug: r,
		entry: r,
		filter: r,
		log: r,
		resolve: r,
		value: r
	}, c.errorsArray = function() {
		let e = [];
		return e[a.symbols.errors] = !0, e;
	};
})), oe = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, deepEqual: r } = E(), i = P(), a = {};
	t.exports = a.Values = class {
		constructor(e, t) {
			this._values = new Set(e), this._refs = new Set(t), this._lowercase = a.lowercases(e), this._override = !1;
		}
		get length() {
			return this._values.size + this._refs.size;
		}
		add(e, t) {
			if (i.isResolvable(e)) {
				this._refs.has(e) || (this._refs.add(e), t && t.register(e));
				return;
			}
			this.has(e, null, null, !1) || (this._values.add(e), typeof e == "string" && this._lowercase.set(e.toLowerCase(), e));
		}
		static merge(e, t, n) {
			if (e ||= new a.Values(), t) {
				if (t._override) return t.clone();
				for (let n of [...t._values, ...t._refs]) e.add(n);
			}
			if (n) for (let t of [...n._values, ...n._refs]) e.remove(t);
			return e.length ? e : null;
		}
		remove(e) {
			if (i.isResolvable(e)) {
				this._refs.delete(e);
				return;
			}
			this._values.delete(e), typeof e == "string" && this._lowercase.delete(e.toLowerCase());
		}
		has(e, t, n, r) {
			return !!this.get(e, t, n, r);
		}
		get(e, t, n, i) {
			if (!this.length) return !1;
			if (this._values.has(e)) return { value: e };
			if (typeof e == "string" && e && i) {
				let t = this._lowercase.get(e.toLowerCase());
				if (t) return { value: t };
			}
			if (!this._refs.size && typeof e != "object") return !1;
			if (typeof e == "object") {
				for (let t of this._values) if (r(t, e)) return { value: t };
			}
			if (t) for (let a of this._refs) {
				let o = a.resolve(e, t, n, null, { in: !0 });
				if (o === void 0) continue;
				let s = !a.in || typeof o != "object" ? [o] : Array.isArray(o) ? o : Object.keys(o);
				for (let t of s) if (typeof t == typeof e) {
					if (i && e && typeof e == "string") {
						if (t.toLowerCase() === e.toLowerCase()) return {
							value: t,
							ref: a
						};
					} else if (r(t, e)) return {
						value: t,
						ref: a
					};
				}
			}
			return !1;
		}
		override() {
			this._override = !0;
		}
		values(e) {
			if (e && e.display) {
				let e = [];
				for (let t of [...this._values, ...this._refs]) t !== void 0 && e.push(t);
				return e;
			}
			return Array.from([...this._values, ...this._refs]);
		}
		clone() {
			let e = new a.Values(this._values, this._refs);
			return e._override = this._override, e;
		}
		concat(e) {
			n(!e._override, "Cannot concat override set of values");
			let t = new a.Values([...this._values, ...e._values], [...this._refs, ...e._refs]);
			return t._override = this._override, t;
		}
		describe() {
			let e = [];
			this._override && e.push({ override: !0 });
			for (let t of this._values.values()) e.push(t && typeof t == "object" ? { value: t } : t);
			for (let t of this._refs.values()) e.push(t.describe());
			return e;
		}
	}, a.Values.prototype[i.symbols.values] = !0, a.Values.prototype.slice = a.Values.prototype.clone, a.lowercases = function(e) {
		let t = /* @__PURE__ */ new Map();
		if (e) for (let n of e) typeof n == "string" && t.set(n.toLowerCase(), n);
		return t;
	};
})), se = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, clone: r, deepEqual: i, merge: a } = E(), o = F(), s = P(), c = I(), l = A(), u = L(), d = x(), f = N(), p = re(), m = j(), h = x(), g = ae(), _ = oe(), v = {
		standardTypes: new Set([
			"string",
			"number",
			"integer",
			"boolean",
			"object",
			"array",
			"null"
		]),
		jsonSchemaTarget: "draft-2020-12",
		primitiveTypes: new Set([
			"string",
			"number",
			"boolean"
		]),
		nullSchema: () => ({ type: "null" })
	};
	v.Base = class {
		constructor(e) {
			this.type = e, this.$_root = null, this._definition = {}, this._reset();
		}
		_reset() {
			this._ids = new p.Ids(), this._preferences = null, this._refs = new m.Manager(), this._cache = null, this._valids = null, this._invalids = null, this._flags = {}, this._rules = [], this._singleRules = /* @__PURE__ */ new Map(), this.$_terms = {}, this.$_temp = {
				ruleset: null,
				whens: {}
			};
		}
		describe() {
			return n(typeof d.describe == "function", "Manifest functionality disabled"), d.describe(this);
		}
		$_jsonSchema(e, t = {}) {
			if (t.target !== void 0 && t.target !== v.jsonSchemaTarget) throw Error(`Unsupported JSON Schema target: ${t.target}`);
			let n = !t.$defs, r = t.$defs ?? {}, a = {}, o = this.type === "any", c = this._flags.only, l = this._valids && Array.from(this._valids._values).filter((e) => e !== null), u = !0;
			if (l && l.length && c && !o) {
				let e = new Set(l.map((e) => typeof e));
				u = e.has(this.type) || this.type === "date" && e.has("object");
			}
			!o && u && v.standardTypes.has(this.type) && (a.type = this.type), this._flags.description && (a.description = this._flags.description), this._flags.default !== void 0 && typeof this._flags.default != "function" && (a.default = this._flags.default);
			let d = {
				...t,
				$defs: r
			};
			this._definition.jsonSchema && u && (a = this._definition.jsonSchema(this, a, e, d));
			for (let t of this._rules) {
				let n = this._definition.rules[t.name];
				n.jsonSchema && u && (a = n.jsonSchema(t, a, c, e, d));
			}
			if (this.$_terms.shared) for (let t of this.$_terms.shared) r[t._flags.id] = t.$_jsonSchema(e, d);
			if (n && Object.keys(r).length && (a.$defs = r), this._valids) {
				let e = l.filter((e) => typeof e != "symbol");
				if (e.length) if (this._flags.only) {
					a.enum = e;
					let t = s.intersect(new Set(e.map((e) => typeof e)), v.primitiveTypes);
					if (t.size) {
						let e = [...t];
						a.type = e.length === 1 ? e[0] : e;
					}
				} else {
					let t = e.filter((e) => typeof e !== this.type || o);
					t.length && !(o && !c) && (a.anyOf || (a = { anyOf: [a] }), a.anyOf.push({ enum: t }));
				}
			}
			if (this._valids && this._valids.has(null) && !(o && !c) && (this._valids.length === 1 && (o || c) ? a.type = "null" : a.type ? a.type = [a.type, "null"] : a.anyOf ? a.anyOf.unshift(v.nullSchema()) : a = { anyOf: [v.nullSchema(), a] }), this.$_terms.whens) {
				let t = this.clone();
				t.$_terms.whens = null;
				let n = [];
				for (let r of this.$_terms.whens) {
					let i = r.is ? [r] : r.switch;
					for (let r = 0; r < i.length; ++r) {
						let a = i[r];
						a.then && n.push(t.concat(a.then).$_jsonSchema(e, d)), a.otherwise && n.push(t.concat(a.otherwise).$_jsonSchema(e, d)), (!a.then || r === i.length - 1 && !a.otherwise) && n.push(t.$_jsonSchema(e, d));
					}
				}
				let r = [];
				for (let e of n) r.some((t) => i(t, e)) || r.push(e);
				return { anyOf: r };
			}
			return a;
		}
		allow(...e) {
			return s.verifyFlat(e, "allow"), this._values(e, "_valids");
		}
		alter(e) {
			n(e && typeof e == "object" && !Array.isArray(e), "Invalid targets argument"), n(!this._inRuleset(), "Cannot set alterations inside a ruleset");
			let t = this.clone();
			t.$_terms.alterations = t.$_terms.alterations || [];
			for (let r in e) {
				let i = e[r];
				n(typeof i == "function", "Alteration adjuster for", r, "must be a function"), t.$_terms.alterations.push({
					target: r,
					adjuster: i
				});
			}
			return t.$_temp.ruleset = !1, t;
		}
		artifact(e) {
			return n(e !== void 0, "Artifact cannot be undefined"), n(!this._cache, "Cannot set an artifact with a rule cache"), this.$_setFlag("artifact", e);
		}
		cast(e) {
			return n(e === !1 || typeof e == "string", "Invalid to value"), n(e === !1 || this._definition.cast[e], "Type", this.type, "does not support casting to", e), this.$_setFlag("cast", e === !1 ? void 0 : e);
		}
		default(e, t) {
			return this._default("default", e, t);
		}
		description(e) {
			return n(e && typeof e == "string", "Description must be a non-empty string"), this.$_setFlag("description", e);
		}
		empty(e) {
			let t = this.clone();
			return e !== void 0 && (e = t.$_compile(e, { override: !1 })), t.$_setFlag("empty", e, { clone: !1 });
		}
		error(e) {
			return n(e, "Missing error"), n(e instanceof Error || typeof e == "function", "Must provide a valid Error object or a function"), this.$_setFlag("error", e);
		}
		example(e, t = {}) {
			return n(e !== void 0, "Missing example"), s.assertOptions(t, ["override"]), this._inner("examples", e, {
				single: !0,
				override: t.override
			});
		}
		external(e, t) {
			return typeof e == "object" && (n(!t, "Cannot combine options with description"), t = e.description, e = e.method), n(typeof e == "function", "Method must be a function"), n(t === void 0 || t && typeof t == "string", "Description must be a non-empty string"), this._inner("externals", {
				method: e,
				description: t
			}, { single: !0 });
		}
		failover(e, t) {
			return this._default("failover", e, t);
		}
		forbidden() {
			return this.presence("forbidden");
		}
		id(e) {
			return e ? (n(typeof e == "string", "id must be a non-empty string"), n(/^[^\.]+$/.test(e), "id cannot contain period character"), this.$_setFlag("id", e)) : this.$_setFlag("id", void 0);
		}
		invalid(...e) {
			return this._values(e, "_invalids");
		}
		label(e) {
			return n(e && typeof e == "string", "Label name must be a non-empty string"), this.$_setFlag("label", e);
		}
		meta(e) {
			return n(e !== void 0, "Meta cannot be undefined"), this._inner("metas", e, { single: !0 });
		}
		note(...e) {
			n(e.length, "Missing notes");
			for (let t of e) n(t && typeof t == "string", "Notes must be non-empty strings");
			return this._inner("notes", e);
		}
		only(e = !0) {
			return n(typeof e == "boolean", "Invalid mode:", e), this.$_setFlag("only", e);
		}
		optional() {
			return this.presence("optional");
		}
		prefs(e) {
			n(e, "Missing preferences"), n(e.context === void 0, "Cannot override context"), n(e.externals === void 0, "Cannot override externals"), n(e.warnings === void 0, "Cannot override warnings"), n(e.debug === void 0, "Cannot override debug"), s.checkPreferences(e);
			let t = this.clone();
			return t._preferences = s.preferences(t._preferences, e), t;
		}
		presence(e) {
			return n([
				"optional",
				"required",
				"forbidden"
			].includes(e), "Unknown presence mode", e), this.$_setFlag("presence", e);
		}
		raw(e = !0) {
			return this.$_setFlag("result", e ? "raw" : void 0);
		}
		result(e) {
			return n(["raw", "strip"].includes(e), "Unknown result mode", e), this.$_setFlag("result", e);
		}
		required() {
			return this.presence("required");
		}
		strict(e) {
			let t = this.clone(), n = e === void 0 ? !1 : !e;
			return t._preferences = s.preferences(t._preferences, { convert: n }), t;
		}
		strip(e = !0) {
			return this.$_setFlag("result", e ? "strip" : void 0);
		}
		tag(...e) {
			n(e.length, "Missing tags");
			for (let t of e) n(t && typeof t == "string", "Tags must be non-empty strings");
			return this._inner("tags", e);
		}
		unit(e) {
			return n(e && typeof e == "string", "Unit name must be a non-empty string"), this.$_setFlag("unit", e);
		}
		valid(...e) {
			s.verifyFlat(e, "valid");
			let t = this.allow(...e);
			return t.$_setFlag("only", !!t._valids, { clone: !1 }), t;
		}
		when(e, t) {
			let r = this.clone();
			r.$_terms.whens || (r.$_terms.whens = []);
			let i = c.when(r, e, t);
			if (!["any", "link"].includes(r.type)) {
				let e = i.is ? [i] : i.switch;
				for (let t of e) n(!t.then || t.then.type === "any" || t.then.type === r.type, "Cannot combine", r.type, "with", t.then && t.then.type), n(!t.otherwise || t.otherwise.type === "any" || t.otherwise.type === r.type, "Cannot combine", r.type, "with", t.otherwise && t.otherwise.type);
			}
			return r.$_terms.whens.push(i), r.$_mutateRebuild();
		}
		cache(e) {
			n(!this._inRuleset(), "Cannot set caching inside a ruleset"), n(!this._cache, "Cannot override schema cache"), n(this._flags.artifact === void 0, "Cannot cache a rule with an artifact");
			let t = this.clone();
			return t._cache = e || o.provider.provision(), t.$_temp.ruleset = !1, t;
		}
		clone() {
			let e = Object.create(Object.getPrototypeOf(this));
			return this._assign(e);
		}
		concat(e) {
			n(s.isSchema(e), "Invalid schema object"), n(this.type === "any" || e.type === "any" || e.type === this.type, "Cannot merge type", this.type, "with another type:", e.type), n(!this._inRuleset(), "Cannot concatenate onto a schema with open ruleset"), n(!e._inRuleset(), "Cannot concatenate a schema with open ruleset");
			let t = this.clone();
			if (this.type === "any" && e.type !== "any") {
				let n = e.clone();
				for (let e of Object.keys(t)) e !== "type" && (n[e] = t[e]);
				t = n;
			}
			t._ids.concat(e._ids), t._refs.register(e, m.toSibling), t._preferences = t._preferences ? s.preferences(t._preferences, e._preferences) : e._preferences, t._valids = _.merge(t._valids, e._valids, e._invalids), t._invalids = _.merge(t._invalids, e._invalids, e._valids);
			for (let n of e._singleRules.keys()) t._singleRules.has(n) && (t._rules = t._rules.filter((e) => e.keep || e.name !== n), t._singleRules.delete(n));
			for (let n of e._rules) e._definition.rules[n.method].multi || t._singleRules.set(n.name, n), t._rules.push(n);
			if (t._flags.empty && e._flags.empty) {
				t._flags.empty = t._flags.empty.concat(e._flags.empty);
				let n = Object.assign({}, e._flags);
				delete n.empty, a(t._flags, n);
			} else if (e._flags.empty) {
				t._flags.empty = e._flags.empty;
				let n = Object.assign({}, e._flags);
				delete n.empty, a(t._flags, n);
			} else a(t._flags, e._flags);
			for (let n in e.$_terms) {
				let r = e.$_terms[n];
				if (!r) {
					t.$_terms[n] || (t.$_terms[n] = r);
					continue;
				}
				if (!t.$_terms[n]) {
					t.$_terms[n] = r.slice();
					continue;
				}
				t.$_terms[n] = t.$_terms[n].concat(r);
			}
			return this.$_root._tracer && this.$_root._tracer._combine(t, [this, e]), t.$_mutateRebuild();
		}
		extend(e) {
			return n(!e.base, "Cannot extend type with another base"), u.type(this, e);
		}
		extract(e) {
			return e = Array.isArray(e) ? e : e.split("."), this._ids.reach(e);
		}
		fork(e, t) {
			n(!this._inRuleset(), "Cannot fork inside a ruleset");
			let r = this;
			for (let n of [].concat(e)) n = Array.isArray(n) ? n : n.split("."), r = r._ids.fork(n, t, r);
			return r.$_temp.ruleset = !1, r;
		}
		isAsync() {
			if (this.$_terms.externals?.length) return !0;
			if (this.$_terms.whens) for (let e of this.$_terms.whens) {
				if (e.then?.isAsync() || e.otherwise?.isAsync()) return !0;
				if (e.switch) {
					for (let t of e.switch) if (t.then?.isAsync() || t.otherwise?.isAsync()) return !0;
				}
			}
			return !1;
		}
		rule(e) {
			let t = this._definition;
			s.assertOptions(e, Object.keys(t.modifiers)), n(this.$_temp.ruleset !== !1, "Cannot apply rules to empty ruleset or the last rule added does not support rule properties");
			let i = this.$_temp.ruleset === null ? this._rules.length - 1 : this.$_temp.ruleset;
			n(i >= 0 && i < this._rules.length, "Cannot apply rules to empty ruleset");
			let a = this.clone();
			for (let o = i; o < a._rules.length; ++o) {
				let i = a._rules[o], s = r(i);
				for (let r in e) t.modifiers[r](s, e[r]), n(s.name === i.name, "Cannot change rule name");
				a._rules[o] = s, a._singleRules.get(s.name) === i && a._singleRules.set(s.name, s);
			}
			return a.$_temp.ruleset = !1, a.$_mutateRebuild();
		}
		get ruleset() {
			n(!this._inRuleset(), "Cannot start a new ruleset without closing the previous one");
			let e = this.clone();
			return e.$_temp.ruleset = e._rules.length, e;
		}
		get $() {
			return this.ruleset;
		}
		tailor(e) {
			e = [].concat(e), n(!this._inRuleset(), "Cannot tailor inside a ruleset");
			let t = this;
			if (this.$_terms.alterations) for (let { target: r, adjuster: i } of this.$_terms.alterations) e.includes(r) && (t = i(t), n(s.isSchema(t), "Alteration adjuster for", r, "failed to return a schema object"));
			return t = t.$_modify({
				each: (t) => t.tailor(e),
				ref: !1
			}), t.$_temp.ruleset = !1, t.$_mutateRebuild();
		}
		tracer() {
			return h.location ? h.location(this) : this;
		}
		validate(e, t) {
			return g.entry(e, this, t);
		}
		validateAsync(e, t) {
			return g.entryAsync(e, this, t);
		}
		$_addRule(e) {
			typeof e == "string" && (e = { name: e }), n(e && typeof e == "object", "Invalid options"), n(e.name && typeof e.name == "string", "Invalid rule name");
			for (let t in e) n(t[0] !== "_", "Cannot set private rule properties");
			let t = Object.assign({}, e);
			t._resolve = [], t.method = t.method || t.name;
			let r = this._definition.rules[t.method], i = t.args;
			n(r, "Unknown rule", t.method);
			let a = this.clone();
			if (i) {
				n(Object.keys(i).length === 1 || Object.keys(i).length === this._definition.rules[t.name].args.length, "Invalid rule definition for", this.type, t.name);
				for (let e in i) {
					let o = i[e];
					if (r.argsByName) {
						let c = r.argsByName.get(e);
						if (c.ref && s.isResolvable(o)) t._resolve.push(e), a.$_mutateRegister(o);
						else if (c.normalize && (o = c.normalize(o), i[e] = o), c.assert) {
							let t = s.validateArg(o, e, c);
							n(!t, t, "or reference");
						}
					}
					if (o === void 0) {
						delete i[e];
						continue;
					}
					i[e] = o;
				}
			}
			return r.multi || (a._ruleRemove(t.name, { clone: !1 }), a._singleRules.set(t.name, t)), a.$_temp.ruleset === !1 && (a.$_temp.ruleset = null), r.priority ? a._rules.unshift(t) : a._rules.push(t), a;
		}
		$_compile(e, t) {
			return c.schema(this.$_root, e, t);
		}
		$_createError(e, t, n, r, i, a = {}) {
			let o = a.flags === !1 ? {} : this._flags, s = a.messages ? f.merge(this._definition.messages, a.messages) : this._definition.messages;
			return new l.Report(e, t, n, o, s, r, i);
		}
		$_getFlag(e) {
			return this._flags[e];
		}
		$_getRule(e) {
			return this._singleRules.get(e);
		}
		$_mapLabels(e) {
			return e = Array.isArray(e) ? e : e.split("."), this._ids.labels(e);
		}
		$_match(e, t, n, r) {
			n = Object.assign({}, n), n.abortEarly = !0, n._externals = !1, t.snapshot();
			let i = !g.validate(e, this, t, n, r).errors;
			return t.restore(), i;
		}
		$_modify(e) {
			return s.assertOptions(e, [
				"each",
				"once",
				"ref",
				"schema"
			]), p.schema(this, e) || this;
		}
		$_mutateRebuild() {
			return n(!this._inRuleset(), "Cannot add this rule inside a ruleset"), this._refs.reset(), this._ids.reset(), this.$_modify({ each: (e, { source: t, name: n, path: r, key: i }) => {
				let a = this._definition[t][n] && this._definition[t][n].register;
				a !== !1 && this.$_mutateRegister(e, {
					family: a,
					key: i
				});
			} }), this._definition.rebuild && this._definition.rebuild(this), this.$_temp.ruleset = !1, this;
		}
		$_mutateRegister(e, { family: t, key: n } = {}) {
			this._refs.register(e, t), this._ids.register(e, { key: n });
		}
		$_property(e) {
			return this._definition.properties[e];
		}
		$_reach(e) {
			return this._ids.reach(e);
		}
		$_rootReferences() {
			return this._refs.roots();
		}
		$_setFlag(e, t, r = {}) {
			n(e[0] === "_" || !this._inRuleset(), "Cannot set flag inside a ruleset");
			let a = this._definition.flags[e] || {};
			if (i(t, a.default) && (t = void 0), i(t, this._flags[e])) return this;
			let o = r.clone === !1 ? this : this.clone();
			return t === void 0 ? delete o._flags[e] : (o._flags[e] = t, o.$_mutateRegister(t)), e[0] !== "_" && (o.$_temp.ruleset = !1), o;
		}
		$_parent(e, ...t) {
			return this[e][s.symbols.parent].call(this, ...t);
		}
		$_validate(e, t, n) {
			return g.validate(e, this, t, n);
		}
		_assign(e) {
			e.type = this.type, e.$_root = this.$_root, e.$_temp = Object.assign({}, this.$_temp), e.$_temp.whens = {}, e._ids = this._ids.clone(), e._preferences = this._preferences, e._valids = this._valids && this._valids.clone(), e._invalids = this._invalids && this._invalids.clone(), e._rules = this._rules.slice(), e._singleRules = r(this._singleRules, { shallow: !0 }), e._refs = this._refs.clone(), e._flags = Object.assign({}, this._flags), e._cache = null, e.$_terms = {};
			for (let t in this.$_terms) e.$_terms[t] = this.$_terms[t] ? this.$_terms[t].slice() : null;
			e.$_super = {};
			for (let t in this.$_super) e.$_super[t] = this._super[t].bind(e);
			return e;
		}
		_bare() {
			let e = this.clone();
			e._reset();
			let t = e._definition.terms;
			for (let n in t) {
				let r = t[n];
				e.$_terms[n] = r.init;
			}
			return e.$_mutateRebuild();
		}
		_default(e, t, r = {}) {
			return s.assertOptions(r, "literal"), n(t !== void 0, "Missing", e, "value"), n(typeof t == "function" || !r.literal, "Only function value supports literal option"), typeof t == "function" && r.literal && (t = {
				[s.symbols.literal]: !0,
				literal: t
			}), this.$_setFlag(e, t);
		}
		_generate(e, t, n) {
			if (!this.$_terms.whens) return { schema: this };
			let r = [], i = [];
			for (let a = 0; a < this.$_terms.whens.length; ++a) {
				let o = this.$_terms.whens[a];
				if (o.concat) {
					r.push(o.concat), i.push(`${a}.concat`);
					continue;
				}
				let s = o.ref ? o.ref.resolve(e, t, n) : e, c = o.is ? [o] : o.switch, l = i.length;
				for (let l = 0; l < c.length; ++l) {
					let { is: u, then: d, otherwise: f } = c[l], p = `${a}${o.switch ? "." + l : ""}`;
					if (u.$_match(s, t.nest(u, `${p}.is`), n)) {
						if (d) {
							let a = t.localize([...t.path, `${p}.then`], t.ancestors, t.schemas), { schema: o, id: s } = d._generate(e, a, n);
							r.push(o), i.push(`${p}.then${s ? `(${s})` : ""}`);
							break;
						}
					} else if (f) {
						let a = t.localize([...t.path, `${p}.otherwise`], t.ancestors, t.schemas), { schema: o, id: s } = f._generate(e, a, n);
						r.push(o), i.push(`${p}.otherwise${s ? `(${s})` : ""}`);
						break;
					}
				}
				if (o.break && i.length > l) break;
			}
			let a = i.join(", ");
			if (t.mainstay.tracer.debug(t, "rule", "when", a), !a) return { schema: this };
			if (!t.mainstay.tracer.active && this.$_temp.whens[a]) return {
				schema: this.$_temp.whens[a],
				id: a
			};
			let o = this;
			this._definition.generate && (o = this._definition.generate(this, e, t, n));
			for (let e of r) o = o.concat(e);
			return this.$_root._tracer && this.$_root._tracer._combine(o, [this, ...r]), this.$_temp.whens[a] = o, {
				schema: o,
				id: a
			};
		}
		_inner(e, t, r = {}) {
			n(!this._inRuleset(), `Cannot set ${e} inside a ruleset`);
			let i = this.clone();
			return (!i.$_terms[e] || r.override) && (i.$_terms[e] = []), r.single ? i.$_terms[e].push(t) : i.$_terms[e].push(...t), i.$_temp.ruleset = !1, i;
		}
		_inRuleset() {
			return this.$_temp.ruleset !== null && this.$_temp.ruleset !== !1;
		}
		_ruleRemove(e, t = {}) {
			if (!this._singleRules.has(e)) return this;
			let n = t.clone === !1 ? this : this.clone();
			n._singleRules.delete(e);
			let r = [];
			for (let t = 0; t < n._rules.length; ++t) {
				let i = n._rules[t];
				if (i.name === e && !i.keep) {
					n._inRuleset() && t < n.$_temp.ruleset && --n.$_temp.ruleset;
					continue;
				}
				r.push(i);
			}
			return n._rules = r, n;
		}
		_values(e, t) {
			s.verifyFlat(e, t.slice(1, -1));
			let r = this.clone(), i = e[0] === s.symbols.override;
			if (i && (e = e.slice(1)), !r[t] && e.length ? r[t] = new _() : i && (r[t] = e.length ? new _() : null, r.$_mutateRebuild()), !r[t]) return r;
			i && r[t].override();
			for (let i of e) {
				n(i !== void 0, "Cannot call allow/valid/invalid with undefined"), n(i !== s.symbols.override, "Override must be the first value");
				let e = t === "_invalids" ? "_valids" : "_invalids";
				r[e] && (r[e].remove(i), r[e].length || (n(t === "_valids" || !r._flags.only, "Setting invalid value", i, "leaves schema rejecting all values due to previous valid rule"), r[e] = null)), r[t].add(i, r._refs);
			}
			return r;
		}
		get "~standard"() {
			let e = (e) => {
				let t;
				return t = l.ValidationError.isError(e) ? e.details.map(({ message: e, path: t }) => ({
					message: e,
					path: t
				})) : [{ message: e.message }], { issues: t };
			}, t = (e) => ({ value: e });
			return {
				version: 1,
				vendor: "joi",
				validate: (n, r) => {
					let i = g.standard(n, this, r);
					return i instanceof Promise ? i.then(t, e) : i.error ? e(i.error) : t(i.value);
				},
				jsonSchema: {
					input: (e) => this.$_jsonSchema("input", e),
					output: (e) => this.$_jsonSchema("output", e)
				}
			};
		}
	}, v.Base.prototype[s.symbols.any] = {
		version: s.version,
		compile: c.compile,
		root: "$_root"
	}, v.Base.prototype.isImmutable = !0, v.Base.prototype.deny = v.Base.prototype.invalid, v.Base.prototype.disallow = v.Base.prototype.invalid, v.Base.prototype.equal = v.Base.prototype.valid, v.Base.prototype.exist = v.Base.prototype.required, v.Base.prototype.not = v.Base.prototype.invalid, v.Base.prototype.options = v.Base.prototype.prefs, v.Base.prototype.preferences = v.Base.prototype.prefs, t.exports = new v.Base();
})), R = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = se(), i = P(), a = N();
	t.exports = r.extend({
		type: "any",
		flags: { only: { default: !1 } },
		terms: {
			alterations: { init: null },
			examples: { init: null },
			externals: { init: null },
			metas: { init: [] },
			notes: { init: [] },
			shared: { init: null },
			tags: { init: [] },
			whens: { init: null }
		},
		rules: {
			custom: {
				method(e, t) {
					return n(typeof e == "function", "Method must be a function"), n(t === void 0 || t && typeof t == "string", "Description must be a non-empty string"), this.$_addRule({
						name: "custom",
						args: {
							method: e,
							description: t
						}
					});
				},
				validate(e, t, { method: n }) {
					try {
						return n(e, t);
					} catch (e) {
						return t.error("any.custom", { error: e });
					}
				},
				args: ["method", "description"],
				multi: !0
			},
			messages: { method(e) {
				return this.prefs({ messages: e });
			} },
			shared: { method(e) {
				n(i.isSchema(e) && e._flags.id, "Schema must be a schema with an id");
				let t = this.clone();
				return t.$_terms.shared = t.$_terms.shared || [], t.$_terms.shared.push(e), t.$_mutateRegister(e), t;
			} },
			warning: {
				method(e, t) {
					return n(e && typeof e == "string", "Invalid warning code"), this.$_addRule({
						name: "warning",
						args: {
							code: e,
							local: t
						},
						warn: !0
					});
				},
				validate(e, t, { code: n, local: r }) {
					return t.error(n, r);
				},
				args: ["code", "local"],
				multi: !0
			}
		},
		modifiers: {
			keep(e, t = !0) {
				e.keep = t;
			},
			message(e, t) {
				e.message = a.compile(t);
			},
			warn(e, t = !0) {
				e.warn = t;
			}
		},
		manifest: { build(e, t) {
			for (let n in t) {
				let r = t[n];
				if ([
					"examples",
					"externals",
					"metas",
					"notes",
					"tags"
				].includes(n)) {
					for (let t of r) e = e[n.slice(0, -1)](t);
					continue;
				}
				if (n === "alterations") {
					let t = {};
					for (let { target: e, adjuster: n } of r) t[e] = n;
					e = e.alter(t);
					continue;
				}
				if (n === "whens") {
					for (let t of r) {
						let { ref: n, is: r, not: i, then: a, otherwise: o, concat: s } = t;
						e = s ? e.concat(s) : n ? e.when(n, {
							is: r,
							not: i,
							then: a,
							otherwise: o,
							switch: t.switch,
							break: t.break
						}) : e.when(r, {
							then: a,
							otherwise: o,
							break: t.break
						});
					}
					continue;
				}
				if (n === "shared") for (let t of r) e = e.shared(t);
			}
			return e;
		} },
		messages: {
			"any.custom": "{{#label}} failed custom validation because {{#error.message}}",
			"any.default": "{{#label}} threw an error when running default method",
			"any.failover": "{{#label}} threw an error when running failover method",
			"any.invalid": "{{#label}} contains an invalid value",
			"any.only": "{{#label}} must be {if(#valids.length == 1, \"\", \"one of \")}{{#valids}}",
			"any.ref": "{{#label}} {{#arg}} references {{:#ref}} which {{#reason}}",
			"any.required": "{{#label}} is required",
			"any.unknown": "{{#label}} is not allowed"
		}
	});
})), ce = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, merge: r } = E(), i = R(), a = P(), o = I(), s = A(), c = j(), l = {};
	t.exports = i.extend({
		type: "alternatives",
		flags: { match: { default: "any" } },
		terms: { matches: {
			init: [],
			register: c.toSibling
		} },
		args(e, ...t) {
			return t.length === 1 && Array.isArray(t[0]) ? e.try(...t[0]) : e.try(...t);
		},
		validate(e, t) {
			let { schema: n, error: i, state: a, prefs: o } = t;
			if (n._flags.match) {
				let t = [], c = [];
				for (let r = 0; r < n.$_terms.matches.length; ++r) {
					let i = n.$_terms.matches[r], s = a.nest(i.schema, `match.${r}`);
					s.snapshot();
					let l = i.schema.$_validate(e, s, o);
					l.errors ? (c.push(l.errors), s.restore()) : (t.push(l.value), s.commit());
				}
				if (t.length === 0) return { errors: i("alternatives.any", { details: c.map((e) => s.details(e, { override: !1 })) }) };
				if (n._flags.match === "one") return t.length === 1 ? { value: t[0] } : { errors: i("alternatives.one") };
				if (t.length !== n.$_terms.matches.length) return { errors: i("alternatives.all", { details: c.map((e) => s.details(e, { override: !1 })) }) };
				let l = (e) => e.$_terms.matches.some((e) => e.schema.type === "object" || e.schema.type === "alternatives" && l(e.schema));
				return l(n) ? { value: t.reduce((e, t) => r(e, t, { mergeArrays: !1 })) } : { value: t[t.length - 1] };
			}
			let c = [];
			for (let t = 0; t < n.$_terms.matches.length; ++t) {
				let r = n.$_terms.matches[t];
				if (r.schema) {
					let n = a.nest(r.schema, `match.${t}`);
					n.snapshot();
					let i = r.schema.$_validate(e, n, o);
					if (!i.errors) return n.commit(), i;
					n.restore(), c.push({
						schema: r.schema,
						reports: i.errors
					});
					continue;
				}
				let i = r.ref ? r.ref.resolve(e, a, o) : e, s = r.is ? [r] : r.switch;
				for (let n = 0; n < s.length; ++n) {
					let { is: c, then: l, otherwise: u } = s[n], d = `match.${t}${r.switch ? "." + n : ""}`;
					if (!c.$_match(i, a.nest(c, `${d}.is`), o)) {
						if (u) return u.$_validate(e, a.nest(u, `${d}.otherwise`), o);
					} else if (l) return l.$_validate(e, a.nest(l, `${d}.then`), o);
				}
			}
			return l.errors(c, t);
		},
		jsonSchema(e, t, n, r) {
			let i = [];
			for (let t of e.$_terms.matches) if (t.schema) i.push(t.schema.$_jsonSchema(n, r));
			else {
				let e = t.is ? [t] : t.switch;
				for (let t of e) t.then && i.push(t.then.$_jsonSchema(n, r)), t.otherwise && i.push(t.otherwise.$_jsonSchema(n, r));
			}
			return i.length && (delete t.type, (e._flags.match ?? "any") === "one" ? t.oneOf = i : t.anyOf = i), t;
		},
		rules: {
			conditional: { method(e, t) {
				n(!this._flags._endedSwitch, "Unreachable condition"), n(!this._flags.match, "Cannot combine match mode", this._flags.match, "with conditional rule"), n(t.break === void 0, "Cannot use break option with alternatives conditional");
				let r = this.clone(), i = o.when(r, e, t), a = i.is ? [i] : i.switch;
				for (let e of a) if (e.then && e.otherwise) {
					r.$_setFlag("_endedSwitch", !0, { clone: !1 });
					break;
				}
				return r.$_terms.matches.push(i), r.$_mutateRebuild();
			} },
			match: { method(e) {
				if (n([
					"any",
					"one",
					"all"
				].includes(e), "Invalid alternatives match mode", e), e !== "any") for (let t of this.$_terms.matches) n(t.schema, "Cannot combine match mode", e, "with conditional rules");
				return this.$_setFlag("match", e);
			} },
			try: { method(...e) {
				n(e.length, "Missing alternative schemas"), a.verifyFlat(e, "try"), n(!this._flags._endedSwitch, "Unreachable condition");
				let t = this.clone();
				for (let n of e) t.$_terms.matches.push({ schema: t.$_compile(n) });
				return t.$_mutateRebuild();
			} }
		},
		overrides: {
			label(e) {
				return this.$_parent("label", e).$_modify({
					each: (t, n) => n.path[0] !== "is" && typeof t._flags.label != "string" ? t.label(e) : void 0,
					ref: !1
				});
			},
			isAsync() {
				if (this.$_terms.externals?.length) return !0;
				for (let e of this.$_terms.matches) if (e.schema?.isAsync() || e.then?.isAsync() || e.otherwise?.isAsync()) return !0;
				return !1;
			}
		},
		rebuild(e) {
			e.$_modify({ each: (t) => {
				a.isSchema(t) && t.type === "array" && e.$_setFlag("_arrayItems", !0, { clone: !1 });
			} });
		},
		manifest: { build(e, t) {
			if (t.matches) for (let n of t.matches) {
				let { schema: t, ref: r, is: i, not: a, then: o, otherwise: s } = n;
				e = t ? e.try(t) : r ? e.conditional(r, {
					is: i,
					then: o,
					not: a,
					otherwise: s,
					switch: n.switch
				}) : e.conditional(i, {
					then: o,
					otherwise: s
				});
			}
			return e;
		} },
		messages: {
			"alternatives.all": "{{#label}} does not match all of the required types",
			"alternatives.any": "{{#label}} does not match any of the allowed types",
			"alternatives.match": "{{#label}} does not match any of the allowed types",
			"alternatives.one": "{{#label}} matches more than one allowed type",
			"alternatives.types": "{{#label}} must be one of {{#types}}"
		}
	}), l.errors = function(e, { error: t, state: n }) {
		if (!e.length) return { errors: t("alternatives.any") };
		if (e.length === 1) return { errors: e[0].reports };
		let r = /* @__PURE__ */ new Set(), i = [];
		for (let { reports: a, schema: o } of e) {
			if (a.length > 1) return l.unmatched(e, t);
			let c = a[0];
			if (!(c instanceof s.Report)) return l.unmatched(e, t);
			if (c.state.path.length !== n.path.length) {
				i.push({
					type: o.type,
					report: c
				});
				continue;
			}
			if (c.code === "any.only") {
				for (let e of c.local.valids) r.add(e);
				continue;
			}
			let [u, d] = c.code.split(".");
			d === "base" ? c.code === "object.base" ? r.add(c.local.type) : r.add(u) : i.push({
				type: o.type,
				report: c
			});
		}
		return i.length ? i.length === 1 ? { errors: i[0].report } : l.unmatched(e, t) : { errors: t("alternatives.types", { types: [...r] }) };
	}, l.unmatched = function(e, t) {
		let n = [];
		for (let t of e) n.push(...t.reports);
		return { errors: t("alternatives.match", s.details(n, { override: !1 })) };
	};
})), le = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, deepEqual: r, reach: i } = E(), a = R(), o = P(), s = I(), c = {};
	t.exports = a.extend({
		type: "array",
		flags: {
			single: { default: !1 },
			sparse: { default: !1 }
		},
		terms: {
			items: {
				init: [],
				manifest: "schema"
			},
			ordered: {
				init: [],
				manifest: "schema"
			},
			_exclusions: { init: [] },
			_inclusions: { init: [] },
			_requireds: { init: [] }
		},
		coerce: {
			from: "object",
			method(e, { schema: t, state: n, prefs: r }) {
				if (!Array.isArray(e)) return;
				let i = t.$_getRule("sort");
				if (i) return c.sort(t, e, i.args.options, n, r);
			}
		},
		validate(e, { schema: t, error: n }) {
			if (!Array.isArray(e)) {
				if (t._flags.single) {
					let t = [e];
					return t[o.symbols.arraySingle] = !0, { value: t };
				}
				return { errors: n("array.base") };
			}
			if (!(!t.$_getRule("items") && !t.$_terms.externals)) return { value: e.slice() };
		},
		jsonSchema(e, t, n, r) {
			let i = e.$_terms.ordered;
			if (i.length && (t.prefixItems = i.map((e) => e.$_jsonSchema(n, r))), e.$_terms.items.length) {
				let a;
				a = e.$_terms.items.length === 1 ? e.$_terms.items[0].$_jsonSchema(n, r) : { anyOf: e.$_terms.items.map((e) => e.$_jsonSchema(n, r)) }, i.length ? (t.unevaluatedItems = a, t.minItems = i.length) : t.items = a;
			} else i.length && (t.unevaluatedItems = !1, t.minItems = i.length, t.maxItems = i.length);
			let a = [];
			for (let t of e._rules) t.name === "has" && a.push(t.args.schema.$_jsonSchema(n, r));
			if (a.length && (a.length === 1 ? t.contains = a[0] : t.allOf = a.map((e) => ({ contains: e }))), e._flags.single && e.$_terms.items.length) {
				let i;
				i = e.$_terms.items.length === 1 ? e.$_terms.items[0].$_jsonSchema(n, r) : { anyOf: e.$_terms.items.map((e) => e.$_jsonSchema(n, r)) }, t = { anyOf: [t, i] };
			}
			return t;
		},
		rules: {
			has: {
				method(e) {
					e = this.$_compile(e, { appendPath: !0 });
					let t = this.$_addRule({
						name: "has",
						args: { schema: e }
					});
					return t.$_mutateRegister(e), t;
				},
				validate(e, { state: t, prefs: n, error: r }, { schema: i }) {
					let a = [e, ...t.ancestors];
					for (let r = 0; r < e.length; ++r) {
						let o = t.localize([...t.path, r], a, i);
						if (i.$_match(e[r], o, n)) return e;
					}
					let o = i._flags.label;
					return o ? r("array.hasKnown", { patternLabel: o }) : r("array.hasUnknown", null);
				},
				multi: !0
			},
			items: {
				method(...e) {
					o.verifyFlat(e, "items");
					let t = this.$_addRule("items");
					for (let n = 0; n < e.length; ++n) {
						let r = o.tryWithPath(() => this.$_compile(e[n]), n, { append: !0 });
						t.$_terms.items.push(r);
					}
					return t.$_mutateRebuild();
				},
				validate(e, { schema: t, error: n, state: r, prefs: i, errorsArray: a }) {
					let s = t.$_terms._requireds.slice(), l = t.$_terms.ordered.slice(), u = [...t.$_terms._inclusions, ...s], d = !e[o.symbols.arraySingle];
					delete e[o.symbols.arraySingle];
					let f = a(), p = e.length;
					for (let a = 0; a < p; ++a) {
						let o = e[a], m = !1, h = !1, g = d ? a : new Number(a), _ = [...r.path, g];
						if (!t._flags.sparse && o === void 0) {
							if (f.push(n("array.sparse", {
								key: g,
								path: _,
								pos: a,
								value: void 0
							}, r.localize(_))), i.abortEarly) return f;
							l.shift();
							continue;
						}
						let v = [e, ...r.ancestors];
						for (let e of t.$_terms._exclusions) if (e.$_match(o, r.localize(_, v, e), i, { presence: "ignore" })) {
							if (f.push(n("array.excludes", {
								pos: a,
								value: o
							}, r.localize(_))), i.abortEarly) return f;
							m = !0, l.shift();
							break;
						}
						if (m) continue;
						if (t.$_terms.ordered.length) {
							if (l.length) {
								let s = l.shift(), u = s.$_validate(o, r.localize(_, v, s), i);
								if (!u.errors) if (s._flags.result === "strip") c.fastSplice(e, a), --a, --p;
								else if (!t._flags.sparse && u.value === void 0) {
									if (f.push(n("array.sparse", {
										key: g,
										path: _,
										pos: a,
										value: void 0
									}, r.localize(_))), i.abortEarly) return f;
									continue;
								} else e[a] = u.value;
								else if (f.push(...u.errors), i.abortEarly) return f;
								continue;
							} else if (!t.$_terms.items.length) {
								if (f.push(n("array.orderedLength", {
									pos: a,
									limit: t.$_terms.ordered.length
								})), i.abortEarly) return f;
								break;
							}
						}
						let y = [], b = s.length;
						for (let l = 0; l < b; ++l) {
							let u = r.localize(_, v, s[l]);
							u.snapshot();
							let d = s[l].$_validate(o, u, i);
							if (y[l] = d, !d.errors) {
								if (u.commit(), e[a] = d.value, h = !0, c.fastSplice(s, l), --l, --b, !t._flags.sparse && d.value === void 0 && (f.push(n("array.sparse", {
									key: g,
									path: _,
									pos: a,
									value: void 0
								}, r.localize(_))), i.abortEarly)) return f;
								break;
							}
							u.restore();
						}
						if (h) continue;
						let x = i.stripUnknown && !!i.stripUnknown.arrays || !1;
						b = u.length;
						for (let l of u) {
							let u, d = s.indexOf(l);
							if (d !== -1) u = y[d];
							else {
								let s = r.localize(_, v, l);
								if (s.snapshot(), u = l.$_validate(o, s, i), !u.errors) {
									s.commit(), l._flags.result === "strip" ? (c.fastSplice(e, a), --a, --p) : !t._flags.sparse && u.value === void 0 ? (f.push(n("array.sparse", {
										key: g,
										path: _,
										pos: a,
										value: void 0
									}, r.localize(_))), m = !0) : e[a] = u.value, h = !0;
									break;
								}
								s.restore();
							}
							if (b === 1) {
								if (x) {
									c.fastSplice(e, a), --a, --p, h = !0;
									break;
								}
								if (f.push(...u.errors), i.abortEarly) return f;
								m = !0;
								break;
							}
						}
						if (!m && (t.$_terms._inclusions.length || t.$_terms._requireds.length) && !h) {
							if (x) {
								c.fastSplice(e, a), --a, --p;
								continue;
							}
							if (f.push(n("array.includes", {
								pos: a,
								value: o
							}, r.localize(_))), i.abortEarly) return f;
						}
					}
					return s.length && c.fillMissedErrors(t, f, s, e, r, i), l.length && (c.fillOrderedErrors(t, f, l, e, r, i), f.length || c.fillDefault(l, e, r, i)), f.length ? f : e;
				},
				priority: !0,
				manifest: !1
			},
			length: {
				method(e) {
					return this.$_addRule({
						name: "length",
						args: { limit: e },
						operator: "="
					});
				},
				validate(e, t, { limit: n }, { name: r, operator: i, args: a }) {
					return o.compare(e.length, n, i) ? e : t.error("array." + r, {
						limit: a.limit,
						value: e
					});
				},
				jsonSchema(e, t) {
					return t.minItems = e.args.limit, t.maxItems = e.args.limit, t;
				},
				args: [{
					name: "limit",
					ref: !0,
					assert: o.limit,
					message: "must be a positive integer"
				}]
			},
			max: {
				method(e) {
					return this.$_addRule({
						name: "max",
						method: "length",
						args: { limit: e },
						operator: "<="
					});
				},
				jsonSchema(e, t) {
					return t.maxItems = e.args.limit, t;
				}
			},
			min: {
				method(e) {
					return this.$_addRule({
						name: "min",
						method: "length",
						args: { limit: e },
						operator: ">="
					});
				},
				jsonSchema(e, t) {
					return t.minItems = e.args.limit, t;
				}
			},
			ordered: { method(...e) {
				o.verifyFlat(e, "ordered");
				let t = this.$_addRule("items");
				for (let n = 0; n < e.length; ++n) {
					let r = o.tryWithPath(() => this.$_compile(e[n]), n, { append: !0 });
					c.validateSingle(r, t), t.$_mutateRegister(r), t.$_terms.ordered.push(r);
				}
				return t.$_mutateRebuild();
			} },
			single: { method(e) {
				let t = e === void 0 ? !0 : !!e;
				return n(!t || !this._flags._arrayItems, "Cannot specify single rule when array has array items"), this.$_setFlag("single", t);
			} },
			sort: {
				method(e = {}) {
					o.assertOptions(e, ["by", "order"]);
					let t = { order: e.order || "ascending" };
					return e.by && (t.by = s.ref(e.by, { ancestor: 0 }), n(!t.by.ancestor, "Cannot sort by ancestor")), this.$_addRule({
						name: "sort",
						args: { options: t }
					});
				},
				validate(e, { error: t, state: n, prefs: r, schema: i }, { options: a }) {
					let { value: o, errors: s } = c.sort(i, e, a, n, r);
					if (s) return s;
					for (let n = 0; n < e.length; ++n) if (e[n] !== o[n]) return t("array.sort", {
						order: a.order,
						by: a.by ? a.by.key : "value"
					});
					return e;
				},
				convert: !0
			},
			sparse: { method(e) {
				let t = e === void 0 ? !0 : !!e;
				return this._flags.sparse === t ? this : (t ? this.clone() : this.$_addRule("items")).$_setFlag("sparse", t, { clone: !1 });
			} },
			unique: {
				method(e, t = {}) {
					n(!e || typeof e == "function" || typeof e == "string", "comparator must be a function or a string"), o.assertOptions(t, ["ignoreUndefined", "separator"]);
					let r = {
						name: "unique",
						args: {
							options: t,
							comparator: e
						}
					};
					if (e) if (typeof e == "string") {
						let n = o.default(t.separator, ".");
						r.path = n ? e.split(n) : [e];
					} else r.comparator = e;
					return this.$_addRule(r);
				},
				validate(e, { state: t, error: a, schema: o }, { comparator: s, options: c }, { comparator: l, path: u }) {
					let d = {
						string: Object.create(null),
						number: Object.create(null),
						undefined: Object.create(null),
						boolean: Object.create(null),
						bigint: Object.create(null),
						object: /* @__PURE__ */ new Map(),
						function: /* @__PURE__ */ new Map(),
						custom: /* @__PURE__ */ new Map()
					}, f = l || r, p = c.ignoreUndefined;
					for (let r = 0; r < e.length; ++r) {
						let o = u ? i(e[r], u) : e[r], c = l ? d.custom : d[typeof o];
						if (n(c, "Failed to find unique map container for type", typeof o), c instanceof Map) {
							let n = c.entries(), i;
							for (; !(i = n.next()).done;) if (f(i.value[0], o)) {
								let n = t.localize([...t.path, r], [e, ...t.ancestors]), o = {
									pos: r,
									value: e[r],
									dupePos: i.value[1],
									dupeValue: e[i.value[1]]
								};
								return u && (o.path = s), a("array.unique", o, n);
							}
							c.set(o, r);
						} else {
							if ((!p || o !== void 0) && c[o] !== void 0) {
								let n = {
									pos: r,
									value: e[r],
									dupePos: c[o],
									dupeValue: e[c[o]]
								};
								return u && (n.path = s), a("array.unique", n, t.localize([...t.path, r], [e, ...t.ancestors]));
							}
							c[o] = r;
						}
					}
					return e;
				},
				jsonSchema(e, t) {
					return t.uniqueItems = !0, t;
				},
				args: ["comparator", "options"],
				multi: !0
			}
		},
		overrides: { isAsync() {
			if (this.$_terms.externals?.length) return !0;
			for (let e of this.$_terms.items) if (e.isAsync()) return !0;
			for (let e of this.$_terms.ordered) if (e.isAsync()) return !0;
			return !1;
		} },
		cast: { set: {
			from: Array.isArray,
			to(e, t) {
				return new Set(e);
			}
		} },
		rebuild(e) {
			e.$_terms._inclusions = [], e.$_terms._exclusions = [], e.$_terms._requireds = [];
			for (let t of e.$_terms.items) c.validateSingle(t, e), t._flags.presence === "required" ? e.$_terms._requireds.push(t) : t._flags.presence === "forbidden" ? e.$_terms._exclusions.push(t) : e.$_terms._inclusions.push(t);
			for (let t of e.$_terms.ordered) c.validateSingle(t, e);
		},
		manifest: { build(e, t) {
			return t.items && (e = e.items(...t.items)), t.ordered && (e = e.ordered(...t.ordered)), e;
		} },
		messages: {
			"array.base": "{{#label}} must be an array",
			"array.excludes": "{{#label}} contains an excluded value",
			"array.hasKnown": "{{#label}} does not contain at least one required match for type {:#patternLabel}",
			"array.hasUnknown": "{{#label}} does not contain at least one required match",
			"array.includes": "{{#label}} does not match any of the allowed types",
			"array.includesRequiredBoth": "{{#label}} does not contain {{#knownMisses}} and {{#unknownMisses}} other required value(s)",
			"array.includesRequiredKnowns": "{{#label}} does not contain {{#knownMisses}}",
			"array.includesRequiredUnknowns": "{{#label}} does not contain {{#unknownMisses}} required value(s)",
			"array.length": "{{#label}} must contain {{#limit}} items",
			"array.max": "{{#label}} must contain less than or equal to {{#limit}} items",
			"array.min": "{{#label}} must contain at least {{#limit}} items",
			"array.orderedLength": "{{#label}} must contain at most {{#limit}} items",
			"array.sort": "{{#label}} must be sorted in {#order} order by {{#by}}",
			"array.sort.mismatching": "{{#label}} cannot be sorted due to mismatching types",
			"array.sort.unsupported": "{{#label}} cannot be sorted due to unsupported type {#type}",
			"array.sparse": "{{#label}} must not be a sparse array item",
			"array.unique": "{{#label}} contains a duplicate value"
		}
	}), c.fillMissedErrors = function(e, t, n, r, i, a) {
		let o = [], s = 0;
		for (let e of n) {
			let t = e._flags.label;
			t ? o.push(t) : ++s;
		}
		o.length ? s ? t.push(e.$_createError("array.includesRequiredBoth", r, {
			knownMisses: o,
			unknownMisses: s
		}, i, a)) : t.push(e.$_createError("array.includesRequiredKnowns", r, { knownMisses: o }, i, a)) : t.push(e.$_createError("array.includesRequiredUnknowns", r, { unknownMisses: s }, i, a));
	}, c.fillOrderedErrors = function(e, t, n, r, i, a) {
		let o = [];
		for (let e of n) e._flags.presence === "required" && o.push(e);
		o.length && c.fillMissedErrors(e, t, o, r, i, a);
	}, c.fillDefault = function(e, t, n, r) {
		let i = [], a = !0;
		for (let o = e.length - 1; o >= 0; --o) {
			let s = e[o], c = [t, ...n.ancestors], l = s.$_validate(void 0, n.localize(n.path, c, s), r).value;
			if (a) {
				if (l === void 0) continue;
				a = !1;
			}
			i.unshift(l);
		}
		i.length && t.push(...i);
	}, c.fastSplice = function(e, t) {
		let n = t;
		for (; n < e.length;) e[n++] = e[n];
		--e.length;
	}, c.validateSingle = function(e, t) {
		(e.type === "array" || e._flags._arrayItems) && (n(!t._flags.single, "Cannot specify array item with single rule enabled"), t.$_setFlag("_arrayItems", !0, { clone: !1 }));
	}, c.sort = function(e, t, n, r, i) {
		let a = n.order === "ascending" ? 1 : -1, o = -1 * a, s = a, l = (l, u) => {
			let d = c.compare(l, u, o, s);
			if (d !== null || (n.by && (l = n.by.resolve(l, r, i), u = n.by.resolve(u, r, i)), d = c.compare(l, u, o, s), d !== null)) return d;
			let f = typeof l;
			if (f !== typeof u) throw e.$_createError("array.sort.mismatching", t, null, r, i);
			if (f !== "number" && f !== "string") throw e.$_createError("array.sort.unsupported", t, { type: f }, r, i);
			return f === "number" ? (l - u) * a : l < u ? o : s;
		};
		try {
			return { value: t.slice().sort(l) };
		} catch (e) {
			return { errors: e };
		}
	}, c.compare = function(e, t, n, r) {
		return e === t ? 0 : e === void 0 ? 1 : t === void 0 ? -1 : e === null ? r : t === null ? n : null;
	};
})), ue = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = R(), i = P(), a = oe(), o = {};
	o.isBool = function(e) {
		return typeof e == "boolean";
	}, t.exports = r.extend({
		type: "boolean",
		flags: { sensitive: { default: !1 } },
		terms: {
			falsy: {
				init: null,
				manifest: "values"
			},
			truthy: {
				init: null,
				manifest: "values"
			}
		},
		coerce(e, { schema: t }) {
			if (typeof e != "boolean") {
				if (typeof e == "string") {
					let n = e.trim(), r = t._flags.sensitive ? n : n.toLowerCase();
					e = r === "true" ? !0 : r === "false" ? !1 : e;
				}
				return typeof e != "boolean" && (e = t.$_terms.truthy && t.$_terms.truthy.has(e, null, null, !t._flags.sensitive) || (t.$_terms.falsy && t.$_terms.falsy.has(e, null, null, !t._flags.sensitive) ? !1 : e)), { value: e };
			}
		},
		validate(e, { error: t }) {
			if (typeof e != "boolean") return {
				value: e,
				errors: t("boolean.base")
			};
		},
		rules: {
			truthy: { method(...e) {
				i.verifyFlat(e, "truthy");
				let t = this.clone();
				t.$_terms.truthy = t.$_terms.truthy || new a();
				for (let r = 0; r < e.length; ++r) {
					let i = e[r];
					n(i !== void 0, "Cannot call truthy with undefined"), t.$_terms.truthy.add(i);
				}
				return t;
			} },
			falsy: { method(...e) {
				i.verifyFlat(e, "falsy");
				let t = this.clone();
				t.$_terms.falsy = t.$_terms.falsy || new a();
				for (let r = 0; r < e.length; ++r) {
					let i = e[r];
					n(i !== void 0, "Cannot call falsy with undefined"), t.$_terms.falsy.add(i);
				}
				return t;
			} },
			sensitive: { method(e = !0) {
				return this.$_setFlag("sensitive", e);
			} }
		},
		cast: {
			number: {
				from: o.isBool,
				to(e, t) {
					return +!!e;
				}
			},
			string: {
				from: o.isBool,
				to(e, t) {
					return e ? "true" : "false";
				}
			}
		},
		manifest: { build(e, t) {
			return t.truthy && (e = e.truthy(...t.truthy)), t.falsy && (e = e.falsy(...t.falsy)), e;
		} },
		messages: { "boolean.base": "{{#label}} must be a boolean" }
	});
})), de = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = R(), i = P(), a = M(), o = { formats: [
		"iso",
		"javascript",
		"unix"
	] };
	o.isDate = function(e) {
		return e instanceof Date;
	}, t.exports = r.extend({
		type: "date",
		coerce: {
			from: ["number", "string"],
			method(e, { schema: t }) {
				return { value: o.parse(e, t._flags.format) || e };
			}
		},
		validate(e, { schema: t, error: n, prefs: r }) {
			if (e instanceof Date && !isNaN(e.getTime())) return;
			let i = t._flags.format;
			return !r.convert || !i || typeof e != "string" ? {
				value: e,
				errors: n("date.base")
			} : {
				value: e,
				errors: n("date.format", { format: i })
			};
		},
		jsonSchema(e, t, n, r) {
			return t.type = "string", t.format = "date-time", t;
		},
		rules: {
			compare: {
				method: !1,
				validate(e, t, { date: n }, { name: r, operator: a, args: o }) {
					let s = n === "now" ? Date.now() : n.getTime();
					return i.compare(e.getTime(), s, a) ? e : t.error("date." + r, {
						limit: o.date,
						value: e
					});
				},
				args: [{
					name: "date",
					ref: !0,
					normalize: (e) => e === "now" ? e : o.parse(e),
					assert: (e) => e !== null,
					message: "must have a valid date format"
				}]
			},
			format: { method(e) {
				return n(o.formats.includes(e), "Unknown date format", e), this.$_setFlag("format", e);
			} },
			greater: {
				method(e) {
					return this.$_addRule({
						name: "greater",
						method: "compare",
						args: { date: e },
						operator: ">"
					});
				},
				jsonSchema(e, t) {
					let n = e.args.date;
					return n instanceof Date && (t["x-constraint"] = {
						...t["x-constraint"],
						greater: n.toISOString()
					}), t;
				}
			},
			iso: { method() {
				return this.format("iso");
			} },
			less: {
				method(e) {
					return this.$_addRule({
						name: "less",
						method: "compare",
						args: { date: e },
						operator: "<"
					});
				},
				jsonSchema(e, t) {
					let n = e.args.date;
					return n instanceof Date && (t["x-constraint"] = {
						...t["x-constraint"],
						less: n.toISOString()
					}), t;
				}
			},
			max: {
				method(e) {
					return this.$_addRule({
						name: "max",
						method: "compare",
						args: { date: e },
						operator: "<="
					});
				},
				jsonSchema(e, t) {
					let n = e.args.date;
					return n instanceof Date && (t["x-constraint"] = {
						...t["x-constraint"],
						max: n.toISOString()
					}), t;
				}
			},
			min: {
				method(e) {
					return this.$_addRule({
						name: "min",
						method: "compare",
						args: { date: e },
						operator: ">="
					});
				},
				jsonSchema(e, t) {
					let n = e.args.date;
					return n instanceof Date && (t["x-constraint"] = {
						...t["x-constraint"],
						min: n.toISOString()
					}), t;
				}
			},
			timestamp: { method(e = "javascript") {
				return n(["javascript", "unix"].includes(e), "\"type\" must be one of \"javascript, unix\""), this.format(e);
			} }
		},
		cast: {
			number: {
				from: o.isDate,
				to(e, t) {
					return e.getTime();
				}
			},
			string: {
				from: o.isDate,
				to(e, { prefs: t }) {
					return a.date(e, t);
				}
			}
		},
		messages: {
			"date.base": "{{#label}} must be a valid date",
			"date.format": "{{#label}} must be in {msg(\"date.format.\" + #format) || #format} format",
			"date.greater": "{{#label}} must be greater than {{:#limit}}",
			"date.less": "{{#label}} must be less than {{:#limit}}",
			"date.max": "{{#label}} must be less than or equal to {{:#limit}}",
			"date.min": "{{#label}} must be greater than or equal to {{:#limit}}",
			"date.format.iso": "ISO 8601 date",
			"date.format.javascript": "timestamp or number of milliseconds",
			"date.format.unix": "timestamp or number of seconds"
		}
	}), o.parse = function(e, t) {
		if (e instanceof Date) return e;
		if (typeof e != "string" && (isNaN(e) || !isFinite(e)) || /^\s*$/.test(e)) return null;
		if (t === "iso") return i.isIsoDate(e) ? o.date(e.toString()) : null;
		let n = e;
		if (typeof e == "string" && /^[+-]?\d+(\.\d+)?$/.test(e) && (e = parseFloat(e)), t) {
			if (t === "javascript") return o.date(1 * e);
			if (t === "unix") return o.date(1e3 * e);
			if (typeof n == "string") return null;
		}
		return o.date(e);
	}, o.date = function(e) {
		let t = new Date(e);
		return isNaN(t.getTime()) ? null : t;
	};
})), fe = /* @__PURE__ */ s(((e) => {
	var { assert: t } = E(), n = {};
	e.Sorter = class {
		constructor() {
			this._items = [], this.nodes = [];
		}
		add(e, n) {
			n ??= {};
			let r = [].concat(n.before ?? []), i = [].concat(n.after ?? []), a = n.group ?? "?", o = n.sort ?? 0;
			t(!r.includes(a), `Item cannot come before itself: ${a}`), t(!r.includes("?"), "Item cannot come before unassociated items"), t(!i.includes(a), `Item cannot come after itself: ${a}`), t(!i.includes("?"), "Item cannot come after unassociated items"), Array.isArray(e) || (e = [e]);
			for (let t of e) {
				let e = {
					seq: this._items.length,
					sort: o,
					before: r,
					after: i,
					group: a,
					node: t
				};
				this._items.push(e);
			}
			return n.manual || t(this._sort(), "item", a === "?" ? "" : `added into group ${a}`, "created a dependencies error"), this.nodes;
		}
		merge(e) {
			Array.isArray(e) || (e = [e]);
			for (let t of e) if (t) for (let e of t._items) this._items.push(Object.assign({}, e));
			this._items.sort(n.mergeSort);
			for (let e = 0; e < this._items.length; ++e) this._items[e].seq = e;
			return t(this._sort(), "merge created a dependencies error"), this.nodes;
		}
		sort() {
			return t(this._sort(), "sort created a dependencies error"), this.nodes;
		}
		_sort() {
			let e = {}, t = Object.create(null), n = Object.create(null);
			for (let r of this._items) {
				let i = r.seq, a = r.group;
				n[a] = n[a] ?? [], n[a].push(i), e[i] = r.before;
				for (let e of r.after) t[e] = t[e] ?? [], t[e].push(i);
			}
			for (let t in e) {
				let r = [];
				for (let i in e[t]) {
					let a = e[t][i];
					n[a] = n[a] ?? [], r.push(...n[a]);
				}
				e[t] = r;
			}
			for (let r in t) if (n[r]) for (let i of n[r]) e[i].push(...t[r]);
			let r = {};
			for (let t in e) {
				let n = e[t];
				for (let e of n) r[e] = r[e] ?? [], r[e].push(t);
			}
			let i = {}, a = [];
			for (let e = 0; e < this._items.length; ++e) {
				let t = e;
				if (r[e]) {
					t = null;
					for (let e = 0; e < this._items.length; ++e) {
						if (i[e] === !0) continue;
						r[e] || (r[e] = []);
						let n = r[e].length, a = 0;
						for (let t = 0; t < n; ++t) i[r[e][t]] && ++a;
						if (a === n) {
							t = e;
							break;
						}
					}
				}
				t !== null && (i[t] = !0, a.push(t));
			}
			if (a.length !== this._items.length) return !1;
			let o = {};
			for (let e of this._items) o[e.seq] = e;
			this._items = [], this.nodes = [];
			for (let e of a) {
				let t = o[e];
				this.nodes.push(t.node), this._items.push(t);
			}
			return !0;
		}
	}, n.mergeSort = (e, t) => e.sort === t.sort ? 0 : e.sort < t.sort ? -1 : 1;
})), z = /* @__PURE__ */ s(((e, t) => {
	var { applyToDefaults: n, assert: r, clone: i } = E(), a = fe(), o = R(), s = P(), c = I(), l = A(), u = j(), d = M(), f = { renameDefaults: {
		alias: !1,
		multiple: !1,
		override: !1
	} };
	t.exports = o.extend({
		type: "_keys",
		properties: { typeof: "object" },
		flags: { unknown: { default: void 0 } },
		terms: {
			dependencies: { init: null },
			keys: {
				init: null,
				manifest: { mapped: {
					from: "schema",
					to: "key"
				} }
			},
			patterns: { init: null },
			renames: { init: null }
		},
		args(e, t) {
			return e.keys(t);
		},
		jsonSchema(e, t, n, r) {
			if (t.type = "object", e.$_terms.keys) {
				t.properties = {};
				let i = [];
				for (let a of e.$_terms.keys) {
					let e = a.schema.$_jsonSchema(n, r);
					t.properties[a.key] = e, (a.schema._flags.presence === "required" || n === "output" && a.schema._flags.default !== void 0) && i.push(a.key);
				}
				i.length && (t.required = i.sort());
			}
			if (e.$_terms.patterns) {
				let i = {};
				for (let a of e.$_terms.patterns) a.regex ? i[a.regex.source] = a.rule.$_jsonSchema(n, r) : a.schema.type === "any" ? t.additionalProperties = a.rule.$_jsonSchema(n, r) : i[".*"] = a.rule.$_jsonSchema(n, r);
				Object.keys(i).length && (t.patternProperties = i);
			}
			return t.additionalProperties === void 0 && (e._flags.unknown === !0 || e._flags.unknown === void 0 && !e.$_terms.keys && !e.$_terms.patterns && !e._flags.only || (t.additionalProperties = !1)), t;
		},
		validate(e, { schema: t, error: n, state: r, prefs: i }) {
			if (!e || typeof e !== t.$_property("typeof") || Array.isArray(e)) return {
				value: e,
				errors: n("object.base", { type: t.$_property("typeof") })
			};
			if (!t.$_terms.renames && !t.$_terms.dependencies && !t.$_terms.keys && !t.$_terms.patterns && !t.$_terms.externals) return;
			e = f.clone(e, i);
			let a = [];
			if (t.$_terms.renames && !f.rename(t, e, r, i, a) || !t.$_terms.keys && !t.$_terms.patterns && !t.$_terms.dependencies) return {
				value: e,
				errors: a
			};
			let o = new Set(Object.keys(e));
			if (t.$_terms.keys) {
				let n = [e, ...r.ancestors];
				for (let s of t.$_terms.keys) {
					let t = s.key, c = e[t];
					o.delete(t);
					let l = r.localize([...r.path, t], n, s), u = s.schema.$_validate(c, l, i);
					if (u.errors) {
						if (i.abortEarly) return {
							value: e,
							errors: u.errors
						};
						u.value !== void 0 && (e[t] = u.value), a.push(...u.errors);
					} else s.schema._flags.result === "strip" || u.value === void 0 && c !== void 0 ? delete e[t] : u.value !== void 0 && (e[t] = u.value);
				}
			}
			if (o.size || t._flags._hasPatternMatch) {
				let n = f.unknown(t, e, o, a, r, i);
				if (n) return n;
			}
			if (t.$_terms.dependencies) for (let n of t.$_terms.dependencies) {
				if (n.key !== null && f.isPresent(n.options)(n.key.resolve(e, r, i, null, { shadow: !1 })) === !1) continue;
				let o = f.dependencies[n.rel](t, n, e, r, i);
				if (o) {
					let n = t.$_createError(o.code, e, o.context, r, i);
					if (i.abortEarly) return {
						value: e,
						errors: n
					};
					a.push(n);
				}
			}
			return {
				value: e,
				errors: a
			};
		},
		rules: {
			and: { method(...e) {
				return s.verifyFlat(e, "and"), f.dependency(this, "and", null, e);
			} },
			append: { method(e) {
				return e == null || Object.keys(e).length === 0 ? this : this.keys(e);
			} },
			assert: {
				method(e, t, n) {
					d.isTemplate(e) || (e = c.ref(e)), r(n === void 0 || typeof n == "string", "Message must be a string"), t = this.$_compile(t, { appendPath: !0 });
					let i = this.$_addRule({
						name: "assert",
						args: {
							subject: e,
							schema: t,
							message: n
						}
					});
					return i.$_mutateRegister(e), i.$_mutateRegister(t), i;
				},
				validate(e, { error: t, prefs: n, state: r }, { subject: i, schema: a, message: o }) {
					let s = i.resolve(e, r, n), c = u.isRef(i) ? i.absolute(r) : [];
					return a.$_match(s, r.localize(c, [e, ...r.ancestors], a), n) ? e : t("object.assert", {
						subject: i,
						message: o
					});
				},
				args: [
					"subject",
					"schema",
					"message"
				],
				multi: !0
			},
			instance: {
				method(e, t) {
					return r(typeof e == "function", "constructor must be a function"), t ||= e.name, this.$_addRule({
						name: "instance",
						args: {
							constructor: e,
							name: t
						}
					});
				},
				validate(e, t, { constructor: n, name: r }) {
					return e instanceof n ? e : t.error("object.instance", {
						type: r,
						value: e
					});
				},
				args: ["constructor", "name"]
			},
			keys: { method(e) {
				r(e === void 0 || typeof e == "object", "Object schema must be a valid object"), r(!s.isSchema(e), "Object schema cannot be a joi schema");
				let t = this.clone();
				if (!e) t.$_terms.keys = null;
				else if (!Object.keys(e).length) t.$_terms.keys = new f.Keys();
				else {
					t.$_terms.keys = t.$_terms.keys ? t.$_terms.keys.filter((t) => !e.hasOwnProperty(t.key)) : new f.Keys();
					for (let n in e) s.tryWithPath(() => t.$_terms.keys.push({
						key: n,
						schema: this.$_compile(e[n])
					}), n);
				}
				return t.$_mutateRebuild();
			} },
			length: {
				method(e) {
					return this.$_addRule({
						name: "length",
						args: { limit: e },
						operator: "="
					});
				},
				validate(e, t, { limit: n }, { name: r, operator: i, args: a }) {
					return s.compare(Object.keys(e).length, n, i) ? e : t.error("object." + r, {
						limit: a.limit,
						value: e
					});
				},
				jsonSchema(e, t) {
					return t.minProperties = e.args.limit, t.maxProperties = e.args.limit, t;
				},
				args: [{
					name: "limit",
					ref: !0,
					assert: s.limit,
					message: "must be a positive integer"
				}]
			},
			max: {
				method(e) {
					return this.$_addRule({
						name: "max",
						method: "length",
						args: { limit: e },
						operator: "<="
					});
				},
				jsonSchema(e, t) {
					return t.maxProperties = e.args.limit, t;
				}
			},
			min: {
				method(e) {
					return this.$_addRule({
						name: "min",
						method: "length",
						args: { limit: e },
						operator: ">="
					});
				},
				jsonSchema(e, t) {
					return t.minProperties = e.args.limit, t;
				}
			},
			nand: { method(...e) {
				return s.verifyFlat(e, "nand"), f.dependency(this, "nand", null, e);
			} },
			or: { method(...e) {
				return s.verifyFlat(e, "or"), f.dependency(this, "or", null, e);
			} },
			oxor: { method(...e) {
				return f.dependency(this, "oxor", null, e);
			} },
			pattern: { method(e, t, n = {}) {
				let i = e instanceof RegExp;
				i || (e = this.$_compile(e, { appendPath: !0 })), r(t !== void 0, "Invalid rule"), s.assertOptions(n, ["fallthrough", "matches"]), i && r(!e.flags.includes("g") && !e.flags.includes("y"), "pattern should not use global or sticky mode"), t = this.$_compile(t, { appendPath: !0 });
				let a = this.clone();
				a.$_terms.patterns = a.$_terms.patterns || [];
				let o = {
					[i ? "regex" : "schema"]: e,
					rule: t
				};
				return n.matches && (o.matches = this.$_compile(n.matches), o.matches.type !== "array" && (o.matches = o.matches.$_root.array().items(o.matches)), a.$_mutateRegister(o.matches), a.$_setFlag("_hasPatternMatch", !0, { clone: !1 })), n.fallthrough && (o.fallthrough = !0), a.$_terms.patterns.push(o), a.$_mutateRegister(t), a;
			} },
			ref: {
				method() {
					return this.$_addRule("ref");
				},
				validate(e, t) {
					return u.isRef(e) ? e : t.error("object.refType", { value: e });
				}
			},
			regex: {
				method() {
					return this.$_addRule("regex");
				},
				validate(e, t) {
					return e instanceof RegExp ? e : t.error("object.regex", { value: e });
				}
			},
			rename: { method(e, t, i = {}) {
				r(typeof e == "string" || e instanceof RegExp, "Rename missing the from argument"), r(typeof t == "string" || t instanceof d, "Invalid rename to argument"), r(t !== e, "Cannot rename key to same name:", e), s.assertOptions(i, [
					"alias",
					"ignoreUndefined",
					"override",
					"multiple"
				]);
				let a = this.clone();
				a.$_terms.renames = a.$_terms.renames || [];
				for (let t of a.$_terms.renames) r(t.from !== e, "Cannot rename the same key multiple times");
				return t instanceof d && a.$_mutateRegister(t), a.$_terms.renames.push({
					from: e,
					to: t,
					options: n(f.renameDefaults, i)
				}), a;
			} },
			schema: {
				method(e = "any") {
					return this.$_addRule({
						name: "schema",
						args: { type: e }
					});
				},
				validate(e, t, { type: n }) {
					return s.isSchema(e) && (n === "any" || e.type === n) ? e : t.error("object.schema", { type: n });
				}
			},
			unknown: { method(e) {
				return this.$_setFlag("unknown", e !== !1);
			} },
			with: { method(e, t, n = {}) {
				return f.dependency(this, "with", e, t, n);
			} },
			without: { method(e, t, n = {}) {
				return f.dependency(this, "without", e, t, n);
			} },
			xor: { method(...e) {
				return s.verifyFlat(e, "xor"), f.dependency(this, "xor", null, e);
			} }
		},
		overrides: {
			default(e, t) {
				return e === void 0 && (e = s.symbols.deepDefault), this.$_parent("default", e, t);
			},
			isAsync() {
				if (this.$_terms.externals?.length) return !0;
				if (this.$_terms.keys?.length) {
					for (let e of this.$_terms.keys) if (e.schema.isAsync()) return !0;
				}
				if (this.$_terms.patterns?.length) {
					for (let e of this.$_terms.patterns) if (e.rule.isAsync()) return !0;
				}
				return !1;
			}
		},
		rebuild(e) {
			if (e.$_terms.keys) {
				let t = new a.Sorter();
				for (let n of e.$_terms.keys) s.tryWithPath(() => t.add(n, {
					after: n.schema.$_rootReferences(),
					group: n.key
				}), n.key);
				e.$_terms.keys = new f.Keys(...t.nodes);
			}
		},
		manifest: { build(e, t) {
			if (t.keys && (e = e.keys(t.keys)), t.dependencies) for (let { rel: n, key: r = null, peers: i, options: a } of t.dependencies) e = f.dependency(e, n, r, i, a);
			if (t.patterns) for (let { regex: n, schema: r, rule: i, fallthrough: a, matches: o } of t.patterns) e = e.pattern(n || r, i, {
				fallthrough: a,
				matches: o
			});
			if (t.renames) for (let { from: n, to: r, options: i } of t.renames) e = e.rename(n, r, i);
			return e;
		} },
		messages: {
			"object.and": "{{#label}} contains {{#presentWithLabels}} without its required peers {{#missingWithLabels}}",
			"object.assert": "{{#label}} is invalid because {if(#subject.key, `\"` + #subject.key + `\" failed to ` + (#message || \"pass the assertion test\"), #message || \"the assertion failed\")}",
			"object.base": "{{#label}} must be of type {{#type}}",
			"object.instance": "{{#label}} must be an instance of {{:#type}}",
			"object.length": "{{#label}} must have {{#limit}} key{if(#limit == 1, \"\", \"s\")}",
			"object.max": "{{#label}} must have less than or equal to {{#limit}} key{if(#limit == 1, \"\", \"s\")}",
			"object.min": "{{#label}} must have at least {{#limit}} key{if(#limit == 1, \"\", \"s\")}",
			"object.missing": "{{#label}} must contain at least one of {{#peersWithLabels}}",
			"object.nand": "{{:#mainWithLabel}} must not exist simultaneously with {{#peersWithLabels}}",
			"object.oxor": "{{#label}} contains a conflict between optional exclusive peers {{#peersWithLabels}}",
			"object.pattern.match": "{{#label}} keys failed to match pattern requirements",
			"object.refType": "{{#label}} must be a Joi reference",
			"object.regex": "{{#label}} must be a RegExp object",
			"object.rename.multiple": "{{#label}} cannot rename {{:#from}} because multiple renames are disabled and another key was already renamed to {{:#to}}",
			"object.rename.override": "{{#label}} cannot rename {{:#from}} because override is disabled and target {{:#to}} exists",
			"object.schema": "{{#label}} must be a Joi schema of {{#type}} type",
			"object.unknown": "{{#label}} is not allowed",
			"object.with": "{{:#mainWithLabel}} missing required peer {{:#peerWithLabel}}",
			"object.without": "{{:#mainWithLabel}} conflict with forbidden peer {{:#peerWithLabel}}",
			"object.xor": "{{#label}} contains a conflict between exclusive peers {{#peersWithLabels}}"
		}
	}), f.clone = function(e, t) {
		if (typeof e == "object") {
			if (t.nonEnumerables) return i(e, { shallow: !0 });
			let n = Object.create(Object.getPrototypeOf(e));
			return Object.assign(n, e), n;
		}
		let n = function(...t) {
			return e.apply(this, t);
		};
		return n.prototype = i(e.prototype), Object.defineProperty(n, "name", {
			value: e.name,
			writable: !1
		}), Object.defineProperty(n, "length", {
			value: e.length,
			writable: !1
		}), Object.assign(n, e), n;
	}, f.dependency = function(e, t, n, i, a) {
		r(n === null || typeof n == "string", t, "key must be a strings"), a ||= i.length > 1 && typeof i[i.length - 1] == "object" ? i.pop() : {}, s.assertOptions(a, ["separator", "isPresent"]), i = [].concat(i);
		let o = s.default(a.separator, "."), l = [];
		for (let e of i) r(typeof e == "string", t, "peers must be strings"), l.push(c.ref(e, {
			separator: o,
			ancestor: 0,
			prefix: !1
		}));
		n !== null && (n = c.ref(n, {
			separator: o,
			ancestor: 0,
			prefix: !1
		}));
		let u = e.clone();
		return u.$_terms.dependencies = u.$_terms.dependencies || [], u.$_terms.dependencies.push(new f.Dependency(t, n, l, i, a)), u;
	}, f.dependencies = {
		and(e, t, n, r, i) {
			let a = [], o = [], s = t.peers.length, c = f.isPresent(t.options);
			for (let e of t.peers) c(e.resolve(n, r, i, null, { shadow: !1 })) === !1 ? a.push(e.key) : o.push(e.key);
			if (a.length !== s && o.length !== s) return {
				code: "object.and",
				context: {
					present: o,
					presentWithLabels: f.keysToLabels(e, o),
					missing: a,
					missingWithLabels: f.keysToLabels(e, a)
				}
			};
		},
		nand(e, t, n, r, i) {
			let a = [], o = f.isPresent(t.options);
			for (let e of t.peers) o(e.resolve(n, r, i, null, { shadow: !1 })) && a.push(e.key);
			if (a.length !== t.peers.length) return;
			let s = t.paths[0], c = t.paths.slice(1);
			return {
				code: "object.nand",
				context: {
					main: s,
					mainWithLabel: f.keysToLabels(e, s),
					peers: c,
					peersWithLabels: f.keysToLabels(e, c)
				}
			};
		},
		or(e, t, n, r, i) {
			let a = f.isPresent(t.options);
			for (let e of t.peers) if (a(e.resolve(n, r, i, null, { shadow: !1 }))) return;
			return {
				code: "object.missing",
				context: {
					peers: t.paths,
					peersWithLabels: f.keysToLabels(e, t.paths)
				}
			};
		},
		oxor(e, t, n, r, i) {
			let a = [], o = f.isPresent(t.options);
			for (let e of t.peers) o(e.resolve(n, r, i, null, { shadow: !1 })) && a.push(e.key);
			if (!a.length || a.length === 1) return;
			let s = {
				peers: t.paths,
				peersWithLabels: f.keysToLabels(e, t.paths)
			};
			return s.present = a, s.presentWithLabels = f.keysToLabels(e, a), {
				code: "object.oxor",
				context: s
			};
		},
		with(e, t, n, r, i) {
			let a = f.isPresent(t.options);
			for (let o of t.peers) if (a(o.resolve(n, r, i, null, { shadow: !1 })) === !1) return {
				code: "object.with",
				context: {
					main: t.key.key,
					mainWithLabel: f.keysToLabels(e, t.key.key),
					peer: o.key,
					peerWithLabel: f.keysToLabels(e, o.key)
				}
			};
		},
		without(e, t, n, r, i) {
			let a = f.isPresent(t.options);
			for (let o of t.peers) if (a(o.resolve(n, r, i, null, { shadow: !1 }))) return {
				code: "object.without",
				context: {
					main: t.key.key,
					mainWithLabel: f.keysToLabels(e, t.key.key),
					peer: o.key,
					peerWithLabel: f.keysToLabels(e, o.key)
				}
			};
		},
		xor(e, t, n, r, i) {
			let a = [], o = f.isPresent(t.options);
			for (let e of t.peers) o(e.resolve(n, r, i, null, { shadow: !1 })) && a.push(e.key);
			if (a.length === 1) return;
			let s = {
				peers: t.paths,
				peersWithLabels: f.keysToLabels(e, t.paths)
			};
			return a.length === 0 ? {
				code: "object.missing",
				context: s
			} : (s.present = a, s.presentWithLabels = f.keysToLabels(e, a), {
				code: "object.xor",
				context: s
			});
		}
	}, f.keysToLabels = function(e, t) {
		return Array.isArray(t) ? t.map((t) => e.$_mapLabels(t)) : e.$_mapLabels(t);
	}, f.isPresent = function(e) {
		return typeof e.isPresent == "function" ? e.isPresent : (e) => e !== void 0;
	}, f.rename = function(e, t, n, r, i) {
		let a = {};
		for (let o of e.$_terms.renames) {
			let s = [], c = typeof o.from != "string";
			if (!c) Object.prototype.hasOwnProperty.call(t, o.from) && (t[o.from] !== void 0 || !o.options.ignoreUndefined) && s.push(o);
			else for (let e in t) {
				if (t[e] === void 0 && o.options.ignoreUndefined || e === o.to) continue;
				let n = o.from.exec(e);
				n && s.push({
					from: e,
					to: o.to,
					match: n
				});
			}
			for (let l of s) {
				let s = l.from, u = l.to;
				if (u instanceof d && (u = u.render(t, n, r, l.match)), s !== u) {
					if (!o.options.multiple && a[u] && (i.push(e.$_createError("object.rename.multiple", t, {
						from: s,
						to: u,
						pattern: c
					}, n, r)), r.abortEarly) || Object.prototype.hasOwnProperty.call(t, u) && !o.options.override && !a[u] && (i.push(e.$_createError("object.rename.override", t, {
						from: s,
						to: u,
						pattern: c
					}, n, r)), r.abortEarly)) return !1;
					t[s] === void 0 ? delete t[u] : t[u] = t[s], a[u] = !0, o.options.alias || delete t[s];
				}
			}
		}
		return !0;
	}, f.unknown = function(e, t, n, r, i, a) {
		if (e.$_terms.patterns) {
			let o = !1, s = e.$_terms.patterns.map((e) => {
				if (e.matches) return o = !0, [];
			}), c = [t, ...i.ancestors];
			for (let o of n) {
				let l = t[o], u = [...i.path, o];
				for (let d = 0; d < e.$_terms.patterns.length; ++d) {
					let f = e.$_terms.patterns[d];
					if (f.regex) {
						let e = f.regex.test(o);
						if (i.mainstay.tracer.debug(i, "rule", `pattern.${d}`, e ? "pass" : "error"), !e) continue;
					} else if (!f.schema.$_match(o, i.nest(f.schema, `pattern.${d}`), a)) continue;
					n.delete(o);
					let p = i.localize(u, c, {
						schema: f.rule,
						key: o
					}), m = f.rule.$_validate(l, p, a);
					if (m.errors) {
						if (a.abortEarly) return {
							value: t,
							errors: m.errors
						};
						r.push(...m.errors);
					}
					if (f.matches && s[d].push(o), t[o] = m.value, !f.fallthrough) break;
				}
			}
			if (o) for (let n = 0; n < s.length; ++n) {
				let o = s[n];
				if (!o) continue;
				let u = e.$_terms.patterns[n].matches, d = i.localize(i.path, c, u), f = u.$_validate(o, d, a);
				if (f.errors) {
					let n = l.details(f.errors, { override: !1 });
					n.matches = o;
					let s = e.$_createError("object.pattern.match", t, n, i, a);
					if (a.abortEarly) return {
						value: t,
						errors: s
					};
					r.push(s);
				}
			}
		}
		if (!(!n.size || !e.$_terms.keys && !e.$_terms.patterns)) {
			if (a.stripUnknown && e._flags.unknown === void 0 || a.skipFunctions) {
				let e = a.stripUnknown ? a.stripUnknown === !0 ? !0 : !!a.stripUnknown.objects : !1;
				for (let r of n) e ? (delete t[r], n.delete(r)) : typeof t[r] == "function" && n.delete(r);
			}
			if (!s.default(e._flags.unknown, a.allowUnknown)) for (let o of n) {
				let n = i.localize([...i.path, o], []), s = e.$_createError("object.unknown", t[o], { child: o }, n, a, { flags: !1 });
				if (a.abortEarly) return {
					value: t,
					errors: s
				};
				r.push(s);
			}
		}
	}, f.Dependency = class {
		constructor(e, t, n, r, i) {
			this.rel = e, this.key = t, this.peers = n, this.paths = r, this.options = i;
		}
		describe() {
			let e = {
				rel: this.rel,
				peers: this.paths
			};
			return this.key !== null && (e.key = this.key.key), this.peers[0].separator !== "." && (e.options = {
				...e.options,
				separator: this.peers[0].separator
			}), this.options.isPresent && (e.options = {
				...e.options,
				isPresent: this.options.isPresent
			}), e;
		}
	}, f.Keys = class extends Array {
		concat(e) {
			let t = this.slice(), n = /* @__PURE__ */ new Map();
			for (let e = 0; e < t.length; ++e) n.set(t[e].key, e);
			for (let r of e) {
				let e = r.key, i = n.get(e);
				i === void 0 ? t.push(r) : t[i] = {
					key: e,
					schema: t[i].schema.concat(r.schema)
				};
			}
			return t;
		}
	};
})), pe = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E();
	t.exports = z().extend({
		type: "function",
		properties: { typeof: "function" },
		rules: {
			arity: {
				method(e) {
					return n(Number.isSafeInteger(e) && e >= 0, "n must be a positive integer"), this.$_addRule({
						name: "arity",
						args: { n: e }
					});
				},
				validate(e, t, { n }) {
					return e.length === n ? e : t.error("function.arity", { n });
				}
			},
			class: {
				method() {
					return this.$_addRule("class");
				},
				validate(e, t) {
					return /^\s*class\s/.test(e.toString()) ? e : t.error("function.class", { value: e });
				}
			},
			minArity: {
				method(e) {
					return n(Number.isSafeInteger(e) && e > 0, "n must be a strict positive integer"), this.$_addRule({
						name: "minArity",
						args: { n: e }
					});
				},
				validate(e, t, { n }) {
					return e.length >= n ? e : t.error("function.minArity", { n });
				}
			},
			maxArity: {
				method(e) {
					return n(Number.isSafeInteger(e) && e >= 0, "n must be a positive integer"), this.$_addRule({
						name: "maxArity",
						args: { n: e }
					});
				},
				validate(e, t, { n }) {
					return e.length <= n ? e : t.error("function.maxArity", { n });
				}
			}
		},
		messages: {
			"function.arity": "{{#label}} must have an arity of {{#n}}",
			"function.class": "{{#label}} must be a class",
			"function.maxArity": "{{#label}} must have an arity lesser or equal to {{#n}}",
			"function.minArity": "{{#label}} must have an arity greater or equal to {{#n}}"
		}
	});
})), me = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = R(), i = P(), a = I(), o = A(), s = {};
	t.exports = r.extend({
		type: "link",
		properties: { schemaChain: !0 },
		terms: { link: {
			init: null,
			manifest: "single",
			register: !1
		} },
		args(e, t) {
			return e.ref(t);
		},
		jsonSchema(e, t, n, r) {
			if (!e.$_terms.link) return t;
			let { ref: i } = e.$_terms.link[0];
			return i.ancestor === "root" || i.ancestor > 0 ? (t.$ref = `#/${i.path.map((e) => `properties/${e}`).join("/")}`, t) : (i.path.length === 1 ? t.$ref = `#/$defs/${i.path[0]}` : t.$ref = `#/${i.path.slice(1).map((e) => `properties/${e}`).join("/")}`, t);
		},
		validate(e, { schema: t, state: r, prefs: i, error: a }) {
			n(t.$_terms.link, "Uninitialized link schema");
			let o = t._flags.maxRecursion;
			if (o !== void 0 && r.schemas.filter((e) => e.schema === t).length > o) return {
				value: e,
				errors: a("link.maxRecursion", { limit: o })
			};
			let c = s.generate(t, e, r, i), l = t.$_terms.link[0].ref;
			try {
				return c.$_validate(e, r.nest(c, `link:${l.display}:${c.type}`), i);
			} catch (t) {
				if (!(t instanceof RangeError)) throw t;
				return {
					value: e,
					errors: a("link.depth")
				};
			}
		},
		generate(e, t, n, r) {
			return s.generate(e, t, n, r);
		},
		rules: {
			ref: { method(e) {
				n(!this.$_terms.link, "Cannot reinitialize schema"), e = a.ref(e), n(e.type === "value" || e.type === "local", "Invalid reference type:", e.type), n(e.type === "local" || e.ancestor === "root" || e.ancestor > 0, "Link cannot reference itself");
				let t = this.clone();
				return t.$_terms.link = [{ ref: e }], t;
			} },
			relative: { method(e = !0) {
				return this.$_setFlag("relative", e);
			} },
			maxRecursion: { method(e) {
				return n(Number.isSafeInteger(e) && e >= 1, "limit must be a positive integer"), this.$_setFlag("maxRecursion", e);
			} }
		},
		messages: {
			"link.depth": "{{#label}} exceeds maximum recursion depth supported by the runtime",
			"link.maxRecursion": "{{#label}} exceeds maximum recursion depth of {{#limit}}"
		},
		overrides: { concat(e) {
			n(this.$_terms.link, "Uninitialized link schema"), n(i.isSchema(e), "Invalid schema object"), n(e.type !== "link", "Cannot merge type link with another link");
			let t = this.clone();
			return t.$_terms.whens || (t.$_terms.whens = []), t.$_terms.whens.push({ concat: e }), t.$_mutateRebuild();
		} },
		manifest: { build(e, t) {
			return n(t.link, "Invalid link description missing link"), e.ref(t.link);
		} }
	}), s.generate = function(e, t, n, r) {
		let i = n.mainstay.links.get(e);
		if (i) return i._generate(t, n, r).schema;
		let a = e.$_terms.link[0].ref, { perspective: o, path: c } = s.perspective(a, n);
		s.assert(o, "which is outside of schema boundaries", a, e, n, r);
		try {
			i = c.length ? o.$_reach(c) : o;
		} catch {
			s.assert(!1, "to non-existing schema", a, e, n, r);
		}
		return s.assert(i.type !== "link", "which is another link", a, e, n, r), e._flags.relative || n.mainstay.links.set(e, i), i._generate(t, n, r).schema;
	}, s.perspective = function(e, t) {
		if (e.type === "local") {
			for (let { schema: n, key: r } of t.schemas) {
				if ((n._flags.id || r) === e.path[0]) return {
					perspective: n,
					path: e.path.slice(1)
				};
				if (n.$_terms.shared) {
					for (let t of n.$_terms.shared) if (t._flags.id === e.path[0]) return {
						perspective: t,
						path: e.path.slice(1)
					};
				}
			}
			return {
				perspective: null,
				path: null
			};
		}
		return e.ancestor === "root" ? {
			perspective: t.schemas[t.schemas.length - 1].schema,
			path: e.path
		} : {
			perspective: t.schemas[e.ancestor] && t.schemas[e.ancestor].schema,
			path: e.path
		};
	}, s.assert = function(e, t, r, i, a, s) {
		e || n(!1, `"${o.label(i._flags, a, s)}" contains link reference "${r.display}" ${t}`);
	};
})), he = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = R(), i = P(), a = {
		numberRx: /^\s*[+-]?(?:(?:\d+(?:\.\d*)?)|(?:\.\d+))(?:e([+-]?\d+))?\s*$/i,
		precisionRx: /(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/,
		exponentialPartRegex: /[eE][+-]?\d+$/,
		leadingSignAndZerosRegex: /^[+-]?(0*)?/,
		dotRegex: /\./,
		trailingZerosRegex: /0+$/,
		decimalPlaces(e) {
			let t = e.toString(), n = t.indexOf("."), r = t.indexOf("e");
			return (n < 0 ? 0 : (r < 0 ? t.length : r) - n - 1) + (r < 0 ? 0 : Math.max(0, -parseInt(t.slice(r + 1))));
		}
	};
	t.exports = r.extend({
		type: "number",
		flags: { unsafe: { default: !1 } },
		coerce: {
			from: "string",
			method(e, { schema: t, error: n }) {
				if (!e.match(a.numberRx)) return;
				e = e.trim();
				let r = { value: parseFloat(e) };
				if (r.value === 0 && (r.value = 0), !t._flags.unsafe) if (e.match(/e/i)) {
					if (a.extractSignificantDigits(e) !== a.extractSignificantDigits(String(r.value))) return r.errors = n("number.unsafe"), r;
				} else {
					let t = r.value.toString();
					if (t.match(/e/i)) return r;
					if (t !== a.normalizeDecimal(e)) return r.errors = n("number.unsafe"), r;
				}
				return r;
			}
		},
		validate(e, { schema: t, error: n, prefs: r }) {
			if (e === Infinity || e === -Infinity) return {
				value: e,
				errors: n("number.infinity")
			};
			if (!i.isNumber(e)) return {
				value: e,
				errors: n("number.base")
			};
			let a = { value: e };
			if (r.convert) {
				let e = t.$_getRule("precision");
				if (e) {
					let t = 10 ** e.args.limit;
					a.value = Math.round(a.value * t) / t;
				}
			}
			return a.value === 0 && (a.value = 0), !t._flags.unsafe && (e > 2 ** 53 - 1 || e < -(2 ** 53 - 1)) && (a.errors = n("number.unsafe")), a;
		},
		rules: {
			compare: {
				method: !1,
				validate(e, t, { limit: n }, { name: r, operator: a, args: o }) {
					return i.compare(e, n, a) ? e : t.error("number." + r, {
						limit: o.limit,
						value: e
					});
				},
				args: [{
					name: "limit",
					ref: !0,
					assert: i.isNumber,
					message: "must be a number"
				}]
			},
			greater: {
				method(e) {
					return this.$_addRule({
						name: "greater",
						method: "compare",
						args: { limit: e },
						operator: ">"
					});
				},
				jsonSchema(e, t) {
					return t.exclusiveMinimum = e.args.limit, t;
				}
			},
			integer: {
				method() {
					return this.$_addRule("integer");
				},
				validate(e, t) {
					return Math.trunc(e) - e === 0 ? e : t.error("number.integer");
				},
				jsonSchema(e, t) {
					return t.type = "integer", t;
				}
			},
			less: {
				method(e) {
					return this.$_addRule({
						name: "less",
						method: "compare",
						args: { limit: e },
						operator: "<"
					});
				},
				jsonSchema(e, t) {
					return t.exclusiveMaximum = e.args.limit, t;
				}
			},
			max: {
				method(e) {
					return this.$_addRule({
						name: "max",
						method: "compare",
						args: { limit: e },
						operator: "<="
					});
				},
				jsonSchema(e, t) {
					return t.maximum = e.args.limit, t;
				}
			},
			min: {
				method(e) {
					return this.$_addRule({
						name: "min",
						method: "compare",
						args: { limit: e },
						operator: ">="
					});
				},
				jsonSchema(e, t) {
					return t.minimum = e.args.limit, t;
				}
			},
			multiple: {
				method(e) {
					let t = typeof e == "number" ? a.decimalPlaces(e) : null, n = 10 ** t;
					return this.$_addRule({
						name: "multiple",
						args: {
							base: e,
							baseDecimalPlace: t,
							pfactor: n
						}
					});
				},
				validate(e, t, { base: n, baseDecimalPlace: r, pfactor: i }, o) {
					return a.decimalPlaces(e) > r ? t.error("number.multiple", {
						multiple: o.args.base,
						value: e
					}) : Math.round(i * e) % Math.round(i * n) === 0 ? e : t.error("number.multiple", {
						multiple: o.args.base,
						value: e
					});
				},
				jsonSchema(e, t) {
					return t.multipleOf = e.args.base, t;
				},
				args: [
					{
						name: "base",
						ref: !0,
						assert: (e) => typeof e == "number" && isFinite(e) && e > 0,
						message: "must be a positive number"
					},
					"baseDecimalPlace",
					"pfactor"
				],
				multi: !0
			},
			negative: { method() {
				return this.sign("negative");
			} },
			port: {
				method() {
					return this.$_addRule("port");
				},
				validate(e, t) {
					return Number.isSafeInteger(e) && e >= 0 && e <= 65535 ? e : t.error("number.port");
				},
				jsonSchema(e, t) {
					return t.type = "integer", t.minimum = 0, t.maximum = 65535, t;
				}
			},
			positive: { method() {
				return this.sign("positive");
			} },
			precision: {
				method(e) {
					return n(Number.isSafeInteger(e), "limit must be an integer"), this.$_addRule({
						name: "precision",
						args: { limit: e }
					});
				},
				validate(e, t, { limit: n }) {
					let r = e.toString().match(a.precisionRx);
					return Math.max((r[1] ? r[1].length : 0) - (r[2] ? parseInt(r[2], 10) : 0), 0) <= n ? e : t.error("number.precision", {
						limit: n,
						value: e
					});
				},
				convert: !0
			},
			sign: {
				method(e) {
					return n(["negative", "positive"].includes(e), "Invalid sign", e), this.$_addRule({
						name: "sign",
						args: { sign: e }
					});
				},
				validate(e, t, { sign: n }) {
					return n === "negative" && e < 0 || n === "positive" && e > 0 ? e : t.error(`number.${n}`);
				},
				jsonSchema(e, t) {
					return e.args.sign === "positive" ? t.exclusiveMinimum = 0 : t.exclusiveMaximum = 0, t;
				}
			},
			unsafe: { method(e = !0) {
				return n(typeof e == "boolean", "enabled must be a boolean"), this.$_setFlag("unsafe", e);
			} }
		},
		cast: { string: {
			from: (e) => typeof e == "number",
			to(e, t) {
				return e.toString();
			}
		} },
		messages: {
			"number.base": "{{#label}} must be a number",
			"number.greater": "{{#label}} must be greater than {{#limit}}",
			"number.infinity": "{{#label}} cannot be infinity",
			"number.integer": "{{#label}} must be an integer",
			"number.less": "{{#label}} must be less than {{#limit}}",
			"number.max": "{{#label}} must be less than or equal to {{#limit}}",
			"number.min": "{{#label}} must be greater than or equal to {{#limit}}",
			"number.multiple": "{{#label}} must be a multiple of {{#multiple}}",
			"number.negative": "{{#label}} must be a negative number",
			"number.port": "{{#label}} must be a valid port",
			"number.positive": "{{#label}} must be a positive number",
			"number.precision": "{{#label}} must have no more than {{#limit}} decimal places",
			"number.unsafe": "{{#label}} must be a safe number"
		}
	}), a.extractSignificantDigits = function(e) {
		return e.replace(a.exponentialPartRegex, "").replace(a.dotRegex, "").replace(a.trailingZerosRegex, "").replace(a.leadingSignAndZerosRegex, "");
	}, a.normalizeDecimal = function(e) {
		return e = e.replace(/^\+/, "").replace(/\.0*$/, "").replace(/^(-?)\.([^\.]*)$/, "$10.$2").replace(/^(-?)0+([0-9])/, "$1$2"), e.includes(".") && e.endsWith("0") && (e = e.replace(/0+$/, "")), e === "-0" ? "0" : e;
	};
})), ge = /* @__PURE__ */ s(((e, t) => {
	t.exports = z().extend({
		type: "object",
		cast: { map: {
			from: (e) => e && typeof e == "object",
			to(e, t) {
				return new Map(Object.entries(e));
			}
		} }
	});
}));
//#endregion
//#region ../node_modules/.pnpm/@hapi+address@5.1.1/node_modules/@hapi/address/esm/errors.js
function B(e) {
	return {
		code: e,
		error: V[e]
	};
}
var V, H = o((() => {
	V = {
		EMPTY_STRING: "Address must be a non-empty string",
		FORBIDDEN_UNICODE: "Address contains forbidden Unicode characters",
		MULTIPLE_AT_CHAR: "Address cannot contain more than one @ character",
		MISSING_AT_CHAR: "Address must contain one @ character",
		EMPTY_LOCAL: "Address local part cannot be empty",
		ADDRESS_TOO_LONG: "Address too long",
		LOCAL_TOO_LONG: "Address local part too long",
		EMPTY_LOCAL_SEGMENT: "Address local part contains empty dot-separated segment",
		INVALID_LOCAL_CHARS: "Address local part contains invalid character",
		DOMAIN_NON_EMPTY_STRING: "Domain must be a non-empty string",
		DOMAIN_TOO_LONG: "Domain too long",
		DOMAIN_INVALID_UNICODE_CHARS: "Domain contains forbidden Unicode characters",
		DOMAIN_INVALID_CHARS: "Domain contains invalid character",
		DOMAIN_INVALID_TLDS_CHARS: "Domain contains invalid tld character",
		DOMAIN_SEGMENTS_COUNT: "Domain lacks the minimum required number of segments",
		DOMAIN_SEGMENTS_COUNT_MAX: "Domain contains too many segments",
		DOMAIN_FORBIDDEN_TLDS: "Domain uses forbidden TLD",
		DOMAIN_EMPTY_SEGMENT: "Domain contains empty dot-separated segment",
		DOMAIN_LONG_SEGMENT: "Domain contains dot-separated segment that is too long"
	};
}));
//#endregion
//#region ../node_modules/.pnpm/@hapi+address@5.1.1/node_modules/@hapi/address/esm/domain.js
function U(e) {
	return !!e.allow;
}
function W(e, t = {}) {
	if (!e) return B("DOMAIN_NON_EMPTY_STRING");
	if (typeof e != "string") throw Error("Invalid input: domain must be a string");
	if (e.length > 256) return B("DOMAIN_TOO_LONG");
	if (q.test(e)) {
		if (t.allowUnicode === !1) return B("DOMAIN_INVALID_UNICODE_CHARS");
		e = e.normalize("NFC");
	}
	if (be.test(e)) return B("DOMAIN_INVALID_CHARS");
	e = ve(e), t.allowFullyQualified && e[e.length - 1] === "." && (e = e.slice(0, -1));
	let n = t.minDomainSegments || K, r = e.split(".");
	if (r.length < n) return B("DOMAIN_SEGMENTS_COUNT");
	if (t.maxDomainSegments && r.length > t.maxDomainSegments) return B("DOMAIN_SEGMENTS_COUNT_MAX");
	let i = t.tlds;
	if (i) {
		let e = r[r.length - 1].toLowerCase();
		if (U(i)) {
			if (!i.allow.has(e)) return B("DOMAIN_FORBIDDEN_TLDS");
		} else if (i.deny.has(e)) return B("DOMAIN_FORBIDDEN_TLDS");
	}
	for (let e = 0; e < r.length; ++e) {
		let n = r[e];
		if (!n.length) return B("DOMAIN_EMPTY_SEGMENT");
		if (n.length > 63) return B("DOMAIN_LONG_SEGMENT");
		if (e < r.length - 1) {
			if (t.allowUnderscore) {
				if (!Ce.test(n)) return B("DOMAIN_INVALID_CHARS");
			} else if (!Se.test(n)) return B("DOMAIN_INVALID_CHARS");
		} else if (!xe.test(n)) return B("DOMAIN_INVALID_TLDS_CHARS");
	}
	return null;
}
function _e(e, t) {
	return !W(e, t);
}
function ve(e) {
	e.includes("%") && (e = e.replace(/%/g, "%25"));
	try {
		return new we(`http://${e}`).host;
	} catch {
		return e;
	}
}
function ye(e) {
	if (e) {
		if (typeof e.tlds != "object") throw Error("Invalid options: tlds must be a boolean or an object");
		if (U(e.tlds)) {
			if (!(e.tlds.allow instanceof Set)) throw Error("Invalid options: tlds.allow must be a Set object or true");
			if (e.tlds.deny) throw Error("Invalid options: cannot specify both tlds.allow and tlds.deny lists");
		} else if (!(e.tlds.deny instanceof Set)) throw Error("Invalid options: tlds.deny must be a Set object");
	}
}
var G, K, q, be, xe, Se, Ce, we, Te = o((() => {
	G = /* @__PURE__ */ u(x()), H(), K = 2, q = /[^\x00-\x7f]/, be = /[\x00-\x20@\:\/\\#!\$&\'\(\)\*\+,;=\?]/, xe = /^[a-zA-Z](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?$/, Se = /^[a-zA-Z0-9](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?$/, Ce = /^[a-zA-Z0-9_](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?$/, we = G.URL || URL;
}));
//#endregion
//#region ../node_modules/.pnpm/@hapi+address@5.1.1/node_modules/@hapi/address/esm/email.js
function Ee(e, t) {
	return Oe(e, t);
}
function De(e, t) {
	return !Oe(e, t);
}
function Oe(e, t = {}) {
	if (typeof e != "string") throw Error("Invalid input: email must be a string");
	if (!e) return B("EMPTY_STRING");
	let n = !Me.test(e);
	if (!n) {
		if (t.allowUnicode === !1) return B("FORBIDDEN_UNICODE");
		e = e.normalize("NFC");
	}
	let r = e.split("@");
	if (r.length !== 2) return r.length > 2 ? B("MULTIPLE_AT_CHAR") : B("MISSING_AT_CHAR");
	let [i, a] = r;
	if (!i) return B("EMPTY_LOCAL");
	if (!t.ignoreLength) {
		if (e.length > 254) return B("ADDRESS_TOO_LONG");
		if (J.encode(i).length > 64) return B("LOCAL_TOO_LONG");
	}
	return ke(i, n) || W(a, t);
}
function ke(e, t) {
	let n = e.split(".");
	for (let e of n) {
		if (!e.length) return B("EMPTY_LOCAL_SEGMENT");
		if (t) {
			if (!Y.test(e)) return B("INVALID_LOCAL_CHARS");
			continue;
		}
		for (let t of e) {
			if (Y.test(t)) continue;
			let e = Ae(t);
			if (!Ne.test(e)) return B("INVALID_LOCAL_CHARS");
		}
	}
	return null;
}
function Ae(e) {
	return Array.from(J.encode(e), (e) => String.fromCharCode(e)).join("");
}
var je, Me, J, Y, Ne, Pe = o((() => {
	je = /* @__PURE__ */ u(x()), Te(), H(), Me = /[^\x00-\x7f]/, J = new (je.TextEncoder || TextEncoder)(), Y = /^[\w!#\$%&'\*\+\-/=\?\^`\{\|\}~]+$/, Ne = new RegExp([
		"(?:[\\xc2-\\xdf][\\x80-\\xbf])",
		"(?:\\xe0[\\xa0-\\xbf][\\x80-\\xbf])|(?:[\\xe1-\\xec][\\x80-\\xbf]{2})|(?:\\xed[\\x80-\\x9f][\\x80-\\xbf])|(?:[\\xee-\\xef][\\x80-\\xbf]{2})",
		"(?:\\xf0[\\x90-\\xbf][\\x80-\\xbf]{2})|(?:[\\xf1-\\xf3][\\x80-\\xbf]{3})|(?:\\xf4[\\x80-\\x8f][\\x80-\\xbf]{2})"
	].join("|"));
})), X, Fe, Ie = o((() => {
	b(), X = /* @__PURE__ */ u(m(), 1), x(), x(), v(), x(), S(), f(), x(), C(), x(), Fe = /* @__PURE__ */ u(w(), 1), x(), T(), x(), x(), y(), x(), h(), x(), p(), x();
}));
//#endregion
//#region ../node_modules/.pnpm/@hapi+address@5.1.1/node_modules/@hapi/address/esm/uri.js
function Le() {
	let e = {}, t = "\\dA-Fa-f", n = "[" + t + "]", r = "\\w-\\.~", i = "!\\$&'\\(\\)\\*\\+,;=", a = "%" + t, o = r + a + i + ":@", s = "[" + o + "]";
	e.ipv4address = "(?:(?:0{0,2}\\d|0?[1-9]\\d|1\\d\\d|2[0-4]\\d|25[0-5])\\.){3}(?:0{0,2}\\d|0?[1-9]\\d|1\\d\\d|2[0-4]\\d|25[0-5])";
	let c = n + "{1,4}", l = "(?:" + c + ":[\\dA-Fa-f]{1,4}|" + e.ipv4address + ")", u = "(?:" + c + ":){6}" + l, d = "::(?:" + c + ":){5}" + l, f = "(?:" + c + ")?::(?:[\\dA-Fa-f]{1,4}:){4}" + l, p = "(?:(?:" + c + ":){0,1}[\\dA-Fa-f]{1,4})?::(?:[\\dA-Fa-f]{1,4}:){3}" + l, m = "(?:(?:" + c + ":){0,2}[\\dA-Fa-f]{1,4})?::(?:[\\dA-Fa-f]{1,4}:){2}" + l, h = "(?:(?:" + c + ":){0,3}[\\dA-Fa-f]{1,4})?::[\\dA-Fa-f]{1,4}:" + l, g = "(?:(?:" + c + ":){0,4}[\\dA-Fa-f]{1,4})?::" + l;
	"" + c, "" + c, e.ipv4Cidr = "(?:\\d|[1-2]\\d|3[0-2])", e.ipv6Cidr = "(?:0{0,2}\\d|0?[1-9]\\d|1[01]\\d|12[0-8])", e.ipv6address = "(?:" + u + "|" + d + "|" + f + "|" + p + "|" + m + "|" + h + "|" + g + "|(?:(?:[\\dA-Fa-f]{1,4}:){0,5}[\\dA-Fa-f]{1,4})?::[\\dA-Fa-f]{1,4}|(?:(?:[\\dA-Fa-f]{1,4}:){0,6}[\\dA-Fa-f]{1,4})?::)", e.ipvFuture = "v" + n + "+\\.[\\w-\\.~!\\$&'\\(\\)\\*\\+,;=:]+", e.scheme = "[a-zA-Z][a-zA-Z\\d+-\\.]*", e.schemeRegex = new RegExp(e.scheme);
	let _ = "[" + r + a + i + ":]*", v = "\\[(?:" + e.ipv6address + "|" + e.ipvFuture + ")\\]";
	"" + r + a + i;
	let y = "(?:" + v + "|" + e.ipv4address + "|[\\w-\\.~%\\dA-Fa-f!\\$&'\\(\\)\\*\\+,;=]{1,255})", b = "(?:" + _ + "@)?" + y + "(?::\\d*)?", x = "(?:" + _ + "@)?(" + y + ")(?::\\d*)?", S = s + "*", C = s + "+";
	"" + r + a + i;
	let w = "(?:\\/" + S + ")*", T = "\\/(?:" + C + w + ")?", E = C + w, D = "[\\w-\\.~%\\dA-Fa-f!\\$&'\\(\\)\\*\\+,;=@]+" + w, O = "(?:\\/\\/\\/" + S + w + ")";
	return e.hierPart = "(?:(?:\\/\\/" + b + w + ")|" + T + "|" + E + "|" + O + ")", e.hierPartCapture = "(?:(?:\\/\\/" + x + w + ")|" + T + "|" + E + ")", e.relativeRef = "(?:(?:\\/\\/" + b + w + ")|" + T + "|" + D + "|)", e.relativeRefCapture = "(?:(?:\\/\\/" + x + w + ")|" + T + "|" + D + "|)", e.query = "[" + o + "\\/\\?]*(?=#|$)", e.queryWithSquareBrackets = "[" + o + "\\[\\]\\/\\?]*(?=#|$)", e.fragment = "[" + o + "\\/\\?]*", e;
}
function Re(e) {
	let t = Z, n = "(?:\\?" + (e.allowQuerySquareBrackets ? t.queryWithSquareBrackets : t.query) + ")?(?:#" + t.fragment + ")?", r = e.domain ? t.relativeRefCapture : t.relativeRef;
	if (e.relativeOnly) return ze(r + n);
	let i = "";
	if (e.scheme) {
		(0, X.default)(e.scheme instanceof RegExp || typeof e.scheme == "string" || Array.isArray(e.scheme), "scheme must be a RegExp, String, or Array");
		let n = [].concat(e.scheme);
		(0, X.default)(n.length >= 1, "scheme must have at least 1 scheme specified");
		let r = [];
		for (let e = 0; e < n.length; ++e) {
			let i = n[e];
			(0, X.default)(i instanceof RegExp || typeof i == "string", "scheme at position " + e + " must be a RegExp or String"), i instanceof RegExp ? r.push(i.source.toString()) : ((0, X.default)(t.schemeRegex.test(i), "scheme at position " + e + " must be a valid scheme"), r.push((0, Fe.default)(i)));
		}
		i = r.join("|");
	}
	let a = "(?:" + (i ? "(?:" + i + ")" : t.scheme) + ":" + (e.domain ? t.hierPartCapture : t.hierPart) + ")";
	return ze((e.allowRelative ? "(?:" + a + "|" + r + ")" : a) + n, i);
}
function ze(e, t = null) {
	return e = `(?=.)(?!https?\:/(?:$|[^/]))(?!https?\:///)(?!https?\:[^/])${e}`, {
		raw: e,
		regex: RegExp(`^${e}$`),
		scheme: t
	};
}
function Be(e = {}) {
	return e.scheme || e.allowRelative || e.relativeOnly || e.allowQuerySquareBrackets || e.domain ? Re(e) : Ve;
}
var Z, Q, Ve, He = o((() => {
	Ie(), Z = Le(), Q = {
		v4Cidr: Z.ipv4Cidr,
		v6Cidr: Z.ipv6Cidr,
		ipv4: Z.ipv4address,
		ipv6: Z.ipv6address,
		ipvfuture: Z.ipvFuture
	}, Ve = Re({});
}));
//#endregion
//#region ../node_modules/.pnpm/@hapi+address@5.1.1/node_modules/@hapi/address/esm/ip.js
function Ue(e = {}) {
	let t = e.cidr || "optional";
	(0, X.default)([
		"required",
		"optional",
		"forbidden"
	].includes(t), "options.cidr must be one of required, optional, forbidden"), (0, X.default)(e.version === void 0 || typeof e.version == "string" || Array.isArray(e.version), "options.version must be a string or an array of string");
	let n = e.version || [
		"ipv4",
		"ipv6",
		"ipvfuture"
	];
	Array.isArray(n) || (n = [n]), (0, X.default)(n.length >= 1, "options.version must have at least 1 version specified");
	for (let e of n) (0, X.default)(typeof e == "string" && e === e.toLowerCase(), "Invalid options.version value"), (0, X.default)([
		"ipv4",
		"ipv6",
		"ipvfuture"
	].includes(e), "options.version contains unknown version " + e + " - must be one of ipv4, ipv6, ipvfuture");
	n = Array.from(new Set(n));
	let r = `(?:${n.map((e) => {
		if (t === "forbidden") return Q[e];
		let n = `\\/${e === "ipv4" ? Q.v4Cidr : Q.v6Cidr}`;
		return t === "required" ? `${Q[e]}${n}` : `${Q[e]}(?:${n})?`;
	}).join("|")})`, i = RegExp(`^${r}$`);
	return {
		cidr: t,
		versions: n,
		regex: i,
		raw: r
	};
}
var We = o((() => {
	Ie(), He();
})), Ge = /* @__PURE__ */ c({
	analyzeDomain: () => W,
	analyzeEmail: () => Ee,
	errorCodes: () => V,
	ipRegex: () => Ue,
	isDomainValid: () => _e,
	isEmailValid: () => De,
	uriDecode: () => Ke.uriDecode,
	uriRegex: () => Be,
	validateDomainOptions: () => ye
}), Ke, qe = o((() => {
	Te(), Pe(), H(), We(), He(), Ke = /* @__PURE__ */ u(x());
})), Je = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, escapeRegex: r } = E(), { isDomainValid: i, isEmailValid: a, ipRegex: o, uriRegex: s } = (qe(), d(Ge)), c = x(), l = R(), u = P(), f = {
		tlds: c.tlds instanceof Set ? { tlds: {
			allow: c.tlds,
			deny: null
		} } : !1,
		base64Regex: {
			true: {
				true: /^(?:[\w\-]{2}[\w\-]{2})*(?:[\w\-]{2}==|[\w\-]{3}=)?$/,
				false: /^(?:[A-Za-z0-9+\/]{2}[A-Za-z0-9+\/]{2})*(?:[A-Za-z0-9+\/]{2}==|[A-Za-z0-9+\/]{3}=)?$/
			},
			false: {
				true: /^(?:[\w\-]{2}[\w\-]{2})*(?:[\w\-]{2}(==)?|[\w\-]{3}=?)?$/,
				false: /^(?:[A-Za-z0-9+\/]{2}[A-Za-z0-9+\/]{2})*(?:[A-Za-z0-9+\/]{2}(==)?|[A-Za-z0-9+\/]{3}=?)?$/
			}
		},
		dataUriRegex: /^data:[\w+.-]+\/[\w+.-]+;((charset=[\w-]+|base64),)?(.*)$/,
		hexRegex: {
			withPrefix: /^0x[0-9a-f]+$/i,
			withOptionalPrefix: /^(?:0x)?[0-9a-f]+$/i,
			withoutPrefix: /^[0-9a-f]+$/i
		},
		ipRegex: o({ cidr: "forbidden" }).regex,
		isoDurationRegex: /^P(?!$)(\d+Y)?(\d+M)?(\d+W)?(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+S)?)?$/,
		guidBrackets: {
			"{": "}",
			"[": "]",
			"(": ")",
			"": ""
		},
		guidVersions: {
			uuidv1: "1",
			uuidv2: "2",
			uuidv3: "3",
			uuidv4: "4",
			uuidv5: "5",
			uuidv6: "6",
			uuidv7: "7",
			uuidv8: "8"
		},
		guidSeparators: new Set([
			void 0,
			!0,
			!1,
			"-",
			":"
		]),
		normalizationForms: [
			"NFC",
			"NFD",
			"NFKC",
			"NFKD"
		]
	};
	t.exports = l.extend({
		type: "string",
		flags: {
			insensitive: { default: !1 },
			truncate: { default: !1 }
		},
		terms: { replacements: { init: null } },
		coerce: {
			from: "string",
			method(e, { schema: t, state: n, prefs: r }) {
				let i = t.$_getRule("normalize");
				i && (e = e.normalize(i.args.form));
				let a = t.$_getRule("case");
				a && (e = a.args.direction === "upper" ? e.toLocaleUpperCase() : e.toLocaleLowerCase());
				let o = t.$_getRule("trim");
				if (o && o.args.enabled && (e = e.trim()), t.$_terms.replacements) for (let n of t.$_terms.replacements) e = e.replace(n.pattern, n.replacement);
				let s = t.$_getRule("hex");
				if (s && s.args.options.byteAligned && e.length % 2 != 0 && (e = `0${e}`), t.$_getRule("isoDate")) {
					let t = f.isoDate(e);
					t && (e = t);
				}
				if (t._flags.truncate) {
					let i = t.$_getRule("max");
					if (i) {
						let a = i.args.limit;
						if (u.isResolvable(a) && (a = a.resolve(e, n, r), !u.limit(a))) return {
							value: e,
							errors: t.$_createError("any.ref", a, {
								ref: i.args.limit,
								arg: "limit",
								reason: "must be a positive integer"
							}, n, r)
						};
						e = e.slice(0, a);
					}
				}
				return { value: e };
			}
		},
		validate(e, { schema: t, error: n }) {
			if (typeof e != "string") return {
				value: e,
				errors: n("string.base")
			};
			if (e === "") {
				let r = t.$_getRule("min");
				return r && r.args.limit === 0 ? void 0 : {
					value: e,
					errors: n("string.empty")
				};
			}
		},
		jsonSchema(e, t, n, r) {
			if (!e._valids?.has("") && !e._flags.only) {
				let n = e.$_getRule("min"), r = e.$_getRule("length");
				(!n || n.args.limit > 0) && (!r || r.args.limit > 0) && (t.minLength = 1);
			}
			return t;
		},
		rules: {
			alphanum: {
				method() {
					return this.$_addRule("alphanum");
				},
				validate(e, t) {
					return /^[a-zA-Z0-9]+$/.test(e) ? e : t.error("string.alphanum");
				}
			},
			base64: {
				method(e = {}) {
					return u.assertOptions(e, ["paddingRequired", "urlSafe"]), e = {
						urlSafe: !1,
						paddingRequired: !0,
						...e
					}, n(typeof e.paddingRequired == "boolean", "paddingRequired must be boolean"), n(typeof e.urlSafe == "boolean", "urlSafe must be boolean"), this.$_addRule({
						name: "base64",
						args: { options: e }
					});
				},
				validate(e, t, { options: n }) {
					return f.base64Regex[n.paddingRequired][n.urlSafe].test(e) ? e : t.error("string.base64");
				},
				jsonSchema(e, t) {
					return t.format = "base64", t;
				}
			},
			case: {
				method(e) {
					return n(["lower", "upper"].includes(e), "Invalid case:", e), this.$_addRule({
						name: "case",
						args: { direction: e }
					});
				},
				validate(e, t, { direction: n }) {
					return n === "lower" && e === e.toLocaleLowerCase() || n === "upper" && e === e.toLocaleUpperCase() ? e : t.error(`string.${n}case`);
				},
				convert: !0
			},
			creditCard: {
				method() {
					return this.$_addRule("creditCard");
				},
				validate(e, t) {
					let n = e.length, r = 0, i = 1;
					for (; n--;) {
						let t = e.charAt(n) * i;
						r += t - (t > 9) * 9, i ^= 3;
					}
					return r > 0 && r % 10 == 0 ? e : t.error("string.creditCard");
				}
			},
			dataUri: {
				method(e = {}) {
					return u.assertOptions(e, ["paddingRequired"]), e = {
						paddingRequired: !0,
						...e
					}, n(typeof e.paddingRequired == "boolean", "paddingRequired must be boolean"), this.$_addRule({
						name: "dataUri",
						args: { options: e }
					});
				},
				validate(e, t, { options: n }) {
					let r = e.match(f.dataUriRegex);
					return r && (!r[2] || r[2] !== "base64" || f.base64Regex[n.paddingRequired].false.test(r[3])) ? e : t.error("string.dataUri");
				},
				jsonSchema(e, t) {
					return t.format = "data-uri", t;
				}
			},
			domain: {
				method(e) {
					e && u.assertOptions(e, [
						"allowFullyQualified",
						"allowUnicode",
						"allowUnderscore",
						"maxDomainSegments",
						"minDomainSegments",
						"tlds"
					]);
					let t = f.addressOptions(e);
					return this.$_addRule({
						name: "domain",
						args: { options: e },
						address: t
					});
				},
				validate(e, t, n, { address: r }) {
					return i(e, r) ? e : t.error("string.domain");
				}
			},
			email: {
				method(e = {}) {
					u.assertOptions(e, [
						"allowFullyQualified",
						"allowUnicode",
						"ignoreLength",
						"maxDomainSegments",
						"minDomainSegments",
						"multiple",
						"separator",
						"tlds"
					]), n(e.multiple === void 0 || typeof e.multiple == "boolean", "multiple option must be an boolean");
					let t = f.addressOptions(e), i = RegExp(`\\s*[${e.separator ? r(e.separator) : ","}]\\s*`);
					return this.$_addRule({
						name: "email",
						args: { options: e },
						regex: i,
						address: t
					});
				},
				validate(e, t, { options: n }, { regex: r, address: i }) {
					let o = n.multiple ? e.split(r) : [e], s = [];
					for (let e of o) a(e, i) || s.push(e);
					return s.length ? t.error("string.email", {
						value: e,
						invalids: s
					}) : e;
				},
				jsonSchema(e, t) {
					return t.format = "email", t;
				}
			},
			guid: {
				alias: "uuid",
				method(e = {}) {
					u.assertOptions(e, [
						"version",
						"separator",
						"wrapper"
					]), n(e.wrapper === void 0 || typeof e.wrapper == "boolean" || typeof e.wrapper == "string" && typeof f.guidBrackets[e.wrapper] == "string", `"wrapper" must be true, false, or one of "${Object.keys(f.guidBrackets).filter(Boolean).join("\", \"")}"`);
					let t = "";
					if (e.version) {
						let r = [].concat(e.version);
						n(r.length >= 1, "version must have at least 1 valid version specified");
						let i = /* @__PURE__ */ new Set();
						for (let e = 0; e < r.length; ++e) {
							let a = r[e];
							n(typeof a == "string", "version at position " + e + " must be a string");
							let o = f.guidVersions[a.toLowerCase()];
							n(o, "version at position " + e + " must be one of " + Object.keys(f.guidVersions).join(", ")), n(!i.has(o), "version at position " + e + " must not be a duplicate"), t += o, i.add(o);
						}
					}
					n(f.guidSeparators.has(e.separator), "separator must be one of true, false, \"-\", or \":\"");
					let i = e.separator === void 0 ? "[:-]?" : e.separator === !0 ? "[:-]" : e.separator === !1 ? "[]?" : `\\${e.separator}`, a, o;
					e.wrapper === void 0 ? (a = "[\\[{\\(]?", o = "[\\]}\\)]?") : e.wrapper === !0 ? (a = "[\\[{\\(]", o = "[\\]}\\)]") : e.wrapper === !1 ? (a = "", o = "") : (a = r(e.wrapper), o = r(f.guidBrackets[e.wrapper]));
					let s = RegExp(`^(${a})[0-9A-F]{8}(${i})[0-9A-F]{4}\\2?[${t || "0-9A-F"}][0-9A-F]{3}\\2?[${t ? "89AB" : "0-9A-F"}][0-9A-F]{3}\\2?[0-9A-F]{12}(${o})$`, "i");
					return this.$_addRule({
						name: "guid",
						args: { options: e },
						regex: s
					});
				},
				validate(e, t, n, { regex: r }) {
					let i = r.exec(e);
					if (!i) return t.error("string.guid");
					let a = i[1], o = i[i.length - 1];
					return (a || o) && f.guidBrackets[a] !== o ? t.error("string.guid") : e;
				},
				jsonSchema(e, t) {
					return t.format = "uuid", t;
				}
			},
			hex: {
				method(e = {}) {
					return u.assertOptions(e, ["byteAligned", "prefix"]), e = {
						byteAligned: !1,
						prefix: !1,
						...e
					}, n(typeof e.byteAligned == "boolean", "byteAligned must be boolean"), n(typeof e.prefix == "boolean" || e.prefix === "optional", "prefix must be boolean or \"optional\""), this.$_addRule({
						name: "hex",
						args: { options: e }
					});
				},
				validate(e, t, { options: n }) {
					return (n.prefix === "optional" ? f.hexRegex.withOptionalPrefix : n.prefix === !0 ? f.hexRegex.withPrefix : f.hexRegex.withoutPrefix).test(e) ? n.byteAligned && e.length % 2 != 0 ? t.error("string.hexAlign") : e : t.error("string.hex");
				},
				jsonSchema(e, t) {
					return t.format = "hex", t;
				}
			},
			hostname: {
				method() {
					return this.$_addRule("hostname");
				},
				validate(e, t) {
					return i(e, { minDomainSegments: 1 }) || f.ipRegex.test(e) ? e : t.error("string.hostname");
				},
				jsonSchema(e, t) {
					return t.format = "hostname", t;
				}
			},
			insensitive: { method() {
				return this.$_setFlag("insensitive", !0);
			} },
			ip: {
				method(e = {}) {
					u.assertOptions(e, ["cidr", "version"]);
					let { cidr: t, versions: n, regex: r } = o(e), i = e.version ? n : void 0;
					return this.$_addRule({
						name: "ip",
						args: { options: {
							cidr: t,
							version: i
						} },
						regex: r
					});
				},
				validate(e, t, { options: n }, { regex: r }) {
					return r.test(e) ? e : n.version ? t.error("string.ipVersion", {
						value: e,
						cidr: n.cidr,
						version: n.version
					}) : t.error("string.ip", {
						value: e,
						cidr: n.cidr
					});
				},
				jsonSchema(e, t) {
					let n = e.args.options.version;
					return n && n.length === 1 ? t.format = n[0] : t.format = "ip", t;
				}
			},
			isoDate: {
				method() {
					return this.$_addRule("isoDate");
				},
				validate(e, { error: t }) {
					return f.isoDate(e) ? e : t("string.isoDate");
				},
				jsonSchema(e, t) {
					return t.format = "date-time", t;
				}
			},
			isoDuration: {
				method() {
					return this.$_addRule("isoDuration");
				},
				validate(e, t) {
					return f.isoDurationRegex.test(e) ? e : t.error("string.isoDuration");
				},
				jsonSchema(e, t) {
					return t.format = "duration", t;
				}
			},
			length: {
				method(e, t) {
					return f.length(this, "length", e, "=", t);
				},
				validate(e, t, { limit: n, encoding: r }, { name: i, operator: a, args: o }) {
					let s = r ? !1 : e.length;
					return u.compare(s, n, a) ? e : t.error("string." + i, {
						limit: o.limit,
						value: e,
						encoding: r
					});
				},
				jsonSchema(e, t) {
					return t.minLength = e.args.limit, t.maxLength = e.args.limit, t;
				},
				args: [{
					name: "limit",
					ref: !0,
					assert: u.limit,
					message: "must be a positive integer"
				}, "encoding"]
			},
			lowercase: { method() {
				return this.case("lower");
			} },
			max: {
				method(e, t) {
					return f.length(this, "max", e, "<=", t);
				},
				jsonSchema(e, t) {
					return t.maxLength = e.args.limit, t;
				},
				args: ["limit", "encoding"]
			},
			min: {
				method(e, t) {
					return f.length(this, "min", e, ">=", t);
				},
				jsonSchema(e, t) {
					return e.args.limit > 0 && (t.minLength = e.args.limit), t;
				},
				args: ["limit", "encoding"]
			},
			normalize: {
				method(e = "NFC") {
					return n(f.normalizationForms.includes(e), "normalization form must be one of " + f.normalizationForms.join(", ")), this.$_addRule({
						name: "normalize",
						args: { form: e }
					});
				},
				validate(e, { error: t }, { form: n }) {
					return e === e.normalize(n) ? e : t("string.normalize", {
						value: e,
						form: n
					});
				},
				convert: !0
			},
			pattern: {
				alias: "regex",
				method(e, t = {}) {
					n(e instanceof RegExp, "regex must be a RegExp"), n(!e.flags.includes("g") && !e.flags.includes("y"), "regex should not use global or sticky mode"), typeof t == "string" && (t = { name: t }), u.assertOptions(t, ["invert", "name"]);
					let r = [
						"string.pattern",
						t.invert ? ".invert" : "",
						t.name ? ".name" : ".base"
					].join("");
					return this.$_addRule({
						name: "pattern",
						args: {
							regex: e,
							options: t
						},
						errorCode: r
					});
				},
				validate(e, t, { regex: n, options: r }, { errorCode: i }) {
					return n.test(e) ^ r.invert ? e : t.error(i, {
						name: r.name,
						regex: n,
						value: e
					});
				},
				jsonSchema(e, t) {
					return t.pattern = e.args.regex.source, t;
				},
				args: ["regex", "options"],
				multi: !0
			},
			replace: { method(e, t) {
				typeof e == "string" && (e = new RegExp(r(e), "g")), n(e instanceof RegExp, "pattern must be a RegExp"), n(typeof t == "string", "replacement must be a String");
				let i = this.clone();
				return i.$_terms.replacements || (i.$_terms.replacements = []), i.$_terms.replacements.push({
					pattern: e,
					replacement: t
				}), i;
			} },
			token: {
				method() {
					return this.$_addRule("token");
				},
				validate(e, t) {
					return /^\w+$/.test(e) ? e : t.error("string.token");
				},
				jsonSchema(e, t) {
					return t.format = "token", t;
				}
			},
			trim: {
				method(e = !0) {
					return n(typeof e == "boolean", "enabled must be a boolean"), this.$_addRule({
						name: "trim",
						args: { enabled: e }
					});
				},
				validate(e, t, { enabled: n }) {
					return !n || e === e.trim() ? e : t.error("string.trim");
				},
				convert: !0
			},
			truncate: { method(e = !0) {
				return n(typeof e == "boolean", "enabled must be a boolean"), this.$_setFlag("truncate", e);
			} },
			uppercase: { method() {
				return this.case("upper");
			} },
			uri: {
				method(e = {}) {
					u.assertOptions(e, [
						"allowRelative",
						"allowQuerySquareBrackets",
						"domain",
						"relativeOnly",
						"scheme",
						"encodeUri"
					]), e.domain && u.assertOptions(e.domain, [
						"allowFullyQualified",
						"allowUnicode",
						"maxDomainSegments",
						"minDomainSegments",
						"tlds"
					]);
					let { regex: t, scheme: n } = s(e), r = e.domain ? f.addressOptions(e.domain) : null;
					return this.$_addRule({
						name: "uri",
						args: { options: e },
						regex: t,
						domain: r,
						scheme: n
					});
				},
				validate(e, t, { options: n }, { regex: r, domain: a, scheme: o }) {
					if (["http:/", "https:/"].includes(e)) return t.error("string.uri");
					let s = r.exec(e);
					if (!s && t.prefs.convert && n.encodeUri) {
						let t = encodeURI(e);
						s = r.exec(t), s && (e = t);
					}
					if (s) {
						let r = s[1] || s[2];
						return a && (!n.allowRelative || r) && !i(r, a) ? t.error("string.domain", { value: r }) : e;
					}
					return n.relativeOnly ? t.error("string.uriRelativeOnly") : n.scheme ? t.error("string.uriCustomScheme", {
						scheme: o,
						value: e
					}) : t.error("string.uri");
				},
				jsonSchema(e, t) {
					return t.format = "uri", t;
				}
			}
		},
		manifest: { build(e, t) {
			if (t.replacements) for (let { pattern: n, replacement: r } of t.replacements) e = e.replace(n, r);
			return e;
		} },
		messages: {
			"string.alphanum": "{{#label}} must only contain alpha-numeric characters",
			"string.base": "{{#label}} must be a string",
			"string.base64": "{{#label}} must be a valid base64 string",
			"string.creditCard": "{{#label}} must be a credit card",
			"string.dataUri": "{{#label}} must be a valid dataUri string",
			"string.domain": "{{#label}} must contain a valid domain name",
			"string.email": "{{#label}} must be a valid email",
			"string.empty": "{{#label}} is not allowed to be empty",
			"string.guid": "{{#label}} must be a valid GUID",
			"string.hex": "{{#label}} must only contain hexadecimal characters",
			"string.hexAlign": "{{#label}} hex decoded representation must be byte aligned",
			"string.hostname": "{{#label}} must be a valid hostname",
			"string.ip": "{{#label}} must be a valid ip address with a {{#cidr}} CIDR",
			"string.ipVersion": "{{#label}} must be a valid ip address of one of the following versions {{#version}} with a {{#cidr}} CIDR",
			"string.isoDate": "{{#label}} must be in iso format",
			"string.isoDuration": "{{#label}} must be a valid ISO 8601 duration",
			"string.length": "{{#label}} length must be {{#limit}} characters long",
			"string.lowercase": "{{#label}} must only contain lowercase characters",
			"string.max": "{{#label}} length must be less than or equal to {{#limit}} characters long",
			"string.min": "{{#label}} length must be at least {{#limit}} characters long",
			"string.normalize": "{{#label}} must be unicode normalized in the {{#form}} form",
			"string.token": "{{#label}} must only contain alpha-numeric and underscore characters",
			"string.pattern.base": "{{#label}} with value {:[.]} fails to match the required pattern: {{#regex}}",
			"string.pattern.name": "{{#label}} with value {:[.]} fails to match the {{#name}} pattern",
			"string.pattern.invert.base": "{{#label}} with value {:[.]} matches the inverted pattern: {{#regex}}",
			"string.pattern.invert.name": "{{#label}} with value {:[.]} matches the inverted {{#name}} pattern",
			"string.trim": "{{#label}} must not have leading or trailing whitespace",
			"string.uri": "{{#label}} must be a valid uri",
			"string.uriCustomScheme": "{{#label}} must be a valid uri with a scheme matching the {{#scheme}} pattern",
			"string.uriRelativeOnly": "{{#label}} must be a valid relative uri",
			"string.uppercase": "{{#label}} must only contain uppercase characters"
		}
	}), f.addressOptions = function(e) {
		if (!e) return f.tlds || e;
		if (n(e.minDomainSegments === void 0 || Number.isSafeInteger(e.minDomainSegments) && e.minDomainSegments > 0, "minDomainSegments must be a positive integer"), n(e.maxDomainSegments === void 0 || Number.isSafeInteger(e.maxDomainSegments) && e.maxDomainSegments > 0, "maxDomainSegments must be a positive integer"), e.tlds === !1) return e;
		if (e.tlds === !0 || e.tlds === void 0) return n(f.tlds, "Built-in TLD list disabled"), Object.assign({}, e, f.tlds);
		n(typeof e.tlds == "object", "tlds must be true, false, or an object");
		let t = e.tlds.deny;
		if (t) return Array.isArray(t) && (e = Object.assign({}, e, { tlds: { deny: new Set(t) } })), n(e.tlds.deny instanceof Set, "tlds.deny must be an array, Set, or boolean"), n(!e.tlds.allow, "Cannot specify both tlds.allow and tlds.deny lists"), f.validateTlds(e.tlds.deny, "tlds.deny"), e;
		let r = e.tlds.allow;
		return r ? r === !0 ? (n(f.tlds, "Built-in TLD list disabled"), Object.assign({}, e, f.tlds)) : (Array.isArray(r) && (e = Object.assign({}, e, { tlds: { allow: new Set(r) } })), n(e.tlds.allow instanceof Set, "tlds.allow must be an array, Set, or boolean"), f.validateTlds(e.tlds.allow, "tlds.allow"), e) : {
			...e,
			tlds: !1
		};
	}, f.validateTlds = function(e, t) {
		for (let r of e) n(i(r, {
			minDomainSegments: 1,
			maxDomainSegments: 1
		}), `${t} must contain valid top level domain names`);
	}, f.isoDate = function(e) {
		if (!u.isIsoDate(e)) return null;
		/.*T.*[+-]\d\d$/.test(e) && (e += "00");
		let t = new Date(e);
		return isNaN(t.getTime()) ? null : t.toISOString();
	}, f.length = function(e, t, r, i, a) {
		return n(!a || !1, "Invalid encoding:", a), e.$_addRule({
			name: t,
			method: "length",
			args: {
				limit: r,
				encoding: a
			},
			operator: i
		});
	};
})), Ye = /* @__PURE__ */ s(((e, t) => {
	var { assert: n } = E(), r = R(), i = {};
	i.Map = class extends Map {
		slice() {
			return new i.Map(this);
		}
	}, t.exports = r.extend({
		type: "symbol",
		terms: { map: { init: new i.Map() } },
		coerce: { method(e, { schema: t, error: n }) {
			let r = t.$_terms.map.get(e);
			return r && (e = r), !t._flags.only || typeof e == "symbol" ? { value: e } : {
				value: e,
				errors: n("symbol.map", { map: t.$_terms.map })
			};
		} },
		validate(e, { error: t }) {
			if (typeof e != "symbol") return {
				value: e,
				errors: t("symbol.base")
			};
		},
		rules: { map: { method(e) {
			e && !e[Symbol.iterator] && typeof e == "object" && (e = Object.entries(e)), n(e && e[Symbol.iterator], "Iterable must be an iterable or object");
			let t = this.clone(), r = [];
			for (let i of e) {
				n(i && i[Symbol.iterator], "Entry must be an iterable");
				let [e, a] = i;
				n(typeof e != "object" && typeof e != "function" && typeof e != "symbol", "Key must not be of type object, function, or Symbol"), n(typeof a == "symbol", "Value must be a Symbol"), t.$_terms.map.set(e, a), r.push(a);
			}
			return t.valid(...r);
		} } },
		manifest: { build(e, t) {
			return t.map && (e = e.map(t.map)), e;
		} },
		jsonSchema(e, t, n, r) {
			let i = e.$_terms.map;
			return i.size ? { anyOf: Array.from(i.keys()).map((e) => ({ const: e })) } : {};
		},
		messages: {
			"symbol.base": "{{#label}} must be a symbol",
			"symbol.map": "{{#label}} must be one of {{#map}}"
		}
	});
})), $ = /* @__PURE__ */ s(((e, t) => {
	var { assert: n, clone: r } = E(), i = F(), a = P(), o = I(), s = A(), c = L(), l = x(), u = j(), d = M(), f = x(), p, m = {
		types: {
			alternatives: ce(),
			any: R(),
			array: le(),
			boolean: ue(),
			date: de(),
			function: pe(),
			link: me(),
			number: he(),
			object: ge(),
			string: Je(),
			symbol: Ye()
		},
		aliases: {
			alt: "alternatives",
			bool: "boolean",
			func: "function"
		}
	};
	m.root = function() {
		let e = { _types: new Set(Object.keys(m.types)) };
		for (let t of e._types) e[t] = function(...e) {
			return n(!e.length || [
				"alternatives",
				"link",
				"object"
			].includes(t), "The", t, "type does not allow arguments"), m.generate(this, m.types[t], e);
		};
		for (let t of [
			"allow",
			"custom",
			"disallow",
			"equal",
			"exist",
			"forbidden",
			"invalid",
			"not",
			"only",
			"optional",
			"options",
			"prefs",
			"preferences",
			"required",
			"strip",
			"valid",
			"when"
		]) e[t] = function(...e) {
			return this.any()[t](...e);
		};
		Object.assign(e, m.methods);
		for (let t in m.aliases) e[t] = e[m.aliases[t]];
		return e.x = e.expression, f.setup && f.setup(e), e;
	}, m.methods = {
		ValidationError: s.ValidationError,
		version: a.version,
		cache: i.provider,
		assert(e, t, ...n) {
			m.assert(e, t, !0, n);
		},
		attempt(e, t, ...n) {
			return m.assert(e, t, !1, n);
		},
		build(e) {
			return n(typeof l.build == "function", "Manifest functionality disabled"), l.build(this, e);
		},
		checkPreferences(e) {
			a.checkPreferences(e);
		},
		compile(e, t) {
			return o.compile(this, e, t);
		},
		defaults(e) {
			n(typeof e == "function", "modifier must be a function");
			let t = Object.assign({}, this);
			for (let r of t._types) {
				let i = e(t[r]());
				n(a.isSchema(i), "modifier must return a valid schema object"), t[r] = function(...e) {
					return m.generate(this, i, e);
				};
			}
			return t;
		},
		expression(...e) {
			return new d(...e);
		},
		extend(...e) {
			a.verifyFlat(e, "extend"), p ||= te(), n(e.length, "You need to provide at least one extension"), this.assert(e, p.extensions);
			let t = Object.assign({}, this);
			t._types = new Set(t._types);
			for (let r of e) {
				typeof r == "function" && (r = r(t)), this.assert(r, p.extension);
				let e = m.expandExtension(r, t);
				for (let r of e) {
					n(t[r.type] === void 0 || t._types.has(r.type), "Cannot override name", r.type);
					let e = r.base || this.any(), i = c.type(e, r);
					t._types.add(r.type), t[r.type] = function(...e) {
						return m.generate(this, i, e);
					};
				}
			}
			return t;
		},
		isError: s.ValidationError.isError,
		isExpression: d.isTemplate,
		isRef: u.isRef,
		isSchema: a.isSchema,
		in(...e) {
			return u.in(...e);
		},
		override: a.symbols.override,
		ref(...e) {
			return u.create(...e);
		},
		types() {
			let e = {};
			for (let t of this._types) e[t] = this[t]();
			for (let t in m.aliases) e[t] = this[t]();
			return e;
		}
	}, m.assert = function(e, t, n, i) {
		let o = i[0] instanceof Error || typeof i[0] == "string" ? i[0] : null, c = o === null ? i[0] : i[1], l = t.validate(e, a.preferences({ errors: { stack: !0 } }, c || {})), u = l.error;
		if (!u) return l.value;
		if (o instanceof Error) throw o;
		let d = n && typeof u.annotate == "function" ? u.annotate() : u.message;
		throw u instanceof s.ValidationError || (u = r(u)), u.message = o ? `${o} ${d}` : d, u;
	}, m.generate = function(e, t, r) {
		return n(e, "Must be invoked on a Joi instance."), t.$_root = e, !t._definition.args || !r.length ? t : t._definition.args(t, ...r);
	}, m.expandExtension = function(e, t) {
		if (typeof e.type == "string") return [e];
		let n = [];
		for (let r of t._types) if (e.type.test(r)) {
			let i = Object.assign({}, e);
			i.type = r, i.base = t[r](), n.push(i);
		}
		return n;
	}, t.exports = m.root();
}));
//#endregion
export default $();

//# sourceMappingURL=joi-browser.min.mjs.map