import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ServiceForm from '../lib/components/ServiceForm.svelte';

async function setInput(el, value) {
  el.value = value;
  await fireEvent.input(el);
}

async function submitForm(container) {
  const form = container.querySelector('form');
  await fireEvent.submit(form);
}

describe('ServiceForm validation', () => {
  it('refuse un nom vide', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), '');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('Le nom du service est requis');
  });

  it('refuse le nom reserve "api"', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'api');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('reserve');
  });

  it('refuse le nom reserve "index.html"', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'index.html');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('reserve');
  });

  it('accepte un listen_path vide (catch-all)', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my-svc');
    await setInput(getByLabelText(/Chemin d'ecoute/), '');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('accepte un listen_path "/" (catch-all)', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my-svc');
    await setInput(getByLabelText(/Chemin d'ecoute/), '/');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('accepte un listen_path "/*" (catch-all explicite)', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my-svc');
    await setInput(getByLabelText(/Chemin d'ecoute/), '/*');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('accepte un listen_path valide avec sous-chemin', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my-svc');
    await setInput(getByLabelText(/Chemin d'ecoute/), '/v1/users/*');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('refuse un nom contenant /', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my/svc');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('separateur');
  });

  it('permet la soumission meme si nom existe (unicite geree par le backend par groupe)', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, {
      props: { onSave, existingNames: ['existing-svc'] },
    });

    await setInput(getByLabelText('Nom du service'), 'existing-svc');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('autorise le meme nom en edition', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const existingService = {
      name: 'existing-svc',
      listen_path: '/v1/*',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [],
    };
    const { getByLabelText, container } = render(ServiceForm, {
      props: { service: existingService, existingNames: ['existing-svc'], onSave },
    });

    await setInput(getByLabelText('URL cible réelle'), 'http://new-backend:9090');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });
});
