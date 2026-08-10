import { beforeEach, describe, expect, it, vi } from 'vitest';

const { get, post } = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
}));

vi.mock('../request', () => ({
  baseRequestClient: { get, post },
}));

import {
  exportIsolatedTrialRegistrationRequest,
  getControlIngestOverviewView,
  getControlIngestRecordsView,
  getIsolatedTrialRegistrationView,
  initializeEdgeMediaCandidate,
  initializeUnregisteredEdgeTransportDisk,
  importIsolatedTrialRegistrationResponse,
} from '../local-views';

describe('isolated-trial registration local API', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
  });

  it('reads the dedicated isolated projection rather than the standard registration view', () => {
    getIsolatedTrialRegistrationView();

    expect(get).toHaveBeenCalledWith(
      '/api/local/v1/edge/isolated-trial/registration',
      { responseReturn: 'body' },
    );
  });

  it('uses fixed Agent endpoints and sends no browser-provided registration material', () => {
    exportIsolatedTrialRegistrationRequest();
    importIsolatedTrialRegistrationResponse();

    expect(post).toHaveBeenNthCalledWith(
      1,
      '/api/local/v1/edge/isolated-trial/registration/request',
      undefined,
      { responseReturn: 'body' },
    );
    expect(post).toHaveBeenNthCalledWith(
      2,
      '/api/local/v1/edge/isolated-trial/registration/response',
      undefined,
      { responseReturn: 'body' },
    );
  });

  it('initializes only an Agent-issued discovery token, never a browser path or media id', () => {
    initializeUnregisteredEdgeTransportDisk('opaque-discovery-token');

    expect(post).toHaveBeenCalledWith(
      '/api/local/v1/edge/nas-disks/initialize',
      { discovery_token: 'opaque-discovery-token' },
      { responseReturn: 'body' },
    );
  });

  it('initializes a v2 candidate by the Agent-issued opaque candidate id only', () => {
    initializeEdgeMediaCandidate('candidate-id', 'candidate-session-id');

    expect(post).toHaveBeenCalledWith(
      '/api/local/v2/edge/nas-disks/candidates/initialize',
      {
        candidateId: 'candidate-id',
        candidateSessionId: 'candidate-session-id',
      },
      { responseReturn: 'body' },
    );
  });

  it('reads CONTROL import projections from fixed read-only endpoints', () => {
    getControlIngestOverviewView();
    getControlIngestRecordsView();

    expect(get).toHaveBeenNthCalledWith(
      1,
      '/api/local/v1/control/ingest-overview',
      { responseReturn: 'body' },
    );
    expect(get).toHaveBeenNthCalledWith(
      2,
      '/api/local/v1/control/ingest-records',
      { responseReturn: 'body' },
    );
  });
});
