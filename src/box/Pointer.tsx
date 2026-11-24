import { createSignal, onMount, Show, type JSX } from "solid-js";
import { onPointer } from "../lib/api";
import { go, page } from "./App";

type Ev =
  | { t: "on" | "move"; x: number; y: number; ts?: number }
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
  const [pressed, setPressed] = createSignal(false);
  let el!: HTMLDivElement;
  /*
   * The cable delivers moves in bursts, so drawing each one as it lands looks like skipped frames.
   * Every move carries the PC's timestamp; the pointer is replayed a fixed few ms behind, on the
   * PC's own timeline, interpolating between samples: even motion at the cost of a tiny delay.
   */
  // Playout delay follows how bursty delivery is right now (how late the latest samples arrive).
  let jitter = 30;
  const buf: { ts: number; x: number; y: number }[] = [];
  let offset = Number.POSITIVE_INFINITY; // local clock − PC clock, smallest seen (the fastest delivery)
  let offsetAt = 0;
  let frame = 0;
  const place = (x: number, y: number) => {
    if (el) el.style.transform = `translate3d(${x}px, ${y}px, 0)`;
  };
  const tick = () => {
    frame = requestAnimationFrame(tick);
    if (!buf.length) return;
    const t = performance.now() - offset - Math.min(120, Math.max(12, jitter * 1.15 + 6));
    while (buf.length > 2 && buf[1].ts <= t) buf.shift();
    const [a, b] = buf;
    if (!b || t <= a.ts) return place(a.x, a.y);
    if (t >= b.ts) return place(b.x, b.y);
    const k = (t - a.ts) / (b.ts - a.ts);
    place(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
  };
  const push = (p: { x: number; y: number }, ts?: number) => {
    const local = performance.now();
    if (ts == null) {
      buf.length = 0;
      buf.push({ ts: local - (Number.isFinite(offset) ? offset : 0), x: p.x, y: p.y });
      return;
    }
    // Re-learn the clock link every few seconds (drift, sleep, the 49-day tick wrap).
    if (local - ts < offset || local - offsetAt > 4000) {
      if (Math.abs(local - ts - offset) > 1000) buf.length = 0;
      offset = Math.min(local - ts, local - offsetAt > 4000 ? Infinity : offset);
      offsetAt = local;
    }
    jitter = Math.max(local - ts - offset, jitter * 0.997);
    if (buf.length && ts <= buf[buf.length - 1].ts) return;
    buf.push({ ts, x: p.x, y: p.y });
    if (buf.length > 120) buf.splice(0, buf.length - 120);
  };
  const start = () => {
    if (!frame) frame = requestAnimationFrame(tick);
  };
  const stop = () => {
    cancelAnimationFrame(frame);
    frame = 0;
    buf.length = 0;
  };
  const setPos = (p: { x: number; y: number }) => push(p);
  const draw = () => {
    const last = buf[buf.length - 1];
    if (last) place(last.x, last.y);
  };
  let downAt: Element | null = null;
  // Holding the button and moving drags: scrolls the list under the pointer (or flips pages).
  let drag: { x: number; y: number; box: HTMLElement | null; moved: boolean; lastY: number } | null = null;
  let lastWheelPage = 0;

  const px = (x: number, y: number) => ({ x: x * innerWidth, y: y * innerHeight });
  const hit = (x: number, y: number) => document.elementFromPoint(x, y);

  onMount(() =>
    onPointer((raw) => {
      const e = raw as Ev;
      if (e.t === "off") {
        stop();
        return void setShown(false);
      }
      if (e.t === "on" || e.t === "move") {
        const p = px(e.x, e.y);
        if (drag) {
          if (!drag.moved && Math.hypot(p.x - drag.x, p.y - drag.y) > 8) drag.moved = true;
          if (drag.moved && drag.box) drag.box.scrollTop -= p.y - drag.lastY;
          drag.lastY = p.y;
        }
        push(p, e.t === "move" ? e.ts : undefined);
        if (!shown()) {
          setShown(true);
          draw();
        }
        start();
        return;
      }
      if (e.t === "down" || e.t === "up") {
        const p = px(e.x, e.y);
        setPos(p);
        const at = hit(p.x, p.y);
        if (e.button === "right") {
          if (e.t === "up" && at) at.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, button: 2, view: window }));
          return;
        }
        if (e.t === "down") {
          setPressed(true);
          downAt = at;
          drag = { x: p.x, y: p.y, box: scrollable(at), moved: false, lastY: p.y };
          return;
        }
        setPressed(false);
        const d = drag;
        drag = null;
        if (d?.moved) {
          // A sideways drag over something that does not scroll flips the page.
          const dx = p.x - d.x;
          if (!d.box && Math.abs(dx) > 80 && Math.abs(dx) > Math.abs(p.y - d.y)) go(ORDER[Math.min(ORDER.length - 1, Math.max(0, page() + (dx < 0 ? 1 : -1)))]);
          return;
        }
        if (!at || (downAt && !downAt.contains(at) && !at.contains(downAt))) return;
        if (editable(at)) {
          at.focus();
          return;
        }
        (document.activeElement as HTMLElement | null)?.blur?.();
        at.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, view: window }));
        return;
      }
      if (e.t === "wheel") {
        const p = px(e.x, e.y);
        const box = scrollable(hit(p.x, p.y));
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
      <div class="pointer" ref={(r) => ((el = r), draw())} classList={{ pressed: pressed() }}>
        <svg width="22" height="22" viewBox="0 0 24 24">
          <path d="M4 2l16 9.5-7 1.6-3.6 6.9L4 2z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round" />
        </svg>
      </div>
    </Show>
  );
}
