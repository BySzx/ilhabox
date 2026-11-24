import {
  CalendarDays, Check, ChevronLeft, ChevronRight, ClipboardCopy, ListChecks, NotebookPen, Pause, Play, Plus, Trash2, Utensils, X,
} from "lucide-solid";
import { createMemo, createSignal, For, Index, Show, type JSX } from "solid-js";
import { cmd } from "../../lib/api";
import {
  atClock, byKind, clock, config, DAY, dayText, dur, entries, entriesOf, fullEntry, gaps, lastUnsentDay, hhmm, isLunch, isPaused, isRunning, isTicket, kind, length,
  LUNCH, now, paused, pending, periods, running, segText, startOfDay, tag, worked, type Entry, type Seg,
} from "../../lib/state";
import { feedback, note } from "../ui";

const [day, setDay] = createSignal(startOfDay(Date.now()));
const [view, setView] = createSignal<"dia" | "semana">("dia");
const [editing, setEditing] = createSignal<Entry | null>(null);
const [copying, setCopying] = createSignal<Entry | null>(null);
const [adding, setAdding] = createSignal<{ s: number; e: number } | null>(null);
const [noting, setNoting] = createSignal<number | null>(null);
const [closing, setClosing] = createSignal(false);
const [tipo, setTipo] = createSignal("Ticket");

/** Opens the posting list: today, or the last earlier day with periods left to post. */
export const openClosing = (earlier = false): void => {
  setDay(earlier ? (lastUnsentDay() ?? startOfDay(Date.now())) : startOfDay(Date.now()));
  setView("dia");
  setClosing(true);
};

const markSent = (e: Entry, g: Seg) => cmd("work_sent", { periods: [{ id: e.id, s: g.s }] });

/** One period goes straight to the clipboard; several open a list, one copy per period. */
async function copyEntry(e: Entry): Promise<void> {
  if (e.segs.length > 1) return void setCopying(e);
  await cmd("copiar", { texto: segText(e, e.segs[0]) });
  if (e.segs[0].e != null && !e.segs[0].sent) await markSent(e, e.segs[0]);
  note("Copiado no PC: cole no Movidesk");
}

async function copyDay(): Promise<void> {
  await cmd("copiar", { texto: dayText(day()) });
  note("Resumo do dia copiado no PC");
}

export const pause = (): void => void cmd("work_pause").then(() => note("Pausado"), feedback);
export const finish = (id?: number): void => void cmd("work_finish", id == null ? {} : { id }).then(() => note("Concluído"), feedback);
export const resume = (e: Entry): void => void cmd("work_resume", { id: e.id }).then(() => note(`Continuando: ${tag(e)}`), feedback);
export const lunch = (): void => void cmd("work_start", { tipo: LUNCH }).then(() => note("Bom almoço!"), feedback);

/** Ends lunch and picks up what was paused before it. */
export async function backFromLunch(l: Entry): Promise<void> {
  await cmd("work_finish", { id: l.id });
  const p = paused()[0];
  if (p) await cmd("work_resume", { id: p.id });
  note(p ? `De volta: ${tag(p)}` : "De volta do almoço");
}

type Row = { gap: { s: number; e: number } } | { entry: Entry };

