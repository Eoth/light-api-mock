import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import UrlHealthBadge from '../lib/components/UrlHealthBadge.svelte';
import { pingService } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  pingService: vi.fn(),
}));

describe('UrlHealthBadge', () => {
  it('affiche "Non testé" avant tout test', () => {
    const { getByText } = render(UrlHealthBadge, { props: { serviceName: 'svc-a' } });
    expect(getByText('Non testé')).toBeInTheDocument();
  });

  it('teste la cible au clic et affiche "Accessible" si joignable', async () => {
    pingService.mockResolvedValue({ reachable: true, checked_at: Date.now(), error: null });
    const { getByText } = render(UrlHealthBadge, { props: { serviceName: 'svc-a' } });

    await fireEvent.click(getByText('Tester la cible'));

    await waitFor(() => expect(getByText('Accessible')).toBeInTheDocument());
    expect(pingService).toHaveBeenCalledWith('svc-a');
  });

  it('affiche "Inaccessible" et un avertissement si la cible ne repond pas', async () => {
    pingService.mockResolvedValue({ reachable: false, checked_at: Date.now(), error: 'connection refused' });
    const { getByText } = render(UrlHealthBadge, { props: { serviceName: 'svc-b' } });

    await fireEvent.click(getByText('Tester la cible'));

    await waitFor(() => expect(getByText('Inaccessible')).toBeInTheDocument());
    expect(getByText(/Seul le mode mock est utilisable/)).toBeInTheDocument();
  });

  it('affiche une erreur si l\'appel echoue', async () => {
    pingService.mockRejectedValue(new Error('502 Bad Gateway'));
    const { getByText } = render(UrlHealthBadge, { props: { serviceName: 'svc-c' } });

    await fireEvent.click(getByText('Tester la cible'));

    await waitFor(() => expect(getByText('502 Bad Gateway')).toBeInTheDocument());
  });
});
