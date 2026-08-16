"use strict";

/*
 * Compact Unicode 17.0 DerivedBidiClass data for every explicit non-L range.
 * UTS-46 rejects unassigned scalars before this lookup, so the UCD default L
 * value is sufficient for code points absent from the generated table.
 *
 * Source: https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedBidiClass.txt
 */
const NON_L_BIDI_CLASSES = "AL,AN,B,BN,CS,EN,ES,ET,FSI,LRE,LRI,LRO,NSM,ON,PDF,PDI,R,RLE,RLI,RLO,S,WS".split(",");
const PACKED_NON_L_BIDI_RANGES =
  "`~j1!%!$!0QE$$~*!2#*#-5$#$?@z.!~u!,(A3K~q~?(~8!3~x$5a%WK.~:!&$~L!~!HE~Q%'~/!7.*~6~`'eo^~-#%$<$#$!%!$+$%$!$!)!'!$!$!$$$#$!%!$!$!$!$!$#$!%%$($%$%$!$+$2($$&$2+~b1'}+$#~7!!~e%#s!~@~q(+)+~%$@',!%!1$~-!!~&y!B!*(/~*!'$;0!~'b!~77$u'''~#~a'A~z#!~[#*(#~s~a&%~vZ)~J~F!#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(#~A~e(~k#~[#~-H~m~8(#~A~e(#~A~e(#/N!$#-!~,!!~*0!~ei!7!~r~b'!$!%!~}!!$#-!0R+~?!#(!~=2+~qg!&')+~-,5~s~X'+~R+<~{~*'+~/?S~Wf,~@?++M!$!~sz#1#~9%!~)~U'!~{)#~n!!$!:E$~C!%-#~T.!~@!#~&!!~n*#*!~`#!~r#!~@'!~f;!~38&~2!C~3!!~O#!~;~&%#~d~X#!,#~_!$~E##&#~yy%~y~s&!#~2{!#~Pz!#~0{!#~Sz!~v$~B)~5!~!$(~r#N$!$#$#$!k,S63!~,!(%'%#$%F!A<~!!,]*,!;%$*$$$&N$^*M9$AZ!$!')'!&(-#@!]!'%+!7#=!%#\\!'#'#%$&!A#&!.#\\!'&$#'!7#9'$!]!%!$%+!*#.#A!`!/!U!&!Z!$$($$%*#.#@!]!2#7#?#\\#'%+!7#@!k!*$$!}!%(/)~)!!%*.(l#>!$!$!Z/$&$#(,$E,!~-!%$'$#%#<#'$3%0!%#)!2!~^($~=+$@#A#A#c#$(+!%,,!P$$!~<!#E!~=!$'#,!)$~F##%!]!$($!$!%))+%!SO%-7%S!$&$!(!K*/#C%%#$$[!$#&!$$])%#~_!$$.$('!)!&#~1#a~o(B~EB$~T!!~'!A~%'%~2!#~s~T$%$+C#s#~{#!&!'!<#(!~^!#=30!I)<,Q$S!%%%#J!f'%#%#/!+!R!V!$$%#(#$!M#+!~Y#!%!'!~C~E#!~$)131~X+!~M#!~\\!&~13$$#(%K$'!~l!#~8'%d&~L$#p'i,T%~B!!Y0L!%#-$T%%#*!`$G&$)a!/#W*-%%!~&!$%!$#)!%!~d!!&)8#\\#&!H(&&i'0!$!$!1#x)%$$!:!w'$!'#$#~Y#%)#$#>#x)%!$#~1!!$!%'$!~,!!$!%%$&~n#*$#~k##$!'!~W!%%#'!C'%#K'%%+!,'%$Q.$#~1#!$$$!~4#($'w7%($#$#~A!'&!$#$($!k#&!$!~j$#.#W&(!$!:!~0[!)0~[~A!-&$~/<&^(~G,!b%t!~O~>##~0TO%8~w&$4)%(A%~[!$~R6X'S+!1!9&$0~k/($2%($#$&~+!!~g!(~*%!`%~S&%~i##~^#!%!*#(!~1&(~4!(~0~[~!!~Y#~7#C#&&3'='=%E!'%$#$#'!$$%&:!B!~t%#*/%/(*$2~;!#+!(#$!~5!!~F%!%#~>!#)#~9#!-!~w#%~(,'$!~D!(~Z(%~%-+~-!!~9(#~b$+),~D$!&#~_!C~|0!$$.$0$0$0#4900$;@$0$~8!#$%$#-!$$('$!$!$!3#'&(%%1L$'~G!%~/$h;$~Y!9,8I~)!~s%$~`$~k#~m'%~O!~P#'1(~k#~#!E;$z/~?#=1$%&:2!(#($~!!#&!}!~/#G,!P#T1?$U0/%~n!%~*!#B!~_ha~s~Y#X~U$$~*!!-#~G!C~-!!~f!%k%~5)#~V~H#1~z$3c#Y)P$3+9A$!%!$*$#%$$!%!~\\!#&&3'='=,~C!$&(-&~n#!an&.&!~q5!~p&(~)'!~p$*~,%5~]1.~3;)'2~K~G#!~@~i#~?#G$&~e%)822~;)#8c&!~%#x~Q*!<!B!<!B!<!B!<!B!<!~Oa#~y#M'~)!/0%0$0$F8&B!]'`!~y!'~a!~b+&2&.&~C#)-'!2-'Y++)I+?%-'#1*J~e$+/%.&,&Z$!'1%-'+*~X!$}-!#~Rz!#~3{!n~|0!$!%!%!,<''~!&L,#'!&9'!,!&!*0$:(!~wb!~T~Z'!$+$.$&$!$#$#$+~?D'%!$M$#&!%8$i+*S4$#(B';(<IY'5%P2%$$$>-****aCF)-,W-7%<(;*%/(sjZT0T*'m=,8+#~[#K&!%#qIk3'%I=78~0~R'~.#%*Se*!'+'##~Qz!#~1{!#~Tz!%+!$!6!).!6!~:_!~J;,@!Y!~OL!";

