import { render, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import GroupManager from '../lib/components/GroupManager.svelte';
import { getGroups, createGroup } from '../lib/api.js';

vi.mock('../lib/api.js', () => ({
  getGroups: vi.fn(),
  createGroup: vi.fn(),
  deleteGroup: vi.fn(),
  updateGroupMembers: vi.fn(),
  updateService: vi.fn(),
}));

describe('GroupManager - creation form', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getGroups.mockResolvedValue([]);
  });

  it('n\'affiche plus qu\'un champ "Nom" (pas de champ code/cle manuel)', async () => {
    const { getByText, container } = render(GroupManager);
    await waitFor(() => expect(getByText(/Aucun groupe/)).toBeInTheDocument());

    await fireEvent.click(getByText('+ Nouveau groupe'));

    const form = container.querySelector('.group-create-form');
    expect(form).toBeInTheDocument();
    const inputs = form.querySelectorAll('input');
    expect(inputs.length).toBe(1);
    expect(inputs[0].id).toBe('new-group-name');
  });

  it('cree un groupe avec uniquement le nom saisi, le code est envoye vide (auto-genere par le backend)', async () => {
    createGroup.mockResolvedValue({ name: 'API Internes', code: 'ab3f9', admins: [], members: [] });
    const onNotify = vi.fn();
    const { getByText, container } = render(GroupManager, { props: { onNotify } });
    await waitFor(() => expect(getByText(/Aucun groupe/)).toBeInTheDocument());

    await fireEvent.click(getByText('+ Nouveau groupe'));
    const nameInput = container.querySelector('#new-group-name');
    nameInput.value = 'API Internes';
    await fireEvent.input(nameInput);
    await fireEvent.submit(container.querySelector('form'));

    await waitFor(() => expect(createGroup).toHaveBeenCalledWith({
      name: 'API Internes',
      code: '',
      admins: [],
      members: [],
    }));
    expect(onNotify).toHaveBeenCalledWith(expect.stringContaining('cree'), 'success');
  });

  it('accepte un nom avec accents/espaces (la generation du code est laissee au backend)', async () => {
    createGroup.mockResolvedValue({ name: 'Équipe Café', code: 'k2m81', admins: [], members: [] });
    const { getByText, container } = render(GroupManager);
    await waitFor(() => expect(getByText(/Aucun groupe/)).toBeInTheDocument());

    await fireEvent.click(getByText('+ Nouveau groupe'));
    const nameInput = container.querySelector('#new-group-name');
    nameInput.value = 'Équipe Café';
    await fireEvent.input(nameInput);
    await fireEvent.submit(container.querySelector('form'));

    await waitFor(() => expect(createGroup).toHaveBeenCalledWith({
      name: 'Équipe Café',
      code: '',
      admins: [],
      members: [],
    }));
  });

  it('affiche l\'erreur du backend (ex: collision de nom) au niveau du champ nom', async () => {
    createGroup.mockRejectedValue(new Error('Un groupe avec le nom "Ops" existe deja.'));
    const { getByText, container } = render(GroupManager);
    await waitFor(() => expect(getByText(/Aucun groupe/)).toBeInTheDocument());

    await fireEvent.click(getByText('+ Nouveau groupe'));
    const nameInput = container.querySelector('#new-group-name');
    nameInput.value = 'Ops';
    await fireEvent.input(nameInput);
    await fireEvent.submit(container.querySelector('form'));

    await waitFor(() => expect(getByText('Un groupe avec le nom "Ops" existe deja.')).toBeInTheDocument());
    expect(nameInput.getAttribute('aria-invalid')).toBe('true');
  });

  it('rejette localement un nom vide sans appeler l\'API', async () => {
    const { getByText, container } = render(GroupManager);
    await waitFor(() => expect(getByText(/Aucun groupe/)).toBeInTheDocument());

    await fireEvent.click(getByText('+ Nouveau groupe'));
    await fireEvent.submit(container.querySelector('form'));

    await waitFor(() => expect(getByText('Le nom du groupe est requis.')).toBeInTheDocument());
    expect(createGroup).not.toHaveBeenCalled();
  });
});
