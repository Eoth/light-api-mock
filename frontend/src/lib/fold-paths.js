// The detailed builders (JsonResponseBuilder.svelte, XmlResponseBuilder.svelte) key the folded fields by position:
// their test path, such as "1-children-0". A field that moves or goes away must take its fold, and the folds inside
// it, along; otherwise the fold stays at the position and lands on whichever field comes there.

/** `key` renamed when it is the path `from` or lies inside it, else null. */
function moved(key, from, to) {
  if (key === from) return to;
  if (key.startsWith(`${from}-`)) return to + key.slice(from.length);
  return null;
}

const pathOf = (parent, index) => [...parent, index].join('-');

/** The folded paths once the fields at `first` and `second` of the array at `parent` have swapped places. */
export function swapFolds(folded, parent, first, second) {
  const a = pathOf(parent, first);
  const b = pathOf(parent, second);
  return new Set([...folded].map((key) => moved(key, a, b) ?? moved(key, b, a) ?? key));
}

/**
 * The folded paths once the field at `index` of the array at `parent`, of `length` fields, is removed: its folds go,
 * and those of the fields after it move one place up.
 */
export function removeFolds(folded, parent, index, length) {
  const result = new Set();
  for (const key of folded) {
    if (moved(key, pathOf(parent, index), '') !== null) continue;
    let renamed = key;
    for (let later = index + 1; later < length; later++) {
      const shifted = moved(key, pathOf(parent, later), pathOf(parent, later - 1));
      if (shifted !== null) {
        renamed = shifted;
        break;
      }
    }
    result.add(renamed);
  }
  return result;
}
