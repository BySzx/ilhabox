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
export type Entry = { id: number; ticket: string; desc: string; start: number; end: number | null; sent: boolean };
export type Atalho = { id: string; nome: string; tipo: "texto" | "site" | "programa" | "celular"; valor: string; colar: boolean; cor: string };
export type Config = {
  atalhos: Atalho[]; modelo: string; metaMin: number; abrirAoConectar: boolean;
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
export const running = createMemo(() => entries().find((e) => e.end == null) ?? null);
export const [config, setConfig] = createStore<Config>({
  atalhos: [], modelo: "", metaMin: 480, abrirAoConectar: true, easypacPacote: "", inicioTurno: "08:00", fimTurno: "18:00", mouseLado: "direita",
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

export const length = (e: Entry, at = now()): number => (e.end ?? at) - e.start;

export function entriesOf(day: number): Entry[] {
  return entries().filter((e) => startOfDay(e.start) === day);
}

/** Movidesk text of one entry from the template in the settings. */
export function entryText(e: Entry): string {
  const end = e.end ?? Date.now();
  return (config.modelo || "{inicio} às {fim} ({duracao}) · Ticket {ticket}\n{descricao}")
    .split("{ticket}").join(e.ticket || "—")
    .split("{descricao}").join(e.desc || "")
    .split("{inicio}").join(hhmm(e.start))
    .split("{fim}").join(hhmm(end))
    .split("{duracao}").join(dur(end - e.start))
    .split("{horas}").join(hhmmDur(end - e.start))
    .split("{data}").join(ddmmyyyy(e.start))
    .trim();
}

export function dayText(day: number): string {
  const list = entriesOf(day);
  const total = list.reduce((s, e) => s + length(e, Date.now()), 0);
  const lines = list.map((e) => `• ${hhmm(e.start)}–${hhmm(e.end ?? Date.now())} (${dur(length(e, Date.now()))}) Ticket ${e.ticket || "—"}${e.desc ? ` — ${e.desc}` : ""}`);
  return [`Resumo de ${ddmmyyyy(day)}`, ...lines, `Total: ${dur(total)}`].join("\n");
}
