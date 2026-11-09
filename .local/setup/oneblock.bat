@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0oneblock.ps1" > "%~dp0oneblock.log" 2>&1
echo === END >> "%~dp0oneblock.log"
