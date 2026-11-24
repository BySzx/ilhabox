import { createMemo, createSignal } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { cmd, on } from "./api";
import { dominant, hashHue, hsl, QUIET, type RGB } from "./color";
import { parseLrc, type Line } from "./lrc";

// ------------------------------------------------------------------ types (mirror the Rust side)

export type Media = {
  active: boolean; title: string; artist: string; album: string; playing: boolean;
  positionMs: number; durationMs: number; sampledAt: number; app: string;
};
export type WMsg = { id: number; at: number; chat: string; text: string; avatar: string | null };
export type Seg = { s: number; e: number | null };
export type Entry = { id: number; tipo: string; ticket: string; desc: string; segs: Seg[]; check?: string[]; done: boolean; start: number; end: number | null; sent: boolean };
export type Atalho = { id: string; nome: string; tipo: "texto" | "site" | "programa" | "celular"; valor: string; colar: boolean; cor: string };
export type Config = {
  atalhos: Atalho[]; modelo: string; tipos: string[]; metaMin: number; abrirAoConectar: boolean;
  easypacPacote: string; inicioTurno: string; fimTurno: string; mouseLado: string;
  movideskPalavra: string; lembreteMin: number; lembreteLongoMin: number; whatsMin: number; pausarAoTravar: boolean;
  checklists: { nome: string; itens: string[] }[];
};
export type Action = { label: string; cmd?: string; args?: Record<string, unknown>; go?: string };
export type Suggestion = { id: string; text: string; detail: string; actions: Action[]; at: number };
export type Phone = { status: string; serial: string; modelo: string; easypacs: string[]; appVersao: number; erro: string };
export type Update = { current: string; version: string | null; progress?: number | null; erro?: string };

// ------------------------------------------------------------------ media + lyrics

export const [media, setMedia] = createStore<Media>({
  active: false, title: "", artist: "", album: "", playing: false, positionMs: 0, durationMs: 0, sampledAt: 0, app: "",
});
export const [art, setArt] = createSignal<string | null>(null);
export const [artColor, setArtColor] = createSignal<RGB>(QUIET);
export const [lyrics, setLyrics] = createSignal<Line[]>([]);
export const [now, setNow] = createSignal(Date.now());

export function position(at = now()): number {
  const p = media.playing ? media.positionMs + (at - media.sampledAt) : media.positionMs;
  return Math.max(0, media.durationMs ? Math.min(p, media.durationMs) : p);
}

let lyricKey = "";
async function loadLyrics(): Promise<void> {
  const k = `${media.title}\u0001${media.artist}`;
  if (k === lyricKey || !media.title) return;
  lyricKey = k;
  setLyrics([]);
  if (!media.artist.trim() || /whatsapp/i.test(media.app)) return;
  const lrc = await cmd<string | null>("lyrics", {
    artist: media.artist, title: media.title, album: media.album, durationS: Math.round(media.durationMs / 1000),
  }).catch(() => null);
  if (lrc && k === `${media.title}\u0001${media.artist}`) setLyrics(parseLrc(lrc));
}

export function control(action: "toggle" | "next" | "prev"): void {
  if (action === "toggle") setMedia({ playing: !media.playing, positionMs: position(), sampledAt: Date.now() });
  void cmd("media", { action }).catch(() => undefined);
}

// ------------------------------------------------------------------ whatsapp

export const [msgs, setMsgs] = createSignal<WMsg[]>([]);
export const [toast, setToast] = createSignal<WMsg | null>(null);
let toastT = 0;
const newListeners = new Set<(m: WMsg) => void>();
export const onNewMessage = (fn: (m: WMsg) => void): void => void newListeners.add(fn);

// ------------------------------------------------------------------ work, config, phone, update

export const [entries, setEntries] = createSignal<Entry[]>([]);
export const isRunning = (e: Entry): boolean => e.segs.length > 0 && e.segs[e.segs.length - 1].e == null;
export const isPaused = (e: Entry): boolean => !e.done && !isRunning(e);
export const running = createMemo(() => entries().find(isRunning) ?? null);
/** Paused activities, most recently touched first. */
export const paused = createMemo(() => entries().filter((e) => isPaused(e) && !isLunch(e)).sort((a, b) => lastEnd(b) - lastEnd(a)));
export const [config, setConfig] = createStore<Config>({
  atalhos: [], modelo: "", tipos: ["Ticket", "WhatsApp", "Reunião", "Curso", "Dúvida interna", "Outro"], metaMin: 480, abrirAoConectar: true, easypacPacote: "", inicioTurno: "08:00", fimTurno: "18:00", mouseLado: "direita",
  movideskPalavra: "movidesk", lembreteMin: 10, lembreteLongoMin: 120, whatsMin: 2, pausarAoTravar: true, checklists: [],
});
export const [suggestion, setSuggestion] = createSignal<Suggestion | null>(null);
export const [pcLocked, setPcLocked] = createSignal(false);
export const [phoneState, setPhoneState] = createSignal<Phone>({ status: "desconectado", serial: "", modelo: "", easypacs: [], appVersao: 0, erro: "" });

