@echo off
rem Closes a Horizon test game left running by a test job (only the test copy, never your own game).
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='horizon-client.exe'\" | Where-Object { $_.CommandLine -like '*horizon-native-test*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; 'closed ' + $_.ProcessId }"
timeout /t 3 >nul