export function Horas(): JSX.Element {
  let ticketEl: HTMLInputElement | undefined;
  let descEl!: HTMLInputElement;
  const list = createMemo(() => entriesOf(day()));
  const rows = createMemo<Row[]>(() => {
    const r: Row[] = [...list().map((entry) => ({ entry })), ...gaps(day(), now()).map((gap) => ({ gap }))];
    const at = (x: Row) => ("gap" in x ? x.gap.s : x.entry.start);
    return r.sort((a, b) => at(b) - at(a));
  });
  const total = () => worked(list(), now());
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
        setView("dia");
      })
      .catch(feedback);
  };

  return (
    <div class="hours">
      <div class="card starter" classList={{ live: !!running() && !isLunch(running()!), lunch: !!running() && isLunch(running()!) }}>
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
              <div class="label-row">
                <span class="label">Novo apontamento</span>
                <button type="button" class="mini" onClick={lunch}>
                  <Utensils size={13} /> Almoço
                </button>
              </div>
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
            <Show
              when={!isLunch(r())}
              fallback={
                <div class="live-box">
                  <span class="label">Almoço desde {hhmm(r().segs[r().segs.length - 1].s)}</span>
                  <div class="ticket">
                    <Utensils size={22} /> Bom almoço
                  </div>
                  <div class="desc">{paused()[0] ? `Na volta continua ${tag(paused()[0])}` : "Não conta nas horas"}</div>
                  <div class="watch mono">{clock(length(r(), now()))}</div>
                  <button class="btn green" onClick={() => void backFromLunch(r()).catch(feedback)}>
                    <Play size={16} /> Voltei do almoço
                  </button>
                </div>
              }
            >
              <div class="live-box">
                <div class="label-row">
                  <span class="label">
                    Em andamento · desde {hhmm(r().start)}
                    <Show when={r().segs.length > 1}> · {r().segs.length}º período</Show>
                  </span>
                  <button type="button" class="mini" onClick={lunch}>
                    <Utensils size={13} /> Almoço
                  </button>
                </div>
                <div class="ticket" classList={{ mono: isTicket(r()) }}>{tag(r())}</div>
                <div class="desc" onClick={() => setNoting(r().id)}>
                  {r().desc || "Toque em Notas para descrever"}
                  <Show when={(r().check ?? []).length}>
                    <span class="checks-tag">
                      <ListChecks size={12} /> {(r().check ?? []).length}
                    </span>
                  </Show>
                </div>
                <div class="watch mono">{clock(length(r(), now()))}</div>
                <div class="row">
                  <button class="btn amber wide" onClick={pause}>
                    <Pause size={16} /> Pausar
                  </button>
                  <button class="btn red wide" onClick={() => finish(r().id)}>
                    <Check size={16} /> Concluir
                  </button>
                  <button class="btn ghost" onClick={() => setNoting(r().id)} aria-label="Notas">
                    <NotebookPen size={16} />
                  </button>
                </div>
              </div>
            </Show>
          )}
        </Show>
      </div>

      <div class="card daylist">
        <div class="day-head">
          <div class="seg view-seg">
            <button classList={{ on: view() === "dia" }} onClick={() => setView("dia")}>Dia</button>
            <button classList={{ on: view() === "semana" }} onClick={() => setView("semana")}>
              <CalendarDays size={14} />
            </button>
          </div>
          <Show when={view() === "dia"} fallback={<div class="day-title"><b>Esta semana</b></div>}>
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
            <button class="btn ghost sm" disabled={!list().length} onClick={() => setClosing(true)}>
              <ClipboardCopy size={15} /> Lançar
            </button>
          </Show>
        </div>
        <Show when={view() === "dia"} fallback={<Week pick={(d) => (setDay(d), setView("dia"))} />}>
          <Timeline day={day()} />
          <div class="entries">
            <For each={rows()} fallback={<div class="empty dim">Nenhum apontamento neste dia</div>}>
              {(row) =>
                "gap" in row ? (
                  <div class="gap-row">
                    <span class="mono">
                      {hhmm(row.gap.s)}–{hhmm(row.gap.e)}
                    </span>
                    <span class="dim">{dur(row.gap.e - row.gap.s)} sem apontamento</span>
                    <span class="grow" />
                    <button class="btn ghost sm" onClick={() => void cmd("work_free", row.gap).then(() => note("Marcado como livre"), feedback)}>
                      Livre
                    </button>
                    <button class="btn ghost sm" onClick={() => setAdding(row.gap)}>
                      <Plus size={14} /> Preencher
                    </button>
                  </div>
                ) : (
                  <EntryRow e={row.entry} />
                )
              }
            </For>
          </div>
        </Show>
      </div>

      <Show when={editing()}>{(e) => <EditSheet entry={e()} close={() => setEditing(null)} />}</Show>
      <Show when={copying()}>{(e) => <CopySheet entry={e()} close={() => setCopying(null)} />}</Show>
      <Show when={adding()}>{(g) => <AddSheet gap={g()} close={() => setAdding(null)} />}</Show>
      <Show when={noting() != null && entries().find((x) => x.id === noting())}>{(e) => <NotesSheet entry={e()} close={() => setNoting(null)} />}</Show>
      <Show when={closing()}>
        <ClosingSheet close={() => setClosing(false)} />
      </Show>
    </div>
  );
}

