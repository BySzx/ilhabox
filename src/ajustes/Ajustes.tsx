import { ArrowDown, ArrowUp, Check, Monitor, Plus, Smartphone, Trash2 } from "lucide-solid";
import { createEffect, createSignal, For, on, onMount, Show, type JSX } from "solid-js";
import { createStore, reconcile, unwrap } from "solid-js/store";
import { cmd, online } from "../lib/api";
import { config, entryText, phoneState, update, type Atalho, type Config } from "../lib/state";

const STATUS: Record<string, string> = {
  conectado: "Conectado",
  desconectado: "Nenhum celular no cabo",
  autorizar: "Aceite a depuração USB na tela do celular",
  instalando: "Preparando o celular…",
  "baixando-adb": "Baixando as ferramentas do Android…",
  "sem-adb": "Sem adb",
  erro: "Erro",
};

const TIPOS: { v: Atalho["tipo"]; label: string; hint: string }[] = [
  { v: "texto", label: "Texto", hint: "Mensagem que vai pro PC; {nome} e {empresa} viram os dados acima" },
  { v: "site", label: "Site", hint: "https://…" },
  { v: "programa", label: "Programa", hint: "C:\\caminho\\programa.exe" },
  { v: "celular", label: "App do celular", hint: "pacote, ex.: com.whatsapp" },
];

async function invoke<T>(name: string, args?: Record<string, unknown>): Promise<T | undefined> {
  if (!("__TAURI_INTERNALS__" in window)) return undefined;
  const core = await import("@tauri-apps/api/core");
  return core.invoke<T>(name, args);
}

