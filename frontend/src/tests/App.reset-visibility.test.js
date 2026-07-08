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
  getAuthStatus: vi.fn(),
  validateToken: vi.fn(),
  getGroups: vi.fn().mockResolvedValue([]),
  createGroup: vi.fn(),
}));

describe('App — visibilite du bouton Reset (SHOW_RESET_BUTTON)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.getServices.mockResolvedValue([]);
    api.getGroups.mockResolvedValue([]);
    window.matchMedia = window.matchMedia || vi.fn().mockReturnValue({ matches: false });
  });

  it('cache le bouton Reset quand auth desactivee et show_reset_button=false', async () => {
    api.getAuthStatus.mockResolvedValue({ enabled: false, show_reset_button: false });
    const { queryByTitle } = render(App);
    await waitFor(() => expect(api.getServices).toHaveBeenCalled());
    expect(queryByTitle('Supprimer tous les services')).not.toBeInTheDocument();
  });

  it('affiche le bouton Reset quand auth desactivee et show_reset_button=true', async () => {
    api.getAuthStatus.mockResolvedValue({ enabled: false, show_reset_button: true });
    const { queryByTitle } = render(App);
    await waitFor(() => expect(queryByTitle('Supprimer tous les services')).toBeInTheDocument());
  });
});
