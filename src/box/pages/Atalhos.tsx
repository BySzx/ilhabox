import { Check, ChevronLeft, ChevronRight, ClipboardCopy, Globe, House, MessageCircle, Pencil, Plus, Rocket, Smartphone, Trash2, Type, X } from "lucide-solid";
import { createSignal, For, Show, type JSX } from "solid-js";
import { unwrap } from "solid-js/store";
import { cmd, phone } from "../../lib/api";
import { config, dayText, phoneState, startOfDay, type Atalho } from "../../lib/state";
import { feedback, note } from "../ui";

const ICON: Record<Atalho["tipo"], (p: { size: number }) => JSX.Element> = {
  texto: Type,
  site: Globe,
  programa: Rocket,
  celular: Smartphone,
};

const DONE: Record<Atalho["tipo"], string> = {
  texto: "Colado no PC",
  site: "Abrindo no PC",
  programa: "Abrindo no PC",
  celular: "Abrindo no celular",
};

const TIPOS: { v: Atalho["tipo"]; label: string; hint: string }[] = [
  { v: "texto", label: "Frase", hint: "Texto que vai pro PC" },
  { v: "site", label: "Site", hint: "https://…" },
  { v: "programa", label: "Programa do PC", hint: "C:\\caminho\\programa.exe" },
  { v: "celular", label: "App do celular", hint: "pacote, ex.: com.whatsapp" },
];

const CORES = ["#34d399", "#60a5fa", "#f59e0b", "#a78bfa", "#f472b6", "#f87171", "#22d3ee", "#e5e7eb"];

function run(a: Atalho): void {
  void cmd("atalho", { id: a.id })
    .then(() => note(a.tipo === "texto" && !a.colar ? "Copiado no PC" : DONE[a.tipo]))
    .catch(feedback);
}

/** Saves the whole shortcut list (the rest of the settings go back unchanged). */
async function saveList(atalhos: Atalho[]): Promise<void> {
  await cmd("config_set", { ...structuredClone(unwrap(config)), atalhos });
}