let decodedBidiRanges;

function decodeBidiRanges() {
  if (decodedBidiRanges !== undefined) {
    return decodedBidiRanges;
  }

  const alphabet = "!#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}";
  let offset = 0;
  const readVarint = () => {
    let value = 0;
    let multiplier = 1;
    while (PACKED_NON_L_BIDI_RANGES[offset] === "~") {
      offset += 1;
      value += alphabet.indexOf(PACKED_NON_L_BIDI_RANGES[offset]) * multiplier;
      offset += 1;
      multiplier *= alphabet.length;
      value += multiplier;
    }
    value += alphabet.indexOf(PACKED_NON_L_BIDI_RANGES[offset]) * multiplier;
    offset += 1;
    return value;
  };
  const ranges = [];
  for (const type of NON_L_BIDI_CLASSES) {
    const count = readVarint();
    let previousEnd = 0;
    for (let index = 0; index < count; index += 1) {
      const start = previousEnd + readVarint();
      const end = start + readVarint();
      ranges.push({ start, end, type });
      previousEnd = end;
    }
  }
  ranges.sort((left, right) => left.start - right.start);
  decodedBidiRanges = Object.freeze(ranges);
  return decodedBidiRanges;
}

function bidiClass(character) {
  const codePoint = character.codePointAt(0);
  const ranges = decodeBidiRanges();
  let low = 0;
  let high = ranges.length - 1;
  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    const range = ranges[middle];
    if (codePoint < range.start) {
      high = middle - 1;
    } else if (codePoint > range.end) {
      low = middle + 1;
    } else {
      return range.type;
    }
  }
  return "L";
}

const RTL_ALLOWED = /* @__PURE__ */ new Set(["R", "AL", "AN", "EN", "ES", "CS", "ET", "ON", "BN", "NSM"]);
const LTR_ALLOWED = /* @__PURE__ */ new Set(["L", "EN", "ES", "CS", "ET", "ON", "BN", "NSM"]);

/**
 * Apply all six RFC 5893 rules when the decoded label is a Bidi label.
 *
 * WHATWG URL host processing supplies nontransitional UTS-46 mapping and
 * ContextJ checks. This explicit classifier closes platform gaps where an ACE
 * input is otherwise round-tripped without applying the RFC 5893 rules.
 */
export function satisfiesIdnaBidiRule(value) {
  const types = Array.from(value, bidiClass);
  if (!types.some((type) => type === "R" || type === "AL" || type === "AN")) {
    return true;
  }

  const first = types[0];
  const rtl = first === "R" || first === "AL";
  if (!rtl && first !== "L") {
    return false;
  }

  const allowed = rtl ? RTL_ALLOWED : LTR_ALLOWED;
  if (!types.every((type) => allowed.has(type))) {
    return false;
  }

  let finalIndex = types.length - 1;
  while (finalIndex >= 0 && types[finalIndex] === "NSM") {
    finalIndex -= 1;
  }
  const finalType = types[finalIndex];
  if (rtl) {
    if (!["R", "AL", "EN", "AN"].includes(finalType)) {
      return false;
    }
    if (types.includes("EN") && types.includes("AN")) {
      return false;
    }
    return true;
  }

  return finalType === "L" || finalType === "EN";
}
