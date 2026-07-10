import { render, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import App from '../App.svelte';
import * as api from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  getServices: vi.fn().mockResolvedValue([]),
  getConfig: vi.fn().mockResolvedValue({ services: [], groups: [] }),
  putConfig: vi.fn(),
  toggleService: vi.fn(),
  createService: vi.fn(),
  updateService: vi.fn(),
  resetConfig: vi.fn(),
  getAuthStatus: vi.fn().mockResolvedValue({ enabled: false, show_reset_button: false }),
  validateToken: vi.fn(),
  getGroups: vi.fn().mockResolvedValue([]),
  createGroup: vi.fn(),
  getMessagingStatus: vi.fn(),
}));

describe('App — visibilite du bouton "Messages Kafka" (feature messaging-kafka)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.getServices.mockResolvedValue([]);
    api.getGroups.mockResolvedValue([]);
    api.getAuthStatus.mockResolvedValue({ enabled: false, show_reset_button: false });
    window.matchMedia = window.matchMedia || vi.fn().mockReturnValue({ matches: false });
  });

  it('cache le bouton quand /api/messaging/status renvoie 404 (binaire sans la feature)', async () => {
    api.getMessagingStatus.mockRejectedValue(new Error('404 Not Found'));
    const { queryByTitle } = render(App);
    await waitFor(() => expect(api.getServices).toHaveBeenCalled());
    expect(queryByTitle('Journal des messages Kafka')).not.toBeInTheDocument();
  });

  it('affiche le bouton quand /api/messaging/status renvoie available=true (binaire avec la feature)', async () => {
    api.getMessagingStatus.mockResolvedValue({ available: true });
    const { queryByTitle } = render(App);
    await waitFor(() => expect(queryByTitle('Journal des messages Kafka')).toBeInTheDocument());
  });
});
