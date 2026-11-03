@echo off
set "ACA_NATIVE_ROOT=%~dp0.."
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 exit /b 1
for %%R in (photocraft vectorcraft effectcraft filmcraft soundcraft) do (
 "%ACA_NATIVE_ROOT%\.toolchain\rustup\toolchains\1.95.0-x86_64-pc-windows-msvc\bin\rustc.exe" --edition=2024 --test "%ACA_NATIVE_ROOT%\%%R\apps\%%R-web\src\academy.rs" -o "%ACA_NATIVE_ROOT%\.target-web\aca-%%R-theme-tests.exe"
 if errorlevel 1 exit /b 1
 "%ACA_NATIVE_ROOT%\.target-web\aca-%%R-theme-tests.exe"
 if errorlevel 1 exit /b 1
)