/** "br.com.easypac.monitoramento" → "Monitoramento" (no labels over adb, so the package name tells). */
export function appName(pkg: string): string {
  const low = pkg.toLowerCase();
  if (low.includes("superv")) return "Supervisor";
  if (low.includes("monit")) return "Monitoramento";
  const skip = new Set(["com", "br", "app", "android", "mobile", "easypac", "prod", "release"]);
  const part = pkg.split(".").filter((x) => !skip.has(x.toLowerCase())).pop() ?? pkg;
  return part.charAt(0).toUpperCase() + part.slice(1);
}
export const [update, setUpdate] = createSignal<Update>({ current: "", version: null });
export const [loaded, setLoaded] = createSignal(false);

export function initState(): void {
  on<Media>("media", (m) => {
    setMedia(m);
    void loadLyrics();
  });
  on<{ key: string; dataUrl: string | null }>("media-art", async (p) => {
    setArt(p.dataUrl);
    setArtColor(p.dataUrl ? await dominant(p.dataUrl) : hsl(hashHue(media.title || "ilha"), 60, 55));
  });
  on<WMsg[]>("whats-history", (l) => setMsgs(l));
  on<WMsg>("whats", (m) => {
    setMsgs((l) => (l.some((x) => x.id === m.id) ? l : [...l, m].slice(-120)));
    setToast(m);
    clearTimeout(toastT);
    toastT = window.setTimeout(() => setToast(null), 6000);
    newListeners.forEach((fn) => fn(m));
  });
  on<{ entries: Entry[] }>("work", (w) => {
    setEntries(w.entries);
    setLoaded(true);
  });
  on<Config>("config", (c) => setConfig(reconcile(c)));
  on<Phone>("phone", setPhoneState);
  on<Update>("update", setUpdate);
  on<Suggestion | null>("sugestao", (v) => {
    setSuggestion(v);
    if (v) newListeners.forEach((fn) => fn({ id: -1, at: v.at, chat: "", text: v.text, avatar: null }));
  });
  on<{ locked: boolean }>("pc", (v) => setPcLocked(!!v?.locked));
  setInterval(() => setNow(Date.now()), 250);
}

// ------------------------------------------------------------------ time helpers

export const DAY = 86_400_000;
export const startOfDay = (t: number): number => {
  const d = new Date(t);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};
export const hhmm = (t: number): string => new Date(t).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
export const ddmmyyyy = (t: number): string => new Date(t).toLocaleDateString("pt-BR");

