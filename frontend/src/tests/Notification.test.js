import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import Notification from '../lib/components/Notification.svelte';

describe('Notification', () => {
  it('n affiche rien quand visible est false', () => {
    const { queryByRole } = render(Notification, { props: { message: 'test', visible: false } });
    expect(queryByRole('alert')).not.toBeInTheDocument();
  });

  it('affiche le message quand visible est true', () => {
    const { getByText } = render(Notification, { props: { message: 'Operation reussie', type: 'success', visible: true } });
    expect(getByText('Operation reussie')).toBeInTheDocument();
  });

  it('a le role alert pour l accessibilite', () => {
    const { getByRole } = render(Notification, { props: { message: 'Erreur', type: 'error', visible: true } });
    expect(getByRole('alert')).toBeInTheDocument();
  });
});
