$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path -Parent $PSScriptRoot
$serverLog = Join-Path $projectDirectory '.preview-vite.log'
$serverError = Join-Path $projectDirectory '.preview-vite-error.log'
$nodeExecutable = (Get-Command node).Source
Start-Process -FilePath $nodeExecutable -ArgumentList @('node_modules/vite/bin/vite.js','--host','127.0.0.1','--port','1420') -WorkingDirectory $projectDirectory -WindowStyle Hidden -RedirectStandardOutput $serverLog -RedirectStandardError $serverError -PassThru | Select-Object Id
