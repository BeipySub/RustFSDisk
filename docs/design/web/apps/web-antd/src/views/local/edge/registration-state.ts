import type { EdgeRegistrationView } from '#/api/local-views';

/**
 * The registration screen may reveal the isolated rehearsal projection only
 * after the local Agent explicitly opts into its read-only mode. Do not infer
 * this from a package state or registration phase: those values are not an
 * authorization to perform a trial.
 */
export function isIsolatedReadOnlyRegistration(
  view: Pick<EdgeRegistrationView, 'trial_mode'> | undefined,
) {
  return view?.trial_mode === 'ISOLATED_READ_ONLY';
}
