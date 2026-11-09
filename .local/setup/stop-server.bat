@echo off
cd /d "%~dp0..\.."
".local\native\target\fast\horizon-server.exe" --dir server-oneblock stop
