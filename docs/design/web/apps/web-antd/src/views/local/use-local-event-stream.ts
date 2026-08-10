import type { LocalEventTopic, LocalViewEvent } from '#/api/local-views';

import { onBeforeUnmount, onMounted, ref } from 'vue';

export type LocalEventStreamStatus = 'CONNECTED' | 'FALLBACK' | 'RECONNECTING';

interface LocalEventStreamOptions {
  onEvent: (event: LocalViewEvent) => void;
  topics?: LocalEventTopic[];
}

const eventNames: LocalViewEvent['event'][] = [
  'media.changed',
  'runtime.changed',
  'task.changed',
];

export function parseLocalViewEvent(
  event: MessageEvent<string>,
): LocalViewEvent | null {
  try {
    const payload: unknown = JSON.parse(event.data);
    if (
      typeof payload !== 'object' ||
      payload === null ||
      !eventNames.includes(event.type as LocalViewEvent['event']) ||
      !Number.isSafeInteger(Reflect.get(payload, 'revision'))
    ) {
      return null;
    }
    const topic = event.type.split('.')[0] as LocalEventTopic;
    return {
      event: event.type as LocalViewEvent['event'],
      revision: Reflect.get(payload, 'revision') as number,
      topic,
    };
  } catch {
    return null;
  }
}

/**
 * Subscribes to the local Agent event stream. HTTP snapshots remain the source
 * of rendered truth: events only tell a page which snapshot to refresh.
 */
export function useLocalEventStream(options: LocalEventStreamOptions) {
  const status = ref<LocalEventStreamStatus>('RECONNECTING');
  let source: EventSource | undefined;
  let stopped = false;

  function connect() {
    if (stopped || typeof EventSource === 'undefined') {
      status.value = 'FALLBACK';
      return;
    }
    const topics = options.topics?.join(',') ?? 'media,runtime,task';
    source = new EventSource(`/api/local/v2/events?topics=${encodeURIComponent(topics)}`);
    source.onopen = () => {
      status.value = 'CONNECTED';
    };
    source.onerror = () => {
      // EventSource retries itself.  Keep the last HTTP snapshot visible and
      // let the page enable its low-frequency fallback while reconnecting.
      status.value = 'RECONNECTING';
    };
    for (const eventName of eventNames) {
      source.addEventListener(eventName, (event) => {
        const parsed = parseLocalViewEvent(event as MessageEvent<string>);
        if (parsed) options.onEvent(parsed);
      });
    }
  }

  onMounted(connect);
  onBeforeUnmount(() => {
    stopped = true;
    source?.close();
  });

  return { status };
}
