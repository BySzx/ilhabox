import { createSignal } from "solid-js";

/**
 * Talks to the IlhaBox agent on the PC (127.0.0.1:8777). The phone page is served by the agent
 * itself (same origin, key from the URL fragment the Android app passes); the settings window
 * runs inside Tauri and asks the key over IPC.
 */
const inTauri = "__TAURI_INTERNALS__" in window;
export const demo = new URLSearchParams(location.search).has("demo");
const base = location.port === "8777" ? "" : "http://127.0.0.1:8777";

let key = "";
export const [online, setOnline] = createSignal(false);

const handlers = new Map<string, Set<(v: never) => void>>();

export function on<T>(topic: string, fn: (v: T) => void): void {
  const set = handlers.get(topic) ?? new Set();
  set.add(fn as (v: never) => void);
  handlers.set(topic, set);
}

export function fire(topic: string, v: unknown): void {
  handlers.get(topic)?.forEach((fn) => fn(v as never));
}

async function findKey(): Promise<string> {
  const fromHash = new URLSearchParams(location.hash.slice(1)).get("k");
  if (fromHash) {
    try {
      localStorage.setItem("ilhabox.k", fromHash);
    } catch {
      /* storage blocked: the hash keeps working */
    }
    return fromHash;
  }
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string>("box_key");
  }
  try {
    return localStorage.getItem("ilhabox.k") ?? "";
  } catch {
    return "";
  }
}

export async function cmd<T = unknown>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (demo) return (await import("./demo")).demoCmd(name, args) as T;
  const r = await fetch(`${base}/api/cmd/${name}`, {
    method: "POST",
    headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
  const body = (await r.json().catch(() => ({ ok: false, erro: `HTTP ${r.status}` }))) as { ok: boolean; data?: T; erro?: string };
  if (!body.ok) throw new Error(body.erro ?? "falhou");
  return body.data as T;
}

/** Live feed; EventSource reconnects by itself and every connection starts with a full snapshot. */
export async function connect(): Promise<void> {
  if (demo) {
    setOnline(true);
    (await import("./demo")).seed(fire);
    return;
  }
  key = await findKey();
  const es = new EventSource(`${base}/api/events?k=${encodeURIComponent(key)}`);
  es.addEventListener("snapshot", (e) => {
    setOnline(true);
    const all = JSON.parse((e as MessageEvent<string>).data) as Record<string, unknown>;
    for (const [topic, v] of Object.entries(all)) fire(topic, v);
  });
  for (const topic of ["media", "media-art", "whats", "whats-history", "work", "config", "phone", "update", "server"]) {
    es.addEventListener(topic, (e) => fire(topic, JSON.parse((e as MessageEvent<string>).data)));
  }
  es.onerror = () => setOnline(false);
}

/**
 * The PC mouse comes on its own low-latency stream (port 8778, no Nagle delay on the PC side),
 * separate from the state feed.
 */
export function onPointer(fn: (v: unknown) => void): void {
  if (demo) return on("ponteiro", fn);
  const open = () => {
    if (!key) return void setTimeout(open, 300);
    const host = location.port === "8777" ? location.hostname : "127.0.0.1";
    const es = new EventSource(`http://${host}:8778/ponteiro?k=${encodeURIComponent(key)}`);
    es.onmessage = (e: MessageEvent<string>) => fn(JSON.parse(e.data));
  };
  open();
}

/** The Android shell's bridge (absent in the PC window and in a desktop browser). */
type Bridge = { home(): void; vibrate(ms: number): void; version(): string };
export const phone: Bridge | undefined = (window as unknown as { IlhaBoxApp?: Bridge }).IlhaBoxApp;
