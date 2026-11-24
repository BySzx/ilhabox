import { Lightbulb, Lock, X } from "lucide-solid";
import { createEffect, For, on, Show, type JSX } from "solid-js";
import { cmd, phone } from "../lib/api";
import { now, pcLocked, suggestion, type Action } from "../lib/state";
import { go } from "./App";
import { openClosing } from "./pages/Horas";
import { feedback } from "./ui";

/** What the PC noticed (a Movidesk ticket, WhatsApp, nothing being timed…), one card at a time. */
export function SuggestionCard(): JSX.Element {
  const close = (id: string) => void cmd("sugestao_fechar", { id }).catch(() => undefined);
  const act = (id: string, a: Action) => {
    if (a.go === "fechamento") {
      go("horas");
      openClosing(!!a.args?.anterior);
    } else if (a.go) go(a.go);
    if (a.cmd) void cmd(a.cmd, a.args ?? {}).catch(feedback);
    close(id);
  };
  return (
    <Show when={suggestion()}>
      {(s) => (
        <div class="suggest">
          <span class="sg-icon">
            <Lightbulb size={18} />
          </span>
          <div class="sg-body">
            <b>{s().text}</b>
            <Show when={s().detail}>
              <span class="dim">{s().detail}</span>
            </Show>
            <div class="sg-actions">
              <For each={s().actions}>
                {(a, i) => (
                  <button class="btn sm" classList={{ green: i() === 0 && !!(a.cmd || a.go) }} onClick={() => act(s().id, a)}>
                    {a.label}
                  </button>
                )}
              </For>
            </div>
          </div>
          <button class="icon" onClick={() => close(s().id)} aria-label="Fechar">
            <X size={18} />
          </button>
        </div>
      )}
    </Show>
  );
}

/** PC locked (Win+L): the phone rests with a dim clock and comes back on unlock. */
export function Resting(): JSX.Element {
  createEffect(on(pcLocked, (l) => phone?.dim?.(l)));
  return (
    <Show when={pcLocked()}>
      <div class="resting">
        <div class="rest-time mono">{new Date(now()).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</div>
        <span>
          <Lock size={13} /> PC travado · apontamento pausado
        </span>
      </div>
    </Show>
  );
}
