const BRIDGE_URL = "http://127.0.0.1:17654";

/**
 * 收集指定页面 URL 相关的浏览器 Cookie
 * 优先按当前 Tab 所在的 cookie store 查询，兜底按域名及父域查询
 *
 * @param {string} url 目标网页完整 URL
 * @param {number} [tabId] 当前标签页 ID（可选）
 * @returns {Promise<Array<{domain: string, hostOnly: boolean, path: string, secure: boolean, httpOnly: boolean, expirationDate?: number, name: string, value: string}>>} 提取的标准 Cookie 数组
 */
export const collectCookies = async (url, tabId) => {
  const parsed = new URL(url);
  const hostname = parsed.hostname;

  // 先尝试用 tab 关联的 cookie store
  let storeIds = [];
  if (Number.isInteger(tabId)) {
    const stores = await chrome.cookies.getAllCookieStores();
    const matchedStore = stores.find((candidate) => candidate.tabIds.includes(tabId));
    if (matchedStore) storeIds.push(matchedStore.id);
  }
  // 兜底：如果没找到 store，或者第一次查询结果为空，就遍历所有 cookie store
  if (storeIds.length === 0) {
    const stores = await chrome.cookies.getAllCookieStores();
    storeIds = stores.map((storeCandidate) => storeCandidate.id);
  }
  if (!storeIds.includes("0")) storeIds.push("0");

  const allCookies = [];
  const seen = new Set();
  for (const storeId of storeIds) {
    const details = { url, storeId };
    const cookies = await chrome.cookies.getAll(details);
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
    for (const storeId of storeIds) {
      const domainCookies = await chrome.cookies.getAll({
        domain: hostname,
        storeId,
      });
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
};

/**
 * 批量收集多个标签页 URL 的 Cookie 并进行全局去重
 *
 * @param {Array<{url: string, tabId?: number}>} items 待收集的网页信息列表
 * @returns {Promise<Array<object>>} 去重后的 Cookie 数组
 */
export const collectCookiesForUrls = async (items) => {
  const allCookies = [];
  const seen = new Set();
  for (const item of items) {
    if (!item?.url) continue;
    const cookies = await collectCookies(item.url, item.tabId);
    for (const cookie of cookies) {
      const key = `${cookie.domain}|${cookie.path}|${cookie.name}`;
      if (!seen.has(key)) {
        seen.add(key);
        allCookies.push(cookie);
      }
    }
  }
  return allCookies;
};

/**
 * 发送带超时控制与 JSON 解析的本地 HTTP 桥接请求
 *
 * @param {string} path 请求接口相对路径
 * @param {RequestInit} [options] Fetch 配置项
 * @param {number} [timeout=2500] 超时时间（毫秒）
 * @returns {Promise<any>} 响应数据
 */
const request = async (path, options = {}, timeout = 2500) => {
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
};

/**
 * 检查桌面端应用的本地 HTTP 桥接服务是否已准备就绪
 *
 * @returns {Promise<boolean>} 应用是否正常运行中
 */
export const isAppReady = async () => {
  try {
    const health = await request("/v1/health", {}, 800);
    return health?.app === "ydl-gui" && health?.version === 1;
  } catch {
    return false;
  }
};

/**
 * 通过 Windows 协议深链接冷启动拉起桌面应用，并轮询等待服务可用：
 * 预留 28 次 * 300ms（约 8.4 秒）冷启动缓冲，就绪后平滑销毁唤醒标签页
 *
 * @returns {Promise<boolean>} 是否成功唤醒并就绪
 */
export const wakeApp = async () => {
  const wakeUrl = `ytdlp-gui://bridge/wake?requestId=${crypto.randomUUID()}`;
  let tab = null;
  try {
    tab = await chrome.tabs.create({ url: wakeUrl, active: false });
  } catch (err) {
    console.warn("[YDL GUI] failed to create wake tab:", err);
  }

  const cleanupTab = () => {
    if (tab?.id) {
      chrome.tabs.remove(tab.id).catch(() => {});
      tab = null;
    }
  };

  for (let attempt = 0; attempt < 28; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 300));
    if (await isAppReady()) {
      cleanupTab();
      return true;
    }
  }

  cleanupTab();
  return false;
};

/**
 * 将单个或多个网页链接及对应 Cookies 发送至 YDL GUI 桌面端
 *
 * @param {string | string[]} target 单个目标 URL 或 URL 列表
 * @param {object | boolean} [options={}] 配置项（或布尔值表示 withCookies）
 * @param {boolean} [options.withCookies=true] 是否一并提取并发送 Cookie
 * @param {"standard" | "batch"} [options.mode="standard"] 目标模式（标准或批量）
 * @param {number} [options.tabId] 单个标签页 ID
 * @param {Array<{url: string, tabId?: number}>} [options.items] 批量标签页信息列表
 * @param {number} [legacyTabId] 兼容旧签名的 tabId 参数
 * @returns {Promise<any>} 后端返回的处理结果
 */
export const sendToApp = async (target, options = {}, legacyTabId = null) => {
  let urls = [];
  let withCookies = true;
  let mode = "standard";
  let tabId = undefined;
  let items = [];

  if (typeof options === "boolean") {
    withCookies = options;
    tabId = legacyTabId;
  } else if (options && typeof options === "object") {
    if (typeof options.withCookies === "boolean") withCookies = options.withCookies;
    if (options.mode) mode = options.mode;
    if (options.tabId) tabId = options.tabId;
    if (Array.isArray(options.items)) items = options.items;
  }

  if (Array.isArray(target)) {
    urls = target.filter(Boolean);
  } else if (typeof target === "string" && target) {
    urls = [target];
  }

  if (urls.length === 0) {
    throw new Error("no_urls");
  }

  if (!(await isAppReady()) && !(await wakeApp())) {
    console.warn("[YDL GUI] app is not reachable at", BRIDGE_URL);
    throw new Error("app_unavailable");
  }

  let cookies = [];
  if (withCookies) {
    if (items.length > 0) {
      cookies = await collectCookiesForUrls(items);
    } else if (urls.length === 1) {
      cookies = await collectCookies(urls[0], tabId);
    } else {
      cookies = await collectCookiesForUrls(urls.map((urlItem) => ({ url: urlItem })));
    }
  }

  const primaryUrl = urls[0];
  console.log(
    `[YDL GUI] sending to app: mode=${mode}, urls=${urls.length}, cookies=${cookies.length}`,
  );

  const result = await request(
    "/v1/import",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        url: primaryUrl,
        urls,
        mode,
        requestId: crypto.randomUUID(),
        cookies,
      }),
    },
    10000,
  );
  console.log("[YDL GUI] app response:", result);
  return result;
};
