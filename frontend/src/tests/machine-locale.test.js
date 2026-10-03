// The unit tests run as an en-US browser in UTC would, whatever the machine: its own locale and time zone (fr-FR and
// Europe/Paris on a French developer's machine, en-US and UTC on a CI runner) would otherwise decide how dates and
// numbers read, and a test could pass on one machine and fail on the other. setup.js pins both.
import { describe, it, expect } from 'vitest';

describe('the machine the tests run on', () => {
  it('formats dates as an en-US browser in UTC does', () => {
    expect(new Date(Date.UTC(2026, 0, 10, 12, 5, 9)).toLocaleString()).toMatch(/^1\/10\/2026, 12:05:09\sPM$/);
  });

  it('formats numbers as an en-US browser does', () => {
    expect((1234.5).toLocaleString()).toBe('1,234.5');
  });
});
