import type { Router } from "vue-router";

/** 有应用内上一页时后退，否则返回工具列表。 */
export const goToolList = (router: Router): void => {
  const state = window.history.state as { back?: unknown } | null;
  if (state?.back) router.back();
  else void router.push({ name: "toolbox" });
};
