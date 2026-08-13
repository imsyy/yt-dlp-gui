const BRIDGE_URL = "http://127.0.0.1:17654";

export async function collectCookies(url, tabId) {
  const parsed = new URL(url);
  const hostname = parsed.hostname;

  // 先尝试用 tab 关联的 cookie store
  let storeIds = [];
  if (Number.isInteger(tabId)) {
    const stores = await chrome.cookies.getAllCookieStores();
    console.log("[YDL GUI] all cookie stores:", stores);
    const store = stores.find((candidate) => candidate.tabIds.includes(tabId));
    if (store) storeIds.push(store.id);
  }
  // 兜底：如果没找到 store，或者第一次查询结果为空，
  // 就遍历所有 cookie store（多配置文件 / 容器场景）
  if (storeIds.length === 0) {
    const stores = await chrome.cookies.getAllCookieStores();
    storeIds = stores.map((s) => s.id);
  }
  if (!storeIds.includes("0")) storeIds.push("0");

  console.log("[YDL GUI] storeIds to query:", storeIds);

  const allCookies = [];
  const seen = new Set();
  for (const storeId of storeIds) {
    const details = { url, storeId };
    console.log(`[YDL GUI] cookie getAll:`, details);
    const cookies = await chrome.cookies.getAll(details);
    console.log(
      `[YDL GUI] storeId=${storeId}: collected ${cookies?.length || 0} cookies`,
    );
    for (const cookie of cookies || []) {
      const key = `${cookie.domain}|${cookie.path}|${cookie.name}|${storeId}`;
      if (!seen.has(key)) {
        seen.add(key);
        allCookies.push(cookie);
      }
    }
  }

  // 兜底：如果按 url 查询仍为空，尝试按 domain 查询
  if (allCookies.length === 0) {
    console.log("[YDL GUI] url query returned 0, trying domain query...");
    for (const storeId of storeIds) {
      const domainCookies = await chrome.cookies.getAll({
        domain: hostname,
        storeId,
      });
      console.log(
        `[YDL GUI] domain=${hostname}, storeId=${storeId}: collected ${domainCookies?.length || 0} cookies`,
      );
      for (const cookie of domainCookies || []) {
        const key = `${cookie.domain}|${cookie.path}|${cookie.name}|${storeId}`;
        if (!seen.has(key)) {
          seen.add(key);
          allCookies.push(cookie);
        }
      }
    }
    // 也查父域（例如 youtube.com 的 cookie 可能注册在 .youtube.com）
    const parentDomain = hostname.split(".").slice(1).join(".");
    if (parentDomain && parentDomain !== hostname) {
      for (const storeId of storeIds) {
        const parentCookies = await chrome.cookies.getAll({
          domain: `.${parentDomain}`,
          storeId,
        });
        console.log(
          `[YDL GUI] domain=.${parentDomain}, storeId=${storeId}: collected ${parentCookies?.length || 0} cookies`,
        );
        for (const cookie of parentCookies || []) {
          const key = `${cookie.domain}|${cookie.path}|${cookie.name}|${storeId}`;
          if (!seen.has(key)) {
            seen.add(key);
            allCookies.push(cookie);
          }
        }
      }
    }
  }

  console.log(
    `[YDL GUI] total unique cookies collected: ${allCookies.length}`,
  );
  if (allCookies.length) {
    console.table(
      allCookies.map((c) => ({
        domain: c.domain,
        name: c.name,
        path: c.path,
        hostOnly: c.hostOnly,
        secure: c.secure,
        httpOnly: c.httpOnly,
      })),
    );
  }
  return allCookies.map((cookie) => ({
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
  if (!(await isAppReady()) && !(await wakeApp())) {
    console.warn("[YDL GUI] app is not reachable at", BRIDGE_URL);
    throw new Error("app_unavailable");
  }
  const cookies = withCookies ? await collectCookies(url, tabId) : [];
  console.log(`[YDL GUI] sending to app: url=${url}, cookies=${cookies.length}`);
  const result = await request(
    "/v1/import",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url, requestId: crypto.randomUUID(), cookies }),
    },
    10000,
  );
  console.log("[YDL GUI] app response:", result);
  return result;
}
