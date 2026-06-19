import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import StatusBadge from '../lib/components/StatusBadge.svelte';

describe('StatusBadge', () => {
  it('affiche MOCK quand actif', () => {
    const { getByText } = render(StatusBadge, { props: { active: true } });
    expect(getByText('MOCK')).toBeInTheDocument();
  });

  it('affiche PROXY quand inactif', () => {
    const { getByText } = render(StatusBadge, { props: { active: false } });
    expect(getByText('PROXY')).toBeInTheDocument();
  });

  it('a un role status', () => {
    const { getByRole } = render(StatusBadge, { props: { active: true } });
    expect(getByRole('status')).toBeInTheDocument();
  });
});
