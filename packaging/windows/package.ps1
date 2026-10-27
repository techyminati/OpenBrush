# OpenBrush
# Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
# Author: Aryan Sinha <techyminati>
# Licensed under the CipherOS License 2.0. See LICENSE for the full terms.
#
# Builds OpenBrush.exe and zips it with the licence files.
# Usage: pwsh packaging/windows/package.ps1 [-Out dist]
param([string]$Out = "dist")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$stage = Join-Path ([System.IO.Path]::GetTempPath()) "OpenBrush-package\OpenBrush"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item "target\release\openbrush.exe" (Join-Path $stage "OpenBrush.exe")
Copy-Item "LICENSE" (Join-Path $stage "LICENSE.txt")
Copy-Item "THIRD_PARTY_NOTICES.md" $stage

New-Item -ItemType Directory -Force -Path $Out | Out-Null
$zip = Join-Path $Out "OpenBrush-windows-x86_64.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Output $zip
