import { createRouter, createWebHistory } from "vue-router";
import Home from "@/pages/Home.vue";
import Pending from "@/pages/Pending.vue";
import Downloads from "@/pages/Downloads.vue";
import Toolbox from "@/pages/Toolbox.vue";
import Settings from "@/pages/Settings.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "home",
      component: Home,
    },
    {
      path: "/pending",
      name: "pending",
      component: Pending,
    },
    {
      path: "/downloads",
      name: "downloads",
      component: Downloads,
    },
    {
      path: "/toolbox",
      component: Toolbox,
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
          path: "channel",
          name: "toolbox-channel",
          component: () => import("@/pages/toolbox/ChannelArchive.vue"),
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
      component: Settings,
    },
  ],
});

export default router;
