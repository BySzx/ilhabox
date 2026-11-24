import { Check, Pause, Play, Plus, SkipForward } from "lucide-solid";
import { createMemo, For, Show, type JSX } from "solid-js";
import { art, clock, config, control, dur, entriesOf, isLunch, isTicket, length, media, msgs, now, paused, pending as toPost, running, startOfDay, tag, worked } from "../../lib/state";
import { backFromLunch, finish, pause, resume } from "./Horas";
import { feedback } from "../ui";
import { go } from "../App";
import { ago, initials, tint } from "../whatsUi";

export function Inicio(): JSX.Element {
  const today = createMemo(() => entriesOf(startOfDay(now())));
  const total = () => worked(today(), now());
  const goal = () => Math.max(1, config.metaMin) * 60000;
  const pending = () => toPost(today()).length;
  const date = () => new Date(now()).toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" });
  const recent = () => msgs().slice(-3).reverse();

  return (
    <div class="home">
      <div class="card clockcard">
        <div class="bigtime mono">{new Date(now()).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</div>
        <div class="date">{date()}</div>
        <div class="goal">
          <div class="goal-row">
            <span>Apontado hoje</span>
            <b class="mono">{dur(total())} <span class="dim">/ {dur(goal())}</span></b>
          </div>
          <div class="meter"><i style={{ width: `${Math.min(100, (total() / goal()) * 100)}%` }} /></div>
          <Show when={pending()}>
            <button class="link" onClick={() => go("horas")}>{pending()} {pending() === 1 ? "apontamento" : "apontamentos"} para lançar no Movidesk</button>
          </Show>
        </div>
      </div>

      <div class="card timercard" classList={{ live: !!running() }} onClick={() => go("horas")}>
        <Show
          when={running()}
          fallback={
            <Show
              when={paused()[0]}
              fallback={
                <div class="idle">
                  <div class="plus"><Plus size={30} /></div>
                  <b>Começar apontamento</b>
                  <span class="dim">toque para abrir as horas</span>
                </div>
              }
            >
              {(p) => (
                <>
                  <span class="label amber-text">Em pausa</span>
                  <div class="t-head">
                    <span class="ticket" classList={{ mono: isTicket(p()) }}>{tag(p())}</span>
                    <span class="desc">{p().desc}</span>
                  </div>
                  <div class="watch mono dim">{clock(length(p(), now()))}</div>
                  <button
                    class="btn green"
                    onClick={(e) => {
                      e.stopPropagation();
                      resume(p());
                    }}
                  >
                    <Play size={16} /> Continuar
                  </button>
                </>
              )}
            </Show>
          }
        >
          {(r) => (
            <>
              <span class="label">Em andamento</span>
              <div class="t-head">
                <span class="ticket" classList={{ mono: isTicket(r()) }}>{tag(r())}</span>
                <span class="desc">{r().desc || "Sem descrição"}</span>
              </div>
              <div class="watch mono">{clock(length(r(), now()))}</div>
              <div class="row" onClick={(e) => e.stopPropagation()}>
                <Show when={isLunch(r())}>
                  <button class="btn green" onClick={() => void backFromLunch(r()).catch(feedback)}>
                    <Play size={16} /> Voltei do almoço
                  </button>
                </Show>
                <Show when={!isLunch(r())}>
                <button class="btn amber" onClick={pause}>
                  <Pause size={16} /> Pausar
                </button>
                <button class="btn red" onClick={() => finish(r().id)}>
                  <Check size={16} /> Concluir
                </button>
                </Show>
              </div>
            </>
          )}
        </Show>
      </div>

      <div class="card minimusic" onClick={() => go("musica")}>
        <Show when={media.active} fallback={<span class="dim">Nada tocando</span>}>
          <div class="cover" style={{ "background-image": art() ? `url(${art()})` : undefined }} />
          <div class="mm-text">
            <b>{media.title}</b>
            <span class="dim">{media.artist}</span>
          </div>
          <div class="mm-ctl" onClick={(e) => e.stopPropagation()}>
            <button class="round" onClick={() => control("toggle")}>
              <Show when={media.playing} fallback={<Play size={20} />}>
                <Pause size={20} />
              </Show>
            </button>
            <button class="round ghost" onClick={() => control("next")}>
              <SkipForward size={18} />
            </button>
          </div>
        </Show>
      </div>

      <div class="card miniwhats" onClick={() => go("whats")}>
        <Show when={recent().length} fallback={<span class="dim">Sem mensagens no WhatsApp</span>}>
          <For each={recent()}>
            {(m) => (
              <div class="mw-row">
                <span class="avatar sm" style={{ background: tint(m.chat) }}>{initials(m.chat)}</span>
                <span class="mw-text">
                  <b>{m.chat}</b>
                  <span class="dim">{m.text}</span>
                </span>
                <span class="dim mono small">{ago(m.at, now())}</span>
              </div>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}
