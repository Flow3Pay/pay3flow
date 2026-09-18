param(
    [switch]$CollectLogs,
    [int]$TimeoutSeconds = 3,
    [int]$KeepLogRuns = 5
)

$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$composeFile = Join-Path $root 'docker-compose.yml'
$logDir = Join-Path $root 'diagnostics\logs'

function Invoke-HttpCheck {
    param([string]$Url, [int]$TimeoutMs)
    $client = $null
    try {
        $handler = [System.Net.Http.HttpClientHandler]::new()
        $client = [System.Net.Http.HttpClient]::new($handler, $true)
        $client.Timeout = [TimeSpan]::FromMilliseconds($TimeoutMs)
        $resp = $client.GetAsync($Url).GetAwaiter().GetResult()
        $code = [int]$resp.StatusCode
        $body = if ($null -ne $resp.Content) { $resp.Content.ReadAsStringAsync().GetAwaiter().GetResult() } else { '' }
        [pscustomobject]@{ Code = $code; Body = $body }
    }
    catch {
        $null
    }
    finally {
        if ($null -ne $client) { $client.Dispose() }
    }
}

function Test-TcpPort {
    param([string]$HostName, [int]$Port, [int]$TimeoutMs)
    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync($HostName, $Port)
        if ($task.Wait([TimeSpan]::FromMilliseconds($TimeoutMs))) { $true } else { $false }
    }
    catch {
        $false
    }
    finally {
        $client.Dispose()
    }
}

function Get-ComposeStates {
    $states = @{}
    foreach ($line in (& docker compose -f $composeFile ps --all --format json)) {
        $o = $line | ConvertFrom-Json
        $states[$o.Service] = $o
    }
    $states
}

$services = @(
    @{ Name = 'backend';         Kind = 'http'; Endpoint = 'http://localhost:8080/health'; Expect = 'ok'        },
    @{ Name = 'fmatch';          Kind = 'http'; Endpoint = 'http://localhost:7277/health'; Expect = 'ok'        },
    @{ Name = 'fmatch-typesense';Kind = 'http'; Endpoint = 'http://localhost:8108/health'; Expect = 'ok'        },
    @{ Name = 'frontend';        Kind = 'http'; Endpoint = 'http://localhost:3000/';       Expect = 'any2xx'    },
    @{ Name = 'postgres';        Kind = 'tcp';  Host = 'localhost'; Port = 5435                                 },
    @{ Name = 'fmatch-postgres'; Kind = 'tcp';  Host = 'localhost'; Port = 5433                                 }
)

$states = Get-ComposeStates
$results = foreach ($svc in $services) {
    $name = $svc.Name
    $state = $states[$name]
    $dockerOk = $true
    $dockerNote = ''
    if ($null -eq $state) {
        $dockerOk = $false
        $dockerNote = 'no container in compose'
    }
    elseif ($state.State -ne 'running') {
        $dockerOk = $false
        $dockerNote = "container state: $($state.State)"
    }
    elseif ($state.Health -and $state.Health -ne 'healthy') {
        $dockerOk = $false
        $dockerNote = "docker health: $($state.Health)"
    }
    else {
        $dockerNote = if ($state.Health) { "running/$($state.Health)" } else { 'running' }
    }

    $timeoutMs = $TimeoutSeconds * 1000
    $appOk = $false
    $appNote = ''
    if ($svc.Kind -eq 'http') {
        $r = Invoke-HttpCheck -Url $svc.Endpoint -TimeoutMs $timeoutMs
        if ($null -eq $r) {
            $appNote = 'connect/request failed'
        }
        elseif ($svc.Expect -eq 'any2xx') {
            $appOk = ($r.Code -ge 200 -and $r.Code -lt 300)
            $appNote = "http $($r.Code)"
        }
        else {
            $match = $r.Body -match $svc.Expect
            $appOk = ($r.Code -eq 200 -and $match)
            $appNote = "http $($r.Code) body-match=$match"
        }
    }
    else {
        $open = Test-TcpPort -HostName $svc.Host -Port $svc.Port -TimeoutMs $timeoutMs
        $appOk = $open
        $appNote = if ($open) { "tcp $($svc.Host):$($svc.Port) open" } else { 'tcp connect failed' }
    }

    $ok = $dockerOk -and $appOk
    [pscustomobject]@{
        Service = $name
        Check   = $appNote
        Docker  = $dockerNote
        Ok      = $ok
    }
}

foreach ($r in $results) {
    $mark = if ($r.Ok) { 'OK' } else { 'FAIL' }
    $color = if ($r.Ok) { 'Green' } else { 'Red' }
    Write-Host ("{0,-4} {1,-18} {2,-45} {3}" -f $mark, $r.Service, $r.Check, $r.Docker) -ForegroundColor $color
}

if ($CollectLogs) {
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    foreach ($svc in $services) {
        $out = Join-Path $logDir ("{0}-{1}.log" -f $stamp, $svc.Name)
        & docker compose -f $composeFile logs --no-color --timestamps $svc.Name |
            Out-File -FilePath $out -Encoding utf8
    }
    Write-Host ("logs collected into {0}" -f $logDir)

    foreach ($svc in $services) {
        $files = @(Get-ChildItem -Path $logDir -Filter ("*-{0}.log" -f $svc.Name) -File |
            Sort-Object Name -Descending)
        for ($i = $KeepLogRuns; $i -lt $files.Count; $i++) {
            Remove-Item -LiteralPath $files[$i].FullName -Force
        }
    }
}

$failed = @($results | Where-Object { -not $_.Ok })
if ($failed.Count -gt 0) {
    Write-Host ("{0}/{1} services unhealthy" -f $failed.Count, $results.Count) -ForegroundColor Red
    exit 1
}
Write-Host ("all {0} services healthy" -f $results.Count) -ForegroundColor Green
exit 0
