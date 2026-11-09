@echo off
chcp 65001 >nul
title SeEx 一键编译 Release 版本

echo ======================================================================
echo                     SeEx 一键编译 (Release)
echo ======================================================================
echo.

cd /d "%~dp0"

if /i "%1"=="wasm" goto :BUILD_WASM

where cargo >nul 2>nul
if errorlevel 1 if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if errorlevel 1 if exist "e:\murmur\.cargo\bin\cargo.exe" set "PATH=e:\murmur\.cargo\bin;%PATH%"

where fxc.exe >nul 2>nul
if not errorlevel 1 goto :HAS_FXC

if exist "%~dp0fxc.exe" set "GPUI_FXC_PATH=%~dp0fxc.exe" & goto :HAS_FXC
if exist "%USERPROFILE%\.cargo\bin\fxc.exe" set "GPUI_FXC_PATH=%USERPROFILE%\.cargo\bin\fxc.exe" & goto :HAS_FXC
if exist "e:\murmur\.cargo\bin\fxc.exe" set "GPUI_FXC_PATH=e:\murmur\.cargo\bin\fxc.exe" & goto :HAS_FXC

if exist "%~dp0fxc.c" (
    where gcc >nul 2>nul
    if not errorlevel 1 (
        echo [*] 正在生成 HLSL 编译工具 fxc.exe ...
        gcc -O2 "%~dp0fxc.c" -o "%~dp0fxc.exe" -ld3dcompiler_47
        if exist "%~dp0fxc.exe" set "GPUI_FXC_PATH=%~dp0fxc.exe"
    )
)

:HAS_FXC

echo [1/2] 正在编译 SeEx Release 版本，请稍候...
cargo build --release --bin seex

if errorlevel 1 goto :BUILD_FAIL

echo.
echo ======================================================================
echo [2/2] 编译成功！
echo [输出文件] %~dp0target\release\seex.exe
echo ======================================================================
goto :BUILD_END

:BUILD_WASM
where cargo >nul 2>nul
if errorlevel 1 if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if errorlevel 1 if exist "e:\murmur\.cargo\bin\cargo.exe" set "PATH=e:\murmur\.cargo\bin;%PATH%"

echo [*] 正在编译 SeEx WebAssembly (WASM) 纯前端版本...
rustup target add wasm32-unknown-unknown >nul 2>&1
cargo build --release --manifest-path "%~dp0crates\seex-wasm\Cargo.toml" --target wasm32-unknown-unknown
if errorlevel 1 goto :BUILD_FAIL

if not exist "%~dp0dist\wasm" mkdir "%~dp0dist\wasm"
copy /y "%~dp0crates\seex-wasm\target\wasm32-unknown-unknown\release\seex_wasm.wasm" "%~dp0dist\wasm\seex_wasm.wasm" >nul
copy /y "%~dp0crates\seex-wasm\web\seex.js" "%~dp0dist\wasm\seex.js" >nul
copy /y "%~dp0crates\seex-wasm\web\index.html" "%~dp0dist\wasm\index.html" >nul

echo.
echo ======================================================================
echo 编译成功！WebAssembly 产物已输出至:
echo %~dp0dist\wasm\
echo   - seex_wasm.wasm
echo   - seex.js
echo   - index.html
echo ======================================================================
goto :BUILD_END

:BUILD_FAIL
echo.
echo [错误] 编译未成功，请检查上方错误提示！

:BUILD_END
echo.
echo 按任意键退出...
pause >nul
