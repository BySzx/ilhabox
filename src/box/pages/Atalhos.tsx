import { ClipboardCopy, Globe, House, MessageCircle, Rocket, Smartphone, Type } from "lucide-solid";
import { For, type JSX } from "solid-js";
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

function run(a: Atalho): void {
  void cmd("atalho", { id: a.id })
    .then(() => note(a.tipo === "texto" && !a.colar ? "Copiado no PC" : DONE[a.tipo]))
    .catch(feedback);
}

export function Atalhos(): JSX.Element {
  const home = () => (phone ? phone.home() : void cmd("celular_home").catch(feedback));
  return (
    <div class="shortcuts">
      <div class="tiles">
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
        <For each={config.atalhos}>
          {(a) => {
            const Icon = ICON[a.tipo] ?? Type;
            return (
              <button class="tile" style={{ "--c": a.cor || "#60a5fa" }} onClick={() => run(a)}>
                <Icon size={24} />
                <b>{a.nome}</b>
                <span>{a.tipo === "texto" ? a.valor : a.valor.replace(/^https?:\/\//, "")}</span>
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
}
