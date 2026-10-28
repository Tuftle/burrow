// What only the Windows app can do. On the web (and inside Wren) `isDesktop` is false and none of this is used.

const tauri = globalThis.__TAURI__?.core;

export const isDesktop = Boolean(tauri);
export const appVersion = () => tauri.invoke("app_version");
export const installKind = () => tauri.invoke("app_install_kind"); // "installed", "portable" or "other"
export const installUpdate = (url, sha256) => tauri.invoke("install_update", { url, sha256 });
