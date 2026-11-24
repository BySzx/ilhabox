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
  entries = [
    { id: 1, tipo: "Ticket", ticket: "48211", desc: "Cliente sem acesso ao módulo financeiro, senha resetada", start: at(8, 4), end: at(8, 41), sent: true },
    { id: 2, tipo: "Ticket", ticket: "48230", desc: "Erro ao emitir NF-e, certificado vencido", start: at(8, 45), end: at(9, 52), sent: false },
    { id: 3, tipo: "Reunião", ticket: "", desc: "Alinhamento semanal do suporte", start: at(9, 58), end: at(10, 20), sent: false },
    { id: 4, tipo: "Ticket", ticket: "48241", desc: "Integração travando na sincronização", start: now - 23 * 60000 - 17000, end: null, sent: false },
  ];
  config = {
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
  fire("phone", { status: "conectado", serial: "R58N", modelo: "SM A105M", easypac: "br.com.easypac.app", appVersao: 1, erro: "" });
  fire("update", { current: "0.1.0", version: null });
  setTimeout(() => fire("whats", { id: 4, at: Date.now(), chat: "Marina (Comercial)", text: "Consegue olhar agora?", avatar: null }), 2500);
}

export function demoCmd(name: string, args: Record<string, unknown>): unknown {
  const now = Date.now();
  if (name === "work_start") {
    entries = entries.map((e) => (e.end == null ? { ...e, end: now } : e));
    entries.push({ id: entries.length + 1, tipo: String(args.tipo ?? "Ticket"), ticket: args.tipo === "Ticket" ? String(args.ticket ?? "") : "", desc: String(args.desc ?? ""), start: now, end: null, sent: false });
  } else if (name === "work_stop") entries = entries.map((e) => (e.end == null ? { ...e, end: now } : e));
  else if (name === "work_delete") entries = entries.filter((e) => e.id !== args.id);
  else if (name === "work_update") entries = entries.map((e) => (e.id === args.id ? { ...e, ...args } as Entry : e));
  else if (name === "work_sent") entries = entries.map((e) => ((args.ids as number[]).includes(e.id) ? { ...e, sent: true } : e));
  else if (name === "config_set") {
    config = args as unknown as Config;
    fire("config", config);
  } else if (name === "lyrics") {
    return Array.from({ length: 12 }, (_, i) => `[00:${String(40 + i * 4).padStart(2, "0")}.00]Linha ${i + 1} da letra de exemplo`).join("\n");
  }
  entries = [...entries];
  publish();
  return null;
}
