"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports["default"] = void 0;
var _typeof2 = _interopRequireDefault(require("@babel/runtime/helpers/typeof"));
var _createLanguageAsyncLoader = _interopRequireDefault(require("./create-language-async-loader"));
function _interopRequireWildcard(e, t) { if ("function" == typeof WeakMap) var r = new WeakMap(), n = new WeakMap(); return (_interopRequireWildcard = function _interopRequireWildcard(e, t) { if (!t && e && e.__esModule) return e; var o, i, f = { __proto__: null, "default": e }; if (null === e || "object" != (0, _typeof2["default"])(e) && "function" != typeof e) return f; if (o = t ? n : r) { if (o.has(e)) return o.get(e); o.set(e, f); } for (var _t in e) "default" !== _t && {}.hasOwnProperty.call(e, _t) && ((i = (o = Object.defineProperty) && Object.getOwnPropertyDescriptor(e, _t)) && (i.get || i.set) ? o(f, _t, i) : f[_t] = e[_t]); return f; })(e, t); }
var _default = exports["default"] = {
  abap: (0, _createLanguageAsyncLoader["default"])("abap", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_abap" */"refractor/abap"));
    });
  }),
  abnf: (0, _createLanguageAsyncLoader["default"])("abnf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_abnf" */"refractor/abnf"));
    });
  }),
  actionscript: (0, _createLanguageAsyncLoader["default"])("actionscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_actionscript" */"refractor/actionscript"));
    });
  }),
  ada: (0, _createLanguageAsyncLoader["default"])("ada", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ada" */"refractor/ada"));
    });
  }),
  agda: (0, _createLanguageAsyncLoader["default"])("agda", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_agda" */"refractor/agda"));
    });
  }),
  al: (0, _createLanguageAsyncLoader["default"])("al", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_al" */"refractor/al"));
    });
  }),
  antlr4: (0, _createLanguageAsyncLoader["default"])("antlr4", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_antlr4" */"refractor/antlr4"));
    });
  }),
  apacheconf: (0, _createLanguageAsyncLoader["default"])("apacheconf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_apacheconf" */"refractor/apacheconf"));
    });
  }),
  apex: (0, _createLanguageAsyncLoader["default"])("apex", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_apex" */"refractor/apex"));
    });
  }),
  apl: (0, _createLanguageAsyncLoader["default"])("apl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_apl" */"refractor/apl"));
    });
  }),
  applescript: (0, _createLanguageAsyncLoader["default"])("applescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_applescript" */"refractor/applescript"));
    });
  }),
  aql: (0, _createLanguageAsyncLoader["default"])("aql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_aql" */"refractor/aql"));
    });
  }),
  arduino: (0, _createLanguageAsyncLoader["default"])("arduino", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_arduino" */"refractor/arduino"));
    });
  }),
  arff: (0, _createLanguageAsyncLoader["default"])("arff", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_arff" */"refractor/arff"));
    });
  }),
  armasm: (0, _createLanguageAsyncLoader["default"])("armasm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_armasm" */"refractor/armasm"));
    });
  }),
  arturo: (0, _createLanguageAsyncLoader["default"])("arturo", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_arturo" */"refractor/arturo"));
    });
  }),
  asciidoc: (0, _createLanguageAsyncLoader["default"])("asciidoc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_asciidoc" */"refractor/asciidoc"));
    });
  }),
  asm6502: (0, _createLanguageAsyncLoader["default"])("asm6502", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_asm6502" */"refractor/asm6502"));
    });
  }),
  asmatmel: (0, _createLanguageAsyncLoader["default"])("asmatmel", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_asmatmel" */"refractor/asmatmel"));
    });
  }),
  aspnet: (0, _createLanguageAsyncLoader["default"])("aspnet", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_aspnet" */"refractor/aspnet"));
    });
  }),
  autohotkey: (0, _createLanguageAsyncLoader["default"])("autohotkey", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_autohotkey" */"refractor/autohotkey"));
    });
  }),
  autoit: (0, _createLanguageAsyncLoader["default"])("autoit", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_autoit" */"refractor/autoit"));
    });
  }),
  avisynth: (0, _createLanguageAsyncLoader["default"])("avisynth", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_avisynth" */"refractor/avisynth"));
    });
  }),
  avroIdl: (0, _createLanguageAsyncLoader["default"])("avroIdl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_avroIdl" */"refractor/avro-idl"));
    });
  }),
  awk: (0, _createLanguageAsyncLoader["default"])("awk", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_awk" */"refractor/awk"));
    });
  }),
  bash: (0, _createLanguageAsyncLoader["default"])("bash", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bash" */"refractor/bash"));
    });
  }),
  basic: (0, _createLanguageAsyncLoader["default"])("basic", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_basic" */"refractor/basic"));
    });
  }),
  batch: (0, _createLanguageAsyncLoader["default"])("batch", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_batch" */"refractor/batch"));
    });
  }),
  bbcode: (0, _createLanguageAsyncLoader["default"])("bbcode", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bbcode" */"refractor/bbcode"));
    });
  }),
  bbj: (0, _createLanguageAsyncLoader["default"])("bbj", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bbj" */"refractor/bbj"));
    });
  }),
  bicep: (0, _createLanguageAsyncLoader["default"])("bicep", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bicep" */"refractor/bicep"));
    });
  }),
  birb: (0, _createLanguageAsyncLoader["default"])("birb", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_birb" */"refractor/birb"));
    });
  }),
  bison: (0, _createLanguageAsyncLoader["default"])("bison", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bison" */"refractor/bison"));
    });
  }),
  bnf: (0, _createLanguageAsyncLoader["default"])("bnf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bnf" */"refractor/bnf"));
    });
  }),
  bqn: (0, _createLanguageAsyncLoader["default"])("bqn", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bqn" */"refractor/bqn"));
    });
  }),
  brainfuck: (0, _createLanguageAsyncLoader["default"])("brainfuck", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_brainfuck" */"refractor/brainfuck"));
    });
  }),
  brightscript: (0, _createLanguageAsyncLoader["default"])("brightscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_brightscript" */"refractor/brightscript"));
    });
  }),
  bro: (0, _createLanguageAsyncLoader["default"])("bro", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bro" */"refractor/bro"));
    });
  }),
  bsl: (0, _createLanguageAsyncLoader["default"])("bsl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_bsl" */"refractor/bsl"));
    });
  }),
  c: (0, _createLanguageAsyncLoader["default"])("c", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_c" */"refractor/c"));
    });
  }),
  cfscript: (0, _createLanguageAsyncLoader["default"])("cfscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cfscript" */"refractor/cfscript"));
    });
  }),
  chaiscript: (0, _createLanguageAsyncLoader["default"])("chaiscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_chaiscript" */"refractor/chaiscript"));
    });
  }),
  cil: (0, _createLanguageAsyncLoader["default"])("cil", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cil" */"refractor/cil"));
    });
  }),
  cilkc: (0, _createLanguageAsyncLoader["default"])("cilkc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cilkc" */"refractor/cilkc"));
    });
  }),
  cilkcpp: (0, _createLanguageAsyncLoader["default"])("cilkcpp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cilkcpp" */"refractor/cilkcpp"));
    });
  }),
  clike: (0, _createLanguageAsyncLoader["default"])("clike", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_clike" */"refractor/clike"));
    });
  }),
  clojure: (0, _createLanguageAsyncLoader["default"])("clojure", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_clojure" */"refractor/clojure"));
    });
  }),
  cmake: (0, _createLanguageAsyncLoader["default"])("cmake", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cmake" */"refractor/cmake"));
    });
  }),
  cobol: (0, _createLanguageAsyncLoader["default"])("cobol", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cobol" */"refractor/cobol"));
    });
  }),
  coffeescript: (0, _createLanguageAsyncLoader["default"])("coffeescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_coffeescript" */"refractor/coffeescript"));
    });
  }),
  concurnas: (0, _createLanguageAsyncLoader["default"])("concurnas", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_concurnas" */"refractor/concurnas"));
    });
  }),
  cooklang: (0, _createLanguageAsyncLoader["default"])("cooklang", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cooklang" */"refractor/cooklang"));
    });
  }),
  coq: (0, _createLanguageAsyncLoader["default"])("coq", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_coq" */"refractor/coq"));
    });
  }),
  cpp: (0, _createLanguageAsyncLoader["default"])("cpp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cpp" */"refractor/cpp"));
    });
  }),
  crystal: (0, _createLanguageAsyncLoader["default"])("crystal", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_crystal" */"refractor/crystal"));
    });
  }),
  csharp: (0, _createLanguageAsyncLoader["default"])("csharp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_csharp" */"refractor/csharp"));
    });
  }),
  cshtml: (0, _createLanguageAsyncLoader["default"])("cshtml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cshtml" */"refractor/cshtml"));
    });
  }),
  csp: (0, _createLanguageAsyncLoader["default"])("csp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_csp" */"refractor/csp"));
    });
  }),
  cssExtras: (0, _createLanguageAsyncLoader["default"])("cssExtras", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cssExtras" */"refractor/css-extras"));
    });
  }),
  css: (0, _createLanguageAsyncLoader["default"])("css", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_css" */"refractor/css"));
    });
  }),
  csv: (0, _createLanguageAsyncLoader["default"])("csv", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_csv" */"refractor/csv"));
    });
  }),
  cue: (0, _createLanguageAsyncLoader["default"])("cue", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cue" */"refractor/cue"));
    });
  }),
  cypher: (0, _createLanguageAsyncLoader["default"])("cypher", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_cypher" */"refractor/cypher"));
    });
  }),
  d: (0, _createLanguageAsyncLoader["default"])("d", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_d" */"refractor/d"));
    });
  }),
  dart: (0, _createLanguageAsyncLoader["default"])("dart", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dart" */"refractor/dart"));
    });
  }),
  dataweave: (0, _createLanguageAsyncLoader["default"])("dataweave", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dataweave" */"refractor/dataweave"));
    });
  }),
  dax: (0, _createLanguageAsyncLoader["default"])("dax", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dax" */"refractor/dax"));
    });
  }),
  dhall: (0, _createLanguageAsyncLoader["default"])("dhall", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dhall" */"refractor/dhall"));
    });
  }),
  diff: (0, _createLanguageAsyncLoader["default"])("diff", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_diff" */"refractor/diff"));
    });
  }),
  django: (0, _createLanguageAsyncLoader["default"])("django", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_django" */"refractor/django"));
    });
  }),
  dnsZoneFile: (0, _createLanguageAsyncLoader["default"])("dnsZoneFile", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dnsZoneFile" */"refractor/dns-zone-file"));
    });
  }),
  docker: (0, _createLanguageAsyncLoader["default"])("docker", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_docker" */"refractor/docker"));
    });
  }),
  dot: (0, _createLanguageAsyncLoader["default"])("dot", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_dot" */"refractor/dot"));
    });
  }),
  ebnf: (0, _createLanguageAsyncLoader["default"])("ebnf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ebnf" */"refractor/ebnf"));
    });
  }),
  editorconfig: (0, _createLanguageAsyncLoader["default"])("editorconfig", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_editorconfig" */"refractor/editorconfig"));
    });
  }),
  eiffel: (0, _createLanguageAsyncLoader["default"])("eiffel", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_eiffel" */"refractor/eiffel"));
    });
  }),
  ejs: (0, _createLanguageAsyncLoader["default"])("ejs", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ejs" */"refractor/ejs"));
    });
  }),
  elixir: (0, _createLanguageAsyncLoader["default"])("elixir", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_elixir" */"refractor/elixir"));
    });
  }),
  elm: (0, _createLanguageAsyncLoader["default"])("elm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_elm" */"refractor/elm"));
    });
  }),
  erb: (0, _createLanguageAsyncLoader["default"])("erb", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_erb" */"refractor/erb"));
    });
  }),
  erlang: (0, _createLanguageAsyncLoader["default"])("erlang", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_erlang" */"refractor/erlang"));
    });
  }),
  etlua: (0, _createLanguageAsyncLoader["default"])("etlua", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_etlua" */"refractor/etlua"));
    });
  }),
  excelFormula: (0, _createLanguageAsyncLoader["default"])("excelFormula", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_excelFormula" */"refractor/excel-formula"));
    });
  }),
  factor: (0, _createLanguageAsyncLoader["default"])("factor", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_factor" */"refractor/factor"));
    });
  }),
  falselang: (0, _createLanguageAsyncLoader["default"])("falselang", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_falselang" */"refractor/false"));
    });
  }),
  firestoreSecurityRules: (0, _createLanguageAsyncLoader["default"])("firestoreSecurityRules", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_firestoreSecurityRules" */"refractor/firestore-security-rules"));
    });
  }),
  flow: (0, _createLanguageAsyncLoader["default"])("flow", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_flow" */"refractor/flow"));
    });
  }),
  fortran: (0, _createLanguageAsyncLoader["default"])("fortran", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_fortran" */"refractor/fortran"));
    });
  }),
  fsharp: (0, _createLanguageAsyncLoader["default"])("fsharp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_fsharp" */"refractor/fsharp"));
    });
  }),
  ftl: (0, _createLanguageAsyncLoader["default"])("ftl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ftl" */"refractor/ftl"));
    });
  }),
  gap: (0, _createLanguageAsyncLoader["default"])("gap", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gap" */"refractor/gap"));
    });
  }),
  gcode: (0, _createLanguageAsyncLoader["default"])("gcode", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gcode" */"refractor/gcode"));
    });
  }),
  gdscript: (0, _createLanguageAsyncLoader["default"])("gdscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gdscript" */"refractor/gdscript"));
    });
  }),
  gedcom: (0, _createLanguageAsyncLoader["default"])("gedcom", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gedcom" */"refractor/gedcom"));
    });
  }),
  gettext: (0, _createLanguageAsyncLoader["default"])("gettext", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gettext" */"refractor/gettext"));
    });
  }),
  gherkin: (0, _createLanguageAsyncLoader["default"])("gherkin", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gherkin" */"refractor/gherkin"));
    });
  }),
  git: (0, _createLanguageAsyncLoader["default"])("git", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_git" */"refractor/git"));
    });
  }),
  glsl: (0, _createLanguageAsyncLoader["default"])("glsl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_glsl" */"refractor/glsl"));
    });
  }),
  gml: (0, _createLanguageAsyncLoader["default"])("gml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gml" */"refractor/gml"));
    });
  }),
  gn: (0, _createLanguageAsyncLoader["default"])("gn", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gn" */"refractor/gn"));
    });
  }),
  goModule: (0, _createLanguageAsyncLoader["default"])("goModule", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_goModule" */"refractor/go-module"));
    });
  }),
  go: (0, _createLanguageAsyncLoader["default"])("go", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_go" */"refractor/go"));
    });
  }),
  gradle: (0, _createLanguageAsyncLoader["default"])("gradle", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_gradle" */"refractor/gradle"));
    });
  }),
  graphql: (0, _createLanguageAsyncLoader["default"])("graphql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_graphql" */"refractor/graphql"));
    });
  }),
  groovy: (0, _createLanguageAsyncLoader["default"])("groovy", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_groovy" */"refractor/groovy"));
    });
  }),
  haml: (0, _createLanguageAsyncLoader["default"])("haml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_haml" */"refractor/haml"));
    });
  }),
  handlebars: (0, _createLanguageAsyncLoader["default"])("handlebars", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_handlebars" */"refractor/handlebars"));
    });
  }),
  haskell: (0, _createLanguageAsyncLoader["default"])("haskell", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_haskell" */"refractor/haskell"));
    });
  }),
  haxe: (0, _createLanguageAsyncLoader["default"])("haxe", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_haxe" */"refractor/haxe"));
    });
  }),
  hcl: (0, _createLanguageAsyncLoader["default"])("hcl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_hcl" */"refractor/hcl"));
    });
  }),
  hlsl: (0, _createLanguageAsyncLoader["default"])("hlsl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_hlsl" */"refractor/hlsl"));
    });
  }),
  hoon: (0, _createLanguageAsyncLoader["default"])("hoon", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_hoon" */"refractor/hoon"));
    });
  }),
  hpkp: (0, _createLanguageAsyncLoader["default"])("hpkp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_hpkp" */"refractor/hpkp"));
    });
  }),
  hsts: (0, _createLanguageAsyncLoader["default"])("hsts", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_hsts" */"refractor/hsts"));
    });
  }),
  http: (0, _createLanguageAsyncLoader["default"])("http", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_http" */"refractor/http"));
    });
  }),
  ichigojam: (0, _createLanguageAsyncLoader["default"])("ichigojam", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ichigojam" */"refractor/ichigojam"));
    });
  }),
  icon: (0, _createLanguageAsyncLoader["default"])("icon", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_icon" */"refractor/icon"));
    });
  }),
  icuMessageFormat: (0, _createLanguageAsyncLoader["default"])("icuMessageFormat", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_icuMessageFormat" */"refractor/icu-message-format"));
    });
  }),
  idris: (0, _createLanguageAsyncLoader["default"])("idris", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_idris" */"refractor/idris"));
    });
  }),
  iecst: (0, _createLanguageAsyncLoader["default"])("iecst", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_iecst" */"refractor/iecst"));
    });
  }),
  ignore: (0, _createLanguageAsyncLoader["default"])("ignore", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ignore" */"refractor/ignore"));
    });
  }),
  inform7: (0, _createLanguageAsyncLoader["default"])("inform7", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_inform7" */"refractor/inform7"));
    });
  }),
  ini: (0, _createLanguageAsyncLoader["default"])("ini", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ini" */"refractor/ini"));
    });
  }),
  io: (0, _createLanguageAsyncLoader["default"])("io", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_io" */"refractor/io"));
    });
  }),
  j: (0, _createLanguageAsyncLoader["default"])("j", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_j" */"refractor/j"));
    });
  }),
  java: (0, _createLanguageAsyncLoader["default"])("java", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_java" */"refractor/java"));
    });
  }),
  javadoc: (0, _createLanguageAsyncLoader["default"])("javadoc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_javadoc" */"refractor/javadoc"));
    });
  }),
  javadoclike: (0, _createLanguageAsyncLoader["default"])("javadoclike", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_javadoclike" */"refractor/javadoclike"));
    });
  }),
  javascript: (0, _createLanguageAsyncLoader["default"])("javascript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_javascript" */"refractor/javascript"));
    });
  }),
  javastacktrace: (0, _createLanguageAsyncLoader["default"])("javastacktrace", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_javastacktrace" */"refractor/javastacktrace"));
    });
  }),
  jexl: (0, _createLanguageAsyncLoader["default"])("jexl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jexl" */"refractor/jexl"));
    });
  }),
  jolie: (0, _createLanguageAsyncLoader["default"])("jolie", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jolie" */"refractor/jolie"));
    });
  }),
  jq: (0, _createLanguageAsyncLoader["default"])("jq", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jq" */"refractor/jq"));
    });
  }),
  jsExtras: (0, _createLanguageAsyncLoader["default"])("jsExtras", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsExtras" */"refractor/js-extras"));
    });
  }),
  jsTemplates: (0, _createLanguageAsyncLoader["default"])("jsTemplates", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsTemplates" */"refractor/js-templates"));
    });
  }),
  jsdoc: (0, _createLanguageAsyncLoader["default"])("jsdoc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsdoc" */"refractor/jsdoc"));
    });
  }),
  json: (0, _createLanguageAsyncLoader["default"])("json", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_json" */"refractor/json"));
    });
  }),
  json5: (0, _createLanguageAsyncLoader["default"])("json5", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_json5" */"refractor/json5"));
    });
  }),
  jsonp: (0, _createLanguageAsyncLoader["default"])("jsonp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsonp" */"refractor/jsonp"));
    });
  }),
  jsstacktrace: (0, _createLanguageAsyncLoader["default"])("jsstacktrace", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsstacktrace" */"refractor/jsstacktrace"));
    });
  }),
  jsx: (0, _createLanguageAsyncLoader["default"])("jsx", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_jsx" */"refractor/jsx"));
    });
  }),
  julia: (0, _createLanguageAsyncLoader["default"])("julia", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_julia" */"refractor/julia"));
    });
  }),
  keepalived: (0, _createLanguageAsyncLoader["default"])("keepalived", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_keepalived" */"refractor/keepalived"));
    });
  }),
  keyman: (0, _createLanguageAsyncLoader["default"])("keyman", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_keyman" */"refractor/keyman"));
    });
  }),
  kotlin: (0, _createLanguageAsyncLoader["default"])("kotlin", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_kotlin" */"refractor/kotlin"));
    });
  }),
  kumir: (0, _createLanguageAsyncLoader["default"])("kumir", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_kumir" */"refractor/kumir"));
    });
  }),
  kusto: (0, _createLanguageAsyncLoader["default"])("kusto", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_kusto" */"refractor/kusto"));
    });
  }),
  latex: (0, _createLanguageAsyncLoader["default"])("latex", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_latex" */"refractor/latex"));
    });
  }),
  latte: (0, _createLanguageAsyncLoader["default"])("latte", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_latte" */"refractor/latte"));
    });
  }),
  less: (0, _createLanguageAsyncLoader["default"])("less", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_less" */"refractor/less"));
    });
  }),
  lilypond: (0, _createLanguageAsyncLoader["default"])("lilypond", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_lilypond" */"refractor/lilypond"));
    });
  }),
  linkerScript: (0, _createLanguageAsyncLoader["default"])("linkerScript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_linkerScript" */"refractor/linker-script"));
    });
  }),
  liquid: (0, _createLanguageAsyncLoader["default"])("liquid", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_liquid" */"refractor/liquid"));
    });
  }),
  lisp: (0, _createLanguageAsyncLoader["default"])("lisp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_lisp" */"refractor/lisp"));
    });
  }),
  livescript: (0, _createLanguageAsyncLoader["default"])("livescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_livescript" */"refractor/livescript"));
    });
  }),
  llvm: (0, _createLanguageAsyncLoader["default"])("llvm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_llvm" */"refractor/llvm"));
    });
  }),
  log: (0, _createLanguageAsyncLoader["default"])("log", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_log" */"refractor/log"));
    });
  }),
  lolcode: (0, _createLanguageAsyncLoader["default"])("lolcode", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_lolcode" */"refractor/lolcode"));
    });
  }),
  lua: (0, _createLanguageAsyncLoader["default"])("lua", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_lua" */"refractor/lua"));
    });
  }),
  magma: (0, _createLanguageAsyncLoader["default"])("magma", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_magma" */"refractor/magma"));
    });
  }),
  makefile: (0, _createLanguageAsyncLoader["default"])("makefile", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_makefile" */"refractor/makefile"));
    });
  }),
  markdown: (0, _createLanguageAsyncLoader["default"])("markdown", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_markdown" */"refractor/markdown"));
    });
  }),
  markupTemplating: (0, _createLanguageAsyncLoader["default"])("markupTemplating", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_markupTemplating" */"refractor/markup-templating"));
    });
  }),
  markup: (0, _createLanguageAsyncLoader["default"])("markup", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_markup" */"refractor/markup"));
    });
  }),
  mata: (0, _createLanguageAsyncLoader["default"])("mata", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_mata" */"refractor/mata"));
    });
  }),
  matlab: (0, _createLanguageAsyncLoader["default"])("matlab", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_matlab" */"refractor/matlab"));
    });
  }),
  maxscript: (0, _createLanguageAsyncLoader["default"])("maxscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_maxscript" */"refractor/maxscript"));
    });
  }),
  mel: (0, _createLanguageAsyncLoader["default"])("mel", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_mel" */"refractor/mel"));
    });
  }),
  mermaid: (0, _createLanguageAsyncLoader["default"])("mermaid", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_mermaid" */"refractor/mermaid"));
    });
  }),
  metafont: (0, _createLanguageAsyncLoader["default"])("metafont", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_metafont" */"refractor/metafont"));
    });
  }),
  mizar: (0, _createLanguageAsyncLoader["default"])("mizar", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_mizar" */"refractor/mizar"));
    });
  }),
  mongodb: (0, _createLanguageAsyncLoader["default"])("mongodb", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_mongodb" */"refractor/mongodb"));
    });
  }),
  monkey: (0, _createLanguageAsyncLoader["default"])("monkey", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_monkey" */"refractor/monkey"));
    });
  }),
  moonscript: (0, _createLanguageAsyncLoader["default"])("moonscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_moonscript" */"refractor/moonscript"));
    });
  }),
  n1ql: (0, _createLanguageAsyncLoader["default"])("n1ql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_n1ql" */"refractor/n1ql"));
    });
  }),
  n4js: (0, _createLanguageAsyncLoader["default"])("n4js", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_n4js" */"refractor/n4js"));
    });
  }),
  nand2tetrisHdl: (0, _createLanguageAsyncLoader["default"])("nand2tetrisHdl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nand2tetrisHdl" */"refractor/nand2tetris-hdl"));
    });
  }),
  naniscript: (0, _createLanguageAsyncLoader["default"])("naniscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_naniscript" */"refractor/naniscript"));
    });
  }),
  nasm: (0, _createLanguageAsyncLoader["default"])("nasm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nasm" */"refractor/nasm"));
    });
  }),
  neon: (0, _createLanguageAsyncLoader["default"])("neon", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_neon" */"refractor/neon"));
    });
  }),
  nevod: (0, _createLanguageAsyncLoader["default"])("nevod", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nevod" */"refractor/nevod"));
    });
  }),
  nginx: (0, _createLanguageAsyncLoader["default"])("nginx", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nginx" */"refractor/nginx"));
    });
  }),
  nim: (0, _createLanguageAsyncLoader["default"])("nim", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nim" */"refractor/nim"));
    });
  }),
  nix: (0, _createLanguageAsyncLoader["default"])("nix", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nix" */"refractor/nix"));
    });
  }),
  nsis: (0, _createLanguageAsyncLoader["default"])("nsis", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_nsis" */"refractor/nsis"));
    });
  }),
  objectivec: (0, _createLanguageAsyncLoader["default"])("objectivec", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_objectivec" */"refractor/objectivec"));
    });
  }),
  ocaml: (0, _createLanguageAsyncLoader["default"])("ocaml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ocaml" */"refractor/ocaml"));
    });
  }),
  odin: (0, _createLanguageAsyncLoader["default"])("odin", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_odin" */"refractor/odin"));
    });
  }),
  opencl: (0, _createLanguageAsyncLoader["default"])("opencl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_opencl" */"refractor/opencl"));
    });
  }),
  openqasm: (0, _createLanguageAsyncLoader["default"])("openqasm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_openqasm" */"refractor/openqasm"));
    });
  }),
  oz: (0, _createLanguageAsyncLoader["default"])("oz", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_oz" */"refractor/oz"));
    });
  }),
  parigp: (0, _createLanguageAsyncLoader["default"])("parigp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_parigp" */"refractor/parigp"));
    });
  }),
  parser: (0, _createLanguageAsyncLoader["default"])("parser", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_parser" */"refractor/parser"));
    });
  }),
  pascal: (0, _createLanguageAsyncLoader["default"])("pascal", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_pascal" */"refractor/pascal"));
    });
  }),
  pascaligo: (0, _createLanguageAsyncLoader["default"])("pascaligo", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_pascaligo" */"refractor/pascaligo"));
    });
  }),
  pcaxis: (0, _createLanguageAsyncLoader["default"])("pcaxis", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_pcaxis" */"refractor/pcaxis"));
    });
  }),
  peoplecode: (0, _createLanguageAsyncLoader["default"])("peoplecode", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_peoplecode" */"refractor/peoplecode"));
    });
  }),
  perl: (0, _createLanguageAsyncLoader["default"])("perl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_perl" */"refractor/perl"));
    });
  }),
  phpExtras: (0, _createLanguageAsyncLoader["default"])("phpExtras", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_phpExtras" */"refractor/php-extras"));
    });
  }),
  php: (0, _createLanguageAsyncLoader["default"])("php", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_php" */"refractor/php"));
    });
  }),
  phpdoc: (0, _createLanguageAsyncLoader["default"])("phpdoc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_phpdoc" */"refractor/phpdoc"));
    });
  }),
  plantUml: (0, _createLanguageAsyncLoader["default"])("plantUml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_plantUml" */"refractor/plant-uml"));
    });
  }),
  plsql: (0, _createLanguageAsyncLoader["default"])("plsql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_plsql" */"refractor/plsql"));
    });
  }),
  powerquery: (0, _createLanguageAsyncLoader["default"])("powerquery", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_powerquery" */"refractor/powerquery"));
    });
  }),
  powershell: (0, _createLanguageAsyncLoader["default"])("powershell", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_powershell" */"refractor/powershell"));
    });
  }),
  processing: (0, _createLanguageAsyncLoader["default"])("processing", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_processing" */"refractor/processing"));
    });
  }),
  prolog: (0, _createLanguageAsyncLoader["default"])("prolog", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_prolog" */"refractor/prolog"));
    });
  }),
  promql: (0, _createLanguageAsyncLoader["default"])("promql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_promql" */"refractor/promql"));
    });
  }),
  properties: (0, _createLanguageAsyncLoader["default"])("properties", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_properties" */"refractor/properties"));
    });
  }),
  protobuf: (0, _createLanguageAsyncLoader["default"])("protobuf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_protobuf" */"refractor/protobuf"));
    });
  }),
  psl: (0, _createLanguageAsyncLoader["default"])("psl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_psl" */"refractor/psl"));
    });
  }),
  pug: (0, _createLanguageAsyncLoader["default"])("pug", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_pug" */"refractor/pug"));
    });
  }),
  puppet: (0, _createLanguageAsyncLoader["default"])("puppet", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_puppet" */"refractor/puppet"));
    });
  }),
  pure: (0, _createLanguageAsyncLoader["default"])("pure", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_pure" */"refractor/pure"));
    });
  }),
  purebasic: (0, _createLanguageAsyncLoader["default"])("purebasic", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_purebasic" */"refractor/purebasic"));
    });
  }),
  purescript: (0, _createLanguageAsyncLoader["default"])("purescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_purescript" */"refractor/purescript"));
    });
  }),
  python: (0, _createLanguageAsyncLoader["default"])("python", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_python" */"refractor/python"));
    });
  }),
  q: (0, _createLanguageAsyncLoader["default"])("q", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_q" */"refractor/q"));
    });
  }),
  qml: (0, _createLanguageAsyncLoader["default"])("qml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_qml" */"refractor/qml"));
    });
  }),
  qore: (0, _createLanguageAsyncLoader["default"])("qore", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_qore" */"refractor/qore"));
    });
  }),
  qsharp: (0, _createLanguageAsyncLoader["default"])("qsharp", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_qsharp" */"refractor/qsharp"));
    });
  }),
  r: (0, _createLanguageAsyncLoader["default"])("r", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_r" */"refractor/r"));
    });
  }),
  racket: (0, _createLanguageAsyncLoader["default"])("racket", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_racket" */"refractor/racket"));
    });
  }),
  reason: (0, _createLanguageAsyncLoader["default"])("reason", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_reason" */"refractor/reason"));
    });
  }),
  regex: (0, _createLanguageAsyncLoader["default"])("regex", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_regex" */"refractor/regex"));
    });
  }),
  rego: (0, _createLanguageAsyncLoader["default"])("rego", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_rego" */"refractor/rego"));
    });
  }),
  renpy: (0, _createLanguageAsyncLoader["default"])("renpy", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_renpy" */"refractor/renpy"));
    });
  }),
  rescript: (0, _createLanguageAsyncLoader["default"])("rescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_rescript" */"refractor/rescript"));
    });
  }),
  rest: (0, _createLanguageAsyncLoader["default"])("rest", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_rest" */"refractor/rest"));
    });
  }),
  rip: (0, _createLanguageAsyncLoader["default"])("rip", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_rip" */"refractor/rip"));
    });
  }),
  roboconf: (0, _createLanguageAsyncLoader["default"])("roboconf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_roboconf" */"refractor/roboconf"));
    });
  }),
  robotframework: (0, _createLanguageAsyncLoader["default"])("robotframework", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_robotframework" */"refractor/robotframework"));
    });
  }),
  ruby: (0, _createLanguageAsyncLoader["default"])("ruby", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_ruby" */"refractor/ruby"));
    });
  }),
  rust: (0, _createLanguageAsyncLoader["default"])("rust", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_rust" */"refractor/rust"));
    });
  }),
  sas: (0, _createLanguageAsyncLoader["default"])("sas", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sas" */"refractor/sas"));
    });
  }),
  sass: (0, _createLanguageAsyncLoader["default"])("sass", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sass" */"refractor/sass"));
    });
  }),
  scala: (0, _createLanguageAsyncLoader["default"])("scala", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_scala" */"refractor/scala"));
    });
  }),
  scheme: (0, _createLanguageAsyncLoader["default"])("scheme", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_scheme" */"refractor/scheme"));
    });
  }),
  scss: (0, _createLanguageAsyncLoader["default"])("scss", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_scss" */"refractor/scss"));
    });
  }),
  shellSession: (0, _createLanguageAsyncLoader["default"])("shellSession", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_shellSession" */"refractor/shell-session"));
    });
  }),
  smali: (0, _createLanguageAsyncLoader["default"])("smali", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_smali" */"refractor/smali"));
    });
  }),
  smalltalk: (0, _createLanguageAsyncLoader["default"])("smalltalk", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_smalltalk" */"refractor/smalltalk"));
    });
  }),
  smarty: (0, _createLanguageAsyncLoader["default"])("smarty", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_smarty" */"refractor/smarty"));
    });
  }),
  sml: (0, _createLanguageAsyncLoader["default"])("sml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sml" */"refractor/sml"));
    });
  }),
  solidity: (0, _createLanguageAsyncLoader["default"])("solidity", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_solidity" */"refractor/solidity"));
    });
  }),
  solutionFile: (0, _createLanguageAsyncLoader["default"])("solutionFile", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_solutionFile" */"refractor/solution-file"));
    });
  }),
  soy: (0, _createLanguageAsyncLoader["default"])("soy", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_soy" */"refractor/soy"));
    });
  }),
  sparql: (0, _createLanguageAsyncLoader["default"])("sparql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sparql" */"refractor/sparql"));
    });
  }),
  splunkSpl: (0, _createLanguageAsyncLoader["default"])("splunkSpl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_splunkSpl" */"refractor/splunk-spl"));
    });
  }),
  sqf: (0, _createLanguageAsyncLoader["default"])("sqf", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sqf" */"refractor/sqf"));
    });
  }),
  sql: (0, _createLanguageAsyncLoader["default"])("sql", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_sql" */"refractor/sql"));
    });
  }),
  squirrel: (0, _createLanguageAsyncLoader["default"])("squirrel", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_squirrel" */"refractor/squirrel"));
    });
  }),
  stan: (0, _createLanguageAsyncLoader["default"])("stan", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_stan" */"refractor/stan"));
    });
  }),
  stata: (0, _createLanguageAsyncLoader["default"])("stata", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_stata" */"refractor/stata"));
    });
  }),
  stylus: (0, _createLanguageAsyncLoader["default"])("stylus", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_stylus" */"refractor/stylus"));
    });
  }),
  supercollider: (0, _createLanguageAsyncLoader["default"])("supercollider", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_supercollider" */"refractor/supercollider"));
    });
  }),
  swift: (0, _createLanguageAsyncLoader["default"])("swift", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_swift" */"refractor/swift"));
    });
  }),
  systemd: (0, _createLanguageAsyncLoader["default"])("systemd", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_systemd" */"refractor/systemd"));
    });
  }),
  t4Cs: (0, _createLanguageAsyncLoader["default"])("t4Cs", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_t4Cs" */"refractor/t4-cs"));
    });
  }),
  t4Templating: (0, _createLanguageAsyncLoader["default"])("t4Templating", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_t4Templating" */"refractor/t4-templating"));
    });
  }),
  t4Vb: (0, _createLanguageAsyncLoader["default"])("t4Vb", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_t4Vb" */"refractor/t4-vb"));
    });
  }),
  tap: (0, _createLanguageAsyncLoader["default"])("tap", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_tap" */"refractor/tap"));
    });
  }),
  tcl: (0, _createLanguageAsyncLoader["default"])("tcl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_tcl" */"refractor/tcl"));
    });
  }),
  textile: (0, _createLanguageAsyncLoader["default"])("textile", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_textile" */"refractor/textile"));
    });
  }),
  toml: (0, _createLanguageAsyncLoader["default"])("toml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_toml" */"refractor/toml"));
    });
  }),
  tremor: (0, _createLanguageAsyncLoader["default"])("tremor", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_tremor" */"refractor/tremor"));
    });
  }),
  tsx: (0, _createLanguageAsyncLoader["default"])("tsx", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_tsx" */"refractor/tsx"));
    });
  }),
  tt2: (0, _createLanguageAsyncLoader["default"])("tt2", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_tt2" */"refractor/tt2"));
    });
  }),
  turtle: (0, _createLanguageAsyncLoader["default"])("turtle", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_turtle" */"refractor/turtle"));
    });
  }),
  twig: (0, _createLanguageAsyncLoader["default"])("twig", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_twig" */"refractor/twig"));
    });
  }),
  typescript: (0, _createLanguageAsyncLoader["default"])("typescript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_typescript" */"refractor/typescript"));
    });
  }),
  typoscript: (0, _createLanguageAsyncLoader["default"])("typoscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_typoscript" */"refractor/typoscript"));
    });
  }),
  unrealscript: (0, _createLanguageAsyncLoader["default"])("unrealscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_unrealscript" */"refractor/unrealscript"));
    });
  }),
  uorazor: (0, _createLanguageAsyncLoader["default"])("uorazor", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_uorazor" */"refractor/uorazor"));
    });
  }),
  uri: (0, _createLanguageAsyncLoader["default"])("uri", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_uri" */"refractor/uri"));
    });
  }),
  v: (0, _createLanguageAsyncLoader["default"])("v", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_v" */"refractor/v"));
    });
  }),
  vala: (0, _createLanguageAsyncLoader["default"])("vala", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_vala" */"refractor/vala"));
    });
  }),
  vbnet: (0, _createLanguageAsyncLoader["default"])("vbnet", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_vbnet" */"refractor/vbnet"));
    });
  }),
  velocity: (0, _createLanguageAsyncLoader["default"])("velocity", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_velocity" */"refractor/velocity"));
    });
  }),
  verilog: (0, _createLanguageAsyncLoader["default"])("verilog", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_verilog" */"refractor/verilog"));
    });
  }),
  vhdl: (0, _createLanguageAsyncLoader["default"])("vhdl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_vhdl" */"refractor/vhdl"));
    });
  }),
  vim: (0, _createLanguageAsyncLoader["default"])("vim", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_vim" */"refractor/vim"));
    });
  }),
  visualBasic: (0, _createLanguageAsyncLoader["default"])("visualBasic", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_visualBasic" */"refractor/visual-basic"));
    });
  }),
  warpscript: (0, _createLanguageAsyncLoader["default"])("warpscript", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_warpscript" */"refractor/warpscript"));
    });
  }),
  wasm: (0, _createLanguageAsyncLoader["default"])("wasm", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_wasm" */"refractor/wasm"));
    });
  }),
  webIdl: (0, _createLanguageAsyncLoader["default"])("webIdl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_webIdl" */"refractor/web-idl"));
    });
  }),
  wgsl: (0, _createLanguageAsyncLoader["default"])("wgsl", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_wgsl" */"refractor/wgsl"));
    });
  }),
  wiki: (0, _createLanguageAsyncLoader["default"])("wiki", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_wiki" */"refractor/wiki"));
    });
  }),
  wolfram: (0, _createLanguageAsyncLoader["default"])("wolfram", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_wolfram" */"refractor/wolfram"));
    });
  }),
  wren: (0, _createLanguageAsyncLoader["default"])("wren", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_wren" */"refractor/wren"));
    });
  }),
  xeora: (0, _createLanguageAsyncLoader["default"])("xeora", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_xeora" */"refractor/xeora"));
    });
  }),
  xmlDoc: (0, _createLanguageAsyncLoader["default"])("xmlDoc", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_xmlDoc" */"refractor/xml-doc"));
    });
  }),
  xojo: (0, _createLanguageAsyncLoader["default"])("xojo", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_xojo" */"refractor/xojo"));
    });
  }),
  xquery: (0, _createLanguageAsyncLoader["default"])("xquery", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_xquery" */"refractor/xquery"));
    });
  }),
  yaml: (0, _createLanguageAsyncLoader["default"])("yaml", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_yaml" */"refractor/yaml"));
    });
  }),
  yang: (0, _createLanguageAsyncLoader["default"])("yang", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_yang" */"refractor/yang"));
    });
  }),
  zig: (0, _createLanguageAsyncLoader["default"])("zig", function () {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName: "react-syntax-highlighter_languages_refractor_zig" */"refractor/zig"));
    });
  })
};