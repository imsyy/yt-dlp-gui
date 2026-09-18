<!-- 工具箱布局容器：各工具页自带 URL 输入 -->
<template>
  <div class="toolbox-page">
    <router-view v-slot="{ Component: RouteComponent }">
      <Transition name="fade-slide" mode="out-in">
        <component :is="RouteComponent" />
      </Transition>
    </router-view>
  </div>
</template>

<style scoped lang="scss">
.toolbox-page {
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}
</style>
