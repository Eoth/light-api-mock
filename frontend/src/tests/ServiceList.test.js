import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ServiceList from '../lib/components/ServiceList.svelte';

const mockServices = [
  { name: 'svc-users', listen_path: '/users/*', real_target_url: 'http://users:80', is_mocked: true, group_name: null, rules: [{ name: 'r1' }] },
  { name: 'svc-orders', listen_path: '/orders/*', real_target_url: 'http://orders:80', is_mocked: false, group_name: null, rules: [] },
  { name: 'insee-api', listen_path: '/v4/api/insee/*', real_target_url: 'http://insee:80', is_mocked: true, group_name: 'team-a', rules: [{ name: 'siret' }] },
];

const searchPlaceholder = /Rechercher par nom, chemin, URL ou groupe/;

describe('ServiceList', () => {
  it('affiche un etat vide quand pas de services', () => {
    const { getByText } = render(ServiceList, { props: { services: [] } });
    expect(getByText('Aucun service configure')).toBeInTheDocument();
  });

  it('affiche les groupes plies par defaut', () => {
    const { getByText, queryByText } = render(ServiceList, { props: { services: mockServices } });
    expect(getByText('team-a')).toBeInTheDocument();
    expect(getByText('Sans groupe')).toBeInTheDocument();
    expect(queryByText('svc-users')).not.toBeInTheDocument();
  });

  it('affiche les noms de groupes', () => {
    const { getByText } = render(ServiceList, { props: { services: mockServices } });
    expect(getByText('team-a')).toBeInTheDocument();
    expect(getByText('Sans groupe')).toBeInTheDocument();
  });

  it('affiche la barre de recherche quand il y a des services', () => {
    const { getByPlaceholderText } = render(ServiceList, { props: { services: mockServices } });
    expect(getByPlaceholderText(searchPlaceholder)).toBeInTheDocument();
  });

  it('filtre par nom de service', async () => {
    const { getByPlaceholderText, queryByText } = render(ServiceList, { props: { services: mockServices } });
    const search = getByPlaceholderText(searchPlaceholder);
    await fireEvent.input(search, { target: { value: 'insee' } });
    expect(queryByText('insee-api')).toBeInTheDocument();
    expect(queryByText('svc-users')).not.toBeInTheDocument();
    expect(queryByText('svc-orders')).not.toBeInTheDocument();
  });

  it('filtre par chemin d ecoute', async () => {
    const { getByPlaceholderText, queryByText } = render(ServiceList, { props: { services: mockServices } });
    const search = getByPlaceholderText(searchPlaceholder);
    await fireEvent.input(search, { target: { value: '/orders' } });
    expect(queryByText('svc-orders')).toBeInTheDocument();
    expect(queryByText('svc-users')).not.toBeInTheDocument();
  });

  it('affiche un message quand la recherche ne matche rien', async () => {
    const { getByPlaceholderText, getByText } = render(ServiceList, { props: { services: mockServices } });
    const search = getByPlaceholderText(searchPlaceholder);
    await fireEvent.input(search, { target: { value: 'zzzzz' } });
    expect(getByText(/Aucun service ne correspond/)).toBeInTheDocument();
  });

  it('affiche le compteur de resultats pendant la recherche', async () => {
    const { getByPlaceholderText, getByText } = render(ServiceList, { props: { services: mockServices } });
    const search = getByPlaceholderText(searchPlaceholder);
    await fireEvent.input(search, { target: { value: 'svc' } });
    expect(getByText('2 / 3 services')).toBeInTheDocument();
  });

  it('filtre par nom de groupe', async () => {
    const { getByPlaceholderText, queryByText } = render(ServiceList, { props: { services: mockServices } });
    const search = getByPlaceholderText(searchPlaceholder);
    await fireEvent.input(search, { target: { value: 'team-a' } });
    expect(queryByText('insee-api')).toBeInTheDocument();
    expect(queryByText('svc-users')).not.toBeInTheDocument();
  });
});
