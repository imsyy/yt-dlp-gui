const BRIDGE_URL = "http://127.0.0.1:17654";

export async function collectCookies(url, tabId) {
  const details = { url };
  if (Number.isInteger(tabId)) {
    const stores = await chrome.cookies.getAllCookieStores();
    const store = stores.find((candidate) => candidate.tabIds.includes(tabId));
    if (store) details.storeId = store.id;
  }
  const cookies = await chrome.cookies.getAll(details);
  return (cookies || []).map((cookie) => ({
    domain: cookie.domain,
    hostOnly: cookie.hostOnly,
    path: cookie.path || "/",
    secure: cookie.secure,
    httpOnly: cookie.httpOnly,
    expirationDate: cookie.expirationDate,
    name: cookie.name,
    value: cookie.value,
  }));
}

async function request(path, options = {}, timeout = 2500) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeout);
  try {
    const response = await fetch(`${BRIDGE_URL}${path}`, {
      ...options,
      signal: controller.signal,
      cache: "no-store",
    });
    if (!response.ok) throw new Error(`bridge_http_${response.status}`);
    return response.status === 204 ? null : response.json();
  } finally {
    clearTimeout(timer);
  }
}

export async function isAppReady() {
  try {
    const health = await request("/v1/health", {}, 800);
    return health?.app === "ydl-gui" && health?.version === 1;
  } catch {
    return false;
  }
}

export async function wakeApp() {
  const wakeUrl = `ytdlp-gui://bridge/wake?requestId=${crypto.randomUUID()}`;
  const tab = await chrome.tabs.create({ url: wakeUrl, active: false });
  setTimeout(() => tab?.id && chrome.tabs.remove(tab.id).catch(() => {}), 1200);
  for (let attempt = 0; attempt < 12; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 250));
    if (await isAppReady()) return true;
  }
  return false;
}

export async function sendToApp(url, withCookies, tabId) {
  if (withCookies) {
    const parsed = new URL(url);
    const origins = [`${parsed.protocol}//${parsed.host}/*`];
    const granted = await chrome.permissions.request({ origins });
    if (!granted) throw new Error("cookie_permission_denied");
  }
  if (!(await isAppReady()) && !(await wakeApp())) throw new Error("app_unavailable");
  const cookies = withCookies ? await collectCookies(url, tabId) : [];
  return request(
    "/v1/import",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url, requestId: crypto.randomUUID(), cookies }),
    },
    10000,
  );
}
