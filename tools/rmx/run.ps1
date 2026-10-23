Set-Location $PSScriptRoot
dotnet run -c Release -- @args 2>&1 | Where-Object { $_ -notmatch 'MSB3246' }
