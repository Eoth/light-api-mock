// Codec hexadecimal cote frontend, miroir de src/tcp/hex.rs (encode/decode).
// Sert a offrir une saisie "texte" pour les champs qui sont stockes en
// hexadecimal cote backend (TcpRule.response_hex, TcpMatcher::Prefix) :
// l'utilisateur tape "pong", le formulaire envoie "706f6e67" a l'API.

export function textToHex(text) {
  const bytes = new TextEncoder().encode(text);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

// Retourne null si `hex` n'est pas de l'hexadecimal valide (longueur impaire
// ou caractere hors [0-9a-fA-F]) — a l'appelant de decider du repli.
export function hexToBytes(hex) {
  if (hex.length % 2 !== 0) return null;
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    const byte = Number.parseInt(hex.slice(i, i + 2), 16);
    if (Number.isNaN(byte)) return null;
    bytes[i / 2] = byte;
  }
  return bytes;
}

// Decodage "best effort" pour l'affichage en mode texte : hex invalide ou
// UTF-8 invalide -> null (l'appelant bascule alors sur le mode hexadecimal
// plutot que d'afficher du texte corrompu).
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
