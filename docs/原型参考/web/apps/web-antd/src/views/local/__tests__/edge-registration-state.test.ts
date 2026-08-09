import { describe, expect, it } from 'vitest';

import { isIsolatedReadOnlyRegistration } from '../edge/registration-state';

describe('edge registration trial gate', () => {
  it('opens the read-only projection only for the explicit isolated mode', () => {
    expect(
      isIsolatedReadOnlyRegistration({ trial_mode: 'ISOLATED_READ_ONLY' }),
    ).toBe(true);
  });

  it('fails closed for unavailable, missing, and unrecognized server values', () => {
    expect(isIsolatedReadOnlyRegistration({ trial_mode: 'UNAVAILABLE' })).toBe(
      false,
    );
    expect(isIsolatedReadOnlyRegistration(undefined)).toBe(false);
    expect(
      isIsolatedReadOnlyRegistration({
        trial_mode: 'UNRECOGNIZED',
      } as never),
    ).toBe(false);
  });
});
