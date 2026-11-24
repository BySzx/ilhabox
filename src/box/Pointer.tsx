import { createSignal, onMount, Show, type JSX } from "solid-js";
import { on } from "../lib/api";
import { go, page } from "./App";

type Ev =
  | { t: "on" | "move"; x: number; y: number }
  | { t: "off" }
  | { t: "down" | "up"; button: "left" | "right"; x: number; y: number }
  | { t: "wheel"; dy: number; x: number; y: number }
  | { t: "key"; key: string }
  | { t: "text"; text: string };

const ORDER = ["inicio", "horas", "musica", "whats", "atalhos"];
const editable = (el: Element | null): el is HTMLInputElement | HTMLTextAreaElement =>
  !!el &&
  (el.tagName === "TEXTAREA" || (el.tagName === "INPUT" && !["checkbox", "radio", "color", "button", "submit"].includes((el as HTMLInputElement).type))) &&
  !(el as HTMLInputElement).disabled;

function scrollable(el: Element | null): HTMLElement | null {
  for (let e = el as HTMLElement | null; e; e = e.parentElement) {
    if (e.classList.contains("track")) return null;
    const oy = getComputedStyle(e).overflowY;
    if ((oy === "auto" || oy === "scroll") && e.scrollHeight > e.clientHeight + 2) return e;
  }
  return null;
}

/**
 * The PC mouse on the phone. Events come from the PC (mouse.rs); this draws the pointer and turns
 * them into real clicks, focus, scrolling and typing on the page's own elements.
 */
export function Pointer(): JSX.Element {
  const [shown, setShown] = createSignal(false);
  const [pos, setPos] = createSignal({ x: 0, y: 0 });
  const [pressed, setPressed] = createSignal(false);
  let downAt: Element | null = null;
  let lastWheelPage = 0;

  const px = (x: number, y: number) => ({ x: x * innerWidth, y: y * innerHeight });
  const target = (x: number, y: number) => document.elementFromPoint(x, y);

  onMount(() =>
    on<Ev>("ponteiro", (e) => {
      if (e.t === "off") return void setShown(false);
      if (e.t === "on" || e.t === "move") {
        setShown(true);
        return void setPos(px(e.x, e.y));
      }
      if (e.t === "down" || e.t === "up") {
        const p = px(e.x, e.y);
        setPos(p);
        if (e.button !== "left") return;
        const el = target(p.x, p.y);
        if (e.t === "down") {
          setPressed(true);
          downAt = el;
          return;
        }
        setPressed(false);
        if (!el || (downAt && !downAt.contains(el) && !el.contains(downAt))) return;
        if (editable(el)) {
          el.focus();
          return;
        }
        (document.activeElement as HTMLElement | null)?.blur?.();
        el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, view: window }));
        return;
      }
      if (e.t === "wheel") {
        const p = px(e.x, e.y);
        const box = scrollable(target(p.x, p.y));
        if (box) return void box.scrollBy({ top: -e.dy * 0.9, behavior: "smooth" });
        // Nothing to scroll under the pointer: the wheel flips between pages.
        if (Date.now() - lastWheelPage < 450) return;
        lastWheelPage = Date.now();
        const next = Math.min(ORDER.length - 1, Math.max(0, page() + (e.dy < 0 ? 1 : -1)));
        return void go(ORDER[next]);
      }
      const focus = document.activeElement;
      if (e.t === "text") {
        if (editable(focus)) document.execCommand("insertText", false, e.text);
        return;
      }
      if (e.t !== "key") return;
      if (editable(focus)) {
        if (e.key === "Backspace") document.execCommand("delete");
        else if (e.key === "Delete") document.execCommand("forwardDelete");
        else if (e.key === "Escape") focus.blur();
        else if (e.key === "Tab") {
          const all = [...document.querySelectorAll<HTMLElement>("input:not([disabled]), textarea:not([disabled])")];
          all[(all.indexOf(focus) + 1) % all.length]?.focus();
        } else if (e.key === "Enter") {
          const ev = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true });
          focus.dispatchEvent(ev);
          if (!ev.defaultPrevented && focus.tagName === "INPUT") focus.form?.requestSubmit();
        }
        return;
      }
      if (e.key === "ArrowRight") go(ORDER[Math.min(ORDER.length - 1, page() + 1)]);
      else if (e.key === "ArrowLeft") go(ORDER[Math.max(0, page() - 1)]);
      else if (e.key === "Escape") document.querySelector<HTMLElement>(".sheet-bg")?.click();
    }),
  );

  return (
    <Show when={shown()}>
      <div class="pointer" classList={{ pressed: pressed() }} style={{ transform: `translate(${pos().x}px, ${pos().y}px)` }}>
        <svg width="22" height="22" viewBox="0 0 24 24">
          <path d="M4 2l16 9.5-7 1.6-3.6 6.9L4 2z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round" />
        </svg>
      </div>
    </Show>
  );
}
