@echo off
REM Launches the TAN APO tray indicator (hidden PowerShell host). Double-click
REM this, or use the Start Menu shortcut created by install-tan-shortcuts.ps1.
start "" powershell -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "%~dp0tan-apo-tray.ps1"
