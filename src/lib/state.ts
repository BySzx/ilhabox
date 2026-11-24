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
export type Entry = { id: number; tipo: string; ticket: string; desc: string; segs: Seg[]; done: boolean; start: number; end: number | null; sent: boolean };
export type Atalho = { id: string; nome: string; tipo: "texto" | "site" | "programa" | "celular"; valor: string; colar: boolean; cor: string };
export type Config = {
  atalhos: Atalho[]; modelo: string; tipos: string[]; metaMin: number; abrirAoConectar: boolean;
  easypacPacote: string; inicioTurno: string; fimTurno: string; mouseLado: string;
};
export type Phone = { status: string; serial: string; modelo: string; easypac: string; appVersao: number; erro: string };
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
export const paused = createMemo(() => entries().filter(isPaused).sort((a, b) => lastEnd(b) - lastEnd(a)));
export const [config, setConfig] = createStore<Config>({
  atalhos: [], modelo: "", tipos: ["Ticket", "WhatsApp", "Reunião", "Curso", "Dúvida interna", "Outro"], metaMin: 480, abrirAoConectar: true, easypacPacote: "", inicioTurno: "08:00", fimTurno: "18:00", mouseLado: "direita",
});
export const [phoneState, setPhoneState] = createSignal<Phone>({ status: "desconectado", serial: "", modelo: "", easypac: "", appVersao: 0, erro: "" });
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

/** Movidesk text of one entry from the template in the settings. */
export function entryText(e: Entry): string {
  const end = lastEnd(e);
  const total = length(e, Date.now());
  const model = (config.modelo || "{periodos} ({duracao}) · {atividade}\n{descricao}").split("Ticket {ticket}").join("{atividade}");
  return model
    .split("{atividade}").join(activity(e))
    .split("{tipo}").join(kind(e))
    .split("{ticket}").join(e.ticket || "")
    .split("{descricao}").join(e.desc || "")
    .split("{periodos}").join(periods(e))
    .split("{inicio}").join(hhmm(e.start))
    .split("{fim}").join(hhmm(end))
    .split("{duracao}").join(dur(total))
    .split("{horas}").join(hhmmDur(total))
    .split("{data}").join(ddmmyyyy(e.start))
    .trim();
}

/** Time per kind of activity, biggest first. */
export function byKind(list: Entry[], at = Date.now()): [string, number][] {
  const m = new Map<string, number>();
  for (const e of list) m.set(kind(e), (m.get(kind(e)) ?? 0) + length(e, at));
  return [...m.entries()].sort((a, b) => b[1] - a[1]);
}

export function dayText(day: number): string {
  const list = entriesOf(day);
  const total = list.reduce((s, e) => s + length(e, Date.now()), 0);
  const lines = list.map((e) => `• ${periods(e)} (${dur(length(e, Date.now()))}) ${activity(e)}${e.desc ? ` — ${e.desc}` : ""}${isPaused(e) ? " [em pausa]" : ""}`);
  const kinds = byKind(list).map(([k, ms]) => `${k} ${dur(ms)}`).join(" · ");
  return [`Resumo de ${ddmmyyyy(day)}`, ...lines, `Total: ${dur(total)}${kinds ? ` (${kinds})` : ""}`].join("\n");
}
