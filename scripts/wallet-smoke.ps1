<#
.SYNOPSIS
    Exercise the self-hosted wallet HTTP API end-to-end against the local test
    node: info, balance, native transfer, ERC-20 transfer.

.DESCRIPTION
    Uses the values scripts\wallet-setup.ps1 wrote into data\wallet\setup.env.
    Prerequisites:
      1. scripts\wallet-setup.ps1 ran successfully
      2. the backend was (re)created with WALLET_KEYSTORE_PATH/PASSWORD/RPC set
         (docs/local-test-wallet.md)
.PARAMETER Backend
    Backend base URL (default http://localhost:8080).
.PARAMETER AdminToken
    Admin bearer token (default dev-admin-token-change-me).
.EXAMPLE
    .\scripts\wallet-smoke.ps1
#>
param(
    [string]$Backend = 'http://localhost:8080',
    [string]$AdminToken = 'dev-admin-token-change-me'
)

$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$composeFile = Join-Path $root 'docker-compose.yml'
$keystoreDir = Join-Path $root 'data\wallet'
$envFile = Join-Path $keystoreDir 'setup.env'

if (-not (Test-Path $envFile)) {
    throw "run scripts\wallet-setup.ps1 first (no $envFile)"
}
$setup = @{}
Get-Content $envFile | ForEach-Object {
    if ($_ -match '^\s*([A-Z0-9_]+)=(.*)$') {
        $setup[$Matches[1]] = $Matches[2]
    }
}

$walletAddr = $setup['WALLET_ADDRESS']
$usdt = $setup['USDT_CONTRACT']
$recipient = $setup['RECIPIENT']
if (-not $walletAddr -or -not $usdt -or -not $recipient) {
    throw "setup.env is missing WALLET_ADDRESS/USDT_CONTRACT/RECIPIENT"
}
$nativeAmountWei = '1000000000000000'    # 0.001 ETH
$tokenAmountRaw = '1000000'              # 1.0 USDT (6 decimals)

function Invoke-Api {
    param([string]$Method, [string]$Path, [string]$Body = '')
    $args = @('-s', '-X', $Method, "$Backend$Path", '-H', 'Accept: application/json',
        '-H', "Authorization: Bearer $AdminToken", '-w', "`n%{http_code}")
    if ($Body) {
        $args += @('-H', 'Content-Type: application/json', '-d', $Body)
    }
    $raw = & curl.exe @args 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "curl failed: $Backend$Path" }
    $code = [int]$raw.Trim().Substring($raw.Trim().LastIndexOf("`n") + 1)
    $json = $raw.Trim() -replace '\n\d{3}$', ''
    if ($code -lt 200 -or $code -ge 300) {
        throw "http $code on $Method $Path`n$json"
    }
    $json
}

function Assert-True {
    param([bool]$Cond, [string]$Msg)
    if (-not $Cond) { throw "assertion failed: $Msg" }
    Write-Host "  OK: $Msg" -ForegroundColor Green
}

Write-Host "[1/5] backend health"
$health = & curl.exe -s "$Backend/health"
Assert-True ($health -eq 'ok') "backend health"

Write-Host "[2/5] GET /api/admin/wallet"
$info = Invoke-Api -Method 'GET' -Path '/api/admin/wallet' | ConvertFrom-Json
Assert-True ($info.address -eq $walletAddr) "wallet address matches setup.env"
Assert-True (-not [string]::IsNullOrEmpty($info.rpc_url)) "rpc_url present: $($info.rpc_url)"

Write-Host "[3/5] GET /api/admin/wallet/balance"
$balance = Invoke-Api -Method 'GET' -Path '/api/admin/wallet/balance' | ConvertFrom-Json
Assert-True ($balance.address -eq $walletAddr) "balance address matches"
Assert-True ([bigint]::Parse($balance.wei) -gt 0) "native wei > 0"

Write-Host "[4/5] POST /api/admin/wallet/transfer (native 0.001 ETH)"
$nativeBody = (@{ to = $recipient; value_wei = $nativeAmountWei } | ConvertTo-Json -Compress)
$nativeResp = Invoke-Api -Method 'POST' -Path '/api/admin/wallet/transfer' -Body $nativeBody | ConvertFrom-Json
Assert-True ($nativeResp.to -eq $recipient) "native recipient"
Assert-True ($nativeResp.tx_hash -match '^0x[0-9a-fA-F]{64}') "native tx_hash: $($nativeResp.tx_hash)"

Write-Host "[5/5] POST /api/admin/wallet/token-transfer (1.0 USDT)"
$tokenBody = (@{ token_contract = $usdt; to = $recipient; amount = $tokenAmountRaw } | ConvertTo-Json -Compress)
$tokenResp = Invoke-Api -Method 'POST' -Path '/api/admin/wallet/token-transfer' -Body $tokenBody | ConvertFrom-Json
Assert-True ($tokenResp.tx_hash -match '^0x[0-9a-fA-F]{64}') "token tx_hash: $($tokenResp.tx_hash)"

$onchain = (& docker compose -f $composeFile exec -T anvil cast call `
    --rpc-url http://localhost:8545 $usdt "balanceOf(address)(uint256)" $recipient 2>&1 |
    Out-String).Trim()
$onchainRaw = '0'
if ($onchain -match '^(\d+)') { $onchainRaw = $Matches[1] }
Assert-True ([bigint]::Parse($onchainRaw) -ge [bigint]::Parse($tokenAmountRaw)) `
    "recipient USDT balance on-chain increased: $onchain"

Write-Host ""
Write-Host ("WALLET SMOKE PASSED: native tx {0}, token tx {1}" -f $nativeResp.tx_hash, $tokenResp.tx_hash) -ForegroundColor Green