//! Alias bindings on account rows: the scope query and the strict decoders.
import { test } from "node:test";
import assert from "node:assert/strict";

import * as indexSdk from "../src/index.js";
import * as browserSdk from "../src/browser.js";
import * as toriiSdk from "../src/toriiClient.js";

const {
  ACCOUNT_ALIAS_ROW_FIELDS,
  ListQuery,
  ToriiError,
  accountAliasScope,
  accountsInAliasScope,
  decodeAccountAliasRows,
  decodeAccountAliases,
  decodePage,
  field,
} = indexSdk;

const ACCOUNT = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";

function assertInvalidResponse(message) {
  return (error) => {
    assert(error instanceof ToriiError, `${error} is a ToriiError`);
    assert.equal(error.code, "invalid_response");
    assert.match(error.message, message);
    return true;
  };
}

test("every entry point exports the account alias helpers", () => {
  for (const sdk of [indexSdk, browserSdk, toriiSdk]) {
    assert.deepEqual([...sdk.ACCOUNT_ALIAS_FIELDS], ["alias_scopes", "aliases"]);
    for (const name of ["accountAliasScope", "accountsInAliasScope", "decodeAccountAliasRows", "decodeAccountAliases"]) {
      assert.equal(typeof sdk[name], "function", name);
    }
  }
  assert.deepEqual([...ACCOUNT_ALIAS_ROW_FIELDS], ["id", "uaid", "aliases"]);
});

test("accountAliasScope is the text after `@`", () => {
  assert.equal(accountAliasScope("alice@mibank.bpng"), "mibank.bpng");
  assert.equal(accountAliasScope("alice@bpng"), "bpng");
  for (const bad of ["alice", "@bpng", "alice@", 7, undefined]) {
    assert.throws(() => accountAliasScope(bad), TypeError, String(bad));
  }
});

test("accountsInAliasScope filters on one scope and selects what the decoder requires", () => {
  const query = accountsInAliasScope("mibank.bpng", { limit: 500, cursor: "q1_abc" });
  assert.deepEqual(query.toQueryPairs(), [
    ["filter", 'alias_scopes = "mibank.bpng"'],
    ["select", "id,uaid,aliases"],
    ["limit", "500"],
    ["cursor", "q1_abc"],
  ]);
  assert.deepEqual(query.toJSON(), {
    filter: { op: "eq", args: ["alias_scopes", "mibank.bpng"] },
    select: ["id", "uaid", "aliases"],
    limit: 500,
    cursor: "q1_abc",
  });
  assert(ListQuery.from(query) === query);
  assert.deepEqual(accountsInAliasScope("bpng").toQueryPairs(), [
    ["filter", 'alias_scopes = "bpng"'],
    ["select", "id,uaid,aliases"],
  ]);
  assert.equal(
    accountsInAliasScope("mibank.bpng").filter.toString(),
    field("alias_scopes").eq("mibank.bpng").toString(),
  );
  for (const bad of ["", " mibank.bpng", "alice@mibank.bpng", 5, undefined]) {
    assert.throws(() => accountsInAliasScope(bad), TypeError, String(bad));
  }
});

test("with accountId the query is a point lookup of that account in the scope", () => {
  const query = accountsInAliasScope("mibank.bpng", { accountId: ACCOUNT, limit: 1 });
  assert.deepEqual(query.toQueryPairs(), [
    ["filter", `id = "${ACCOUNT}" and alias_scopes = "mibank.bpng"`],
    ["select", "id,uaid,aliases"],
    ["limit", "1"],
  ]);
  for (const bad of ["", ` ${ACCOUNT}`, 7, null]) {
    assert.throws(() => accountsInAliasScope("mibank.bpng", { accountId: bad }), TypeError, String(bad));
  }
});

test("a page of rows decodes to frozen, exactly shaped accounts with their aliases", () => {
  const page = decodePage({
    items: [
      {
        id: ACCOUNT,
        uaid: null,
        aliases: [
          { alias: "alice@mibank.bpng", dataspace: "bpng", domain: "mibank", is_primary: true },
          { alias: "shop@bpng", dataspace: "bpng", is_primary: false },
        ],
      },
      { id: `${ACCOUNT}x`, uaid: "uaid:abc", aliases: [] },
    ],
    next_cursor: null,
  });
  const rows = decodeAccountAliasRows(page.items);
  assert.deepEqual(rows, [
    {
      id: ACCOUNT,
      uaid: null,
      aliases: [
        { alias: "alice@mibank.bpng", dataspace: "bpng", domain: "mibank", is_primary: true },
        { alias: "shop@bpng", dataspace: "bpng", domain: null, is_primary: false },
      ],
    },
    { id: `${ACCOUNT}x`, uaid: "uaid:abc", aliases: [] },
  ]);
  assert(Object.isFrozen(rows[0]) && Object.isFrozen(rows[0].aliases) && Object.isFrozen(rows[0].aliases[0]));
  assert.deepEqual(decodeAccountAliasRows([]), []);
});

test("an absent uaid reads as null and a row may be missing nothing else", () => {
  assert.deepEqual(decodeAccountAliasRows([{ id: ACCOUNT, aliases: [] }]), [
    { id: ACCOUNT, uaid: null, aliases: [] },
  ]);
  assert.throws(() => decodeAccountAliasRows([{ id: ACCOUNT, uaid: null }]), assertInvalidResponse(/aliases must be an array/));
  assert.throws(() => decodeAccountAliasRows([{ uaid: null, aliases: [] }]), assertInvalidResponse(/id must be a non-empty string/));
});

test("the decoders reject every malformed shape and name its place", () => {
  const binding = { alias: "a@bpng", dataspace: "bpng", domain: null, is_primary: false };
  const cases = [
    [() => decodeAccountAliasRows({}), /items must be an array/],
    [() => decodeAccountAliasRows([null]), /items\[0\] must be an object/],
    [() => decodeAccountAliasRows([[]]), /items\[0\] must be an object/],
    [() => decodeAccountAliasRows([{ id: ACCOUNT, uaid: 3, aliases: [] }]), /uaid must be a string or null/],
    [() => decodeAccountAliasRows([{ id: ACCOUNT, uaid: null, aliases: [], label: null }]), /unknown field `label`/],
    [() => decodeAccountAliasRows([{ id: ACCOUNT, uaid: null, aliases: [{ ...binding, lease: 1 }] }]), /aliases\[0\] contains unknown field `lease`/],
    [() => decodeAccountAliases("a@bpng"), /must be an array/],
    [() => decodeAccountAliases([7]), /\[0\] must be an object/],
    [() => decodeAccountAliases([{ ...binding, alias: "" }]), /alias must be a non-empty string/],
    [() => decodeAccountAliases([{ ...binding, dataspace: 1 }]), /dataspace must be a non-empty string/],
    [() => decodeAccountAliases([{ ...binding, domain: "" }]), /domain must be a non-empty string/],
    [() => decodeAccountAliases([{ ...binding, is_primary: "yes" }]), /is_primary must be a boolean/],
    [() => decodeAccountAliases([{ alias: "a@bpng", dataspace: "bpng", domain: null }]), /is_primary must be a boolean/],
  ];
  for (const [run, message] of cases) assert.throws(run, assertInvalidResponse(message));
});
