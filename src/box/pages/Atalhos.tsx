import { Check, ClipboardCopy, Eye, EyeOff, Globe, House, MessageCircle, Pencil, Plus, Power, Rocket, Smartphone, Trash2, Type, X } from "lucide-solid";
import { createMemo, createSignal, For, Show, type JSX } from "solid-js";
import { unwrap } from "solid-js/store";
import { cmd, phone } from "../../lib/api";
import { appName, config, dayText, phoneState, startOfDay, type Atalho, type Config } from "../../lib/state";
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
  { v: "texto", label: "Frase", hint: "Texto que vai pro PC; {nome} e {empresa} viram os dados dos Ajustes" },
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

/** Saves part of the settings (the rest goes back unchanged). */
async function saveConfig(patch: Partial<Config>): Promise<void> {
  await cmd("config_set", { ...structuredClone(unwrap(config)), ...patch });
}

const saveList = (atalhos: Atalho[]) => saveConfig({ atalhos });

type Tile = {
  id: string;
  icon: (p: { size: number }) => JSX.Element;
  title: string;
  sub: string;
  cls: string;
  color?: string;
  run?: () => void;
  index?: number;
};

/** Tile ids in the user's order: known ones first as arranged, new ones after in their natural order. */
function arrange(tiles: Tile[], order: string[]): Tile[] {
  const pos = (id: string) => {
    const i = order.indexOf(id);
    return i < 0 ? order.length : i;
  };
  return tiles.map((t, i) => ({ t, i })).sort((a, b) => pos(a.t.id) - pos(b.t.id) || a.i - b.i).map((x) => x.t);
}

