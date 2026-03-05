import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "home",
      component: () => import("@/pages/Home.vue"),
    },
    {
      path: "/downloads",
      name: "downloads",
      component: () => import("@/pages/Downloads.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/pages/Settings.vue"),
    },
  ],
});

export default router;
