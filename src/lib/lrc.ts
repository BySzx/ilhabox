export type Word = { t: number; w: string };
export type Line = { t: number; text: string; words?: Word[] };

const ts = (m: string, s: string): number => (Number(m) * 60 + Number(s)) * 1000;

/** Parses standard LRC plus the enhanced A2 extension (`<mm:ss.xx>word`) when present. */
export function parseLrc(src: string): Line[] {
  const out: Line[] = [];
  for (const raw of src.split(/\r?\n/)) {
    const tags = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (!tags.length) continue;
    const body = raw.replace(/\[[^\]]*\]/g, "");
    const text = body.replace(/<\d+:\d+(?:\.\d+)?>/g, "").replace(/\s+/g, " ").trim();
    let words: Word[] | undefined;
    const marks = [...body.matchAll(/<(\d+):(\d+(?:\.\d+)?)>([^<]*)/g)];
    if (marks.length > 1) {
      words = marks.map((m) => ({ t: ts(m[1], m[2]), w: m[3] }));
    }
    for (const m of tags) out.push({ t: ts(m[1], m[2]), text, words });
  }
  return out.sort((a, b) => a.t - b.t);
}

export function lineAt(lines: Line[], ms: number): number {
  let lo = 0, hi = lines.length - 1, ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].t <= ms) {
      ans = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return ans;
}