export function Atalhos(): JSX.Element {
  const [editing, setEditing] = createSignal(false);
  const [sheet, setSheet] = createSignal<{ a: Atalho; index: number } | null>(null);
  const home = () => (phone ? phone.home() : void cmd("celular_home").catch(feedback));
  const fresh = (): Atalho => ({ id: `a${Date.now().toString(36)}`, nome: "", tipo: "texto", valor: "", colar: true, cor: CORES[config.atalhos.length % CORES.length] });

  return (
    <div class="shortcuts">
      <div class="sc-head">
        <b>Atalhos</b>
        <span class="dim">{editing() ? "toque num atalho para editar" : "toque para usar no PC"}</span>
        <span class="grow" />
        <button class="btn ghost sm" classList={{ on: editing() }} onClick={() => setEditing((v) => !v)}>
          <Show when={editing()} fallback={<><Pencil size={15} /> Editar</>}>
            <Check size={15} /> Pronto
          </Show>
        </button>
      </div>
      <div class="tiles">
        <Show when={!editing()}>
          <button class="tile sys easypac" onClick={() => void cmd("abrir_easypac").then(() => note("Abrindo o EasyPac"), feedback)}>
            <Smartphone size={26} />
            <b>Abrir EasyPac</b>
            <span>{phoneState().easypac || "app não encontrado"}</span>
          </button>
          <button class="tile sys" onClick={home}>
            <House size={26} />
            <b>Celular normal</b>
            <span>volta quando replugar ou tocar no ícone</span>
          </button>
          <button class="tile sys" onClick={() => void cmd("whats_abrir").catch(feedback)}>
            <MessageCircle size={26} />
            <b>WhatsApp no PC</b>
            <span>traz a janela pra frente</span>
          </button>
          <button class="tile sys" onClick={() => void cmd("copiar", { texto: dayText(startOfDay(Date.now())) }).then(() => note("Resumo do dia copiado no PC"), feedback)}>
            <ClipboardCopy size={26} />
            <b>Resumo do dia</b>
            <span>copia no PC</span>
          </button>
        </Show>
        <For each={config.atalhos}>
          {(a, i) => {
            const Icon = ICON[a.tipo] ?? Type;
            return (
              <button
                class="tile"
                classList={{ editing: editing() }}
                style={{ "--c": a.cor || "#60a5fa" }}
                onClick={() => (editing() ? setSheet({ a: { ...a }, index: i() }) : run(a))}
                onContextMenu={(ev) => {
                  ev.preventDefault();
                  setSheet({ a: { ...a }, index: i() });
                }}
              >
                <Icon size={24} />
                <Show when={editing()}>
                  <span class="edit-badge">
                    <Pencil size={13} />
                  </span>
                </Show>
                <b>{a.nome || "Sem nome"}</b>
                <span>{a.tipo === "texto" ? a.valor : a.valor.replace(/^https?:\/\//, "")}</span>
              </button>
            );
          }}
        </For>
        <Show when={editing()}>
          <button class="tile add" onClick={() => setSheet({ a: fresh(), index: -1 })}>
            <Plus size={28} />
            <b>Novo atalho</b>
            <span>frase, site, programa ou app</span>
          </button>
        </Show>
      </div>
      <Show when={sheet()}>{(s) => <AtalhoSheet a={s().a} index={s().index} close={() => setSheet(null)} />}</Show>
    </div>
  );
}

function AtalhoSheet(p: { a: Atalho; index: number; close: () => void }): JSX.Element {
  const [a, setA] = createSignal<Atalho>(p.a);
  const set = <K extends keyof Atalho>(k: K, v: Atalho[K]) => setA((x) => ({ ...x, [k]: v }));
  const isNew = p.index < 0;
  const list = () => config.atalhos.map((x) => ({ ...x }));

  const save = () => {
    const cur = a();
    if (!cur.nome.trim() || !cur.valor.trim()) return note("Preencha o nome e o conteúdo", true);
    const l = list();
    if (isNew) l.push(cur);
    else l[p.index] = cur;
    void saveList(l).then(() => {
      note(isNew ? "Atalho criado" : "Atalho salvo");
      p.close();
    }, feedback);
  };
  const remove = () => {
    void saveList(list().filter((_, j) => j !== p.index)).then(() => {
      note("Atalho apagado");
      p.close();
    }, feedback);
  };
  const move = (d: number) => {
    const l = list();
    const j = p.index + d;
    if (j < 0 || j >= l.length) return;
    [l[p.index], l[j]] = [l[j], l[p.index]];
    void saveList(l).then(p.close, feedback);
  };

  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet" onClick={(e) => e.stopPropagation()}>
        <div class="sheet-head">
          <b>{isNew ? "Novo atalho" : "Editar atalho"}</b>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <div class="sheet-grid two">
          <label>
            Nome
            <input value={a().nome} placeholder="Ex.: Pedir CNPJ" onInput={(e) => set("nome", e.currentTarget.value)} />
          </label>
          <div class="field-like">
            Tipo
            <div class="seg">
              <For each={TIPOS}>
                {(t) => (
                  <button classList={{ on: a().tipo === t.v }} onClick={() => set("tipo", t.v)}>
                    {t.label}
                  </button>
                )}
              </For>
            </div>
          </div>
          <label class="span">
            {a().tipo === "texto" ? "Frase" : "Endereço"}
            <textarea rows={a().tipo === "texto" ? 3 : 1} value={a().valor} placeholder={TIPOS.find((t) => t.v === a().tipo)?.hint} onInput={(e) => set("valor", e.currentTarget.value)} />
          </label>
          <div class="span swatches">
            <For each={CORES}>
              {(c) => <button class="swatch" classList={{ on: a().cor === c }} style={{ background: c }} onClick={() => set("cor", c)} aria-label={c} />}
            </For>
          </div>
          <Show when={a().tipo === "texto"}>
            <label class="check span">
              <input type="checkbox" checked={a().colar} onChange={(e) => set("colar", e.currentTarget.checked)} /> Colar sozinho onde o cursor estiver no PC
            </label>
          </Show>
        </div>
        <div class="row">
          <Show when={!isNew}>
            <button class="btn ghost red-text" onClick={remove}>
              <Trash2 size={16} /> Apagar
            </button>
            <button class="icon" onClick={() => move(-1)} aria-label="Mover para trás">
              <ChevronLeft size={20} />
            </button>
            <button class="icon" onClick={() => move(1)} aria-label="Mover para frente">
              <ChevronRight size={20} />
            </button>
          </Show>
          <span class="grow" />
          <button class="btn green" onClick={save}>
            <Check size={16} /> Salvar
          </button>
        </div>
      </div>
    </div>
  );
}
