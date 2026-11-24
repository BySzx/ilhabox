export type RGB = [number, number, number];

export const QUIET: RGB = [118, 118, 128];

export function hashHue(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return Math.abs(h) % 360;
}

export function hsl(h: number, s: number, l: number): RGB {
  s /= 100;
  l /= 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

/** Saturation-weighted average: picks the color the eye remembers, not muddy grey. */
export function dominant(src: string): Promise<RGB> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement("canvas");
      c.width = c.height = 24;
      const ctx = c.getContext("2d", { willReadFrequently: true });
      if (!ctx) return resolve(QUIET);
      ctx.drawImage(img, 0, 0, 24, 24);
      const d = ctx.getImageData(0, 0, 24, 24).data;
      let r = 0, g = 0, b = 0, w = 0;
      for (let i = 0; i < d.length; i += 4) {
        const mx = Math.max(d[i], d[i + 1], d[i + 2]);
        const mn = Math.min(d[i], d[i + 1], d[i + 2]);
        const wt = 0.05 + ((mx - mn) / 255) ** 2 * (mx / 255);
        r += d[i] * wt;
        g += d[i + 1] * wt;
        b += d[i + 2] * wt;
        w += wt;
      }
      resolve(boost([r / w, g / w, b / w]));
    };
    img.onerror = () => resolve(QUIET);
    img.src = src;
  });
}

function boost([r, g, b]: RGB): RGB {
  const mx = Math.max(r, g, b, 1);
  const f = Math.min(1.6, 210 / mx);
  return [Math.min(255, r * f), Math.min(255, g * f), Math.min(255, b * f)].map(Math.round) as RGB;
}

export const rgba = ([r, g, b]: RGB, a: number): string => `rgba(${r | 0},${g | 0},${b | 0},${a})`;
