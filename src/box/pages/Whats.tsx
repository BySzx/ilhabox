import { CheckCheck, ExternalLink, X } from "lucide-solid";
import { createMemo, For, Show, type JSX } from "solid-js";
import { cmd } from "../../lib/api";
import { msgs, now, type WMsg } from "../../lib/state";
import { feedback, note } from "../ui";
import { ago, initials, tint } from "../whatsUi";

type Chat = { name: string; last: WMsg; count: number; avatar: string | null; lines: string[] };

export function Whats(): JSX.Element {
  const chats = createMemo<Chat[]>(() => {
    const map = new Map<string, Chat>();
    for (const m of msgs()) {
      const c = map.get(m.chat);
      if (c) {
        c.last = m;
        c.count++;
        c.avatar = m.avatar ?? c.avatar;
        c.lines = [...c.lines, m.text].slice(-3);
      } else map.set(m.chat, { name: m.chat, last: m, count: 1, avatar: m.avatar, lines: [m.text] });
    }
    return [...map.values()].sort((a, b) => b.last.at - a.last.at);
  });
  const open = () => void cmd("whats_abrir").catch(feedback);
  const clear = (chat = "") =>
    void cmd("whats_limpar", { chat }).then(() => note(chat ? `${chat} limpo` : "Mensagens limpas"), feedback);

  return (
    <div class="whats">
      <div class="whats-head">
        <b>WhatsApp da empresa</b>
        <span class="dim">últimas 24 h · {msgs().length} mensagens</span>
        <span class="grow" />
        <button class="btn ghost sm" disabled={!msgs().length} onClick={() => clear()}>
          <CheckCheck size={15} /> Limpar tudo
        </button>
        <button class="btn ghost sm" onClick={open}>
          <ExternalLink size={15} /> Abrir no PC
        </button>
      </div>
      <div class="chats">
        <For each={chats()} fallback={<div class="empty dim">Nenhuma notificação do WhatsApp no PC. Deixe as notificações do WhatsApp ligadas no Windows.</div>}>
          {(c) => (
            <button class="chat" onClick={open}>
              <span class="avatar" style={{ background: tint(c.name) }}>
                <Show when={c.avatar} fallback={initials(c.name)}>
                  <img src={c.avatar!} alt="" />
                </Show>
              </span>
              <span class="chat-body">
                <span class="chat-top">
                  <b>{c.name}</b>
                  <span class="dim mono small">{ago(c.last.at, now())}</span>
                </span>
                <For each={c.lines}>{(l) => <span class="chat-line">{l}</span>}</For>
              </span>
              <span class="chat-side">
                <span
                  class="x"
                  role="button"
                  aria-label={`Limpar ${c.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    clear(c.name);
                  }}
                >
                  <X size={16} />
                </span>
                <Show when={c.count > 1}>
                  <span class="count mono">{c.count}</span>
                </Show>
              </span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
