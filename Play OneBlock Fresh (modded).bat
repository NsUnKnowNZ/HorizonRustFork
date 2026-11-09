@echo off
rem The fresh Better OneBlock world (no lava or structures below) with the native mods.
cd /d "%~dp0"
if not exist ".local\native\target\fast\horizon-client.exe" (echo Modded build missing: run .local\setup\job.bat & pause & exit /b 1)
start "" ".local\native\target\fast\horizon-client.exe" --world "OneBlockFresh"
