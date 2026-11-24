import { Check, ChevronLeft, ChevronRight, ClipboardCopy, Pause, Pencil, Play, Plus, Trash2, X } from "lucide-solid";
import { createMemo, createSignal, For, Index, Show, type JSX } from "solid-js";
import { cmd } from "../../lib/api";
import {
  byKind, clock, config, DAY, dayText, dur, entriesOf, entryText, hhmm, isPaused, isRunning, isTicket, kind, length, now, paused, periods,
  running, startOfDay, tag, type Entry, type Seg,
} from "../../lib/state";
import { feedback, note } from "../ui";

const [day, setDay] = createSignal(startOfDay(Date.now()));
const [editing, setEditing] = createSignal<Entry | null>(null);
const [tipo, setTipo] = createSignal("Ticket");

async function copyEntry(e: Entry): Promise<void> {
  await cmd("copiar", { texto: entryText(e) });
  if (e.done && !e.sent) await cmd("work_sent", { ids: [e.id] });
  note("Copiado no PC: cole no Movidesk");
}

async function copyDay(list: Entry[]): Promise<void> {
  await cmd("copiar", { texto: dayText(day()) });
  note(`Resumo do dia copiado no PC (${list.length} apontamentos)`);
}

export const pause = (): void => void cmd("work_pause").then(() => note("Pausado"), feedback);
export const finish = (id?: number): void => void cmd("work_finish", id == null ? {} : { id }).then(() => note("Concluído"), feedback);
export const resume = (e: Entry): void => void cmd("work_resume", { id: e.id }).then(() => note(`Continuando: ${tag(e)}`), feedback);

