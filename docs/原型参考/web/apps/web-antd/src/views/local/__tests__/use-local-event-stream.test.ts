import { describe, expect, it } from 'vitest';

import { parseLocalViewEvent } from '../use-local-event-stream';

describe('local event stream contract', () => {
  it('accepts the Agent media event payload without an observation timestamp', () => {
    expect(
      parseLocalViewEvent({
        data: JSON.stringify({ kind: 'media.changed', revision: 42 }),
        type: 'media.changed',
      } as MessageEvent<string>),
    ).toEqual({
      event: 'media.changed',
      revision: 42,
      topic: 'media',
    });
  });

  it('rejects an event without a safe revision', () => {
    expect(
      parseLocalViewEvent({
        data: JSON.stringify({ kind: 'media.changed' }),
        type: 'media.changed',
      } as MessageEvent<string>),
    ).toBeNull();
  });
});
