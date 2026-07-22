import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { mountIcons } from "./icons";
import { invokeState } from "./ipc";
import { translate } from "./i18n";
import { exportPage } from "./pages/export";
import { libraryPage } from "./pages/library";
import { inventoryPage } from "./pages/inventory";
import { monitorPage } from "./pages/monitor";
import {
  closeImportedPreview,
  closeImportedStandalonePreview,
  positionImportedPreview,
  resetImportedStandalonePreview,
  selectImportedStandalonePreviewModel,
  setNoModelsHandler,
  setPreviewContext,
} from "./shared/preview";
import { $, applyTooltips, syncInputValue } from "./utils";
import type { AppState, ExportFinishedPayload, ExportProgressPayload, PageName } from "./types";

let currentPage: PageName = "monitor";
let lastState: AppState | null = null;

function t(key: string): string {
  return translate(key);
}

function applyStaticTranslations() {
  document.documentElement.lang = "zh-CN";
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n")!;
    el.textContent = t(key);
  });

  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const key = el.getAttribute("data-i18n-placeholder")!;
    (el as HTMLInputElement).placeholder = t(key);
  });
  mountIcons();
  applyTooltips();
  libraryPage.render();
  rerenderState();
}

function switchPage(pageName: PageName) {
  if (pageName !== "imported") {
    closeImportedPreview();
  }
  if (pageName !== "imported") {
    closeImportedStandalonePreview();
  }
  currentPage = pageName;
  document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
  document.querySelectorAll(".nav-item").forEach((n) => n.classList.remove("active"));

  const page = document.getElementById(`page-${pageName}`);
  const nav = document.querySelector(`.nav-item[data-page="${pageName}"]`);
  if (page) page.classList.add("active");
  if (nav) nav.classList.add("active");

  if (pageName === "imported") {
    libraryPage.ensureLoaded();
  }
  if (pageName === "inventory") {
    inventoryPage.ensureLoaded();
  }
}

function rerenderState() {
  if (lastState) {
    renderState(lastState);
  }
}

function renderState(state: AppState) {
  syncInputValue("imported-parts-save-path-input", state.imported_parts_save_path);

  monitorPage.render(state);
  exportPage.render(state);

  libraryPage.render(state);
  applyTooltips();
}

async function refreshState() {
  const state = await invokeState();
  lastState = state;
  renderState(state);
}

async function selectDirectory(title: string): Promise<string | null> {
  const selected = await open({ directory: true, title });
  return typeof selected === "string" ? selected : null;
}

async function selectSaveFile(
  title: string,
  defaultPath: string | undefined,
): Promise<string | null> {
  const selected = await save({
    title,
    defaultPath: defaultPath && defaultPath.trim().length > 0 ? defaultPath : undefined,
    filters: [
      { name: "Text", extensions: ["txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

let pendingExportConfigWrite: Promise<void> = Promise.resolve();

function queueExportConfigWrite(operation: () => Promise<void>): Promise<void> {
  const run = pendingExportConfigWrite.then(operation, operation);
  pendingExportConfigWrite = run.catch(() => {});
  return run;
}

window.addEventListener("DOMContentLoaded", async () => {
  setPreviewContext({
    getDefaultModelFormat: () => lastState?.default_model_format ?? "wrl",
  });
  setNoModelsHandler(() => {
    libraryPage.showPreviewNoModels();
  });
  applyStaticTranslations();
  try {
    await refreshState();
  } catch {
    // Keep the static shell usable when the SPA is opened directly in a browser.
  }
  try {
    await listen("clipboard-changed", () => {
      void refreshState();
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }
  try {
    await listen<ExportProgressPayload>("export-progress", (event) => {
      exportPage.onEvent("export-progress", event.payload);
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }
  try {
    await listen<ExportFinishedPayload>("export-finished", async (event) => {
      exportPage.onEvent("export-finished", event.payload);
      await refreshState();
      if (event.payload.tool === "export" && event.payload.success) {
        libraryPage.invalidate();
        if (currentPage === "imported") {
          await libraryPage.load();
        }
      }
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }

  document.querySelectorAll("[data-page]").forEach((item) => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");
      if (page) switchPage(page as PageName);
    });
  });

  $("btn-collapse").addEventListener("click", () => {
    $("sidebar").classList.toggle("collapsed");
  });

  exportPage.mount({
    refresh: refreshState,
    selectDirectory,
    queueConfigWrite: queueExportConfigWrite,
    onExportPathChanged: async () => {
      libraryPage.invalidate(true);
      await refreshState();
      if (currentPage === "imported") {
        await libraryPage.load();
      }
    },
  });

  monitorPage.mount({
    refresh: refreshState,
    queueConfigWrite: queueExportConfigWrite,
    selectSaveFile,
  });

  libraryPage.mount({
    refresh: refreshState,
    queueConfigWrite: queueExportConfigWrite,
    selectSaveFile,
  });

  inventoryPage.mount();

  $("btn-close-imported-standalone-preview").addEventListener("click", () => {
    closeImportedStandalonePreview();
  });

  $("btn-reset-imported-standalone-preview").addEventListener("click", () => {
    resetImportedStandalonePreview();
  });

  $("imported-standalone-preview-model-select").addEventListener("change", async (event) => {
    const select = event.target as HTMLSelectElement;
    await selectImportedStandalonePreviewModel(select.value);
  });

  $("imported-standalone-preview-modal").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) {
      closeImportedStandalonePreview();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeImportedStandalonePreview();
    }
  });

  window.addEventListener("resize", positionImportedPreview);

  document.addEventListener("click", async (e) => {
    const target = e.target as HTMLElement;

    const urlEl = target.closest("[data-url]") as HTMLElement | null;
    if (urlEl) {
      const url = urlEl.getAttribute("data-url");
      if (url) {
        await openUrl(url);
        return;
      }
    }
  });
});