export function Atalhos(): JSX.Element {
  const [editing, setEditing] = createSignal(false);
  const [picked, setPicked] = createSignal<string | null>(null);
  const [sheet, setSheet] = createSignal<{ a: Atalho; index: number } | null>(null);
  const home = () => (phone ? phone.home() : void cmd("celular_home").catch(feedback));
  // Two taps: the first arms it for 3 s, the second shuts the PC down (a brush of the finger never does).
  const [armed, setArmed] = createSignal(false);
  let armT = 0;
  const shutdown = () => {
    if (!armed()) {
      setArmed(true);
      clearTimeout(armT);
      armT = window.setTimeout(() => setArmed(false), 3000);
      return;
    }
    setArmed(false);
    void cmd("desligar_pc").then(() => note("Desligando o PC…"), feedback);
  };
  const fresh = (): Atalho => ({ id: `a${Date.now().toString(36)}`, nome: "", tipo: "texto", valor: "", colar: true, cor: CORES[config.atalhos.length % CORES.length] });

  const tiles = createMemo((): Tile[] => {
    const pkgs = phoneState().easypacs;
    const easypac: Tile[] = pkgs.length
      ? pkgs.map((pkg) => ({
          id: `sys:easypac:${pkg}`,
          icon: Smartphone,
          title: appName(pkg),
          sub: pkg,
          cls: "sys easypac",
          run: () => void cmd("abrir_celular_app", { pacote: pkg }).then(() => note(`Abrindo ${appName(pkg)}`), feedback),
        }))
      : [{ id: "sys:easypac", icon: Smartphone, title: "EasyPac", sub: "nenhum app EasyPac achado no celular", cls: "sys easypac off" }];
    const system: Tile[] = [
      ...easypac,
      { id: "sys:celular", icon: House, title: "Celular normal", sub: "volta quando replugar ou tocar no ícone", cls: "sys", run: home },
      { id: "sys:whats", icon: MessageCircle, title: "WhatsApp no PC", sub: "traz a janela pra frente", cls: "sys", run: () => void cmd("whats_abrir").catch(feedback) },
      {
        id: "sys:desligar",
        icon: Power,
        title: armed() ? "Toque de novo" : "Desligar PC",
        sub: armed() ? "pra desligar agora" : "pausa o apontamento e desliga",
        cls: armed() ? "sys power armed" : "sys power",
        run: shutdown,
      },
      {
        id: "sys:resumo",
        icon: ClipboardCopy,
        title: "Resumo do dia",
        sub: "copia no PC",
        cls: "sys",
        run: () => void cmd("copiar", { texto: dayText(startOfDay(Date.now())) }).then(() => note("Resumo do dia copiado no PC"), feedback),
      },
    ];
    const own: Tile[] = config.atalhos.map((a, index) => ({
      id: a.id,
      icon: ICON[a.tipo] ?? Type,
      title: a.nome || "Sem nome",
      sub: a.tipo === "texto" ? a.valor : a.valor.replace(/^https?:\/\//, ""),
      cls: "",
      color: a.cor || "#60a5fa",
      run: () => run(a),
      index,
    }));
    return arrange([...system, ...own], config.atalhosOrdem);
  });
  const hidden = (id: string) => config.atalhosOcultos.includes(id);
  const shown = () => (editing() ? tiles() : tiles().filter((t) => !hidden(t.id)));

  /** Edit mode: the first tap picks a tile up, the second puts it in the place of the one tapped. */
  const tap = (t: Tile) => {
    const from = picked();
    if (!from) return setPicked(t.id);
    setPicked(null);
    if (from === t.id) return;
    const ids = tiles().map((x) => x.id);
    ids.splice(ids.indexOf(from), 1);
    ids.splice(tiles().findIndex((x) => x.id === t.id), 0, from);
    void saveConfig({ atalhosOrdem: ids }).catch(feedback);
  };
  const toggleHidden = (id: string) => {
    const l = hidden(id) ? config.atalhosOcultos.filter((x) => x !== id) : [...config.atalhosOcultos, id];
    void saveConfig({ atalhosOcultos: l }).catch(feedback);
  };
  const edit = (t: Tile) => t.index != null && setSheet({ a: { ...config.atalhos[t.index] }, index: t.index });

  return (
    <div class="shortcuts">
      <div class="sc-head">
        <b>Atalhos</b>
        <span class="dim">{editing() ? (picked() ? "agora toque no lugar para onde ele vai" : "toque num atalho para mudar de lugar") : "toque para usar no PC"}</span>
        <span class="grow" />
        <Show when={editing() && config.atalhosOrdem.length}>
          <button class="btn ghost sm" onClick={() => void saveConfig({ atalhosOrdem: [], atalhosOcultos: [] }).catch(feedback)}>
            Ordem original
          </button>
        </Show>
        <button
          class="btn ghost sm"
          classList={{ on: editing() }}
          onClick={() => {
            setPicked(null);
            setEditing((v) => !v);
          }}
        >
          <Show when={editing()} fallback={<><Pencil size={15} /> Organizar</>}>
            <Check size={15} /> Pronto
          </Show>
        </button>
      </div>
      <div class="tiles" classList={{ arranging: editing() }}>
        <For each={shown()}>
          {(t) => (
            <button
              class={`tile ${t.cls}`}
              classList={{ editing: editing(), picked: picked() === t.id, hidden: hidden(t.id), target: !!picked() && picked() !== t.id }}
              style={t.color ? { "--c": t.color } : undefined}
              onClick={() => (editing() ? tap(t) : t.run?.())}
              onContextMenu={(ev) => {
                ev.preventDefault();
                edit(t);
              }}
            >
              <t.icon size={t.index != null ? 24 : 26} />
              <Show when={editing()}>
                <span class="tile-tools">
                  <Show
                    when={t.index != null}
                    fallback={
                      <span
                        class="edit-badge"
                        role="button"
                        aria-label={hidden(t.id) ? "Mostrar" : "Esconder"}
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleHidden(t.id);
                        }}
                      >
                        <Show when={hidden(t.id)} fallback={<Eye size={13} />}>
                          <EyeOff size={13} />
                        </Show>
                      </span>
                    }
                  >
                    <span
                      class="edit-badge"
                      role="button"
                      aria-label="Editar"
                      onClick={(e) => {
                        e.stopPropagation();
                        edit(t);
                      }}
                    >
                      <Pencil size={13} />
                    </span>
                  </Show>
                </span>
              </Show>
              <b>{t.title}</b>
              <span>{hidden(t.id) && editing() ? "escondido" : t.sub}</span>
            </button>
          )}
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
