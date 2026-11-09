"""Writes job.ps1 for a Camerapture body: the mod moved into mods\ for the build."""
import subprocess, sys
subprocess.run([sys.executable, 'make_job.py'] + sys.argv[1:], check=True)
s = open('job.ps1', encoding='utf-8-sig').read()
line = 'if (Test-Path "$repo\\.local\\native\\mods-pending\\camerapture") { Move-Item -Force "$repo\\.local\\native\\mods-pending\\camerapture" "$repo\\.local\\native\\mods\\camerapture" }'
s = s.replace('& "$repo\\.local\\native\\build-native.ps1"', line + '\r\n& "$repo\\.local\\native\\build-native.ps1"', 1)
open('job.ps1', 'w', encoding='utf-8-sig', newline='').write(s)
print('cam job written')
