import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ServiceCard from '../lib/components/ServiceCard.svelte';

const mockService = {
  name: 'svc-users',
  listen_path: '/users/*',
  real_target_url: 'http://users:80',
  is_mocked: true,
  rules: [{ name: 'r1' }, { name: 'r2' }],
};

describe('ServiceCard', () => {
  it('affiche le nom du service', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('svc-users')).toBeInTheDocument();
  });

  it('affiche l URL namespacee', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('/svc-users/users/*')).toBeInTheDocument();
  });

  it('affiche l url cible', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('http://users:80')).toBeInTheDocument();
  });

  it('affiche le nombre de regles', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('2')).toBeInTheDocument();
  });

  it('a un bouton Configurer', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('Configurer')).toBeInTheDocument();
  });

  it('appelle onSelect au clic sur Configurer avec le nom et le groupe du service', async () => {
    const onSelect = vi.fn();
    const svc = { ...mockService, group_name: 'team-a' };
    const { getByText } = render(ServiceCard, { props: { service: svc, onSelect } });
    await fireEvent.click(getByText('Configurer'));
    expect(onSelect).toHaveBeenCalledWith('svc-users', 'team-a');
  });

  it('affiche le badge MOCK quand is_mocked est true', () => {
    const { getByText } = render(ServiceCard, { props: { service: mockService } });
    expect(getByText('MOCK')).toBeInTheDocument();
  });

  it('affiche le badge PROXY quand is_mocked est false', () => {
    const svc = { ...mockService, is_mocked: false };
    const { getByText } = render(ServiceCard, { props: { service: svc } });
    expect(getByText('PROXY')).toBeInTheDocument();
  });
});