export function Ajustes(): JSX.Element {
  const [draft, setDraft] = createStore<Config>(structuredClone(unwrap(config)));
  const [dirty, setDirty] = createSignal(false);
  const [saved, setSaved] = createSignal("");
  const [auto, setAuto] = createSignal(false);
  const [probe, setProbe] = createSignal<{ title: string; url: string | null; ticket: [string, string] | null } | null>(null);

  // A fresh copy whenever the agent sends the config, unless there are unsaved edits.
  createEffect(on(() => JSON.stringify(config), () => !dirty() && setDraft(reconcile(structuredClone(unwrap(config))))));
  onMount(() => void invoke<boolean>("autostart").then((v) => setAuto(!!v)));

  const edit = (fn: () => void) => {
    fn();
    setDirty(true);
    setSaved("");
  };
  const save = async () => {
    try {
      const clean = structuredClone(unwrap(draft));
      clean.checklists = clean.checklists.map((c) => ({ nome: c.nome.trim(), itens: c.itens.map((x) => x.trim()).filter(Boolean) })).filter((c) => c.nome);
      await cmd("config_set", clean as unknown as Record<string, unknown>);
      setDirty(false);
      setSaved("Salvo");
    } catch (e) {
      setSaved(`Não salvou: ${e instanceof Error ? e.message : e}`);
    }
  };
  const addAtalho = () =>
    edit(() => setDraft("atalhos", (l) => [...l, { id: `a${Date.now().toString(36)}`, nome: "Novo atalho", tipo: "texto", valor: "", colar: true, cor: "#60a5fa" }]));
  const move = (i: number, d: number) =>
    edit(() =>
      setDraft("atalhos", (l) => {
        const c = l.slice();
        const j = i + d;
        if (j < 0 || j >= c.length) return l;
        [c[i], c[j]] = [c[j], c[i]];
        return c;
      }),
    );
  const preview = () => {
    const t = Date.now();
    return entryText({
      id: 0, tipo: "Ticket", ticket: "48230", desc: "Erro ao emitir NF-e, certificado vencido", done: true, sent: false, start: t - 90 * 60000, end: t,
      segs: [{ s: t - 90 * 60000, e: t - 60 * 60000 }, { s: t - 30 * 60000, e: t }],
    });
  };

  return (
    <div class="wrap">
      <header class="top">
        <div class="brand">
          <span class="dot" classList={{ off: !online() }} />
          IlhaBox
          <span class="dim ver mono">v{update().current}</span>
          <span class="canal" classList={{ test: config.canal === "teste" }}>{config.canal === "teste" ? "canal teste" : "produção"}</span>
        </div>
        <span class="grow" />
        <Show when={saved()}>
          <span class="saved">{saved()}</span>
        </Show>
        <button class="btn green" disabled={!dirty()} onClick={() => void save()}>
          <Check size={16} /> Salvar
        </button>
      </header>

      <main class="sections">
        <section class="box">
          <h2>
            <Smartphone size={18} /> Celular
          </h2>
          <div class="status" classList={{ ok: phoneState().status === "conectado", bad: phoneState().status === "erro" }}>
            <b>{STATUS[phoneState().status] ?? phoneState().status}</b>
            <Show when={phoneState().modelo}>
              <span class="dim">{phoneState().modelo} · app v{phoneState().appVersao}</span>
            </Show>
            <Show when={phoneState().erro}>
              <span class="err">{phoneState().erro}</span>
            </Show>
          </div>
          <div class="row">
            <button class="btn" disabled={phoneState().status !== "conectado"} onClick={() => void cmd("abrir_ilhabox").catch(() => undefined)}>
              <Smartphone size={16} /> Mostrar no celular
            </button>
            <button class="btn" onClick={() => void cmd("abrir_painel_pc").catch(() => undefined)}>
              <Monitor size={16} /> Ver o painel no PC
            </button>
          </div>
          <label class="toggle">
            <input type="checkbox" checked={draft.abrirAoConectar} onChange={(e) => edit(() => setDraft("abrirAoConectar", e.currentTarget.checked))} />
            Abrir a IlhaBox no celular quando ligar o cabo
          </label>
          <label class="field">
            Mouse do PC no celular: o celular fica de que lado das telas?
            <select value={draft.mouseLado} onChange={(e) => edit(() => setDraft("mouseLado", e.currentTarget.value))}>
              <option value="direita">À direita (empurre o mouse pra direita)</option>
              <option value="esquerda">À esquerda</option>
              <option value="baixo">Embaixo</option>
              <option value="cima">Em cima</option>
              <option value="desligado">Desligado</option>
            </select>
          </label>
          <label class="field">
            Apps EasyPac no celular (pacotes separados por vírgula; vazio = acha sozinho)
            <input
              class="mono"
              placeholder={phoneState().easypacs.length ? `achados: ${phoneState().easypacs.join(", ")}` : "achados sozinhos pelo nome"}
              value={draft.easypacPacote}
              onInput={(e) => edit(() => setDraft("easypacPacote", e.currentTarget.value.trim()))}
            />
          </label>
        </section>

        <section class="box">
          <h2>Apontamento de horas</h2>
          <label class="field">
            Texto pro Movidesk (um por período: apontamento pausado e continuado vira um texto pra cada pedaço)
            <textarea rows={3} class="mono" value={draft.modelo} onInput={(e) => edit(() => setDraft("modelo", e.currentTarget.value))} />
          </label>
          <span class="hint">
            Campos: <code>{"{atividade}"}</code> (Ticket 48230 ou Reunião) <code>{"{tipo}"}</code> <code>{"{ticket}"}</code> <code>{"{descricao}"}</code> <code>{"{inicio}"}</code> <code>{"{fim}"}</code> <code>{"{duracao}"}</code> (1h07){" "}
            <code>{"{horas}"}</code> (01:07) <code>{"{data}"}</code>
          </span>
          <pre class="preview">{preview()}</pre>
          <label class="field">
            Tipos de atividade (separados por vírgula; "Ticket" é o que pede número)
            <input
              value={draft.tipos.join(", ")}
              onChange={(e) =>
                edit(() => {
                  const l = e.currentTarget.value.split(",").map((x) => x.trim()).filter(Boolean);
                  setDraft("tipos", l.includes("Ticket") ? l : ["Ticket", ...l]);
                })
              }
            />
          </label>
          <div class="row">
            <label class="field small">
              Meta do dia (horas)
              <input
                type="number"
                min="1"
                max="16"
                step="0.5"
                value={draft.metaMin / 60}
                onInput={(e) => edit(() => setDraft("metaMin", Math.round(Number(e.currentTarget.value) * 60) || 480))}
              />
            </label>
          </div>
        </section>

        <section class="box wide">
          <h2>
            Atalhos de suporte
            <span class="grow" />
            <button class="btn sm" onClick={addAtalho}>
              <Plus size={15} /> Novo
            </button>
          </h2>
          <div class="row">
            <label class="field small">
              Seu nome
              <input value={draft.nome} placeholder="como aparece nas frases" onInput={(e) => edit(() => setDraft("nome", e.currentTarget.value))} />
            </label>
            <label class="field small">
              Empresa
              <input value={draft.empresa} onInput={(e) => edit(() => setDraft("empresa", e.currentTarget.value))} />
            </label>
            <span class="hint">Nas frases, escreva {"{nome}"} e {"{empresa}"}.</span>
          </div>
          <div class="atalhos">
            <For each={draft.atalhos}>
              {(a, i) => (
                <div class="atalho" style={{ "--c": a.cor }}>
                  <input type="color" value={a.cor} onInput={(e) => edit(() => setDraft("atalhos", i(), "cor", e.currentTarget.value))} />
                  <input class="name" value={a.nome} onInput={(e) => edit(() => setDraft("atalhos", i(), "nome", e.currentTarget.value))} />
                  <select value={a.tipo} onChange={(e) => edit(() => setDraft("atalhos", i(), "tipo", e.currentTarget.value as Atalho["tipo"]))}>
                    <For each={TIPOS}>{(t) => <option value={t.v}>{t.label}</option>}</For>
                  </select>
                  <textarea
                    rows={a.tipo === "texto" ? 2 : 1}
                    placeholder={TIPOS.find((t) => t.v === a.tipo)?.hint}
                    value={a.valor}
                    onInput={(e) => edit(() => setDraft("atalhos", i(), "valor", e.currentTarget.value))}
                  />
                  <Show when={a.tipo === "texto"}>
                    <label class="toggle small">
                      <input type="checkbox" checked={a.colar} onChange={(e) => edit(() => setDraft("atalhos", i(), "colar", e.currentTarget.checked))} />
                      colar sozinho
                    </label>
                  </Show>
                  <div class="a-act">
                    <button class="icon" onClick={() => move(i(), -1)} aria-label="Subir">
                      <ArrowUp size={15} />
                    </button>
                    <button class="icon" onClick={() => move(i(), 1)} aria-label="Descer">
                      <ArrowDown size={15} />
                    </button>
                    <button class="icon danger" onClick={() => edit(() => setDraft("atalhos", (l) => l.filter((_, j) => j !== i())))} aria-label="Apagar">
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </section>

        <section class="box">
          <h2>Automático</h2>
          <div class="row">
            <label class="field small">
              Início do turno
              <input type="time" value={draft.inicioTurno} onChange={(e) => edit(() => setDraft("inicioTurno", e.currentTarget.value))} />
            </label>
            <label class="field small">
              Fim do turno
              <input type="time" value={draft.fimTurno} onChange={(e) => edit(() => setDraft("fimTurno", e.currentTarget.value))} />
            </label>
          </div>
          <div class="row">
            <label class="field small">
              Avisar parado após (min)
              <input type="number" min="0" value={draft.lembreteMin} onInput={(e) => edit(() => setDraft("lembreteMin", Number(e.currentTarget.value) || 0))} />
            </label>
            <label class="field small">
              "Ainda nisso?" após (min)
              <input type="number" min="0" value={draft.lembreteLongoMin} onInput={(e) => edit(() => setDraft("lembreteLongoMin", Number(e.currentTarget.value) || 0))} />
            </label>
            <label class="field small">
              WhatsApp na frente por (min)
              <input type="number" min="0" value={draft.whatsMin} onInput={(e) => edit(() => setDraft("whatsMin", Number(e.currentTarget.value) || 0))} />
            </label>
          </div>
          <span class="hint">0 desliga o aviso.</span>
          <div class="row">
            <label class="field small">
              Pausa curta que não divide (min)
              <input type="number" min="0" value={draft.juntarPausaMin} onInput={(e) => edit(() => setDraft("juntarPausaMin", Number(e.currentTarget.value) || 0))} />
            </label>
            <label class="field small">
              Buraco a partir de (min)
              <input type="number" min="1" value={draft.buracoMin} onInput={(e) => edit(() => setDraft("buracoMin", Number(e.currentTarget.value) || 15))} />
            </label>
          </div>
          <label class="toggle">
            <input type="checkbox" checked={draft.pausarAoTravar} onChange={(e) => edit(() => setDraft("pausarAoTravar", e.currentTarget.checked))} />
            Pausar o apontamento e apagar a tela do celular quando travar o PC (Win+L)
          </label>
          <label class="field">
            Ticket: lido do endereço do Movidesk no navegador. Reserva: palavra no título da janela
            <input value={draft.movideskPalavra} onInput={(e) => edit(() => setDraft("movideskPalavra", e.currentTarget.value.trim()))} />
          </label>
          <div class="row">
            <button class="btn sm" onClick={() => setTimeout(() => void cmd<{ title: string; url: string | null; ticket: [string, string] | null }>("janela_atual").then(setProbe, () => undefined), 4000)}>
              Testar em 4 s
            </button>
            <span class="hint">Clique, vá pra aba do ticket no navegador e espere.</span>
          </div>
          <Show when={probe()}>
            {(pr) => (
              <div class="preview">
                Janela: {pr().title || "(sem título)"}
                {"\n"}
                Endereço: {pr().url ?? "(não é navegador ou não deu pra ler)"}
                {"\n"}
                {pr().ticket ? `Ticket ${pr().ticket![0]}${pr().ticket![1] ? ` · ${pr().ticket![1]}` : ""}` : "Nenhum ticket reconhecido"}
              </div>
            )}
          </Show>
        </section>

        <section class="box">
          <h2>
            Checklists
            <span class="grow" />
            <button class="btn sm" onClick={() => edit(() => setDraft("checklists", (l) => [...l, { nome: "Nova checklist", itens: [] }]))}>
              <Plus size={15} /> Nova
            </button>
          </h2>
          <For each={draft.checklists}>
            {(c, i) => (
              <div class="atalho">
                <input class="name" style={{ "grid-column": "1 / 3" }} value={c.nome} onInput={(e) => edit(() => setDraft("checklists", i(), "nome", e.currentTarget.value))} />
                <div class="a-act">
                  <button class="icon danger" onClick={() => edit(() => setDraft("checklists", (l) => l.filter((_, j) => j !== i())))} aria-label="Apagar">
                    <Trash2 size={15} />
                  </button>
                </div>
                <textarea
                  rows={Math.max(3, c.itens.length)}
                  placeholder="Um item por linha"
                  value={c.itens.join("\n")}
                  onInput={(e) => edit(() => setDraft("checklists", i(), "itens", e.currentTarget.value.split("\n")))}
                />
              </div>
            )}
          </For>
        </section>

        <section class="box">
          <h2>Sistema</h2>
          <label class="toggle">
            <input type="checkbox" checked={auto()} onChange={(e) => void invoke<boolean>("autostart", { on: e.currentTarget.checked }).then((v) => setAuto(!!v))} />
            Iniciar com o Windows
          </label>
          <label class="field small">
            Atualizações
            <select value={draft.canal} onChange={(e) => edit(() => setDraft("canal", e.currentTarget.value))}>
              <option value="producao">Produção (só as liberadas)</option>
              <option value="teste">Teste (todas, antes dos outros)</option>
            </select>
          </label>
          <div class="row">
            <Show
              when={update().version}
              fallback={
                <button class="btn" onClick={() => void cmd("update_check").catch(() => undefined)}>
                  Procurar atualização
                </button>
              }
            >
              <button class="btn green" onClick={() => void cmd("update_install").catch(() => undefined)}>
                {update().progress != null ? `Baixando ${Math.round((update().progress ?? 0) * 100)}%` : `Atualizar para v${update().version}`}
              </button>
            </Show>
            <button class="btn ghost" onClick={() => void invoke("quit")}>
              Fechar a IlhaBox
            </button>
          </div>
          <Show when={update().erro}>
            <span class="err">{update().erro}</span>
          </Show>
        </section>
      </main>
    </div>
  );
}
