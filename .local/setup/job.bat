@echo off
title Horizon build
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0job.ps1" > "%~dp0job-run.log" 2>&1
echo === JOB END %DATE% %TIME% >> "%~dp0job-run.log"
