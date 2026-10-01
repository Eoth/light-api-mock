/**
 * Browser keys written under the project's former name (lightMock). Moved once to the current keys, before anything
 * reads them, so that a user keeps their language, theme and session after the rename.
 */
export const RENAMED_KEYS = [
  ['lightmock-locale', 'mimicway-locale'],
  ['lightmock-theme', 'mimicway-theme'],
  ['lightmock-auth', 'mimicway-auth'],
];

/** Copies each former key to its new name when the new one is not set yet, then removes the former key. */
export function migrateLegacyStorage(storage = globalThis.localStorage) {
  try {
    for (const [legacy, current] of RENAMED_KEYS) {
      const value = storage.getItem(legacy);
      if (value === null) continue;
      if (storage.getItem(current) === null) storage.setItem(current, value);
      storage.removeItem(legacy);
    }
  } catch {
    // Storage unavailable (private mode, sandboxed frame): nothing to move.
  }
}
