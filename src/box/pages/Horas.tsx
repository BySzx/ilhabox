import { Check, ChevronLeft, ChevronRight, ClipboardCopy, Pencil, Play, RotateCcw, Square, Trash2, X } from "lucide-solid";
import { createMemo, createSignal, For, Show, type JSX } from "solid-js";
import { cmd } from "../../lib/api";
import {
  byKind, clock, config, DAY, dayText, dur, entriesOf, entryText, hhmm, isTicket, kind, length, now, running, startOfDay, tag, type Entry,
} from "../../lib/state";
import { feedback, note } from "../ui";

const [day, setDay] = createSignal(startOfDay(Date.now()));
const [editing, setEditing] = createSignal<Entry | null>(null);
const [tipo, setTipo] = createSignal("Ticket");

async function copyEntry(e: Entry): Promise<void> {
  await cmd("copiar", { texto: entryText(e) });
  if (e.end != null && !e.sent) await cmd("work_sent", { ids: [e.id] });
  note("Copiado no PC: cole no Movidesk");
}

async function copyDay(list: Entry[]): Promise<void> {
  await cmd("copiar", { texto: dayText(day()) });
  note(`Resumo do dia copiado no PC (${list.length} apontamentos)`);
}

export function Horas(): JSX.Element {
  let ticketEl!: HTMLInputElement;
  let descEl!: HTMLInputElement;
  const list = createMemo(() => entriesOf(day()).slice().reverse());
  const total = () => list().reduce((s, e) => s + length(e, now()), 0);
  const isToday = () => day() === startOfDay(now());
  const label = () =>
    isToday() ? "Hoje" : day() === startOfDay(now()) - DAY ? "Ontem" : new Date(day()).toLocaleDateString("pt-BR", { weekday: "short", day: "2-digit", month: "2-digit" });

  const start = (t = tipo(), ticket = ticketEl?.value ?? "", desc = descEl.value) => {
    void cmd("work_start", { tipo: t, ticket: ticket.replace(/^#/, "").trim(), desc: desc.trim() })
      .then(() => {
        if (ticketEl) ticketEl.value = "";
        descEl.value = "";
        ticketEl?.blur();
        descEl.blur();
        setDay(startOfDay(Date.now()));
      })
      .catch(feedback);
  };

  return (
    <div class="hours">
      <div class="card starter" classList={{ live: !!running() }}>
        <Show
          when={running()}
          fallback={
            <form
              class="start-form"
              onSubmit={(e) => {
                e.preventDefault();
                start();
              }}
            >
              <span class="label">Novo apontamento</span>
              <div class="seg kinds">
                <For each={config.tipos}>
                  {(t) => (
                    <button type="button" classList={{ on: tipo() === t }} onClick={() => setTipo(t)}>
                      {t}
                    </button>
                  )}
                </For>
              </div>
              <Show when={tipo() === "Ticket"}>
                <input ref={ticketEl} class="big-input mono" placeholder="Nº do ticket" inputmode="numeric" enterkeyhint="next" onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), descEl.focus())} />
              </Show>
              <input ref={descEl} class="big-input" placeholder={tipo() === "Ticket" ? "O que está fazendo (opcional)" : `${tipo()}: sobre o quê? (opcional)`} enterkeyhint="go" />
              <button class="btn green wide" type="submit">
                <Play size={18} /> Iniciar
              </button>
            </form>
          }
        >
          {(r) => (
            <div class="live-box">
              <span class="label">Em andamento desde {hhmm(r().start)}</span>
              <div class="ticket" classList={{ mono: isTicket(r()) }}>{tag(r())}</div>
              <div class="desc">{r().desc || "Sem descrição"}</div>
              <div class="watch mono">{clock(length(r(), now()))}</div>
              <div class="row">
                <button class="btn red wide" onClick={() => void cmd("work_stop").catch(feedback)}>
                  <Square size={16} /> Parar
                </button>
                <button class="btn ghost" onClick={() => setEditing(r())} aria-label="Editar">
                  <Pencil size={16} />
                </button>
              </div>
            </div>
          )}
        </Show>
      </div>

      <div class="card daylist">
        <div class="day-head">
          <button class="icon" onClick={() => setDay((d) => d - DAY)} aria-label="Dia anterior">
            <ChevronLeft size={20} />
          </button>
          <div class="day-title">
            <b>{label()}</b>
            <span class="mono dim">
              {dur(total())}
              <Show when={isToday()}> / {dur(config.metaMin * 60000)}</Show>
            </span>
            <Show when={byKind(list(), now()).length > 1}>
              <span class="kinds-sum dim">{byKind(list(), now()).map(([k, ms]) => `${k} ${dur(ms)}`).join(" · ")}</span>
            </Show>
          </div>
          <button class="icon" disabled={isToday()} onClick={() => setDay((d) => Math.min(startOfDay(Date.now()), d + DAY))} aria-label="Próximo dia">
            <ChevronRight size={20} />
          </button>
          <button class="btn ghost sm" disabled={!list().length} onClick={() => void copyDay(list()).catch(feedback)}>
            <ClipboardCopy size={15} /> Resumo
          </button>
        </div>
        <div class="entries">
          <For each={list()} fallback={<div class="empty dim">Nenhum apontamento neste dia</div>}>
            {(e) => (
              <div class="entry" classList={{ live: e.end == null, sent: e.sent }}>
                <div class="e-time mono">
                  <span>{hhmm(e.start)}</span>
                  <span class="dim">{e.end == null ? "agora" : hhmm(e.end)}</span>
                </div>
                <div class="e-body" onClick={() => setEditing(e)}>
                  <div class="e-top">
                    <b classList={{ mono: isTicket(e), kind: !isTicket(e) }}>{tag(e)}</b>
                    <span class="mono dim">{dur(length(e, now()))}</span>
                    <Show when={e.sent}>
                      <span class="sent-tag"><Check size={12} /> lançado</span>
                    </Show>
                  </div>
                  <div class="e-desc dim">{e.desc || "sem descrição"}</div>
                </div>
                <div class="e-act">
                  <button class="icon" onClick={() => void copyEntry(e).catch(feedback)} aria-label="Copiar para o Movidesk">
                    <ClipboardCopy size={18} />
                  </button>
                  <Show when={e.end != null}>
                    <button class="icon" onClick={() => start(kind(e), e.ticket, e.desc)} aria-label="Continuar">
                      <RotateCcw size={18} />
                    </button>
                  </Show>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>

      <Show when={editing()}>{(e) => <EditSheet entry={e()} close={() => setEditing(null)} />}</Show>
    </div>
  );
}

/** "HH:MM" on the entry's own day → epoch ms. */
function atTime(base: number, v: string): number {
  const [h, m] = v.split(":").map(Number);
  const d = new Date(base);
  d.setHours(h || 0, m || 0, 0, 0);
  return d.getTime();
}

function EditSheet(p: { entry: Entry; close: () => void }): JSX.Element {
  const e = p.entry;
  let ticket: HTMLInputElement | undefined, desc!: HTMLTextAreaElement, ini!: HTMLInputElement, fim!: HTMLInputElement, sent!: HTMLInputElement;
  const [t, setT] = createSignal(kind(e));
  const toTime = (t: number) => hhmm(t);
  const save = () => {
    const args: Record<string, unknown> = { id: e.id, tipo: t(), ticket: (ticket?.value ?? "").replace(/^#/, ""), desc: desc.value, start: atTime(e.start, ini.value), sent: sent.checked };
    if (e.end != null) args.end = atTime(e.start, fim.value);
    void cmd("work_update", args).then(p.close, feedback);
  };
  const remove = () => void cmd("work_delete", { id: e.id }).then(p.close, feedback);
  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet" onClick={(ev) => ev.stopPropagation()}>
        <div class="sheet-head">
          <b>Editar apontamento</b>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <div class="seg kinds">
          <For each={config.tipos}>
            {(k) => (
              <button type="button" classList={{ on: t() === k }} onClick={() => setT(k)}>
                {k}
              </button>
            )}
          </For>
        </div>
        <div class="sheet-grid">
          <label classList={{ off: t() !== "Ticket" }}>
            Ticket
            <input ref={ticket} class="mono" value={e.ticket} inputmode="numeric" disabled={t() !== "Ticket"} />
          </label>
          <label>
            Início
            <input ref={ini} type="time" value={toTime(e.start)} />
          </label>
          <label>
            Fim
            <input ref={fim} type="time" value={e.end != null ? toTime(e.end) : ""} disabled={e.end == null} />
          </label>
          <label class="span">
            Descrição
            <textarea ref={desc} rows={2} value={e.desc} />
          </label>
          <label class="check span">
            <input ref={sent} type="checkbox" checked={e.sent} /> Já lançado no Movidesk
          </label>
        </div>
        <div class="row">
          <button class="btn ghost red-text" onClick={remove}>
            <Trash2 size={16} /> Apagar
          </button>
          <span class="grow" />
          <button class="btn green" onClick={save}>
            <Check size={16} /> Salvar
          </button>
        </div>
      </div>
    </div>
  );
}
