import { createSignal, Show, type JSX } from "solid-js";

const [msg, setMsg] = createSignal<{ text: string; bad: boolean } | null>(null);
let t = 0;

/** Short confirmation at the bottom of the screen ("Copiado no PC"). */
export function note(text: string, bad = false): void {
  setMsg({ text, bad });
  clearTimeout(t);
  t = window.setTimeout(() => setMsg(null), bad ? 4200 : 2200);
}

export const feedback = (e: unknown): void => note(e instanceof Error ? e.message : String(e), true);

export function Notes(): JSX.Element {
  return (
    <Show when={msg()}>
      {(m) => <div class="note" classList={{ bad: m().bad }}>{m().text}</div>}
    </Show>
  );
}
