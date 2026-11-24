import { hashHue, hsl, rgba } from "../lib/color";

export function initials(name: string): string {
  const parts = name.replace(/[^\p{L}\p{N} ]/gu, "").trim().split(/\s+/).filter(Boolean);
  return ((parts[0]?.[0] ?? "?") + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase();
}

export const tint = (name: string): string => rgba(hsl(hashHue(name), 45, 42), 1);

export function ago(t: number, now: number): string {
  const m = Math.round((now - t) / 60000);
  if (m < 1) return "agora";
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return h < 24 ? `${h} h` : new Date(t).toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });
}
