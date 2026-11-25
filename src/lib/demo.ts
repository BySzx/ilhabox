// Fake data for looking at the screens in a browser (?demo). Never used by the real app.
import type { Config, Entry, Media, WMsg } from "./state";

type Fire = (topic: string, v: unknown) => void;
let fire: Fire = () => undefined;
let entries: Entry[] = [];
let config: Config;

const at = (h: number, m: number): number => {
  const d = new Date();
  d.setHours(h, m, 0, 0);
  return d.getTime();
};

function publish(): void {
  fire("work", { entries });
}

export function seed(f: Fire): void {
  fire = f;
  (window as unknown as { __fire: Fire }).__fire = f;
  const now = Date.now();
  const mk = (id: number, tipo: string, ticket: string, desc: string, segs: [number, number | null][], done: boolean, sent = false): Entry => ({
    id, tipo, ticket, desc, segs: segs.map(([s, e]) => ({ s, e, sent })), done, sent, start: segs[0][0], end: done ? segs[segs.length - 1][1] : null,
  });
  entries = [
    mk(1, "Ticket", "48211", "Cliente sem acesso ao módulo financeiro, senha resetada", [[at(8, 4), at(8, 41)]], true, true),
    mk(2, "Curso", "", "curso sql rhgestor", [[at(13, 35), at(13, 45)]], false),
    mk(3, "WhatsApp", "", "dúvida sobre status inativo plano de ação", [[at(13, 48), at(14, 10)]], true),
    mk(4, "Reunião", "", "levantamento de requisitos", [[now - 23 * 60000 - 17000, null]], false),
  ];
  config = {
    nome: "Fulano",
    empresa: "EasyPac",
    canal: "teste",
    climaLat: null, climaLon: null, climaNome: "",
    atalhosOrdem: [], atalhosOcultos: [],
    atalhos: [
      { id: "saudacao", nome: "Saudação", tipo: "texto", valor: "Olá! Tudo bem?", colar: true, cor: "#34d399" },
      { id: "aguarde", nome: "Pedir um momento", tipo: "texto", valor: "Só um momento…", colar: true, cor: "#60a5fa" },
      { id: "print", nome: "Pedir print", tipo: "texto", valor: "Consegue me enviar um print?", colar: true, cor: "#f59e0b" },
      { id: "encerrar", nome: "Encerramento", tipo: "texto", valor: "Ficou alguma dúvida?", colar: true, cor: "#a78bfa" },
      { id: "acesso", nome: "Acesso remoto", tipo: "programa", valor: "C:\\anydesk.exe", colar: false, cor: "#f472b6" },
    ],
    modelo: "{inicio} às {fim} ({duracao}) · {atividade}\n{descricao}",
    tipos: ["Ticket", "WhatsApp", "Reunião", "Curso", "Dúvida interna", "Outro"],
    metaMin: 480, abrirAoConectar: true, easypacPacote: "", inicioTurno: "08:00", fimTurno: "18:00", mouseLado: "direita",
    movideskPalavra: "movidesk", lembreteMin: 10, lembreteLongoMin: 120, whatsMin: 2, pausarAoTravar: true, juntarPausaMin: 10, buracoMin: 15,
    checklists: [
      { nome: "Erro de NF-e", itens: ["Certificado digital válido", "Série e numeração", "Status da SEFAZ", "Dados do destinatário"] },
      { nome: "Sem acesso", itens: ["Usuário ativo", "Senha resetada", "Permissões do perfil", "Testado com o cliente"] },
    ],
  };
  const media: Media = {
    active: true, title: "Sorry", artist: "Madonna", album: "Confessions on a Dance Floor", playing: true,
    positionMs: 52_000, durationMs: 282_000, sampledAt: now, app: "Spotify.exe",
  };
  const msgs: WMsg[] = [
    { id: 1, at: now - 40 * 60000, chat: "Suporte N2", text: "Alguém viu o chamado da Farmácia Central?", avatar: null },
    { id: 2, at: now - 18 * 60000, chat: "Marina (Comercial)", text: "Ale, o cliente da 48230 ligou de novo", avatar: null },
    { id: 3, at: now - 3 * 60000, chat: "Equipe Suporte", text: "Reunião às 14h na sala 2", avatar: null },
  ];
  fire("config", config);
  publish();
  fire("media", media);
  fire("media-art", { key: "x", dataUrl: null });
  fire("whats-history", msgs);
  fire("phone", { status: "conectado", serial: "R58N", modelo: "SM A105M", easypacs: ["br.com.easypac.monitoramento", "br.com.easypac.supervisor"], appVersao: 1, erro: "" });
  fire("update", { current: "0.1.0", version: null });
  const h0 = Math.ceil(now / 3_600_000) * 3_600_000;
  fire("clima", {
    local: "Centro, Cidade Exemplo", origem: "windows", temp: 26.5, sensacao: 28.5, umidade: 70, vento: 12, min: 19.5, max: 28.6,
    cond: "parcial", dia: true, chuva: 40, mm: 0.4, diferenca: 1.8, em: now - 4 * 60000,
    horas: Array.from({ length: 12 }, (_, k) => ({ t: h0 + k * 3_600_000, temp: Math.round((27 - Math.abs(k - 2) * 0.8) * 10) / 10, mm: k === 3 || k === 4 ? 1.2 : 0 })),
    previsto: 26.6,
    medido: { estacao: "CIDADE EXEMPLO", km: 6, temp: 26.5, em: h0 - 3_600_000 },
    alertas: [{ evento: "Tempestade", grau: "Perigo potencial", ate: h0 + 10 * 3_600_000, texto: "Chuva entre 20 e 30 mm/h ou até 50 mm/dia, ventos intensos (40-60 km/h) e queda de granizo." }],
    fontes: [
      { nome: "ECMWF", ok: true, temp: 26.5, cond: "parcial" }, { nome: "NOAA GFS", ok: true, temp: 27.5, cond: "chuva" }, { nome: "DWD ICON", ok: true, temp: 25.7, cond: "parcial" },
      { nome: "MET Norway", ok: true, temp: 26.3, cond: "nublado" }, { nome: "wttr.in", ok: false }, { nome: "CPTEC/INPE", ok: true, min: 19, max: 29 },
      { nome: "INMET estação", ok: true }, { nome: "INMET avisos", ok: true },
    ],
  });
  setTimeout(() => fire("whats", { id: 4, at: Date.now(), chat: "Marina (Comercial)", text: "Consegue olhar agora?", avatar: null }), 2500);
  if (new URLSearchParams(location.search).has("sugestao"))
    setTimeout(() => fire("sugestao", {
      id: "ticket-48230", text: "Ticket #48230 aberto no Movidesk", detail: "Erro ao emitir NF-e", at: Date.now(),
      actions: [{ label: "Iniciar #48230", cmd: "work_start", args: { tipo: "Ticket", ticket: "48230", desc: "Erro ao emitir NF-e" } }, { label: "Agora não" }],
    }), 300);
}

