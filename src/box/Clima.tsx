import { Cloud, CloudFog, CloudLightning, CloudMoon, CloudRain, CloudSun, Droplets, MapPin, Moon, RefreshCw, Sun, Thermometer, TriangleAlert, Wind, X } from "lucide-solid";
import { createSignal, For, Show, type JSX } from "solid-js";
import { cmd } from "../lib/api";
import { clima, now, type Cond } from "../lib/state";
import { feedback, note } from "./ui";

const TEXT: Record<Cond, string> = {
  sol: "Céu limpo",
  parcial: "Parcialmente nublado",
  nublado: "Nublado",
  neblina: "Neblina",
  chuva: "Chuva",
  tempestade: "Tempestade",
};

const ORIGEM = { manual: "escolhido nos Ajustes", windows: "localização do Windows", internet: "pela internet (aproximado)" };

export function CondIcon(p: { cond?: Cond; dia?: boolean; size: number }): JSX.Element {
  const day = p.dia !== false;
  switch (p.cond) {
    case "sol":
      return day ? <Sun size={p.size} class="wx-sun" /> : <Moon size={p.size} class="wx-moon" />;
    case "parcial":
      return day ? <CloudSun size={p.size} class="wx-sun" /> : <CloudMoon size={p.size} class="wx-moon" />;
    case "neblina":
      return <CloudFog size={p.size} class="wx-cloud" />;
    case "chuva":
      return <CloudRain size={p.size} class="wx-rain" />;
    case "tempestade":
      return <CloudLightning size={p.size} class="wx-storm" />;
    default:
      return <Cloud size={p.size} class="wx-cloud" />;
  }
}

const deg = (v?: number | null) => (v == null ? "–" : `${Math.round(v)}°`);
const level = (grau: string) => (grau === "Grande perigo" ? "red" : grau === "Perigo" ? "orange" : "yellow");
const day = (t: number) => new Date(t).toLocaleString("pt-BR", { weekday: "short", hour: "2-digit", minute: "2-digit" });
const hour = (t: number) => new Date(t).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

/** Small block for the clock card; a tap opens the details. */
export function ClimaMini(): JSX.Element {
  const [open, setOpen] = createSignal(false);
  return (
    <>
      <button class="wx-mini" onClick={() => setOpen(true)}>
        <Show when={clima()?.temp != null} fallback={<span class="dim wx-wait">{clima()?.erro ? "clima indisponível" : "buscando clima…"}</span>}>
          <div class="wx-now">
            <CondIcon cond={clima()?.cond} dia={clima()?.dia} size={34} />
            <b class="mono">{deg(clima()?.temp)}</b>
          </div>
          <span class="wx-range mono">
            {deg(clima()?.min)} / {deg(clima()?.max)}
          </span>
          <span class="wx-rain-line">
            <Droplets size={13} /> {clima()?.chuva ?? 0}%
          </span>
          <Show when={clima()?.alertas?.length}>
            <span class={`wx-warn ${level(clima()!.alertas![0].grau)}`}>
              <TriangleAlert size={12} /> {clima()!.alertas![0].evento}
            </span>
          </Show>
        </Show>
      </button>
      <Show when={open()}>
        <ClimaSheet close={() => setOpen(false)} />
      </Show>
    </>
  );
}

function ClimaSheet(p: { close: () => void }): JSX.Element {
  const c = () => clima() ?? {};
  const age = () => (c().em ? Math.max(0, Math.round((now() - (c().em ?? 0)) / 60000)) : null);
  const trust = () => {
    const d = c().diferenca ?? 0;
    return d <= 1.5 ? "concordam" : d <= 3 ? "divergem um pouco" : "divergem bastante";
  };
  const refresh = () => void cmd("clima_atualizar").then(() => note("Atualizando o clima…"), feedback);

  return (
    <div class="sheet-bg" onClick={p.close}>
      <div class="sheet wx-sheet" onClick={(e) => e.stopPropagation()}>
        <div class="sheet-head">
          <b>
            <MapPin size={16} /> {c().local || "Sua região"}
          </b>
          <span class="dim wx-origem">{c().origem ? ORIGEM[c().origem!] : ""}</span>
          <span class="grow" />
          <button class="icon" onClick={refresh} aria-label="Atualizar">
            <RefreshCw size={18} />
          </button>
          <button class="icon" onClick={p.close} aria-label="Fechar">
            <X size={20} />
          </button>
        </div>
        <For each={c().alertas ?? []}>
          {(a) => (
            <div class={`wx-alert ${level(a.grau)}`}>
              <TriangleAlert size={18} />
              <div>
                <b>
                  {a.evento} · {a.grau}
                  {a.ate ? ` · até ${day(a.ate)}` : ""}
                </b>
                <span>{a.texto}</span>
              </div>
            </div>
          )}
        </For>
        <Show when={c().temp != null} fallback={<p class="dim">{c().erro ?? "Buscando…"}</p>}>
          <div class="wx-main">
            <CondIcon cond={c().cond} dia={c().dia} size={56} />
            <div>
              <div class="wx-big mono">{deg(c().temp)}</div>
              <div>{TEXT[c().cond ?? "nublado"]}</div>
              <Show when={c().medido}>
                {(m) => (
                  <div class="wx-measured">
                    <Thermometer size={13} /> medido às {hour(m().em)} na estação {m().estacao} ({m().km} km)
                    {c().previsto != null ? ` · modelos: ${deg(c().previsto)}` : ""}
                  </div>
                )}
              </Show>
            </div>
            <div class="wx-facts">
              <span>Sensação <b class="mono">{deg(c().sensacao)}</b></span>
              <span>Mín / máx <b class="mono">{deg(c().min)} / {deg(c().max)}</b></span>
              <span><Droplets size={14} /> Chuva nas próximas 3 h <b class="mono">{c().chuva}%</b>{(c().mm ?? 0) >= 0.1 ? ` · ${c().mm} mm` : ""}</span>
              <span><Wind size={14} /> Vento <b class="mono">{c().vento ?? "–"} km/h</b> · umidade <b class="mono">{c().umidade ?? "–"}%</b></span>
            </div>
          </div>
          <div class="wx-hours">
            <For each={c().horas ?? []}>
              {(h) => (
                <div class="wx-hour" classList={{ wet: h.mm >= 0.2 }}>
                  <span class="dim">{hour(h.t)}</span>
                  <b class="mono">{deg(h.temp)}</b>
                  <span class="wx-mm">{h.mm >= 0.2 ? `${h.mm} mm` : ""}</span>
                </div>
              )}
            </For>
          </div>
          <div class="wx-sources">
            <span class="dim">
              {(c().fontes ?? []).filter((f) => f.ok).length} de {(c().fontes ?? []).length} fontes responderam · modelos {trust()} (diferença de {c().diferenca}°)
              {age() != null ? ` · atualizado há ${age()} min` : ""}
            </span>
            <div class="wx-chips">
              <For each={c().fontes ?? []}>
                {(f) => (
                  <span class="wx-chip" classList={{ off: !f.ok }}>
                    <Show when={f.cond}>
                      <CondIcon cond={f.cond ?? undefined} dia={c().dia} size={14} />
                    </Show>
                    {f.nome}
                    <b class="mono">
                      {!f.ok ? "sem resposta" : f.temp != null ? deg(f.temp) : f.min != null ? `${deg(f.min)} / ${deg(f.max)}` : "ok"}
                    </b>
                  </span>
                )}
              </For>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}
