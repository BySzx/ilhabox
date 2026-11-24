import { Clock3, House, LayoutGrid, MessageCircle, Music, Download } from "lucide-solid";
import { createSignal, For, onMount, Show, type JSX } from "solid-js";
import { cmd, online, phone } from "../lib/api";
import { rgba } from "../lib/color";
import { artColor, clock, length, now, onNewMessage, running, toast, update } from "../lib/state";
import { Atalhos } from "./pages/Atalhos";
import { Horas } from "./pages/Horas";
import { Inicio } from "./pages/Inicio";
import { Musica } from "./pages/Musica";
import { Whats } from "./pages/Whats";
import { Pointer } from "./Pointer";
import { feedback, note, Notes } from "./ui";
import { initials, tint } from "./whatsUi";

const PAGES: { id: string; label: string; icon: (p: { size: number }) => JSX.Element; view: () => JSX.Element }[] = [
  { id: "inicio", label: "Início", icon: House, view: Inicio },
  { id: "horas", label: "Horas", icon: Clock3, view: Horas },
  { id: "musica", label: "Música", icon: Music, view: Musica },
  { id: "whats", label: "Whats", icon: MessageCircle, view: Whats },
  { id: "atalhos", label: "Atalhos", icon: LayoutGrid, view: Atalhos },
];

export const [page, setPage] = createSignal(0);
let track!: HTMLDivElement;

export function go(id: string): void {
  const i = PAGES.findIndex((p) => p.id === id);
  if (i >= 0) track.scrollTo({ left: i * track.clientWidth, behavior: "smooth" });
}

export function App(): JSX.Element {
  onMount(() => {
    // The page in view follows the swipe; the dock follows the page.
    track.addEventListener("scroll", () => setPage(Math.round(track.scrollLeft / Math.max(1, track.clientWidth))), { passive: true });
    window.addEventListener("resize", () => track.scrollTo({ left: page() * track.clientWidth }));
    onNewMessage(() => phone?.vibrate(60));
  });
  const time = () => new Date(now()).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

  return (
    <div class="app" style={{ "--tint": rgba(artColor(), 0.16) }}>
      <header class="bar">
        <div class="brand">
          <span class="dot" classList={{ off: !online() }} />
          IlhaBox
        </div>
        <Show when={running()}>
          {(r) => (
            <button class="pill" onClick={() => go("horas")}>
              <span class="rec" />
              <span class="mono">{clock(length(r(), now()))}</span>
              <span class="dim">#{r().ticket || "sem ticket"}</span>
            </button>
          )}
        </Show>
        <div class="bar-end">
          <Show when={update().version}>
            <button class="chip green" onClick={() => void cmd("update_install").then(() => note("Atualizando o PC…"), feedback)}>
              <Download size={14} />
              {update().progress != null ? `${Math.round((update().progress ?? 0) * 100)}%` : `Atualizar ${update().version}`}
            </button>
          </Show>
          <span class="mono time">{time()}</span>
        </div>
      </header>

      <nav class="dock">
        <For each={PAGES}>
          {(p, i) => (
            <button classList={{ on: page() === i() }} onClick={() => go(p.id)} aria-label={p.label}>
              <p.icon size={22} />
              <span>{p.label}</span>
            </button>
          )}
        </For>
      </nav>

      <main class="track" ref={track}>
        <For each={PAGES}>{(p) => <section class="page">{p.view()}</section>}</For>
      </main>

      <Show when={toast()}>
        {(m) => (
          <button class="wtoast" onClick={() => void cmd("whats_abrir").catch(feedback)}>
            <span class="avatar" style={{ background: tint(m().chat) }}>
              <Show when={m().avatar} fallback={initials(m().chat)}>
                <img src={m().avatar!} alt="" />
              </Show>
            </span>
            <span class="wt-body">
              <b>{m().chat}</b>
              <span>{m().text}</span>
            </span>
          </button>
        )}
      </Show>

      <Notes />
      <Pointer />

      <Show when={!online()}>
        <div class="offline">
          <div class="spinner" />
          <b>Procurando o PC…</b>
          <span class="dim">Deixe o cabo USB conectado e a IlhaBox aberta no computador.</span>
          <Show when={phone}>
            <button class="btn" onClick={() => phone?.home()}>Usar o celular</button>
          </Show>
        </div>
      </Show>
    </div>
  );
}