export function Horas(): JSX.Element {
  let ticketEl: HTMLInputElement | undefined;
  let descEl!: HTMLInputElement;
  const list = createMemo(() => entriesOf(day()).slice().reverse());
  const total = () => list().reduce((s, e) => s + length(e, now()), 0);
  const isToday = () => day() === startOfDay(now());
  const label = () =>
    isToday() ? "Hoje" : day() === startOfDay(now()) - DAY ? "Ontem" : new Date(day()).toLocaleDateString("pt-BR", { weekday: "short", day: "2-digit", month: "2-digit" });

  const start = () => {
    void cmd("work_start", { tipo: tipo(), ticket: (ticketEl?.value ?? "").replace(/^#/, "").trim(), desc: descEl.value.trim() })
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
              <Show when={paused()[0]}>
                {(p) => (
                  <button type="button" class="paused-bar" onClick={() => resume(p())}>
                    <Pause size={14} />
                    <span class="pb-text">
                      Em pausa: <b>{tag(p())}</b> {p().desc && <span class="dim">{p().desc}</span>}
                    </span>
                    <span class="pb-go">
                      <Play size={14} /> Continuar
                    </span>
                  </button>
                )}
              </Show>
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
              <span class="label">
                Em andamento · desde {hhmm(r().start)}
                <Show when={r().segs.length > 1}> · {r().segs.length}º período</Show>
              </span>
              <div class="ticket" classList={{ mono: isTicket(r()) }}>{tag(r())}</div>
              <div class="desc">{r().desc || "Sem descrição"}</div>
              <div class="watch mono">{clock(length(r(), now()))}</div>
              <div class="row">
                <button class="btn amber wide" onClick={pause}>
                  <Pause size={16} /> Pausar
                </button>
                <button class="btn red wide" onClick={() => finish(r().id)}>
                  <Check size={16} /> Concluir
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
              <div class="entry" classList={{ live: isRunning(e), paused: isPaused(e), sent: e.sent }}>
                <div class="e-time mono">
                  <span>{hhmm(e.start)}</span>
                  <span class="dim">{isRunning(e) ? "agora" : isPaused(e) ? "pausa" : hhmm(e.end ?? e.start)}</span>
                </div>
                <div class="e-body" onClick={() => setEditing(e)}>
                  <div class="e-top">
                    <b classList={{ mono: isTicket(e), kind: !isTicket(e) }}>{tag(e)}</b>
                    <span class="mono dim">{dur(length(e, now()))}</span>
                    <Show when={isPaused(e)}>
                      <span class="pause-tag">
                        <Pause size={11} /> pausado
                      </span>
                    </Show>
                    <Show when={e.sent}>
                      <span class="sent-tag">
                        <Check size={12} /> lançado
                      </span>
                    </Show>
                  </div>
                  <div class="e-desc dim">
                    <Show when={e.segs.length > 1}>
                      <span class="mono">{periods(e, now())}</span>
                      {e.desc ? " · " : ""}
                    </Show>
                    {e.desc || (e.segs.length > 1 ? "" : "sem descrição")}
                  </div>
                </div>
                <div class="e-act">
                  <Show when={!isRunning(e)}>
                    <button class="icon go" onClick={() => resume(e)} aria-label="Continuar">
                      <Play size={18} />
                    </button>
                  </Show>
                  <Show when={isRunning(e)}>
                    <button class="icon" onClick={pause} aria-label="Pausar">
                      <Pause size={18} />
                    </button>
                  </Show>
                  <button class="icon" onClick={() => void copyEntry(e).catch(feedback)} aria-label="Copiar para o Movidesk">
                    <ClipboardCopy size={18} />
                  </button>
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

/** "HH:MM" on a given day → epoch ms. */
function atTime(base: number, v: string): number {
  const [h, m] = v.split(":").map(Number);
  const d = new Date(base);
  d.setHours(h || 0, m || 0, 0, 0);
  return d.getTime();
}

function EditSheet(p: { entry: Entry; close: () => void }): JSX.Element {
  const e = p.entry;
  let ticket: HTMLInputElement | undefined, desc!: HTMLTextAreaElement, sent!: HTMLInputElement;
  const [t, setT] = createSignal(kind(e));
  const [segs, setSegs] = createSignal<Seg[]>(e.segs.map((g) => ({ ...g })));
  const setSeg = (i: number, k: "s" | "e", v: string) => setSegs((l) => l.map((g, j) => (j === i ? { ...g, [k]: v ? atTime(g.s, v) : g[k] } : g)));
  const addSeg = () =>
    setSegs((l) => {
      const last = l[l.length - 1];
      const from = (last?.e ?? Date.now()) + 60000;
      return last?.e == null ? l : [...l, { s: from, e: from + 15 * 60000 }];
    });

  const save = () => {
    const args: Record<string, unknown> = {
      id: e.id, tipo: t(), ticket: (ticket?.value ?? "").replace(/^#/, ""), desc: desc.value, sent: sent.checked, segs: segs(),
    };
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
        <div class="sheet-grid two">
          <label classList={{ off: t() !== "Ticket" }}>
            Ticket
            <input ref={ticket} class="mono" value={e.ticket} inputmode="numeric" disabled={t() !== "Ticket"} />
          </label>
          <label class="check">
            <input ref={sent} type="checkbox" checked={e.sent} /> Já lançado no Movidesk
          </label>
          <label class="span">
            Descrição
            <textarea ref={desc} rows={2} value={e.desc} />
          </label>
        </div>
        <div class="segs">
          <span class="label">Períodos · {dur(segs().reduce((s, g) => s + ((g.e ?? Date.now()) - g.s), 0))}</span>
          <Index each={segs()}>
            {(g, i) => (
              <div class="seg-row">
                <input type="time" value={hhmm(g().s)} onChange={(ev) => setSeg(i, "s", ev.currentTarget.value)} />
                <span class="dim">até</span>
                <Show when={g().e != null} fallback={<span class="now-tag">agora</span>}>
                  <input type="time" value={hhmm(g().e!)} onChange={(ev) => setSeg(i, "e", ev.currentTarget.value)} />
                </Show>
                <button class="icon" disabled={segs().length < 2} onClick={() => setSegs((l) => l.filter((_, j) => j !== i))} aria-label="Apagar período">
                  <Trash2 size={16} />
                </button>
              </div>
            )}
          </Index>
          <Show when={segs()[segs().length - 1]?.e != null}>
            <button class="btn ghost sm add-seg" onClick={addSeg}>
              <Plus size={14} /> Período
            </button>
          </Show>
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
