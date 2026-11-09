@echo off
title Horizon job runner (close this window to stop)
rem Runs job.ps1 each time job.trigger appears in this folder, so test runs can start without clicking.
cd /d "%~dp0"
echo Waiting for jobs. Close this window to stop.
:loop
if exist "job.trigger" (
  del "job.trigger"
  echo [%TIME%] job started
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0job.ps1" > "%~dp0job-run.log" 2>&1
  echo === JOB END %DATE% %TIME% >> "%~dp0job-run.log"
  echo [%TIME%] job finished
)
timeout /t 5 /nobreak >nul
goto loop
