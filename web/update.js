// Is there a newer burrow? Asks one small public file on tuftle's site and compares version numbers. Nothing about the
// person is sent, and what comes back is checked hard before anything is shown or run.

export const FEED = "https://tuftle.vercel.app/burrow/latest.json";
const RELEASES = "https://github.com/Tuftle/burrow/releases/";

export function parseVersion(v) {
  const m = /^(\d{1,4})\.(\d{1,4})\.(\d{1,4})$/.exec(String(v));
  return m ? m.slice(1).map(Number) : null;
}

export function isNewer(latest, current) {
  const a = parseVersion(latest), b = parseVersion(current);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] > b[i];
  }
  return false;
}

const fromReleases = (u) => typeof u === "string" && u.startsWith(RELEASES) && u.length < 300 && !u.includes("..") && /^[A-Za-z0-9\-._~:/+]+$/.test(u);

// Only a feed of exactly the right shape is believed. Downloads must come from burrow's own GitHub releases.
export function readFeed(json) {
  if (!json || typeof json !== "object") throw new Error("the update file is not what was expected");
  if (!parseVersion(json.version)) throw new Error("the update file has no usable version");
  if (!fromReleases(json.url)) throw new Error("the update file points somewhere that is not burrow's releases");
  if (!/^[0-9a-f]{64}$/i.test(String(json.sha256 || ""))) throw new Error("the update file has no checksum");
  if (json.portableUrl != null && !fromReleases(json.portableUrl)) throw new Error("the update file points somewhere that is not burrow's releases");
  return {
    version: json.version,
    notes: typeof json.notes === "string" ? json.notes.slice(0, 400) : "",
    url: json.url,
    sha256: json.sha256.toLowerCase(),
    portableUrl: json.portableUrl ?? null,
  };
}

export async function checkForUpdate(current, fetchImpl = globalThis.fetch, url = FEED) {
  const res = await fetchImpl(url, { cache: "no-store", credentials: "omit", referrerPolicy: "no-referrer" });
  if (!res.ok) throw new Error("the update file could not be fetched");
  const feed = readFeed(await res.json());
  return { ...feed, available: isNewer(feed.version, current) };
}
