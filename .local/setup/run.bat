@echo off
cd /d "%~dp0..\.."
cargo run --profile fast -p horizon-client > "%~dp0run.log" 2>&1
echo CLIENT_EXIT %ERRORLEVEL% >> "%~dp0run.log"
