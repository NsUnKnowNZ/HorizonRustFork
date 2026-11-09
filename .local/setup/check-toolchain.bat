@echo off
set LOG=%~dp0check.log
echo === %DATE% %TIME% > "%LOG%"
for %%T in (rustc cargo rustup python py java git) do (echo --- %%T >> "%LOG%" & where %%T >> "%LOG%" 2>&1)
rustc --version >> "%LOG%" 2>&1
cargo --version >> "%LOG%" 2>&1
python --version >> "%LOG%" 2>&1
java -version >> "%LOG%" 2>&1
where winget >> "%LOG%" 2>&1
echo --- msvc >> "%LOG%"
dir /b "%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" >> "%LOG%" 2>&1
"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath >> "%LOG%" 2>&1
echo --- minecraft >> "%LOG%"
dir /b "%APPDATA%\.minecraft\versions" >> "%LOG%" 2>&1
echo DONE >> "%LOG%"
