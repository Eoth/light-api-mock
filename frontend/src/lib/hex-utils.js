// Hexadecimal encoding, the UI's counterpart of src/tcp/hex.rs. It lets the user type text in the fields the server
// stores as hexadecimal (TcpRule.response_hex, TcpMatcher::Prefix): the user types "pong", the form sends "706f6e67".

export function textToHex(text) {
  const bytes = new TextEncoder().encode(text);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

// null when `hex` is not valid hexadecimal (odd length, or a character outside [0-9a-fA-F]): the caller decides what
// to fall back to.
export function hexToBytes(hex) {
  // Checked as a whole first: parseInt reads a pair up to its first non-digit, so "1z" would decode as 1.
  if (!isValidHex(hex)) return null;
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = Number.parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

// The text that hexadecimal stands for, to show it in text mode; null when the hexadecimal or the UTF-8 is invalid, and
// the caller then shows hexadecimal rather than garbled text.
export function hexToTextOrNull(hex) {
  const bytes = hexToBytes(hex);
  if (bytes === null) return null;
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
}

export function isValidHex(hex) {
  return hex.length % 2 === 0 && /^[0-9a-fA-F]*$/.test(hex);
}
