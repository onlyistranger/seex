import { mountIcons } from "../icons";
import { translate } from "../i18n";

export type ToastKind = "info" | "success" | "warn" | "error";

export interface ToastAction {
  label: string;
  onClick: () => void | Promise<void>;
}

export interface ToastOptions {
  duration?: number;
  action?: ToastAction;
}

interface ToastEntry {
  key: string;
  element: HTMLElement;
  timer: number | null;
}

const MAX_TOASTS = 4;
const DEFAULT_DURATION = 5200;
const entries = new Map<string, ToastEntry>();
let region: HTMLElement | null = null;

function iconName(kind: ToastKind): string {
  switch (kind) {
    case "success":
      return "circle-check";
    case "warn":
      return "triangle-alert";
    case "error":
      return "circle-x";
    default:
      return "info";
  }
}

function ensureRegion(): HTMLElement {
  if (region && document.body.contains(region)) return region;

  const existing = document.getElementById("toast-region");
  if (existing) {
    region = existing;
    return existing;
  }

  const next = document.createElement("div");
  next.id = "toast-region";
  next.className = "toast-region";
  next.setAttribute("aria-live", "polite");
  next.setAttribute("aria-atomic", "false");
  document.body.appendChild(next);
  region = next;
  return next;
}

export function mountToast(): void {
  ensureRegion();
}

function removeEntry(entry: ToastEntry): void {
  if (entry.timer !== null) {
    window.clearTimeout(entry.timer);
    entry.timer = null;
  }
  entries.delete(entry.key);
  entry.element.remove();
}

function scheduleRemoval(entry: ToastEntry, duration: number): void {
  if (entry.timer !== null) window.clearTimeout(entry.timer);
  entry.timer = duration > 0 ? window.setTimeout(() => removeEntry(entry), duration) : null;
}

function renderAction(entry: ToastEntry, action: ToastAction | undefined): void {
  const actionHost = entry.element.querySelector("[data-toast-action]");
  if (!(actionHost instanceof HTMLElement)) return;
  actionHost.replaceChildren();
  if (!action) return;

  const button = document.createElement("button");
  button.className = "toast-action";
  button.type = "button";
  button.textContent = action.label;
  button.addEventListener("click", () => {
    removeEntry(entry);
    void action.onClick();
  });
  actionHost.appendChild(button);
}

export function showToast(kind: ToastKind, message: string, options: ToastOptions = {}): string {
  const normalizedMessage = message.trim();
  if (!normalizedMessage) return "";

  const key = `${kind}:${normalizedMessage}`;
  const existing = entries.get(key);
  if (existing) {
    renderAction(existing, options.action);
    scheduleRemoval(existing, options.duration ?? DEFAULT_DURATION);
    existing.element.classList.remove("toast-is-new");
    void existing.element.offsetWidth;
    existing.element.classList.add("toast-is-new");
    return key;
  }

  const host = ensureRegion();
  const entry: ToastEntry = {
    key,
    element: document.createElement("div"),
    timer: null,
  };
  entry.element.className = `toast toast-${kind} toast-is-new`;
  entry.element.setAttribute("role", kind === "error" ? "alert" : "status");

  const icon = document.createElement("i");
  icon.dataset.lucide = iconName(kind);
  icon.className = "toast-icon";
  entry.element.appendChild(icon);

  const text = document.createElement("span");
  text.className = "toast-message";
  text.textContent = normalizedMessage;
  entry.element.appendChild(text);

  const actionHost = document.createElement("span");
  actionHost.dataset.toastAction = "";
  entry.element.appendChild(actionHost);

  const close = document.createElement("button");
  close.className = "toast-close";
  close.type = "button";
  close.setAttribute("aria-label", translate("toast.dismiss"));
  close.textContent = "×";
  close.addEventListener("click", () => removeEntry(entry));
  entry.element.appendChild(close);

  entries.set(key, entry);
  host.appendChild(entry.element);
  renderAction(entry, options.action);
  mountIcons(entry.element);
  scheduleRemoval(entry, options.duration ?? DEFAULT_DURATION);

  while (entries.size > MAX_TOASTS) {
    const oldest = entries.values().next().value as ToastEntry | undefined;
    if (!oldest) break;
    removeEntry(oldest);
  }

  return key;
}

export const toast = {
  info(message: string, options?: ToastOptions): string {
    return showToast("info", message, options);
  },
  success(message: string, options?: ToastOptions): string {
    return showToast("success", message, options);
  },
  warn(message: string, options?: ToastOptions): string {
    return showToast("warn", message, options);
  },
  error(message: string, options?: ToastOptions): string {
    return showToast("error", message, options);
  },
};
