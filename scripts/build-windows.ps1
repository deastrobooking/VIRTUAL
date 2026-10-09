# Run with PowerShell 7 in an x64 Visual Studio developer environment.
# See docs/CROSS_PLATFORM_BUILDS.md for native prerequisites.
[CmdletBinding()]
param(
    [string]$VcpkgRoot = $env:VCPKG_ROOT
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'Build on Windows x64 using PowerShell 7.' }
$ProjectRoot = Split-Path $PSScriptRoot -Parent
function Invoke-Native {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}
foreach ($Tool in @('cargo', 'rustc', 'cl', 'cmake', 'dumpbin')) {
    if (-not (Get-Command $Tool -ErrorAction SilentlyContinue)) {
        throw "Missing $Tool. Use an x64 VS developer shell with Rust, CMake and LLVM installed."
    }
}
if (-not $VcpkgRoot -or -not (Test-Path "$VcpkgRoot/vcpkg.exe")) {
    throw 'Set VCPKG_ROOT to a bootstrapped vcpkg checkout.'
}
$VcpkgRoot = (Resolve-Path $VcpkgRoot).Path
$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKGRS_DYNAMIC = '1'
$env:VCPKGRS_TRIPLET = 'x64-windows'
$Installed = Join-Path $ProjectRoot 'target/vcpkg-installed'
$env:VCPKG_INSTALLED_ROOT = $Installed
$Manifest = Join-Path $ProjectRoot 'packaging/windows'
Invoke-Native "$VcpkgRoot/vcpkg.exe" @('install', "--x-manifest-root=$Manifest", "--x-install-root=$Installed", '--triplet=x64-windows')
$Native = Join-Path $Installed 'x64-windows'
$env:PATH = "$Native/bin;$env:PATH"
$Target = 'x86_64-pc-windows-msvc'
Push-Location $ProjectRoot
try {
    Invoke-Native cargo @('build', '--release', '--locked', '-p', 'virtual-app', '--target', $Target, '--target-dir', "$ProjectRoot/target")
    $Release = Join-Path $ProjectRoot "target/$Target/release"
    $Staging = Join-Path $Release ("virtual-windows-" + [guid]::NewGuid())
    $Bundle = Join-Path $Staging 'VIRTUAL'
    New-Item -ItemType Directory -Path $Bundle -Force | Out-Null
    try {
        Copy-Item "$Release/virtual.exe" $Bundle
        Copy-Item "$ProjectRoot/effects", "$ProjectRoot/docs" $Bundle -Recurse
        Copy-Item "$ProjectRoot/LICENSE", "$ProjectRoot/README.md" $Bundle
        New-Item -ItemType File "$Bundle/virtual-portable" | Out-Null
        # The manifest is minimal; copying its complete DLL set also includes
        # dependencies loaded dynamically rather than through the PE import table.
        Copy-Item "$Native/bin/*.dll" $Bundle
        $Licenses = New-Item -ItemType Directory "$Bundle/licenses"
        Get-ChildItem "$Native/share" -Filter copyright -Recurse | ForEach-Object {
            Copy-Item $_.FullName (Join-Path $Licenses.FullName ($_.Directory.Name + '.copyright'))
        }
        Copy-Item "$Manifest/vcpkg.json" "$Bundle/native-dependencies.json"
        # App-local VC runtime: users do not need Visual Studio or a separate
        # redistributable installation. UCRT comes with the supported Windows OS.
        if (-not $env:VCToolsRedistDir) { throw 'VCToolsRedistDir is missing; use the VS developer shell.' }
        $Crt = @(Get-ChildItem "$env:VCToolsRedistDir/x64" -Directory -Filter '*.CRT')
        if ($Crt.Count -ne 1) { throw 'Could not identify the x64 MSVC redistributable directory.' }
        Copy-Item "$($Crt[0].FullName)/*.dll" $Bundle
        # Audit every packaged PE import. System DLLs are supplied by Windows.
        foreach ($Binary in (Get-ChildItem $Bundle -File | Where-Object { $_.Extension -in '.exe', '.dll' })) {
            $Imports = & dumpbin /DEPENDENTS $Binary.FullName
            if ($LASTEXITCODE -ne 0) { throw "Cannot inspect $($Binary.Name)" }
            foreach ($Line in $Imports) {
                if ($Line -match '^\s+([A-Za-z0-9_.-]+\.dll)\s*$') {
                    $Dll = $Matches[1]
                    if ($Dll -match '^(api-ms-|ext-ms-)') { continue }
                    if (-not (Test-Path "$Bundle/$Dll") -and -not (Test-Path "$env:SystemRoot/System32/$Dll")) {
                        throw "Unbundled dependency $Dll required by $($Binary.Name)"
                    }
                }
            }
        }
        $SavedPath = $env:PATH
        Push-Location $Staging
        try {
            $env:PATH = "$env:SystemRoot/System32;$env:SystemRoot"
            Invoke-Native "$Bundle/virtual.exe" @('--version')
        } finally { $env:PATH = $SavedPath; Pop-Location }
        $VersionLine = Get-Content Cargo.toml | Where-Object { $_ -match '^version = "([^"]+)"' } | Select-Object -First 1
        $Version = [regex]::Match($VersionLine, '"([^"]+)"').Groups[1].Value
        $Artifact = "VIRTUAL-$Version-windows-x86_64.zip"
        Compress-Archive -Path $Bundle -DestinationPath "$Staging/$Artifact"
        $Hash = (Get-FileHash "$Staging/$Artifact" -Algorithm SHA256).Hash.ToLowerInvariant()
        Set-Content "$Staging/$Artifact.sha256" "$Hash  $Artifact" -Encoding ascii
        $Dist = New-Item -ItemType Directory "$ProjectRoot/target/dist" -Force
        Move-Item "$Staging/$Artifact", "$Staging/$Artifact.sha256" $Dist -Force
        Write-Host "Built target/dist/$Artifact"
    } finally {
        if (Test-Path $Staging) { Remove-Item $Staging -Recurse -Force }
    }
} finally { Pop-Location }
