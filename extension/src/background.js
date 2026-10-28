"use strict";

// Burrow as a browser extension. The vault page (app/) does the unlocking and tells this script which logins are open.
// They are kept in memory only (storage.session never touches the disk, and content scripts cannot read it), dropped on
// lock and after ten quiet minutes. A web page only ever gets the login for its own address, after a real click.
//
// The same file runs as a Firefox event page and as a Chrome service worker, which can be stopped at any time, so
// nothing is kept in plain variables between messages.

const api = globalThis.browser ?? globalThis.chrome;
if (typeof importScripts === "function") {
  importScripts("match.classic.js"); // Chrome's service worker; Firefox loads it from the manifest
}

const OWN = api.runtime.getURL("app/");
const QUIET_MINUTES = 10;
const OFFER_MS = 30 * 1000;
const ALARM = "burrow-quiet";

const blank = () => ({ locked: true, entries: [], offers: {}, pending: null, win: null });
let state = blank();

const ready = (async () => {
  try {
    const stored = await api.storage.session.get("burrow");
    if (stored && stored.burrow) state = { ...blank(), ...stored.burrow };
  } catch (e) {
    // no session storage here: state lives only as long as this script does
  }
})();
const save = async () => {
  try {
    await api.storage.session.set({ burrow: state });
  } catch (e) {
    // see above
  }
};

async function lock() {
  state = blank();
  await save();
  api.alarms.clear(ALARM);
  api.runtime.sendMessage({ type: "vault:lockNow" }).catch(() => {}); // an open vault page locks too
}

function touch() {
  if (!state.locked) {
    api.alarms.create(ALARM, { delayInMinutes: QUIET_MINUTES });
  }
}

api.alarms.onAlarm.addListener(alarm => {
  if (alarm.name == ALARM) {
    ready.then(lock);
  }
});

const fromOwnPage = sender => Boolean(sender.url && sender.url.startsWith(OWN));
const pageUrl = sender => (sender.frameId === undefined || sender.frameId === 0 ? sender.url || (sender.tab && sender.tab.url) || "" : "");
const matching = url => state.entries.filter(e => self.burrowMatch.matches(e, url));

async function openApp(fresh) {
  if (!fresh && state.win != null) {
    try {
      await api.windows.update(state.win, { focused: true });
      return;
    } catch (e) {
      // that window is gone
    }
  }
  const w = await api.windows.create({ type: "popup", url: OWN + "index.html", width: 480, height: 700 });
  if (!fresh) {
    state.win = w.id;
    await save();
  }
}

api.windows.onRemoved.addListener(async id => {
  await ready;
  if (state.win === id) {
    state.win = null;
    await save();
  }
});

async function handle(message, sender) {
  await ready;
  switch (message.type) {
    // ---- from the vault page ----
    case "vault:state":
      if (!fromOwnPage(sender)) return null;
      state.locked = Boolean(message.locked);
      state.entries = state.locked ? [] : (message.entries || []).map(e => ({ id: e.id, site: e.site, url: e.url, user: e.user, pass: e.pass }));
      if (state.locked) state.offers = {};
      await save();
      touch();
      return true;
    case "vault:takePending": {
      if (!fromOwnPage(sender)) return null;
      const p = state.pending;
      state.pending = null;
      await save();
      return p;
    }
    case "vault:lock":
      if (!fromOwnPage(sender)) return null;
      await lock();
      return true;
    case "vault:status":
      if (!fromOwnPage(sender)) return null;
      return { locked: state.locked, count: state.entries.length };

    // ---- from a web page's content script ----
    case "vault:matches":
      if (state.locked) return { locked: true, list: [] };
      touch();
      return { locked: false, list: matching(pageUrl(sender)).map(e => ({ id: e.id, user: e.user, site: e.site })) };
    case "vault:fill": {
      if (state.locked) return null;
      touch();
      // the page is checked again here, so a page cannot ask for another site's login by id
      const e = matching(pageUrl(sender)).find(x => x.id == message.id);
      return e ? { user: e.user, pass: e.pass } : null;
    }
    case "vault:submitted": {
      if (state.locked || !sender.tab || !message.pass) return null;
      const url = pageUrl(sender);
      if (matching(url).some(e => e.user == message.user && e.pass == message.pass)) return false; // already saved
      state.offers[sender.tab.id] = { user: String(message.user || ""), pass: String(message.pass), url, at: Date.now() };
      await save();
      return true;
    }
    case "vault:offer": {
      const o = sender.tab && state.offers[sender.tab.id];
      if (!o || state.locked || Date.now() - o.at > OFFER_MS) return null;
      return { user: o.user };
    }
    case "vault:startSave": {
      const o = sender.tab && state.offers[sender.tab.id];
      if (!o || state.locked || Date.now() - o.at > OFFER_MS) return false;
      delete state.offers[sender.tab.id];
      let host = "";
      try {
        host = new URL(o.url).hostname.replace(/^www\./, "");
      } catch (e) {
        // keep it empty
      }
      state.pending = { site: host, url: o.url, user: o.user, pass: o.pass };
      await save();
      await openApp(true);
      return true;
    }
    default:
      return undefined;
  }
}

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || typeof message.type != "string" || !message.type.startsWith("vault:")) {
    return false;
  }
  // sendResponse plus `return true` is the form both browsers understand
  handle(message, sender).then(sendResponse, () => sendResponse(null));
  return true;
});

api.tabs.onRemoved.addListener(async id => {
  await ready;
  if (state.offers[id]) {
    delete state.offers[id];
    await save();
  }
});

(api.action ?? api.browserAction).onClicked.addListener(() => openApp(false));

api.commands.onCommand.addListener(async command => {
  await ready;
  if (command == "open") {
    openApp(false);
  } else if (command == "fill") {
    const [tab] = await api.tabs.query({ active: true, currentWindow: true });
    if (!tab) return;
    if (state.locked) {
      openApp(false);
    } else {
      api.tabs.sendMessage(tab.id, { type: "vault:fillNow" }).catch(() => {});
    }
  }
});
