//! Embedded Web UI for SeEx Server.

pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SeEx: EDA 元件库抓取与 3D 去标工作站 (Web Edition)</title>
    <link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>⚡</text></svg>">
    <style>
        :root {
            --bg: #121518;
            --surface: #1a1e24;
            --surface-hover: #232830;
            --card-bg: #1e222a;
            --border: #2e3540;
            --border-hover: #3e4856;
            --primary: #4aafd6;
            --primary-hover: #389ec6;
            --primary-bg: rgba(74, 175, 214, 0.12);
            --text: #f0f4f8;
            --text-muted: #8a96a4;
            --success: #34d399;
            --success-bg: rgba(52, 211, 153, 0.12);
            --warning: #fbbf24;
            --warning-bg: rgba(251, 191, 36, 0.12);
            --danger: #f87171;
            --danger-bg: rgba(248, 113, 113, 0.12);
            --radius-sm: 6px;
            --radius-md: 10px;
            --radius-lg: 14px;
            --shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
        }

        [data-theme="light"] {
            --bg: #f4f6f9;
            --surface: #ffffff;
            --surface-hover: #f8fafc;
            --card-bg: #ffffff;
            --border: #e2e8f0;
            --border-hover: #cbd5e1;
            --primary: #2563eb;
            --primary-hover: #1d4ed8;
            --primary-bg: rgba(37, 99, 235, 0.08);
            --text: #0f172a;
            --text-muted: #64748b;
            --success: #10b981;
            --success-bg: rgba(16, 185, 129, 0.08);
            --warning: #d97706;
            --warning-bg: rgba(217, 119, 6, 0.08);
            --danger: #ef4444;
            --danger-bg: rgba(239, 68, 68, 0.08);
            --shadow: 0 4px 20px rgba(0, 0, 0, 0.06);
        }

        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
            background-color: var(--bg);
            color: var(--text);
            min-height: 100vh;
            line-height: 1.5;
            transition: background-color 0.2s, color 0.2s;
        }

        /* Layout */
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px 24px 60px;
        }

        /* Header */
        header {
            background-color: var(--surface);
            border-bottom: 1px solid var(--border);
            padding: 14px 24px;
            position: sticky;
            top: 0;
            z-index: 100;
        }
        .header-inner {
            max-width: 1200px;
            margin: 0 auto;
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 16px;
        }
        .logo-group {
            display: flex;
            align-items: center;
            gap: 10px;
        }
        .logo-icon {
            width: 32px;
            height: 32px;
            background: linear-gradient(135deg, var(--primary), #8b5cf6);
            border-radius: var(--radius-sm);
            display: flex;
            align-items: center;
            justify-content: center;
            color: white;
            font-size: 18px;
            font-weight: bold;
        }
        .logo-title {
            font-size: 18px;
            font-weight: 700;
            letter-spacing: -0.5px;
        }
        .logo-badge {
            font-size: 11px;
            background: var(--primary-bg);
            color: var(--primary);
            padding: 2px 8px;
            border-radius: 999px;
            font-weight: 600;
            border: 1px solid rgba(74, 175, 214, 0.25);
        }

        /* Nav Pills */
        .nav-tabs {
            display: flex;
            gap: 6px;
            background: var(--bg);
            padding: 4px;
            border-radius: var(--radius-md);
            border: 1px solid var(--border);
        }
        .nav-btn {
            background: transparent;
            border: none;
            color: var(--text-muted);
            padding: 8px 16px;
            border-radius: var(--radius-sm);
            font-size: 14px;
            font-weight: 500;
            cursor: pointer;
            transition: all 0.15s ease;
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .nav-btn:hover {
            color: var(--text);
            background: var(--surface-hover);
        }
        .nav-btn.active {
            background: var(--surface);
            color: var(--primary);
            font-weight: 600;
            box-shadow: 0 1px 4px rgba(0,0,0,0.15);
        }

        .header-actions {
            display: flex;
            align-items: center;
            gap: 12px;
        }
        .theme-toggle-btn {
            background: var(--surface-hover);
            border: 1px solid var(--border);
            color: var(--text);
            width: 36px;
            height: 36px;
            border-radius: var(--radius-sm);
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 16px;
            transition: all 0.2s;
        }
        .theme-toggle-btn:hover {
            border-color: var(--primary);
        }

        /* Tab Content Panes */
        .tab-pane {
            display: none;
            animation: fadeIn 0.2s ease-in-out;
        }
        .tab-pane.active {
            display: block;
        }
        @keyframes fadeIn {
            from { opacity: 0; transform: translateY(4px); }
            to { opacity: 1; transform: translateY(0); }
        }

        /* Cards */
        .card {
            background: var(--card-bg);
            border: 1px solid var(--border);
            border-radius: var(--radius-lg);
            padding: 24px;
            margin-bottom: 20px;
            box-shadow: var(--shadow);
        }
        .card-title {
            font-size: 17px;
            font-weight: 600;
            margin-bottom: 16px;
            display: flex;
            align-items: center;
            gap: 8px;
        }

        /* Search Section */
        .search-box-row {
            display: flex;
            gap: 12px;
            margin-bottom: 16px;
        }
        .search-input {
            flex: 1;
            background: var(--surface);
            border: 1px solid var(--border);
            color: var(--text);
            font-size: 15px;
            padding: 12px 18px;
            border-radius: var(--radius-md);
            outline: none;
            transition: border-color 0.2s, box-shadow 0.2s;
        }
        .search-input:focus {
            border-color: var(--primary);
            box-shadow: 0 0 0 3px var(--primary-bg);
        }
        .btn-primary {
            background: var(--primary);
            color: white;
            border: none;
            padding: 0 24px;
            border-radius: var(--radius-md);
            font-size: 15px;
            font-weight: 600;
            cursor: pointer;
            transition: background 0.2s, transform 0.1s;
            display: flex;
            align-items: center;
            gap: 8px;
            white-space: nowrap;
        }
        .btn-primary:hover {
            background: var(--primary-hover);
        }
        .btn-primary:active {
            transform: scale(0.98);
        }
        .btn-secondary {
            background: var(--surface);
            color: var(--text);
            border: 1px solid var(--border);
            padding: 8px 16px;
            border-radius: var(--radius-sm);
            font-size: 13px;
            font-weight: 500;
            cursor: pointer;
            transition: all 0.2s;
        }
        .btn-secondary:hover {
            border-color: var(--primary);
            color: var(--primary);
        }

        .quick-chips {
            display: flex;
            align-items: center;
            gap: 8px;
            flex-wrap: wrap;
            font-size: 13px;
            color: var(--text-muted);
        }
        .chip {
            background: var(--surface);
            border: 1px solid var(--border);
            color: var(--text);
            padding: 4px 10px;
            border-radius: 999px;
            cursor: pointer;
            font-size: 12px;
            transition: all 0.15s;
        }
        .chip:hover {
            border-color: var(--primary);
            color: var(--primary);
            background: var(--primary-bg);
        }

        /* Options Panel */
        .options-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
            gap: 16px;
            margin-top: 14px;
            padding-top: 16px;
            border-top: 1px solid var(--border);
        }
        .option-item {
            display: flex;
            align-items: flex-start;
            gap: 10px;
            cursor: pointer;
            user-select: none;
        }
        .option-item input[type="checkbox"] {
            margin-top: 3px;
            width: 17px;
            height: 17px;
            cursor: pointer;
            accent-color: var(--primary);
        }
        .option-label {
            font-size: 14px;
            font-weight: 600;
            color: var(--text);
        }
        .option-desc {
            font-size: 12px;
            color: var(--text-muted);
            margin-top: 2px;
        }

        /* Select controls */
        .select-group {
            display: flex;
            gap: 12px;
            align-items: center;
        }
        .select-label {
            font-size: 14px;
            font-weight: 600;
            color: var(--text-muted);
        }
        select.custom-select {
            background: var(--surface);
            border: 1px solid var(--border);
            color: var(--text);
            padding: 8px 14px;
            border-radius: var(--radius-sm);
            font-size: 14px;
            outline: none;
            cursor: pointer;
        }
        select.custom-select:focus {
            border-color: var(--primary);
        }

        /* Results Grid */
        .results-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
            gap: 16px;
            margin-top: 18px;
        }
        .component-card {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: var(--radius-md);
            padding: 18px;
            display: flex;
            flex-direction: column;
            justify-content: space-between;
            gap: 14px;
            transition: border-color 0.2s, box-shadow 0.2s, transform 0.15s;
        }
        .component-card:hover {
            border-color: var(--border-hover);
            transform: translateY(-2px);
            box-shadow: var(--shadow);
        }
        .comp-header {
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            gap: 10px;
        }
        .comp-id {
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
            font-size: 14px;
            font-weight: 700;
            color: var(--primary);
            background: var(--primary-bg);
            padding: 2px 8px;
            border-radius: 4px;
            border: 1px solid rgba(74, 175, 214, 0.25);
        }
        .comp-title {
            font-size: 16px;
            font-weight: 600;
            margin-top: 4px;
            color: var(--text);
        }
        .comp-meta {
            font-size: 13px;
            color: var(--text-muted);
            margin-top: 4px;
        }
        .comp-badges {
            display: flex;
            gap: 6px;
            flex-wrap: wrap;
            margin-top: 6px;
        }
        .badge {
            font-size: 11px;
            padding: 2px 7px;
            border-radius: 4px;
            font-weight: 500;
        }
        .badge-success { background: var(--success-bg); color: var(--success); }
        .badge-warning { background: var(--warning-bg); color: var(--warning); }
        .badge-muted { background: var(--surface-hover); color: var(--text-muted); }
        .badge-outline { border: 1px solid var(--border); color: var(--text-muted); }

        .comp-actions {
            display: flex;
            gap: 8px;
            border-top: 1px solid var(--border);
            padding-top: 12px;
            align-items: center;
            justify-content: flex-end;
        }

        /* 3D Tools Section */
        .dropzone {
            border: 2px dashed var(--border);
            border-radius: var(--radius-lg);
            padding: 40px 24px;
            text-align: center;
            cursor: pointer;
            background: var(--surface);
            transition: all 0.2s;
            position: relative;
        }
        .dropzone:hover, .dropzone.dragover {
            border-color: var(--primary);
            background: var(--primary-bg);
        }
        .dropzone-icon {
            font-size: 40px;
            margin-bottom: 12px;
        }
        .dropzone-title {
            font-size: 16px;
            font-weight: 600;
            color: var(--text);
        }
        .dropzone-sub {
            font-size: 13px;
            color: var(--text-muted);
            margin-top: 4px;
        }
        .file-hidden-input {
            position: absolute;
            top: 0; left: 0;
            width: 100%; height: 100%;
            opacity: 0;
            cursor: pointer;
        }

        .alert-box {
            padding: 14px 18px;
            border-radius: var(--radius-md);
            font-size: 13px;
            line-height: 1.5;
            margin-top: 16px;
            display: flex;
            gap: 10px;
            align-items: flex-start;
        }
        .alert-warning {
            background: var(--warning-bg);
            color: var(--warning);
            border: 1px solid rgba(251, 191, 36, 0.3);
        }
        .alert-info {
            background: var(--primary-bg);
            color: var(--primary);
            border: 1px solid rgba(74, 175, 214, 0.3);
        }

        /* Textarea for batch */
        .batch-textarea {
            width: 100%;
            height: 180px;
            background: var(--surface);
            border: 1px solid var(--border);
            color: var(--text);
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
            font-size: 14px;
            padding: 14px;
            border-radius: var(--radius-md);
            outline: none;
            resize: vertical;
            line-height: 1.6;
        }
        .batch-textarea:focus {
            border-color: var(--primary);
            box-shadow: 0 0 0 3px var(--primary-bg);
        }

        /* Status & Logs */
        .log-box {
            background: #0d1117;
            border: 1px solid var(--border);
            border-radius: var(--radius-md);
            padding: 14px;
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
            font-size: 12px;
            color: #38bdf8;
            max-height: 240px;
            overflow-y: auto;
            white-space: pre-wrap;
            margin-top: 16px;
        }

        /* Progress Bar */
        .progress-bar-container {
            width: 100%;
            height: 8px;
            background: var(--surface);
            border-radius: 999px;
            overflow: hidden;
            margin: 14px 0 6px;
            display: none;
        }
        .progress-bar-fill {
            height: 100%;
            width: 0%;
            background: linear-gradient(90deg, var(--primary), #8b5cf6);
            transition: width 0.3s;
        }

        /* Modal / Toast */
        .toast {
            position: fixed;
            bottom: 24px;
            right: 24px;
            background: var(--surface);
            color: var(--text);
            border: 1px solid var(--border);
            padding: 14px 20px;
            border-radius: var(--radius-md);
            box-shadow: 0 10px 30px rgba(0,0,0,0.4);
            display: flex;
            align-items: center;
            gap: 10px;
            font-size: 14px;
            z-index: 1000;
            transform: translateY(100px);
            opacity: 0;
            transition: all 0.25s ease-out;
        }
        .toast.show {
            transform: translateY(0);
            opacity: 1;
        }

        /* Modal Dialog */
        .modal-overlay {
            position: fixed;
            top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(0,0,0,0.6);
            display: none;
            align-items: center;
            justify-content: center;
            z-index: 200;
            padding: 20px;
        }
        .modal-overlay.active {
            display: flex;
        }
        .modal-content {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: var(--radius-lg);
            max-width: 640px;
            width: 100%;
            max-height: 85vh;
            overflow-y: auto;
            padding: 24px;
            box-shadow: var(--shadow);
        }

        /* Responsive */
        @media (max-width: 768px) {
            .header-inner {
                flex-direction: column;
                align-items: stretch;
            }
            .nav-tabs {
                overflow-x: auto;
            }
            .results-grid {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>

    <!-- Header Navigation -->
    <header>
        <div class="header-inner">
            <div class="logo-group">
                <div class="logo-icon">⚡</div>
                <div>
                    <span class="logo-title">SeEx</span>
                    <span class="logo-badge">Web Edition</span>
                </div>
            </div>

            <nav class="nav-tabs">
                <button class="nav-btn active" onclick="switchTab('search')">
                    🔍 元件检索与导出
                </button>
                <button class="nav-btn" onclick="switchTab('batch')">
                    📦 批量导出
                </button>
                <button class="nav-btn" onclick="switchTab('step')">
                    🛠 3D 去标工具
                </button>
                <button class="nav-btn" onclick="switchTab('deploy')">
                    ⚙ 部署与说明
                </button>
            </nav>

            <div class="header-actions">
                <button class="theme-toggle-btn" id="themeToggle" title="切换暗黑/明亮主题">🌓</button>
            </div>
        </div>
    </header>

    <div class="container">

        <!-- ================= TAB 1: 检索与单颗导出 ================= -->
        <div id="tab-search" class="tab-pane active">
            <div class="card">
                <div class="card-title">🔍 嘉立创 / 立创商城 元器件库检索</div>
                
                <div class="search-box-row">
                    <input type="text" id="searchInput" class="search-input" 
                           placeholder="输入立创物料编号 (如 C6186, C2040) 或元件型号 (如 AMS1117-3.3, SOT-223, CH340)..." 
                           onkeydown="if(event.key==='Enter') doSearch()">
                    <button class="btn-primary" onclick="doSearch()">
                        <span id="searchSpinner" style="display:none">⏳</span> 检索元件
                    </button>
                </div>

                <div class="quick-chips">
                    <span>快捷示例：</span>
                    <span class="chip" onclick="quickSearch('C6186')">C6186 (SOT-223)</span>
                    <span class="chip" onclick="quickSearch('C2040')">C2040 (SOP-8)</span>
                    <span class="chip" onclick="quickSearch('C77978')">C77978 (1N4148W)</span>
                    <span class="chip" onclick="quickSearch('C3013955')">C3013955 (0805电阻)</span>
                    <span class="chip" onclick="quickSearch('C1653')">C1653 (0603电容)</span>
                    <span class="chip" onclick="quickSearch('AMS1117')">AMS1117</span>
                    <span class="chip" onclick="quickSearch('TYPE-C')">TYPE-C</span>
                </div>

                <!-- Export Options Bar -->
                <div class="options-grid">
                    <div>
                        <div class="select-group">
                            <span class="select-label">目标 EDA:</span>
                            <select id="exportTarget" class="custom-select">
                                <option value="kicad" selected>KiCad (v6/v7/v8/v9)</option>
                                <option value="altium">Altium Designer</option>
                            </select>
                        </div>
                        <div class="select-group" style="margin-top: 10px;">
                            <span class="select-label">导出模式:</span>
                            <select id="exportMode" class="custom-select">
                                <option value="full" selected>完整库 (原理图 + 封装 + 3D模型)</option>
                                <option value="symbol">仅原理图符号 (.kicad_sym / .SchLib)</option>
                                <option value="footprint">仅 PCB 封装 (.kicad_mod / .PcbLib)</option>
                                <option value="3d">仅 3D STEP 模型 (.step)</option>
                            </select>
                        </div>
                    </div>

                    <div>
                        <label class="option-item">
                            <input type="checkbox" id="optPdf" checked>
                            <div>
                                <div class="option-label">下载数据手册 PDF</div>
                                <div class="option-desc">自动探测并下载官方 PDF 至 .doc 目录</div>
                            </div>
                        </label>
                        <label class="option-item" style="margin-top: 8px;">
                            <input type="checkbox" id="optSuffix" checked>
                            <div>
                                <div class="option-label">添加物料号后缀 (_C...)</div>
                                <div class="option-desc">关闭则仅生成干净无后缀复用标准封装</div>
                            </div>
                        </label>
                    </div>

                    <div>
                        <label class="option-item">
                            <input type="checkbox" id="optDot">
                            <div>
                                <div class="option-label">替换小数点 . 为下划线 _</div>
                                <div class="option-desc">尺寸 L1.6-W0.8 转为 L1_6-W0_8</div>
                            </div>
                        </label>
                        <label class="option-item" style="margin-top: 8px;">
                            <input type="checkbox" id="optPkg">
                            <div>
                                <div class="option-label">封装保留 (Preserve Package)</div>
                                <div class="option-desc">额外以商品规格基底导出标准封装模型</div>
                            </div>
                        </label>
                    </div>

                    <div>
                        <label class="option-item">
                            <input type="checkbox" id="optRemoveLogo" checked onchange="toggleLogoOptions()">
                            <div>
                                <div class="option-label">去除 3D 标志 (EasyEDA/LCEDA)</div>
                                <div class="option-desc">自动重构 3D 模型去除厂商水印</div>
                            </div>
                        </label>
                        <label class="option-item" style="margin-top: 8px;">
                            <input type="checkbox" id="optCleanMarks" onchange="toggleAllMarksOption()">
                            <div>
                                <div class="option-label">去除所有标记 (增强)</div>
                                <div class="option-desc">清除型号丝印并保留 Pin 1 极性定位点</div>
                            </div>
                        </label>
                    </div>
                </div>
            </div>

            <!-- Search Results Container -->
            <div id="resultsArea" style="display:none">
                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
                    <div style="font-size: 15px; font-weight: 600; color: var(--text-muted);" id="resultStats">检索结果</div>
                </div>
                <div class="results-grid" id="resultsGrid"></div>
            </div>
        </div>

        <!-- ================= TAB 2: 批量导出 ================= -->
        <div id="tab-batch" class="tab-pane">
            <div class="card">
                <div class="card-title">📦 元器件批量导出与打包下载</div>
                <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 12px;">
                    在下方输入框中粘贴多个立创物料编号（例如 <code>C6186, C2040, C77978, C3013955</code>）。系统支持直接粘贴 BOM 表、表格文本或逗号/换行分隔的内容，自动提取所有 <code>C[0-9]+</code> 编号。
                </p>

                <textarea id="batchText" class="batch-textarea" placeholder="例如：&#10;C6186&#10;C2040&#10;C77978&#10;C3013955&#10;或粘贴整段 BOM 表格文本..."></textarea>

                <div style="display:flex; justify-content:space-between; align-items:center; margin-top: 14px; flex-wrap: wrap; gap: 12px;">
                    <div style="display:flex; align-items:center; gap: 10px;">
                        <span id="batchCountBadge" class="badge badge-outline" style="font-size:13px; padding:4px 10px;">已识别: 0 个元件</span>
                        <button class="btn-secondary" onclick="parseBatchInput()">重新解析</button>
                    </div>

                    <button class="btn-primary" onclick="startBatchExport()">
                        <span>📥 批量导出并下载 ZIP 包</span>
                    </button>
                </div>

                <div class="progress-bar-container" id="batchProgressContainer">
                    <div class="progress-bar-fill" id="batchProgressBar"></div>
                </div>

                <div class="log-box" id="batchLogBox" style="display:none"></div>
            </div>
        </div>

        <!-- ================= TAB 3: 3D 去标工具 ================= -->
        <div id="tab-step" class="tab-pane">
            <div class="card">
                <div class="card-title">🛠 STEP 3D 模型在线去标工具</div>
                <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 16px;">
                    通过浏览器直接上传本地 <code>.step</code> 3D 模型文件，服务器将利用高精度几何流形修补算法切除 EasyEDA 厂商水印或芯片封装文字丝印，清洗完成后直接下载回传。
                </p>

                <div class="dropzone" id="stepDropzone">
                    <div class="dropzone-icon">📐</div>
                    <div class="dropzone-title" id="dropzoneTitle">点击上传或将 .step 模型拖拽至此处</div>
                    <div class="dropzone-sub" id="dropzoneSub">支持任意大小的标准 STEP (ISO-10303-21) 3D 模型文件</div>
                    <input type="file" id="stepFileInput" class="file-hidden-input" accept=".step,.stp" onchange="onStepFileSelected(event)">
                </div>

                <!-- Cleaning Mode Selector -->
                <div style="margin-top: 20px;">
                    <div style="font-size: 14px; font-weight: 600; margin-bottom: 10px;">选择去标范围：</div>
                    <div style="display:flex; gap: 20px; flex-wrap: wrap;">
                        <label class="option-item" onclick="onStepModeChange('logo_only')">
                            <input type="radio" name="stepMode" id="stepModeLogoOnly" value="logo_only" checked>
                            <div>
                                <div class="option-label">仅 EasyEDA / LCEDA 官方 Logo 水印</div>
                                <div class="option-desc">安全无损，完美支持单个器件与复杂板级复合装配体</div>
                            </div>
                        </label>
                        <label class="option-item" onclick="onStepModeChange('all_markings')">
                            <input type="radio" name="stepMode" id="stepModeAllMarks" value="all_markings">
                            <div>
                                <div class="option-label">所有标记与文字丝印 (增强模式)</div>
                                <div class="option-desc">彻底清除芯片文字与封装代号，保护并保留 Pin 1 极性圆点</div>
                            </div>
                        </label>
                    </div>

                    <!-- Warning for All Markings -->
                    <div id="stepAllMarksNotice" class="alert-box alert-warning" style="display:none">
                        <span style="font-size: 18px;">⚠</span>
                        <div>
                            <strong>复合装配体风险提示：</strong> “所有标记与丝印”增强模式会对表面微小文字凹凸特征进行深度剥离与流形重构。<strong>此选项对多个 STEP 组成的复合体（板级装配体）无效/易引起拓扑冲突</strong>，建议仅用于处理单个元器件 STEP 模型以保证几何准确性与 Pin 1 极性保护。
                        </div>
                    </div>

                    <div id="stepLogoOnlyNotice" class="alert-box alert-info">
                        <span style="font-size: 18px;">💡</span>
                        <div>
                            <strong>全板安全模式：</strong> 仅检测并切除 LCEDA / EasyEDA 官方 3D 厂商徽标与水印行，对元器件丝印文字、Pin 1 极性定位点及 PCB 几何完全零触碰。
                        </div>
                    </div>
                </div>

                <div style="margin-top: 24px; display:flex; justify-content:flex-end;">
                    <button class="btn-primary" id="btnCleanStep" onclick="submitCleanStep()" disabled>
                        <span>🚀 开始清洗并下载 STEP 模型</span>
                    </button>
                </div>

                <div class="log-box" id="stepLogBox" style="display:none"></div>
            </div>
        </div>

        <!-- ================= TAB 4: 部署说明 ================= -->
        <div id="tab-deploy" class="tab-pane">
            <div class="card">
                <div class="card-title">⚙ 服务器部署与自动化 API 说明</div>
                
                <h4 style="margin: 16px 0 8px; font-size: 15px;">1. 命令行启动服务</h4>
                <div class="log-box" style="margin-top:0"># 默认监听 0.0.0.0:8080 并支持浏览器访问
seex server --port 8080 --host 0.0.0.0

# 启动并自动在本地唤起默认浏览器
seex server --open

# 或通过简短参数
seex --server</div>

                <h4 style="margin: 20px 0 8px; font-size: 15px;">2. Docker 容器化一键部署</h4>
                <div class="log-box" style="margin-top:0"># 直接运行 Docker 容器
docker run -d \
  --name seex-server \
  --restart always \
  -p 8080:8080 \
  -v ./export:/app/export \
  ghcr.io/murmur/seex:latest

# 或使用 docker-compose.yml:
# docker compose up -d</div>

                <h4 style="margin: 20px 0 8px; font-size: 15px;">3. RESTful 自动化调用 API</h4>
                <div class="log-box" style="margin-top:0"># 检索元器件
curl "http://localhost:8080/api/search?q=C6186"

# 导出 KiCad 库并下载 ZIP 包
curl -X POST "http://localhost:8080/api/export" \
  -H "Content-Type: application/json" \
  -d '{"ids":["C6186","C2040"],"target":"kicad","mode":"full","download_pdf":true}' \
  --output components_kicad.zip

# 上传并清洗 3D STEP 模型
curl -X POST "http://localhost:8080/api/clean-step?mode=logo_only&filename=model.step" \
  --data-binary @model.step \
  --output cleaned_model.step</div>
            </div>
        </div>

    </div>

    <!-- Toast Notification -->
    <div id="toast" class="toast">
        <span id="toastIcon">ℹ️</span>
        <span id="toastMsg">通知消息</span>
    </div>

    <!-- Details Modal -->
    <div id="detailModal" class="modal-overlay" onclick="if(event.target===this) closeDetailModal()">
        <div class="modal-content">
            <div style="display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:16px;">
                <h3 id="modalTitle" style="font-size:18px;">元器件详情</h3>
                <button class="btn-secondary" onclick="closeDetailModal()">✕</button>
            </div>
            <div id="modalBody" style="font-size:14px; line-height:1.6; color:var(--text);">
                正在加载详情...
            </div>
        </div>
    </div>

    <script>
        // ================= Theme & Init =================
        function initTheme() {
            const saved = localStorage.getItem('seex-theme') || 'dark';
            document.documentElement.setAttribute('data-theme', saved);
        }
        document.getElementById('themeToggle').addEventListener('click', () => {
            const current = document.documentElement.getAttribute('data-theme');
            const next = current === 'light' ? 'dark' : 'light';
            document.documentElement.setAttribute('data-theme', next);
            localStorage.setItem('seex-theme', next);
        });
        initTheme();

        // ================= Tab Navigation =================
        function switchTab(tabId) {
            document.querySelectorAll('.nav-btn').forEach(btn => btn.classList.remove('active'));
            document.querySelectorAll('.tab-pane').forEach(pane => pane.classList.remove('active'));
            
            const btn = Array.from(document.querySelectorAll('.nav-btn')).find(b => b.getAttribute('onclick').includes(tabId));
            if (btn) btn.classList.add('active');
            
            const targetPane = document.getElementById(`tab-${tabId}`);
            if (targetPane) targetPane.classList.add('active');
        }

        function showToast(msg, type = 'info') {
            const toast = document.getElementById('toast');
            const toastMsg = document.getElementById('toastMsg');
            const toastIcon = document.getElementById('toastIcon');
            toastMsg.innerText = msg;
            toastIcon.innerText = type === 'success' ? '✅' : type === 'error' ? '❌' : 'ℹ️';
            toast.className = `toast show`;
            setTimeout(() => { toast.classList.remove('show'); }, 3500);
        }

        // ================= Options Logic =================
        function toggleLogoOptions() {
            const removeLogo = document.getElementById('optRemoveLogo').checked;
            const cleanMarks = document.getElementById('optCleanMarks');
            if (!removeLogo) {
                cleanMarks.checked = false;
            }
        }
        function toggleAllMarksOption() {
            const cleanMarks = document.getElementById('optCleanMarks').checked;
            const removeLogo = document.getElementById('optRemoveLogo');
            if (cleanMarks) {
                removeLogo.checked = true;
            }
        }

        // ================= Search Functionality =================
        async function doSearch() {
            const input = document.getElementById('searchInput');
            const keyword = input.value.trim();
            if (!keyword) {
                showToast('请输入物料号或搜索关键词', 'error');
                return;
            }

            const spinner = document.getElementById('searchSpinner');
            spinner.style.display = 'inline-block';
            const resultsArea = document.getElementById('resultsArea');
            const resultsGrid = document.getElementById('resultsGrid');
            const stats = document.getElementById('resultStats');

            resultsGrid.innerHTML = '<div style="color:var(--text-muted); padding:20px;">正在检索云端元器件数据库...</div>';
            resultsArea.style.display = 'block';

            try {
                const res = await fetch(`/api/search?q=${encodeURIComponent(keyword)}`);
                const data = await res.json();
                spinner.style.display = 'none';

                if (!res.ok) {
                    resultsGrid.innerHTML = `<div style="color:var(--danger); padding:20px;">检索错误: ${data.error || '未知错误'}</div>`;
                    return;
                }

                if (!data || data.length === 0) {
                    resultsGrid.innerHTML = '<div style="color:var(--text-muted); padding:20px;">未检索到相关元器件，请确认立创编号或关键词。</div>';
                    stats.innerText = `未找到关于 "${keyword}" 的结果`;
                    return;
                }

                stats.innerText = `找到 ${data.length} 条关于 "${keyword}" 的元器件结果`;
                resultsGrid.innerHTML = data.map(item => renderComponentCard(item)).join('');
            } catch (err) {
                spinner.style.display = 'none';
                resultsGrid.innerHTML = `<div style="color:var(--danger); padding:20px;">请求失败: ${err.message}</div>`;
            }
        }

        function quickSearch(code) {
            document.getElementById('searchInput').value = code;
            doSearch();
        }

        function renderComponentCard(item) {
            const lcscId = item.lcsc_id || item.title || '未知';
            const has3d = !!item.has_3d;
            const pkg = item.package || '标准';
            const mfr = item.manufacturer || '通用';
            const desc = item.description || item.display_title || '';

            return `
            <div class="component-card">
                <div>
                    <div class="comp-header">
                        <span class="comp-id">${escapeHtml(lcscId)}</span>
                        <div class="comp-badges">
                            ${has3d ? '<span class="badge badge-success">3D模型可用</span>' : '<span class="badge badge-muted">无3D</span>'}
                            <span class="badge badge-outline">${escapeHtml(pkg)}</span>
                        </div>
                    </div>
                    <div class="comp-title">${escapeHtml(item.title || item.display_title || lcscId)}</div>
                    <div class="comp-meta">厂商: ${escapeHtml(mfr)}</div>
                    <div class="comp-desc" style="font-size:12px; color:var(--text-muted); margin-top:6px; display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden;">
                        ${escapeHtml(desc)}
                    </div>
                </div>

                <div class="comp-actions">
                    <button class="btn-secondary" onclick="viewDetails('${escapeHtml(lcscId)}')">详情</button>
                    <button class="btn-primary" style="padding:6px 14px; font-size:13px;" onclick="exportSingle('${escapeHtml(lcscId)}')">
                        📥 导出 ZIP
                    </button>
                </div>
            </div>`;
        }

        function escapeHtml(text) {
            if (!text) return '';
            return String(text).replace(/[&<>"']/g, function(m) {
                return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#039;' }[m];
            });
        }

        // ================= Component Details Modal =================
        async function viewDetails(lcscId) {
            const modal = document.getElementById('detailModal');
            const modalTitle = document.getElementById('modalTitle');
            const modalBody = document.getElementById('modalBody');

            modalTitle.innerText = `${lcscId} 元器件完整信息`;
            modalBody.innerHTML = '正在拉取元器件完整详情...';
            modal.classList.add('active');

            try {
                const res = await fetch(`/api/detail?id=${encodeURIComponent(lcscId)}`);
                const data = await res.json();
                if (!res.ok) {
                    modalBody.innerHTML = `<div style="color:var(--danger)">获取详情失败: ${data.error || '未知错误'}</div>`;
                    return;
                }

                modalBody.innerHTML = `
                    <div style="display:grid; grid-template-columns:100px 1fr; gap:8px 12px; margin-bottom:14px;">
                        <strong>立创编号:</strong> <span>${escapeHtml(data.lcsc_id)}</span>
                        <strong>器件型号:</strong> <span>${escapeHtml(data.title)}</span>
                        <strong>生产厂商:</strong> <span>${escapeHtml(data.manufacturer || '未知')}</span>
                        <strong>封装类型:</strong> <span>${escapeHtml(data.package || data.supplier_footprint || '标准')}</span>
                        <strong>商品描述:</strong> <span>${escapeHtml(data.description || '无描述')}</span>
                        <strong>3D 模型:</strong> <span>${data.model_3d ? `✓ 已提供 (${escapeHtml(data.model_3d.title || '3D')})` : '✕ 无 3D 模型'}</span>
                        <strong>数据手册:</strong> <span>${data.datasheet ? `<a href="${data.datasheet}" target="_blank" style="color:var(--primary); text-decoration:none;">📄 在线查阅数据手册</a>` : '无直链'}</span>
                    </div>
                    <div style="display:flex; justify-content:flex-end; gap:8px; margin-top:16px;">
                        <button class="btn-primary" onclick="exportSingle('${escapeHtml(data.lcsc_id)}')">立即导出此元器件</button>
                    </div>
                `;
            } catch (err) {
                modalBody.innerHTML = `<div style="color:var(--danger)">请求错误: ${err.message}</div>`;
            }
        }
        function closeDetailModal() {
            document.getElementById('detailModal').classList.remove('active');
        }

        // ================= Export Logic =================
        function collectExportOptions() {
            return {
                target: document.getElementById('exportTarget').value,
                mode: document.getElementById('exportMode').value,
                library_name: "SeExExport",
                download_pdf: document.getElementById('optPdf').checked,
                add_suffix: document.getElementById('optSuffix').checked,
                replace_dot: document.getElementById('optDot').checked,
                preserve_package: document.getElementById('optPkg').checked,
                remove_model_logo: document.getElementById('optRemoveLogo').checked,
                clean_all_markings: document.getElementById('optCleanMarks').checked,
                lcsc_english: false
            };
        }

        async function exportSingle(lcscId) {
            const opts = collectExportOptions();
            opts.ids = [lcscId];
            showToast(`正在导出 ${lcscId} 并打包为 ZIP...`, 'info');

            try {
                const res = await fetch('/api/export', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(opts)
                });

                if (!res.ok) {
                    const err = await res.json().catch(() => ({ error: '导出失败' }));
                    showToast(`导出失败: ${err.error || res.statusText}`, 'error');
                    return;
                }

                const blob = await res.blob();
                const disposition = res.headers.get('Content-Disposition') || '';
                let filename = `${lcscId}_${opts.target}.zip`;
                const match = disposition.match(/filename="?([^"]+)"?/);
                if (match && match[1]) filename = match[1];

                downloadBlob(blob, filename);
                showToast(`✓ 成功导出并下载 ${filename}`, 'success');
            } catch (err) {
                showToast(`网络请求错误: ${err.message}`, 'error');
            }
        }

        function downloadBlob(blob, filename) {
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.style.display = 'none';
            a.href = url;
            a.download = filename;
            document.body.appendChild(a);
            a.click();
            window.URL.revokeObjectURL(url);
            document.body.removeChild(a);
        }

        // ================= Batch Export Logic =================
        function extractIdsFromText(text) {
            const regex = /C\d+/gi;
            const matches = text.match(regex) || [];
            return Array.from(new Set(matches.map(id => id.toUpperCase())));
        }

        function parseBatchInput() {
            const textarea = document.getElementById('batchText');
            const ids = extractIdsFromText(textarea.value);
            const badge = document.getElementById('batchCountBadge');
            badge.innerText = `已识别: ${ids.length} 个元件 (${ids.slice(0, 5).join(', ')}${ids.length > 5 ? '...' : ''})`;
            return ids;
        }

        document.getElementById('batchText').addEventListener('input', parseBatchInput);

        async function startBatchExport() {
            const ids = parseBatchInput();
            if (ids.length === 0) {
                showToast('请先在输入框中粘贴有效的立创编号 (例如 C6186, C2040)', 'error');
                return;
            }

            const opts = collectExportOptions();
            opts.ids = ids;

            const logBox = document.getElementById('batchLogBox');
            const progressContainer = document.getElementById('batchProgressContainer');
            const progressBar = document.getElementById('batchProgressBar');

            logBox.style.display = 'block';
            logBox.innerText = `[*] 准备开始批量导出 ${ids.length} 个元器件...\n`;
            progressContainer.style.display = 'block';
            progressBar.style.width = '30%';

            showToast(`正在后台并发拉取与转换 ${ids.length} 个元件库...`, 'info');

            try {
                const res = await fetch('/api/export', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(opts)
                });

                if (!res.ok) {
                    const err = await res.json().catch(() => ({ error: '导出失败' }));
                    logBox.innerText += `\n[错误] 导出失败: ${err.error || res.statusText}`;
                    showToast('批量导出失败', 'error');
                    progressBar.style.width = '0%';
                    return;
                }

                progressBar.style.width = '100%';
                logBox.innerText += `[✓] 批量生成完成，正在接收 ZIP 压缩归档...\n`;

                const blob = await res.blob();
                const disposition = res.headers.get('Content-Disposition') || '';
                let filename = `SeEx_Batch_${opts.target}.zip`;
                const match = disposition.match(/filename="?([^"]+)"?/);
                if (match && match[1]) filename = match[1];

                downloadBlob(blob, filename);
                logBox.innerText += `[✓] 成功下载: ${filename} (大小: ${(blob.size / 1024).toFixed(1)} KB)\n`;
                showToast(`✓ 成功批量导出并下载 ${filename}`, 'success');
            } catch (err) {
                logBox.innerText += `\n[异常] 请求错误: ${err.message}\n`;
                showToast(`请求失败: ${err.message}`, 'error');
            }
        }

        // ================= 3D Step Cleaner =================
        let selectedStepFile = null;

        function onStepModeChange(mode) {
            const allMarksNotice = document.getElementById('stepAllMarksNotice');
            const logoOnlyNotice = document.getElementById('stepLogoOnlyNotice');
            if (mode === 'all_markings') {
                allMarksNotice.style.display = 'flex';
                logoOnlyNotice.style.display = 'none';
            } else {
                allMarksNotice.style.display = 'none';
                logoOnlyNotice.style.display = 'flex';
            }
        }

        function onStepFileSelected(event) {
            const files = event.target.files;
            if (files && files.length > 0) {
                selectedStepFile = files[0];
                document.getElementById('dropzoneTitle').innerText = `已选文件: ${selectedStepFile.name} (${(selectedStepFile.size / 1024).toFixed(1)} KB)`;
                document.getElementById('dropzoneSub').innerText = '点击重新选择，或点击下方按钮开始去标';
                document.getElementById('btnCleanStep').disabled = false;
            }
        }

        const dropzone = document.getElementById('stepDropzone');
        dropzone.addEventListener('dragover', (e) => { e.preventDefault(); dropzone.classList.add('dragover'); });
        dropzone.addEventListener('dragleave', () => dropzone.classList.remove('dragover'));
        dropzone.addEventListener('drop', (e) => {
            e.preventDefault();
            dropzone.classList.remove('dragover');
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
                selectedStepFile = e.dataTransfer.files[0];
                document.getElementById('dropzoneTitle').innerText = `已选文件: ${selectedStepFile.name} (${(selectedStepFile.size / 1024).toFixed(1)} KB)`;
                document.getElementById('dropzoneSub').innerText = '文件已准备就绪，点击下方按钮开始去标';
                document.getElementById('btnCleanStep').disabled = false;
            }
        });

        async function submitCleanStep() {
            if (!selectedStepFile) {
                showToast('请先选择或拖拽上传 STEP 文件', 'error');
                return;
            }

            const mode = document.querySelector('input[name="stepMode"]:checked').value;
            const logBox = document.getElementById('stepLogBox');
            logBox.style.display = 'block';
            logBox.innerText = `[*] 正在上传并清洗模型: ${selectedStepFile.name} (模式: ${mode})...\n`;

            const btn = document.getElementById('btnCleanStep');
            btn.disabled = true;
            btn.innerText = '正在清洗 3D 模型...';

            try {
                const url = `/api/clean-step?mode=${encodeURIComponent(mode)}&filename=${encodeURIComponent(selectedStepFile.name)}`;
                const res = await fetch(url, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/octet-stream' },
                    body: selectedStepFile
                });

                btn.disabled = false;
                btn.innerHTML = '<span>🚀 开始清洗并下载 STEP 模型</span>';

                if (!res.ok) {
                    const err = await res.json().catch(() => ({ error: '去标失败' }));
                    logBox.innerText += `[错误] 清洗失败: ${err.error || res.statusText}\n`;
                    showToast('去标处理失败', 'error');
                    return;
                }

                const logos = res.headers.get('X-Removal-Logos') || '0';
                const pieces = res.headers.get('X-Removal-Pieces') || '0';
                const faces = res.headers.get('X-Removal-Faces') || '0';
                const outcome = res.headers.get('X-Outcome') || 'OK';

                const blob = await res.blob();
                const outName = `cleaned_${selectedStepFile.name}`;
                downloadBlob(blob, outName);

                logBox.innerText += `[✓] 去标成功！\n    - 移除标记处: ${logos}\n    - 碎片实体: ${pieces}\n    - 几何拓扑面: ${faces}\n    - 状态: ${outcome}\n    - 已保存为: ${outName} (${(blob.size / 1024).toFixed(1)} KB)\n`;
                showToast(`✓ 3D 模型清洗完成，已下载 ${outName}`, 'success');
            } catch (err) {
                btn.disabled = false;
                btn.innerHTML = '<span>🚀 开始清洗并下载 STEP 模型</span>';
                logBox.innerText += `[异常] 请求错误: ${err.message}\n`;
                showToast(`请求失败: ${err.message}`, 'error');
            }
        }
    </script>
</body>
</html>
"#;
