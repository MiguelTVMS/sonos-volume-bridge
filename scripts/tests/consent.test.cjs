const { test } = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const fs = require("node:fs");
const path = require("node:path");
const source = fs.readFileSync(
  path.join(__dirname, "../../pages/consent.js"),
  "utf8",
);
const KEY = "svb-consent-v1";
function setup(saved, blocked = false) {
  const nodes = new Map(),
    appended = [],
    handlers = {},
    values = new Map();
  if (saved !== undefined)
    values.set(KEY, typeof saved === "string" ? saved : JSON.stringify(saved));
  function node(id) {
    if (!nodes.has(id))
      nodes.set(id, {
        hidden: true,
        checked: false,
        disabled: false,
        handlers: {},
        addEventListener(name, fn) {
          this.handlers[name] = fn;
        },
        setAttribute(name, value) {
          this[name] = value;
        },
        focus() {},
      });
    return nodes.get(id);
  }
  const location = {
    hostname: "svb.example.test",
    pathname: "/",
    reload() {
      location.reloads++;
    },
    reloads: 0,
  };
  const document = {
    getElementById: node,
    createElement: () => ({}),
    head: { appendChild: (s) => appended.push(s) },
    cookie: "",
  };
  const window = { addEventListener: (name, fn) => (handlers[name] = fn) };
  vm.runInNewContext(source, {
    window,
    document,
    location,
    Date,
    localStorage: {
      getItem: (k) => values.get(k) || null,
      setItem: (k, v) => {
        if (blocked) throw Error();
        values.set(k, v);
      },
    },
  });
  return {
    node,
    appended,
    location,
    window,
    values,
    handlers,
    click: (id) => node(id).handlers.click(),
  };
}
function consent(state) {
  return [...state.window.dataLayer]
    .filter((x) => x[0] === "consent")
    .at(-1)[2];
}
const saved = (choices = {}) => ({
  version: 1,
  time: Date.now(),
  analytics: false,
  advertising: false,
  personalization: false,
  ...choices,
});
test("first visit queues denial and makes no Google request", () => {
  const s = setup();
  assert.equal(s.appended.length, 0);
  assert.equal(s.node("consent-banner").hidden, false);
  assert.equal(consent(s).analytics_storage, "denied");
  assert.equal(consent(s).ad_user_data, "denied");
});
test("reject persists denial without loading GTM", () => {
  const s = setup();
  s.click("consent-reject");
  assert.equal(s.appended.length, 0);
  assert.equal(JSON.parse(s.values.get(KEY)).analytics, false);
  assert.equal(s.node("consent-banner").hidden, true);
});
test("analytics only keeps all advertising denied", () => {
  const s = setup();
  s.node("consent-analytics").checked = true;
  s.click("consent-save");
  assert.equal(s.appended.length, 1);
  assert.match(s.appended[0].src, /GTM-P7RPRQBZ$/);
  assert.equal(consent(s).analytics_storage, "granted");
  assert.equal(consent(s).ad_storage, "denied");
  assert.equal(consent(s).ad_personalization, "denied");
});
test("advertising without personalization or analytics", () => {
  const s = setup();
  s.node("consent-advertising").checked = true;
  s.click("consent-save");
  assert.equal(consent(s).analytics_storage, "denied");
  assert.equal(consent(s).ad_user_data, "granted");
  assert.equal(consent(s).ad_personalization, "denied");
});
test("personalization cannot be granted alone", () => {
  const s = setup();
  s.node("consent-personalization").checked = true;
  s.click("consent-save");
  assert.equal(s.appended.length, 0);
  assert.equal(consent(s).ad_personalization, "denied");
});
test("accept all loads once with signals preceding GTM", () => {
  const s = setup();
  s.click("consent-accept");
  s.click("consent-accept");
  assert.equal(s.appended.length, 1);
  assert.equal(consent(s).ad_personalization, "granted");
  const data = s.window.dataLayer;
  assert(
    data.findIndex((x) => x[0] === "consent" && x[1] === "update") <
      data.findIndex((x) => x.event === "gtm.js"),
  );
});
test("valid returning choices respected", () => {
  assert.equal(setup(saved()).appended.length, 0);
  assert.equal(setup(saved({ analytics: true })).appended.length, 1);
});
test("expired, future, invalid and malformed choices fail closed", () => {
  for (const value of [
    "{bad",
    saved({ time: 0 }),
    saved({ time: Date.now() + 100000 }),
    saved({ analytics: "yes" }),
    saved({ version: 0 }),
  ]) {
    const s = setup(value);
    assert.equal(s.appended.length, 0);
    assert.equal(s.node("consent-banner").hidden, false);
  }
});
test("storage failure never grants tracking", () => {
  const s = setup(undefined, true);
  s.click("consent-accept");
  assert.equal(s.appended.length, 0);
  assert.match(s.node("consent-status").textContent, /could not save/);
});
test("withdrawal persists rejection and reloads", () => {
  const s = setup(saved({ analytics: true }));
  s.click("consent-open");
  s.click("consent-reject");
  assert.equal(s.location.reloads, 1);
  assert.equal(JSON.parse(s.values.get(KEY)).analytics, false);
});
test("advanced settings are expandable and personalization follows advertising", () => {
  const s = setup();
  s.click("consent-customize");
  assert.equal(s.node("consent-advanced").hidden, false);
  assert.equal(s.node("consent-customize")["aria-expanded"], "true");
  s.node("consent-advertising").checked = true;
  s.node("consent-advertising").handlers.change();
  assert.equal(s.node("consent-personalization").disabled, false);
});
test("cross-tab and bfcache navigation refresh consent", () => {
  const s = setup();
  s.handlers.storage({ key: KEY });
  s.handlers.pageshow({ persisted: true });
  assert.equal(s.location.reloads, 2);
});
