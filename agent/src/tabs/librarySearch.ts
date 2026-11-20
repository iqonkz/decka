/** A name cut into pieces for the search of the library: the pieces that match the query are shown in bold (DESIGN.md 4.8a). */
export function highlightParts(name: string, query: string): { text: string; bold: boolean }[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [{ text: name, bold: false }];
  const parts: { text: string; bold: boolean }[] = [];
  const lower = name.toLowerCase();
  let at = 0;
  for (;;) {
    const found = lower.indexOf(q, at);
    if (found < 0) break;
    if (found > at) parts.push({ text: name.slice(at, found), bold: false });
    parts.push({ text: name.slice(found, found + q.length), bold: true });
    at = found + q.length;
  }
  if (at < name.length) parts.push({ text: name.slice(at), bold: false });
  return parts.length > 0 ? parts : [{ text: name, bold: false }];
}
