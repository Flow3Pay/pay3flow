<#
.SYNOPSIS
    Provision a local test crypto wallet end-to-end.

.DESCRIPTION
    - starts the local test EVM node (Anvil, docker compose service)
    - creates/loads an encrypted keystore wallet   (backend/src/bin/wallet-init.rs)
    - funds it with test ETH and mocks USDT/USDC/TOKEN (contracts/mock/MockERC20.sol)
    - writes data\wallet\setup.env with everything the backend needs

    Requires: docker, cargo.
.PARAMETER Password
    Keystore password. Defaults to 'dev-local-test-password'.
.PARAMETER RpcUrl
    Node URL as seen from the host. Defaults to http://localhost:8545.
.PARAMETER DevKey
    EOA key that funds and deploys (anvil dev account #0 test key by default).
.PARAMETER Recipient
    Destination address used later by wallet-smoke (anvil dev account #1).
.PARAMETER NativeFundEth
    Amount of test ETH to give the wallet (default 50).
.EXAMPLE
    .\scripts\wallet-setup.ps1
#>
param(
    [string]$KeystoreDir = '',
    [string]$Password = '',
    [string]$RpcUrl = 'http://localhost:8545',
    [string]$DevKey = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
    [string]$Recipient = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
    [decimal]$NativeFundEth = 50
)

$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$composeFile = Join-Path $root 'docker-compose.yml'
$keystoreDir = if ($KeystoreDir) { $KeystoreDir } else { Join-Path $root 'data\wallet' }
$password = if ($Password) { $Password } else { 'dev-local-test-password' }

function Write-Step {
    param([string]$Title)
    Write-Host ""
    Write-Host "== $Title ==" -ForegroundColor Cyan
}

function Invoke-Cast {
    param([Parameter(ValueFromRemainingArguments)][string[]]$Arguments)
    & docker compose -f $composeFile exec -T anvil cast @Arguments 2>&1
}

# --- 1. local test EVM node --------------------------------------------------
Write-Step "anvil test node ($RpcUrl)"
& docker compose -f $composeFile up -d anvil | Out-Null
if ($LASTEXITCODE -ne 0) { throw "docker compose up anvil failed" }

$ready = $false
for ($i = 0; $i -lt 30; $i++) {
    $chain = Invoke-Cast chain-id --rpc-url http://localhost:8545 2>&1 | Out-String
    if ($LASTEXITCODE -eq 0 -and $chain.Trim()) {
        $ready = $true
        Write-Host "chain-id: $($chain.Trim())"
        break
    }
    Start-Sleep -Seconds 1
}
if (-not $ready) { throw "anvil did not become ready in time" }

# --- 2. encrypted keystore ---------------------------------------------------
New-Item -ItemType Directory -Force -Path $keystoreDir | Out-Null
# alloy names the keystore file by bare UUID (no UTC-- prefix); accept any
# file in the dir that is not our setup.env bookkeeping file.
$keystore = @(Get-ChildItem -Path $keystoreDir -File |
    Where-Object { $_.Name -ne 'setup.env' } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName)
if (-not $keystore) {
    Write-Step "create encrypted keystore"
    $env:WALLET_KEYSTORE_PASSWORD = $password
    & cargo run --quiet --manifest-path (Join-Path $root 'backend\Cargo.toml') --bin wallet-init -- $keystoreDir
    if ($LASTEXITCODE -ne 0) { throw "wallet-init failed" }
    Remove-Item Env:WALLET_KEYSTORE_PASSWORD
    $keystore = @(Get-ChildItem -Path $keystoreDir -File |
        Where-Object { $_.Name -ne 'setup.env' } |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName)
}
if (-not $keystore) { throw "keystore was not created under $keystoreDir" }

function Get-WalletAddress {
    param([string]$KeystoreFile, [string]$KeystorePassword)
    $pk = ((Get-Content -LiteralPath $KeystoreFile -Raw) |
        & docker compose -f $composeFile exec -T -e "KP=$KeystorePassword" anvil sh -c `
        'cat > /tmp/ks && cast wallet private-key --keystore /tmp/ks --password "$KP"' 2>&1 |
        Out-String).Trim()
    if ($pk -notmatch '^0x[0-9a-fA-F]{64}$') { throw "failed to decrypt keystore $KeystoreFile" }
    ((& docker compose -f $composeFile exec -T anvil cast wallet address $pk 2>&1 | Out-String).Trim())
}

$walletAddr = Get-WalletAddress -KeystoreFile $keystore -KeystorePassword $password
Write-Host "wallet address: $walletAddr"
Write-Host "keystore file:  $keystore"

# --- 3. native ETH balance ---------------------------------------------------
Write-Step "fund native ETH ($NativeFundEth eth)"
$native = (Invoke-Cast balance --rpc-url $RpcUrl $walletAddr | Out-String).Trim()
if ($native -eq '' -or $native -eq '0') {
    Invoke-Cast send --rpc-url $RpcUrl --private-key $DevKey --value "${NativeFundEth}ether" $walletAddr | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "native funding failed" }
}
else {
    Write-Host "already funded: $native wei"
}
$native = (Invoke-Cast balance --rpc-url $RpcUrl $walletAddr | Out-String).Trim()

# --- 4. mock ERC-20 tokens ---------------------------------------------------
# Anvil keeps state in memory and resets the chain on restart (block number
# drops to 0). If the block number recorded by a previous run is still below
# the current one, the chain is a strict continuation and the stored contract
# addresses may be reused; otherwise the recorded addresses are stale and we
# must deploy fresh contracts.
$envFile = Join-Path $keystoreDir 'setup.env'
$prevBlocks = ''
if (Test-Path $envFile) {
    $line = Get-Content $envFile | Where-Object { $_ -match '^ANVIL_BLOCKS=' } | Select-Object -Last 1
    if ($line) { $prevBlocks = ($line -split '=', 2)[1] }
}
$curBlocks = (Invoke-Cast block-number --rpc-url $RpcUrl | Out-String).Trim()
$canReuse = $false
if ($prevBlocks -match '^\d+$' -and $curBlocks -match '^\d+$') {
    $canReuse = [bigint]::Parse($curBlocks) -ge [bigint]::Parse($prevBlocks)
}

$tokens = @( @{ Name = 'USDT'; Decimals = 6 }, @{ Name = 'USDC'; Decimals = 6 }, @{ Name = 'TOKEN'; Decimals = 18 } )
$contracts = @{}
foreach ($tok in $tokens) {
    $spec = $tok.Name
    $decimals = $tok.Decimals
    $addr = ''
    if ($canReuse -and (Test-Path $envFile)) {
        $prev = Get-Content $envFile | Where-Object {
            $_ -match "^${spec}_CONTRACT="
        } | Select-Object -Last 1
        if ($prev) { $addr = ($prev -split '=', 2)[1] }
    }
    $code = if ($addr) { (Invoke-Cast code --rpc-url $RpcUrl $addr 2>&1 | Out-String).Trim() } else { '' }
    if ($addr -and $code -and $code -ne '0x') {
        Write-Host "reuse mock $spec`: $addr"
    }
    else {
        Write-Step "deploy mock $spec (decimals=$decimals)"
        $payload = 'rm -rf /tmp/f && mkdir -p /tmp/f && cp /contracts/mock/MockERC20.sol /tmp/f/' +
            ' && cd /tmp/f' +
            ' && forge create MockERC20.sol:MockERC20 --broadcast --rpc-url http://localhost:8545' +
            ' --private-key "$DEV_KEY" --constructor-args "$TOKEN_NAME" "$TOKEN_SYMBOL" "$TOKEN_DECIMALS"'
        $out = & docker compose -f $composeFile exec -T `
            -e "TOKEN_NAME=$spec" -e "TOKEN_SYMBOL=$spec" -e "TOKEN_DECIMALS=$decimals" -e "DEV_KEY=$DevKey" `
            anvil sh -c $payload 2>&1
        $m = ($out | Select-String -Pattern 'Deployed to: (0x[0-9a-fA-F]{40})').Matches
        if (-not $m -or -not $m[0]) {
            Write-Host "mock $spec deployment output:" -ForegroundColor Yellow
            $out | ForEach-Object { Write-Host $_ }
            throw "mock $spec deployment failed"
        }
        $addr = $m[0].Groups[1].Value
        Write-Host "deployed $spec`: $addr"
    }
    $contracts[$spec] = $addr
}

# --- 5. mint tokens to the wallet --------------------------------------------
foreach ($tok in $tokens) {
    $spec = $tok.Name
    $decimals = $tok.Decimals
    $amount = [string]([bigint]::Multiply([bigint]::Parse('1000000'), [bigint]::Pow(10, [int]$decimals)))
    Write-Step "mint 1,000,000 $spec to wallet"
    Invoke-Cast send --rpc-url $RpcUrl --private-key $DevKey $contracts[$spec] `
        "mint(address,uint256)" $walletAddr $amount | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "mint $spec failed" }
}

# --- 6. persistence file ------------------------------------------------------
Write-Step "write $keystoreDir\setup.env"
$curBlocks = (Invoke-Cast block-number --rpc-url $RpcUrl | Out-String).Trim()
$lines = @(
    '# Local test wallet generated by scripts/wallet-setup.ps1'
    "WALLET_KEYSTORE_PATH=$keystore"
    '# Same keystore as seen from inside the backend docker container'
    "WALLET_KEYSTORE_PATH_DOCKER=/app/wallet/$(Split-Path $keystore -Leaf)"
    "WALLET_KEYSTORE_PASSWORD=$password"
    '# RPC from the host (used by cargo run / local backend)'
    "WALLET_RPC_URL=$RpcUrl"
    '# RPC from inside a docker container (backend service, anvil is the compose service name)'
    'WALLET_RPC_URL_DOCKER=http://anvil:8545'
    "WALLET_ADDRESS=$walletAddr"
    "WALLET_NATIVE_FUND_ETH=$NativeFundEth"
    "ANVIL_BLOCKS=$curBlocks"
    "USDT_CONTRACT=$($contracts['USDT'])"
    "USDC_CONTRACT=$($contracts['USDC'])"
    "TOKEN_CONTRACT=$($contracts['TOKEN'])"
    "RECIPIENT=$Recipient"
)
[System.IO.File]::WriteAllLines((Join-Path $keystoreDir 'setup.env'), $lines, [System.Text.UTF8Encoding]::new($false))

# --- 7. summary --------------------------------------------------------------
Write-Step "summary"
Write-Host "wallet:   $walletAddr"
Write-Host "native:   $native wei"
foreach ($tok in $tokens) {
    $spec = $tok.Name
    $decimals = $tok.Decimals
    $raw = (Invoke-Cast call --rpc-url $RpcUrl $contracts[$spec] "balanceOf(address)(uint256)" $walletAddr |
        Out-String).Trim()
    if ($raw -match '^(\d+)') {
        $raw = $Matches[1]
    }
    $human = [decimal]0
    if ($raw -match '^\d+$') {
        $human = [decimal]::Parse($raw) / [math]::Pow(10, [int]$decimals)
    }
    Write-Host ("  {0,-5} {1,20:N4}  (contract {2}, {3} raw)" -f $spec, $human, $contracts[$spec], $raw)
}
Write-Host ""
Write-Host "Next: point the backend at this wallet and run scripts\wallet-smoke.ps1" -ForegroundColor Green