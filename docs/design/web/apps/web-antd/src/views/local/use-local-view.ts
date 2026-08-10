import { onBeforeUnmount, onMounted, ref } from 'vue';

function isLocalViewPayload(value: unknown) {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const meta = Reflect.get(value, 'meta');
  const schemaVersion =
    typeof meta === 'object' && meta !== null
      ? Reflect.get(meta, 'schema_version')
      : undefined;
  return (
    typeof meta === 'object' &&
    meta !== null &&
    (schemaVersion === '1' ||
      schemaVersion === '1.0' ||
      schemaVersion === 'i4.1')
  );
}

interface LocalViewOptions {
  /**
   * Optional page-specific projection contract.  Schema version alone is not
   * enough to safely render a view whose required fields are absent.
   */
  isValidPayload?: (value: unknown) => boolean;
  refreshIntervalMs?: number;
}

interface ReloadOptions {
  background?: boolean;
}

export function useLocalView<T>(
  load: () => Promise<T>,
  options: LocalViewOptions = {},
) {
  const data = ref<T>();
  const error = ref('');
  const loading = ref(true);
  let loadingRequest = false;
  let refreshTimer: ReturnType<typeof setInterval> | undefined;

  async function reload(reloadOptions: ReloadOptions = {}) {
    if (loadingRequest) return;
    const background = reloadOptions.background === true;
    loadingRequest = true;
    if (!background) {
      loading.value = true;
      error.value = '';
    }
    try {
      const response = await load();
      if (
        !isLocalViewPayload(response) ||
        (options.isValidPayload && !options.isValidPayload(response))
      ) {
        throw new Error('The local view response has an unsupported schema version.');
      }
      data.value = response;
      // A failed initial read must recover when the local Agent becomes
      // reachable again; otherwise the page remains fail-closed forever even
      // though a later polling response is valid.
      error.value = '';
    } catch (reason) {
      const detail =
        reason instanceof Error && reason.message
          ? reason.message
          : 'unknown local-view error';
      console.error('Local view request failed.', reason);
      if (background && data.value) return;
      error.value =
        `本机只读视图暂不可用：${detail}`;
    } finally {
      if (!background) loading.value = false;
      loadingRequest = false;
    }
  }

  onMounted(() => {
    void reload();
    if (options.refreshIntervalMs && options.refreshIntervalMs > 0) {
      refreshTimer = setInterval(
        () => void reload({ background: true }),
        options.refreshIntervalMs,
      );
    }
  });

  onBeforeUnmount(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });

  return { data, error, loading, reload };
}
