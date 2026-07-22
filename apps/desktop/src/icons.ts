// Lucide icon wiring shared across page modules.
//
// Pages call `mountIcons(root?)` to render `data-lucide` attributes into real
// SVGs. Previously this lived inline in `main.ts` alongside a hand-maintained
// `iconSet` object; centralising it lets future page modules add icons without
// touching the root file.

import {
  Activity,
  ArrowDown,
  ArrowRight,
  ArrowUp,
  Box,
  ClipboardPaste,
  CircleCheck,
  CircleX,
  CircleHelp,
  createIcons,
  FolderCog,
  FolderPlus,
  FolderSearch,
  History,
  Info,
  Layers3,
  LibraryBig,
  Maximize2,
  Package,
  PackageOpen,
  PackageCheck,
  FileSpreadsheet,
  Pencil,
  Plus,
  PanelLeftClose,
  RefreshCw,
  RotateCcw,
  Search,
  Settings2,
  SlidersHorizontal,
  Trash2,
  TriangleAlert,
  X,
} from "lucide";

const iconSet = {
  Activity,
  ArrowDown,
  ArrowRight,
  ArrowUp,
  Box,
  ClipboardPaste,
  CircleCheck,
  CircleX,
  CircleHelp,
  FolderCog,
  FolderPlus,
  FolderSearch,
  History,
  Info,
  Layers3,
  LibraryBig,
  Maximize2,
  Package,
  PackageOpen,
  PackageCheck,
  FileSpreadsheet,
  Pencil,
  Plus,
  PanelLeftClose,
  RefreshCw,
  RotateCcw,
  Search,
  Settings2,
  SlidersHorizontal,
  Trash2,
  TriangleAlert,
  X,
};

/**
 * Render all `data-lucide` icons under `root` (defaults to the whole document).
 * Safe to call repeatedly after injecting new HTML; lucide replaces attributes
 * in place and idempotently.
 */
export function mountIcons(root?: ParentNode): void {
  createIcons({ icons: iconSet, ...(root ? { parent: root } : {}) });
}
