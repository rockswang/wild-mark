@echo off
rem 构建 Scintilla.dll + Lexilla.dll（x64 Release），供 Rust 原型 LoadLibrary 使用
rem 用法：build_native.bat [scintilla|lexilla|all]，默认 all
chcp 65001 >nul
setlocal
set WHAT=%1
if "%WHAT%"=="" set WHAT=all
call D:\dev\VisualStudio2022\VC\Auxiliary\Build\vcvarsall.bat x64 >nul 2>&1
if errorlevel 1 (echo vcvars fail & exit /b 1)

set ROOT=%~dp0native

if not "%WHAT%"=="lexilla" (
  echo == build Scintilla.dll
  cd /d "%ROOT%\scintilla\win32"
  nmake /nologo /quiet -f scintilla.mak || (echo scintilla build fail & exit /b 2)
)

if not "%WHAT%"=="scintilla" (
  echo == build Lexilla.dll
  cd /d "%ROOT%\lexilla\src"
  nmake /nologo /quiet -f lexilla.mak || (echo lexilla build fail & exit /b 3)
)

echo == done
dir /b "%ROOT%\scintilla\bin\*.dll" 2>nul
dir /b "%ROOT%\lexilla\bin\*.dll" 2>nul