/** 1h25 · 25 min · 0 min */
export function dur(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60000));
  if (m < 60) return `${m} min`;
  return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
}
/** 01:25 (the Movidesk time field format) */
export const hhmmDur = (ms: number): string => {
  const m = Math.max(0, Math.round(ms / 60000));
  return `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
};
/** 00:42:07 for the live stopwatch */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return `${h ? `${h}:` : ""}${String(m).padStart(h ? 2 : 1, "0")}:${String(r).padStart(2, "0")}`;
}

export const length = (e: Entry, at = now()): number => e.segs.reduce((sum, g) => sum + ((g.e ?? at) - g.s), 0);
/** Lunch is timed (so it shows on the day) but is not work: no totals, no Movidesk. */
export const LUNCH = "Almoço";
export const isLunch = (e: Pick<Entry, "tipo">): boolean => e.tipo === LUNCH;
export const worked = (list: Entry[], at = now()): number => list.filter((e) => !isLunch(e)).reduce((s, e) => s + length(e, at), 0);
export const pending = (list: Entry[]): Entry[] => list.filter((e) => e.done && !e.sent && !isLunch(e));
export const lastEnd = (e: Entry): number => e.segs[e.segs.length - 1]?.e ?? Date.now();
/** "13:35–13:45, 14:31–14:50" */
export const periods = (e: Entry, at = Date.now()): string => e.segs.map((g) => `${hhmm(g.s)}–${hhmm(g.e ?? at)}`).join(", ");

export function entriesOf(day: number): Entry[] {
  return entries().filter((e) => startOfDay(e.start) === day);
}

/** Old entries have no kind: they were all tickets. */
export const kind = (e: Pick<Entry, "tipo">): string => e.tipo || "Ticket";
export const isTicket = (e: Pick<Entry, "tipo">): boolean => kind(e) === "Ticket";
/** "Ticket 48230", "Ticket" (no number yet) or the kind itself ("Reunião"). */
export const activity = (e: Entry): string => (isTicket(e) ? `Ticket${e.ticket ? ` ${e.ticket}` : ""}` : kind(e));
/** Short label for pills and lists: "#48230" or "Reunião". */
export const tag = (e: Entry): string => (isTicket(e) ? `#${e.ticket || "—"}` : kind(e));

/**
 * Movidesk text of one period of an entry (Movidesk takes each start/end separately), from the
 * template in the settings. {periodos} is that period too.
 */
export function segText(e: Entry, g: Seg): string {
  const end = g.e ?? Date.now();
  const model = (config.modelo || "{inicio} às {fim} ({duracao}) · {atividade}\n{descricao}").split("Ticket {ticket}").join("{atividade}");
  return model
    .split("{atividade}").join(activity(e))
    .split("{tipo}").join(kind(e))
    .split("{ticket}").join(e.ticket || "")
    .split("{descricao}").join(describe(e))
    .split("{periodos}").join(`${hhmm(g.s)}–${hhmm(end)}`)
    .split("{inicio}").join(hhmm(g.s))
    .split("{fim}").join(hhmm(end))
    .split("{duracao}").join(dur(end - g.s))
    .split("{horas}").join(hhmmDur(end - g.s))
    .split("{data}").join(ddmmyyyy(g.s))
    .trim();
}

/** Description plus the checklist items ticked during the work. */
export function describe(e: Entry): string {
  const checked = e.check ?? [];
  return [e.desc, checked.length ? `Verificado: ${checked.join("; ")}` : ""].filter(Boolean).join("\n");
}

/** One Movidesk text per period. */
export const entryTexts = (e: Entry): string[] => e.segs.map((g) => segText(e, g));
export const entryText = (e: Entry): string => entryTexts(e).join("\n\n");

/** Time per kind of activity, biggest first. */
export function byKind(list: Entry[], at = Date.now()): [string, number][] {
  const m = new Map<string, number>();
  for (const e of list.filter((x) => !isLunch(x))) m.set(kind(e), (m.get(kind(e)) ?? 0) + length(e, at));
  return [...m.entries()].sort((a, b) => b[1] - a[1]);
}

export function dayText(day: number): string {
  const list = entriesOf(day).filter((e) => !isLunch(e));
  const total = worked(list, Date.now());
  const lines = list
    .flatMap((e) => e.segs.map((g) => ({ e, g })))
    .sort((a, b) => a.g.s - b.g.s)
    .map(({ e, g }) => `• ${hhmm(g.s)}–${hhmm(g.e ?? Date.now())} (${dur((g.e ?? Date.now()) - g.s)}) ${activity(e)}${e.desc ? ` — ${e.desc}` : ""}${g.e == null ? " [em andamento]" : ""}`);
  const kinds = byKind(list).map(([k, ms]) => `${k} ${dur(ms)}`).join(" · ");
  return [`Resumo de ${ddmmyyyy(day)}`, ...lines, `Total: ${dur(total)}${kinds ? ` (${kinds})` : ""}`].join("\n");
}

// ------------------------------------------------------------------ shift and gaps

export function atClock(day: number, hhmm: string): number {
  const [h, m] = hhmm.split(":").map(Number);
  const d = new Date(day);
  d.setHours(h || 0, m || 0, 0, 0);
  return d.getTime();
}

/**
 * Stretches of the shift with nothing timed (≥ 5 min), from the first activity of the day until
 * now (today) or the end of the shift. Before the first activity is "not started yet", not a gap.
 */
export function gaps(day: number, at = now()): { s: number; e: number }[] {
  const from = atClock(day, config.inicioTurno || "08:00");
  const to = Math.min(atClock(day, config.fimTurno || "18:00"), at);
  const busy = entries()
    .flatMap((e) => e.segs)
    .map((g) => ({ s: g.s, e: g.e ?? at }))
    .filter((g) => g.e > from && g.s < to)
    .sort((a, b) => a.s - b.s);
  if (!busy.length || to <= from) return [];
  const out: { s: number; e: number }[] = [];
  let cur = Math.max(from, busy[0].s);
  for (const g of busy) {
    if (g.s - cur >= 5 * 60000) out.push({ s: cur, e: g.s });
    cur = Math.max(cur, g.e);
  }
  if (to - cur >= 5 * 60000) out.push({ s: cur, e: to });
  return out;
}