export function demoCmd(name: string, args: Record<string, unknown>): unknown {
  const now = Date.now();
  const pauseAll = () => entries.forEach((e) => {
    const l = e.segs[e.segs.length - 1];
    if (l && l.e == null) l.e = now;
  });
  const refresh = (e: Entry) => ({ ...e, segs: e.segs.map((g) => ({ ...g })), start: e.segs[0].s, end: e.done ? e.segs[e.segs.length - 1].e : null, sent: e.segs.every((g) => g.sent) });
  if (name === "work_start") {
    pauseAll();
    entries.push({ id: entries.length + 1, tipo: String(args.tipo ?? "Ticket"), ticket: args.tipo === "Ticket" ? String(args.ticket ?? "") : "", desc: String(args.desc ?? ""), segs: [{ s: now, e: null }], done: false, sent: false, start: now, end: null });
  } else if (name === "work_add") {
    const segs = args.segs as { s: number; e: number }[];
    entries.push({ id: entries.length + 1, tipo: String(args.tipo ?? "Ticket"), ticket: String(args.ticket ?? ""), desc: String(args.desc ?? ""), segs, done: true, sent: false, start: segs[0].s, end: segs[segs.length - 1].e });
  } else if (name === "sugestao_fechar") fire("sugestao", null);
  else if (name === "work_pause") pauseAll();
  else if (name === "work_resume") {
    pauseAll();
    const e = entries.find((x) => x.id === args.id);
    if (e) {
      e.segs.push({ s: now, e: null });
      e.done = false;
    }
  } else if (name === "work_finish" || name === "work_stop") {
    const e = args.id != null ? entries.find((x) => x.id === args.id) : entries.find((x) => x.segs[x.segs.length - 1].e == null);
    if (e) {
      const l = e.segs[e.segs.length - 1];
      if (l.e == null) l.e = now;
      e.done = true;
    }
  } else if (name === "work_delete") entries = entries.filter((e) => e.id !== args.id);
  else if (name === "work_update") entries = entries.map((e) => (e.id === args.id ? ({ ...e, ...args } as Entry) : e));
  else if (name === "work_sent") {
    const ps = (args.periods as { id: number; s: number }[]) ?? [];
    entries = entries.map((e) => ({ ...e, segs: e.segs.map((g) => (ps.some((p) => p.id === e.id && p.s === g.s) || ((args.ids as number[]) ?? []).includes(e.id) ? { ...g, sent: true } : g)) }));
  } else if (name === "work_reopen") {
    const e = entries.find((x) => x.id === args.id);
    if (e) e.segs[e.segs.length - 1].e = null;
  }
  else if (name === "config_set") {
    config = args as unknown as Config;
    fire("config", config);
  } else if (name === "lyrics") {
    return Array.from({ length: 12 }, (_, i) => `[00:${String(40 + i * 4).padStart(2, "0")}.00]Linha ${i + 1} da letra de exemplo`).join("\n");
  }
  entries = entries.map(refresh);
  publish();
  return null;
}
