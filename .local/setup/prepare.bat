@echo off
cd /d "%~dp0..\.."
set LOG=%~dp0prepare.log
set JAR=%~dp0server.jar
echo === START %DATE% %TIME% > "%LOG%"
if not exist "%JAR%" powershell -NoProfile -Command "Invoke-WebRequest -UseBasicParsing -Uri https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar -OutFile $env:JAR" >> "%LOG%" 2>&1
powershell -NoProfile -Command "(Get-FileHash -Algorithm SHA1 $env:JAR).Hash" > "%~dp0jar.sha1" 2>&1
type "%~dp0jar.sha1" >> "%LOG%"
findstr /i "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c" "%~dp0jar.sha1" >nul || (echo SHA1 MISMATCH >> "%LOG%" & goto end)
echo --- prepare >> "%LOG%"
cargo run --release -p horizon-prepare -- --prepare --server-jar "%JAR%" >> "%LOG%" 2>&1
echo PREPARE_EXIT %ERRORLEVEL% >> "%LOG%"
echo --- sounds >> "%LOG%"
cargo run --release -p horizon-prepare -- sounds >> "%LOG%" 2>&1
echo SOUNDS_EXIT %ERRORLEVEL% >> "%LOG%"
:end
echo === END %DATE% %TIME% >> "%LOG%"
