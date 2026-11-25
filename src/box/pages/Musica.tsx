import { Music2, Pause, Play, SkipBack, SkipForward } from "lucide-solid";
import { createEffect, createMemo, For, on, Show, type JSX } from "solid-js";
import { cmd } from "../../lib/api";
import { lineAt } from "../../lib/lrc";
import { art, control, lyrics, lyricsNote, media, position, setMedia } from "../../lib/state";

const mmss = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

export function Musica(): JSX.Element {
  let box!: HTMLDivElement;
  const synced = createMemo(() => lyrics().some((l) => l.t >= 0));
  const idx = createMemo(() => (synced() ? lineAt(lyrics(), position() + 250) : -1));

  // Keeps the sung line in the middle of the lyrics column (plain lyrics just scroll by hand).
  createEffect(
    on(idx, (i) => {
      if (!synced()) return;
      const el = box?.children[Math.max(0, i)] as HTMLElement | undefined;
      if (el) box.scrollTo({ top: el.offsetTop - box.clientHeight / 2 + el.clientHeight / 2, behavior: "smooth" });
    }),
  );

  const seek = (ev: MouseEvent) => {
    const bar = ev.currentTarget as HTMLElement;
    const r = bar.getBoundingClientRect();
    const ms = Math.round(((ev.clientX - r.left) / r.width) * media.durationMs);
    setMedia({ positionMs: ms, sampledAt: Date.now() });
    void cmd("media", { action: "seek", positionMs: ms }).catch(() => undefined);
  };

  return (
    <div class="music" style={{ "--art": art() ? `url(${art()})` : "none" }}>
      <div class="music-bg" />
      <Show
        when={media.active}
        fallback={
          <div class="empty-music">
            <Music2 size={42} />
            <b>Nada tocando no PC</b>
            <span class="dim">Dê play no Spotify ou no navegador</span>
          </div>
        }
      >
        <div class="player">
          <div class="art" style={{ "background-image": art() ? `url(${art()})` : undefined }}>
            <Show when={!art()}>
              <Music2 size={48} />
            </Show>
          </div>
          <div class="meta">
            <b class="title">{media.title}</b>
            <span class="dim">{media.artist}</span>
          </div>
          <div class="progress" onClick={seek}>
            <i style={{ width: `${media.durationMs ? (position() / media.durationMs) * 100 : 0}%` }} />
          </div>
          <div class="times mono dim">
            <span>{mmss(position())}</span>
            <span>{mmss(media.durationMs)}</span>
          </div>
          <div class="controls">
            <button class="round ghost" onClick={() => control("prev")} aria-label="Anterior">
              <SkipBack size={22} />
            </button>
            <button class="round big" onClick={() => control("toggle")} aria-label="Tocar ou pausar">
              <Show when={media.playing} fallback={<Play size={28} />}>
                <Pause size={28} />
              </Show>
            </button>
            <button class="round ghost" onClick={() => control("next")} aria-label="Próxima">
              <SkipForward size={22} />
            </button>
          </div>
        </div>
        <div class="lyrics" classList={{ plain: lyrics().length > 0 && !synced() }} ref={box}>
          <For each={lyrics()} fallback={<div class="no-lyrics dim">{lyricsNote() || "Buscando a letra…"}</div>}>
            {(l, i) => (
              <p classList={{ on: i() === idx(), past: i() < idx() }}>{l.text || "♪"}</p>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