function EntryRow(p: { e: Entry }): JSX.Element {
  const e = () => p.e;
  return (
    <div
      class="entry"
      classList={{ live: isRunning(e()), paused: isPaused(e()), sent: e().sent, lunch: isLunch(e()) }}
      onContextMenu={(ev) => {
        ev.preventDefault();
        setEditing(fullEntry(e().id) ?? e());
      }}
    >
      <div class="e-time mono">
        <span>{hhmm(e().start)}</span>
        <span class="dim">{isRunning(e()) ? "agora" : isPaused(e()) ? "pausa" : hhmm(e().end ?? e().start)}</span>
      </div>
      <div class="e-body" onClick={() => setEditing(fullEntry(e().id) ?? e())}>
        <div class="e-top">
          <b classList={{ mono: isTicket(e()), kind: !isTicket(e()) }}>{tag(e())}</b>
          <span class="mono dim">{dur(length(e(), now()))}</span>
          <Show when={isPaused(e())}>
            <span class="pause-tag">
              <Pause size={11} /> pausado
            </span>
          </Show>
          <Show when={e().sent}>
            <span class="sent-tag">
              <Check size={12} /> lançado
            </span>
          </Show>
          <Show when={(e().check ?? []).length}>
            <span class="checks-tag">
              <ListChecks size={12} /> {(e().check ?? []).length}
            </span>
          </Show>
        </div>
        <div class="e-desc dim">
          <Show when={e().segs.length > 1}>
            <span class="mono">{periods(e(), now())}</span>
            {e().desc ? " · " : ""}
          </Show>
          {e().desc || (e().segs.length > 1 || isLunch(e()) ? "" : "sem descrição")}
        </div>
      </div>
      <Show when={!isLunch(e())}>
        <div class="e-act">
          <Show when={!isRunning(e())}>
            <button class="icon go" onClick={() => resume(e())} aria-label="Continuar">
              <Play size={18} />
            </button>
          </Show>
          <Show when={isRunning(e())}>
            <button class="icon" onClick={pause} aria-label="Pausar">
              <Pause size={18} />
            </button>
          </Show>
          <button class="icon" onClick={() => void copyEntry(e()).catch(feedback)} aria-label="Copiar para o Movidesk">
            <ClipboardCopy size={18} />
          </button>
        </div>
      </Show>
    </div>
  );
}

/** The shift as a bar: activities in color, lunch in gray, gaps striped. */
function Timeline(p: { day: number }): JSX.Element {
  const from = () => atClock(p.day, config.inicioTurno || "08:00");
  const to = () => atClock(p.day, config.fimTurno || "18:00");
  const pct = (t: number) => `${Math.max(0, Math.min(100, ((t - from()) / (to() - from())) * 100))}%`;
  const width = (a: number, b: number) => `${Math.max(0.4, ((Math.min(b, to()) - Math.max(a, from())) / (to() - from())) * 100)}%`;
  const segs = () =>
    entriesOf(p.day).flatMap((e) => e.segs.map((g) => ({ s: g.s, e: g.e ?? now(), lunch: isLunch(e), ticket: isTicket(e) })));
  return (
    <div class="timeline">
      <For each={gaps(p.day, now())}>{(g) => <i class="tl-gap" style={{ left: pct(g.s), width: width(g.s, g.e) }} />}</For>
      <For each={segs()}>{(g) => <i classList={{ "tl-seg": true, lunch: g.lunch, other: !g.ticket && !g.lunch }} style={{ left: pct(g.s), width: width(g.s, g.e) }} />}</For>
      <Show when={p.day === startOfDay(now())}>
        <i class="tl-now" style={{ left: pct(now()) }} />
      </Show>
      <span class="tl-l mono">{config.inicioTurno}</span>
      <span class="tl-r mono">{config.fimTurno}</span>
    </div>
  );
}

