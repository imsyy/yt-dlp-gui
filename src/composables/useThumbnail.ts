import { invoke } from "@tauri-apps/api/core";
import { useSettingStore } from "@/stores/setting";

/** Load proxied images through Rust; object URLs are revoked when replaced or unmounted. */
export const useThumbnail = (url: MaybeRefOrGetter<string>) => {
  const settings = useSettingStore();
  const image = shallowRef<Blob>();
  const objectUrl = useObjectUrl(image);
  const failed = ref(false);

  watch(
    [() => toValue(url), () => settings.proxy],
    async ([currentUrl, proxy], _, onCleanup) => {
      let active = true;
      onCleanup(() => {
        active = false;
      });
      image.value = undefined;
      failed.value = false;
      if (!currentUrl || !proxy) return;
      try {
        const bytes = await invoke<ArrayBuffer>("fetch_thumbnail", { url: currentUrl, proxy });
        if (active) image.value = new Blob([bytes]);
      } catch {
        if (active) failed.value = true;
      }
    },
    { immediate: true },
  );

  const src = computed(() => (settings.proxy ? objectUrl.value || "" : toValue(url)));
  return { src, failed };
};
