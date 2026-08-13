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
      path: "/pending",
      name: "pending",
      component: () => import("@/pages/Pending.vue"),
    },
    {
      path: "/downloads",
      name: "downloads",
      component: () => import("@/pages/Downloads.vue"),
    },
    {
      path: "/toolbox",
      component: () => import("@/pages/Toolbox.vue"),
      children: [
        {
          path: "",
          name: "toolbox",
          component: () => import("@/pages/toolbox/ToolList.vue"),
        },
        {
          path: "thumbnail",
          name: "toolbox-thumbnail",
          component: () => import("@/pages/toolbox/Thumbnail.vue"),
        },
        {
          path: "subtitles",
          name: "toolbox-subtitles",
          component: () => import("@/pages/toolbox/Subtitles.vue"),
        },
        {
          path: "livechat",
          name: "toolbox-livechat",
          component: () => import("@/pages/toolbox/LiveChat.vue"),
        },
        {
          path: "chapters",
          name: "toolbox-chapters",
          component: () => import("@/pages/toolbox/Chapters.vue"),
        },
        {
          path: "comments",
          name: "toolbox-comments",
          component: () => import("@/pages/toolbox/Comments.vue"),
        },
        {
          path: "plugins",
          name: "toolbox-plugins",
          component: () => import("@/pages/toolbox/Plugins.vue"),
        },
        {
          path: "browser-extension",
          name: "toolbox-browser-extension",
          component: () => import("@/pages/toolbox/BrowserExtension.vue"),
        },
      ],
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/pages/Settings.vue"),
    },
  ],
});

export default router;
