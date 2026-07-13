import { render, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ServiceForm from '../lib/components/ServiceForm.svelte';

const PURELY_MOCKED_LABEL = 'Service purement mocké';

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
      props: { service: existingService, existingNames: ['existing-svc'], isEdit: true, onSave },
    });

    await setInput(getByLabelText('URL cible réelle'), 'http://new-backend:9090');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });

  it('desactive le champ nom en mode edition', async () => {
    const existingService = {
      name: 'existing-svc',
      listen_path: '/v1/*',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [],
    };
    const { getByLabelText } = render(ServiceForm, {
      props: { service: existingService, isEdit: true },
    });

    expect(getByLabelText('Nom du service')).toBeDisabled();
  });

  it('laisse le champ nom editable lors d\'un clonage (service pre-rempli sans isEdit)', async () => {
    const clonedService = {
      name: 'existing-svc-copie',
      listen_path: '/v1/*',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [],
    };
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, {
      props: { service: clonedService, onSave },
    });

    const nameInput = getByLabelText('Nom du service');
    expect(nameInput).not.toBeDisabled();
    expect(nameInput.value).toBe('existing-svc-copie');

    await setInput(nameInput, 'renamed-clone');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ name: 'renamed-clone' }));
  });

  it('refuse un nom contenant un espace', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'my svc');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('lettres, chiffres, tirets');
  });

  it('refuse un nom contenant un caractere special', async () => {
    const onSave = vi.fn();
    const { getByLabelText, container, getByRole } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'svc@name!');
    await submitForm(container);
    expect(onSave).not.toHaveBeenCalled();
    expect(getByRole('alert')).toHaveTextContent('lettres, chiffres, tirets');
  });

  it('accepte un nom avec underscores et chiffres', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, container } = render(ServiceForm, { props: { onSave } });

    await setInput(getByLabelText('Nom du service'), 'svc_v2-42');
    await setInput(getByLabelText('URL cible réelle'), 'http://backend:8080');
    await submitForm(container);
    expect(onSave).toHaveBeenCalled();
  });
});

describe('ServiceForm service purement mocké (sujet 22)', () => {
  it('cocher la case masque le champ cible et permet la creation sans cible', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const { getByLabelText, getByRole, queryByLabelText, container } = render(ServiceForm, { props: { onSave } });

    // Non coche par defaut : le champ cible est requis comme avant.
    expect(getByLabelText('URL cible réelle')).toBeInTheDocument();

    await fireEvent.click(getByRole('switch', { name: PURELY_MOCKED_LABEL }));
    expect(queryByLabelText('URL cible réelle')).not.toBeInTheDocument();

    await setInput(getByLabelText('Nom du service'), 'sans-cible');
    await submitForm(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      name: 'sans-cible',
      real_target_url: '',
      is_mocked: true,
    }));
  });

  it('un service existant sans cible ouvre le formulaire avec la case deja cochee', () => {
    const existingService = {
      name: 'deja-purement-mocke',
      listen_path: '',
      real_target_url: '',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [],
    };
    const { getByRole, queryByLabelText } = render(ServiceForm, {
      props: { service: existingService, isEdit: true },
    });

    expect(getByRole('switch', { name: PURELY_MOCKED_LABEL })).toHaveAttribute('aria-checked', 'true');
    expect(queryByLabelText('URL cible réelle')).not.toBeInTheDocument();
  });

  it('decocher reaffiche le champ cible sans perte des regles existantes', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const existingRules = [{ name: 'r1', action: 'mock' }];
    const existingService = {
      name: 'deja-purement-mocke',
      listen_path: '',
      real_target_url: '',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: existingRules,
    };
    const { getByLabelText, getByRole, container } = render(ServiceForm, {
      props: { service: existingService, isEdit: true, onSave },
    });

    await fireEvent.click(getByRole('switch', { name: PURELY_MOCKED_LABEL }));
    const targetInput = getByLabelText('URL cible réelle');
    await setInput(targetInput, 'http://nouvelle-cible:8080');
    await submitForm(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      real_target_url: 'http://nouvelle-cible:8080',
      rules: existingRules,
    }));
  });

  it("bascule a posteriori : avertit sans bloquer quand des regles Proxy existent deja, 'Enregistrer quand meme' sauvegarde", async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const existingService = {
      name: 'avec-regles-proxy',
      listen_path: '',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [{ name: 'proxy-rule', action: 'proxy' }, { name: 'mock-rule', action: 'mock' }],
    };
    const { getByRole, container, queryByTestId, getByTestId } = render(ServiceForm, {
      props: { service: existingService, isEdit: true, onSave },
    });

    await fireEvent.click(getByRole('switch', { name: PURELY_MOCKED_LABEL }));
    await submitForm(container);

    expect(onSave).not.toHaveBeenCalled();
    const warning = getByTestId('service-form-purely-mocked-warning');
    expect(warning).toHaveTextContent('proxy-rule');
    expect(warning).not.toHaveTextContent('mock-rule');

    await fireEvent.click(getByTestId('service-form-purely-mocked-save-anyway-button'));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ real_target_url: '', is_mocked: true }));
    expect(queryByTestId('service-form-purely-mocked-warning')).not.toBeInTheDocument();
  });

  it("bascule a posteriori : 'Revenir en arriere' referme l'avertissement sans sauvegarder", async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const existingService = {
      name: 'avec-regle-proxy',
      listen_path: '',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [{ name: 'proxy-rule', action: 'proxy' }],
    };
    const { getByRole, container, getByTestId, queryByTestId } = render(ServiceForm, {
      props: { service: existingService, isEdit: true, onSave },
    });

    await fireEvent.click(getByRole('switch', { name: PURELY_MOCKED_LABEL }));
    await submitForm(container);
    expect(getByTestId('service-form-purely-mocked-warning')).toBeInTheDocument();

    await fireEvent.click(getByTestId('service-form-purely-mocked-cancel-button'));
    expect(onSave).not.toHaveBeenCalled();
    expect(queryByTestId('service-form-purely-mocked-warning')).not.toBeInTheDocument();
  });

  it('aucun avertissement quand le service purement mocke n\'a aucune regle Proxy', async () => {
    const onSave = vi.fn().mockResolvedValue({});
    const existingService = {
      name: 'sans-regle-proxy',
      listen_path: '',
      real_target_url: 'http://backend:8080',
      is_mocked: true,
      rewrite_directory_urls: false,
      rules: [{ name: 'mock-rule', action: 'mock' }],
    };
    const { getByRole, container } = render(ServiceForm, {
      props: { service: existingService, isEdit: true, onSave },
    });

    await fireEvent.click(getByRole('switch', { name: PURELY_MOCKED_LABEL }));
    await submitForm(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ real_target_url: '', is_mocked: true }));
  });
});
