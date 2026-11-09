@echo off
cd /d "%~dp0..\.."
set LOG=%~dp0build.log
echo === START %DATE% %TIME% > "%LOG%"
echo --- 26.3 files >> "%LOG%"
dir "%APPDATA%\.minecraft\versions\26.3" >> "%LOG%" 2>&1
python "%~dp0server_url.py" >> "%LOG%" 2>&1
echo --- cargo build >> "%LOG%"
cargo build --release -p horizon-prepare -p horizon-client >> "%LOG%" 2>&1
echo EXIT %ERRORLEVEL% >> "%LOG%"
echo === END %DATE% %TIME% >> "%LOG%"