function Week(p: { pick: (day: number) => void }): JSX.Element {
  const days = createMemo(() => {
    const today = startOfDay(now());
    const dow = (new Date(today).getDay() + 6) % 7;
    const monday = today - dow * DAY;
    return Array.from({ length: 7 }, (_, i) => monday + i * DAY).filter((d, i) => i < 5 || entriesOf(d).length);
  });
  const goal = () => config.metaMin * 60000;
  const all = () => days().flatMap((d) => entriesOf(d));
  return (
    <div class="week">
      <div class="wbars">
        <For each={days()}>
          {(d) => {
            const t = () => worked(entriesOf(d), now());
            return (
              <button class="bar-col" classList={{ today: d === startOfDay(now()) }} onClick={() => p.pick(d)}>
                <span class="mono small">{t() ? dur(t()) : "—"}</span>
                <span class="bar-track">
                  <i style={{ height: `${Math.min(100, (t() / goal()) * 100)}%` }} classList={{ ok: t() >= goal() }} />
                </span>
                <span class="small dim">{new Date(d).toLocaleDateString("pt-BR", { weekday: "short" }).replace(".", "")}</span>
              </button>
            );
          }}
        </For>
      </div>
      <div class="week-sum">
        <b class="mono">{dur(worked(all(), now()))}</b>
        <span class="dim">na semana · meta {dur(goal() * days().filter((d) => new Date(d).getDay() % 6 !== 0).length)}</span>
        <div class="kinds-list">
          <For each={byKind(all(), now())}>
            {([k, ms]) => (
              <span>
                {k} <b class="mono">{dur(ms)}</b>
              </span>
            )}
          </For>
        </div>
      </div>
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

function KindPicker(p: { value: string; set: (v: string) => void }): JSX.Element {
  return (
    <div class="seg kinds">
      <For each={config.tipos}>
        {(k) => (
          <button type="button" classList={{ on: p.value === k }} onClick={() => p.set(k)}>
            {k}
          </button>
        )}
      </For>
    </div>
  );
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
      id: e.id, tipo: t(), ticket: (ticket?.value ?? "").replace(/^#/, ""), desc: desc.value, segs: segs(),
    };
    if (sent.checked !== e.sent) args.sent = sent.checked;
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
        <Show when={!isLunch(e)}>
          <KindPicker value={t()} set={setT} />
        </Show>
        <div class="sheet-grid two">
          <label classList={{ off: t() !== "Ticket" }}>
            Ticket
            <input ref={ticket} class="mono" value={e.ticket} inputmode="numeric" disabled={t() !== "Ticket"} />
          </label>
          <label class="check">
            <input ref={sent} type="checkbox" checked={e.sent} /> Todos os períodos já lançados
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
          <div class="row">
            <Show when={segs()[segs().length - 1]?.e != null}>
              <button class="btn ghost sm add-seg" onClick={addSeg}>
                <Plus size={14} /> Período
              </button>
            </Show>
            <Show when={segs().length > 1}>
              <button
                class="btn ghost sm add-seg"
                onClick={() => void cmd("work_join", { id: e.id }).then(() => (note("Períodos juntados em um"), p.close()), feedback)}
              >
                Juntar em um período
              </button>
            </Show>
          </div>
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

/** Filling a gap of the day with what was done then. */
function AddSheet(p: { gap: { s: number; e: number }; close: () => void }): JSX.Element {
  let ticket: HTMLInputElement | undefined, desc!: HTMLInputElement, ini!: HTMLInputElement, fim!: HTMLInputElement;
  const [t, setT] = createSignal("Ticket");
  const save = () => {
    const s = atTime(p.gap.s, ini.value);
    const e = atTime(p.gap.s, fim.value);
    void cmd("work_add", { tipo: t(), ticket: (ticket?.value ?? "").replace(/^#/, ""), desc: desc.value, segs: [{ s, e }] }).then(() => {
      note("Apontamento adicionado");
      p.close();
    }, feedback);
  };
  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet" onClick={(ev) => ev.stopPropagation()}>
        <div class="sheet-head">
          <b>O que você fez nesse horário?</b>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <KindPicker value={t()} set={setT} />
        <div class="sheet-grid">
          <label classList={{ off: t() !== "Ticket" }}>
            Ticket
            <input ref={ticket} class="mono" inputmode="numeric" disabled={t() !== "Ticket"} />
          </label>
          <label>
            Início
            <input ref={ini} type="time" value={hhmm(p.gap.s)} />
          </label>
          <label>
            Fim
            <input ref={fim} type="time" value={hhmm(p.gap.e)} />
          </label>
          <label class="span">
            Descrição
            <input ref={desc} placeholder="opcional" />
          </label>
        </div>
        <div class="row">
          <span class="grow" />
          <button class="btn green" onClick={save}>
            <Check size={16} /> Adicionar
          </button>
        </div>
      </div>
    </div>
  );
}

/** Notes of the activity (its Movidesk description) and a checklist to tick while working. */
function NotesSheet(p: { entry: Entry; close: () => void }): JSX.Element {
  const e = p.entry;
  let desc!: HTMLTextAreaElement;
  const [list, setList] = createSignal(config.checklists.find((c) => c.itens.some((i) => (e.check ?? []).includes(i)))?.nome ?? "");
  const [checked, setChecked] = createSignal<string[]>(e.check ?? []);
  const items = () => config.checklists.find((c) => c.nome === list())?.itens ?? [];
  const toggle = (item: string) => setChecked((l) => (l.includes(item) ? l.filter((x) => x !== item) : [...l, item]));
  const save = () => void cmd("work_update", { id: e.id, desc: desc.value, check: checked() }).then(p.close, feedback);
  return (
    <div class="sheet-bg" onClick={save}>
      <div class="sheet notes-sheet" onClick={(ev) => ev.stopPropagation()}>
        <div class="sheet-head">
          <b>Notas · {tag(e)}</b>
          <button class="btn green sm" onClick={save}>
            <Check size={15} /> Salvar
          </button>
        </div>
        <textarea ref={desc} rows={4} value={e.desc} placeholder="O que o cliente pediu, o que você fez… vira a descrição no Movidesk" />
        <Show when={config.checklists.length}>
          <div class="seg kinds">
            <For each={config.checklists}>
              {(c) => (
                <button type="button" classList={{ on: list() === c.nome }} onClick={() => setList(list() === c.nome ? "" : c.nome)}>
                  <ListChecks size={13} /> {c.nome}
                </button>
              )}
            </For>
          </div>
          <div class="checklist">
            <For each={items()}>
              {(item) => (
                <button type="button" class="check-item" classList={{ on: checked().includes(item) }} onClick={() => toggle(item)}>
                  <span class="box">{checked().includes(item) && <Check size={14} />}</span>
                  {item}
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
}

/** Each period is a separate Movidesk entry: copy one, paste it, come back for the next. */
function CopySheet(p: { entry: Entry; close: () => void }): JSX.Element {
  const e = p.entry;
  const [done, setDone] = createSignal<number[]>([]);
  const copy = async (i: number) => {
    await cmd("copiar", { texto: segText(e, e.segs[i]) });
    const next = done().includes(i) ? done() : [...done(), i];
    setDone(next);
    note(`${i + 1}º período copiado no PC`);
    if (e.segs[i].e != null) await markSent(e, e.segs[i]);
  };
  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet" onClick={(ev) => ev.stopPropagation()}>
        <div class="sheet-head">
          <b>
            Copiar {tag(e)} · {e.segs.length} períodos
          </b>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <span class="dim small">Cada período é um apontamento no Movidesk: copie, cole lá e volte pro próximo.</span>
        <div class="copy-list">
          <Index each={e.segs}>
            {(g, i) => (
              <div class="copy-row" classList={{ ok: done().includes(i) }}>
                <span class="mono">{i + 1}º</span>
                <span class="mono grow">
                  {hhmm(g().s)}–{g().e == null ? "agora" : hhmm(g().e!)} <span class="dim">({dur((g().e ?? Date.now()) - g().s)})</span>
                </span>
                <button class="btn sm" classList={{ green: !done().includes(i) }} onClick={() => void copy(i).catch(feedback)}>
                  <Show when={done().includes(i)} fallback={<><ClipboardCopy size={15} /> Copiar</>}>
                    <Check size={15} /> Copiado
                  </Show>
                </button>
              </div>
            )}
          </Index>
        </div>
      </div>
    </div>
  );
}

/**
 * Posting the day in Movidesk: every period still to post, in order, one copy each. An entry is
 * marked as posted when all its periods were copied.
 */
function ClosingSheet(p: { close: () => void }): JSX.Element {
  const todo = createMemo(() =>
    pending(entriesOf(day()))
      .map(({ e, g }) => ({ e, g, key: `${e.id}-${g.s}` }))
      .sort((a, b) => a.g.s - b.g.s),
  );
  const [copied, setCopied] = createSignal<string[]>([]);
  const copy = async (row: { e: Entry; g: Seg; key: string }) => {
    await cmd("copiar", { texto: segText(row.e, row.g) });
    setCopied((l) => (l.includes(row.key) ? l : [...l, row.key]));
    note("Copiado no PC: cole no Movidesk");
    await markSent(row.e, row.g);
  };
  const still = () => entriesOf(day()).filter((e) => isRunning(e) && !isLunch(e));
  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet" onClick={(ev) => ev.stopPropagation()}>
        <div class="sheet-head">
          <b>Lançar no Movidesk · {day() === startOfDay(Date.now()) ? "hoje" : new Date(day()).toLocaleDateString("pt-BR")}</b>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <Show when={still().length}>
          <span class="warn small">
            Rodando agora: {still().map(tag).join(", ")}. O período aberto entra aqui quando você pausar ou concluir.
          </span>
        </Show>
        <div class="copy-list">
          <For each={todo()} fallback={<div class="empty dim">Tudo lançado. Bom descanso!</div>}>
            {(row) => (
              <div class="copy-row" classList={{ ok: copied().includes(row.key) }}>
                <span class="mono">
                  {hhmm(row.g.s)}–{hhmm(row.g.e ?? Date.now())}
                </span>
                <span class="grow ellipsis">
                  <b classList={{ mono: isTicket(row.e) }}>{tag(row.e)}</b> <span class="dim">{row.e.desc}</span>
                </span>
                <button class="btn sm" classList={{ green: !copied().includes(row.key) }} onClick={() => void copy(row).catch(feedback)}>
                  <Show when={copied().includes(row.key)} fallback={<><ClipboardCopy size={15} /> Copiar</>}>
                    <Check size={15} /> Copiado
                  </Show>
                </button>
              </div>
            )}
          </For>
        </div>
        <div class="row">
          <button class="btn ghost sm" onClick={() => void copyDay().catch(feedback)}>
            <ClipboardCopy size={14} /> Copiar resumo do dia
          </button>
        </div>
      </div>
    </div>
  );
}
