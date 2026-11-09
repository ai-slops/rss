param(
    [string]$WorkRoot = (Join-Path $PSScriptRoot '..\var\windows-build'),
    [string]$CMake = 'cmake.exe'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$env:MSBUILDDISABLENODEREUSE = '1'
$Root = Split-Path $PSScriptRoot -Parent
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
New-Item $WorkRoot -ItemType Directory -Force | Out-Null

# Run in a Windows x64 MSVC 2022 build environment. These are the official
# OBS 33.0.1 dependencies; do not mix their Qt/FFmpeg ABI with older OBS builds.
$Version = '33.0.1'
$DepsVersion = '2026-10-23'
$Cache = Join-Path $WorkRoot 'downloads'
New-Item $Cache -ItemType Directory -Force | Out-Null
function FetchVerified($Name, $Url, $Hash) {
    $File = Join-Path $Cache $Name
    if (!(Test-Path $File)) {
        Invoke-WebRequest -UseBasicParsing $Url -OutFile "$File.partial"
        if ((Get-FileHash "$File.partial" -Algorithm SHA256).Hash -ne $Hash) {
            throw "SHA-256 mismatch: $Name"
        }
        Move-Item "$File.partial" $File
    }
    if ((Get-FileHash $File -Algorithm SHA256).Hash -ne $Hash) {
        throw "Cached SHA-256 mismatch: $Name"
    }
    return $File
}
function RunCMake([string[]]$Arguments) {
    & $CMake @Arguments
    if ($LASTEXITCODE) { throw "CMake failed: $($Arguments -join ' ')" }
}

$PrebuiltHash = '7508972a867a0ef68c26cd8bdba61fd999edd0a92fa9eb9585774de9bf51bfbe'
$QtHash = '78cd7a138464bb6edcdd3b85fcd63a5a7c69e369629963f3bc395bab74fb3f47'
$Deps = FetchVerified "windows-deps-$DepsVersion-x64.zip" `
    "https://github.com/obsproject/obs-deps/releases/download/$DepsVersion/windows-deps-$DepsVersion-x64.zip" $PrebuiltHash
$Qt = FetchVerified "windows-deps-qt6-$DepsVersion-x64.zip" `
    "https://github.com/obsproject/obs-deps/releases/download/$DepsVersion/windows-deps-qt6-$DepsVersion-x64.zip" $QtHash
$Source = FetchVerified "$Version.zip" `
    "https://github.com/obsproject/obs-studio/archive/refs/tags/$Version.zip" `
    '0f228b960c099c6f6bca9f09a0c3516f14fdf1dd96e1d4702145c5d3ba8cba31'
$Obs = FetchVerified "OBS-Studio-$Version-Windows-x64.zip" `
    "https://github.com/obsproject/obs-studio/releases/download/$Version/OBS-Studio-$Version-Windows-x64.zip" `
    '0d9436451c2d772b33428e3fdae9776f071895c335de17283176306fb72651d3'

$DepsRoot = Join-Path $WorkRoot 'deps'
$QtRoot = Join-Path $WorkRoot 'qt'
$ObsRoot = Join-Path $WorkRoot 'obs'
if (!(Test-Path "$DepsRoot\share\obs-deps\VERSION")) { Expand-Archive $Deps $DepsRoot -Force }
if (!(Test-Path "$QtRoot\lib\cmake\Qt6\Qt6Config.cmake")) { Expand-Archive $Qt $QtRoot -Force }
$ObsSource = Join-Path $WorkRoot "obs-studio-$Version"
if (!(Test-Path "$ObsSource\CMakeLists.txt")) { Expand-Archive $Source $WorkRoot -Force }
if (!(Test-Path "$ObsRoot\bin\64bit\obs64.exe")) { Expand-Archive $Obs $ObsRoot -Force }
if ((Get-AuthenticodeSignature "$ObsRoot\bin\64bit\obs64.exe").Status -ne 'Valid') {
    throw 'Official OBS executable signature is invalid'
}

New-Item "$ObsSource\.deps" -ItemType Directory -Force | Out-Null
[IO.File]::WriteAllText("$ObsSource\.deps\.dependency_prebuilt_windows-x64.sha256", $PrebuiltHash)
[IO.File]::WriteAllText("$ObsSource\.deps\.dependency_qt6_windows-x64.sha256", $QtHash)
$QtLink = "$ObsSource\.deps\obs-deps-qt6-$DepsVersion-x64"
if (!(Test-Path $QtLink)) { New-Item $QtLink -ItemType Junction -Target $QtRoot | Out-Null }
$SdkBuild = Join-Path $WorkRoot 'obs-sdk-build'
$Sdk = Join-Path $WorkRoot 'sdk'
RunCMake -Arguments @('-S', $ObsSource, '-B', $SdkBuild, '-G', 'Visual Studio 17 2022', '-A', 'x64',
    '-DENABLE_CORE_MODULES=OFF', '-DENABLE_FRONTEND=OFF', '-DENABLE_BROWSER=OFF', '-DENABLE_SCRIPTING=OFF',
    "-DOBS_VERSION_OVERRIDE=$Version", "-DCMAKE_PREFIX_PATH=$DepsRoot;$QtRoot")
RunCMake -Arguments @('--build', $SdkBuild, '--target', 'obs-frontend-api', '--config', 'Release', '--parallel', '6')
RunCMake -Arguments @('--install', $SdkBuild, '--component', 'Development', '--config', 'Release', '--prefix', $Sdk)

$Build = Join-Path $WorkRoot 'plugin-build'
RunCMake -Arguments @('-S', "$Root\integrations\obs-live-production", '-B', $Build,
    '-G', 'Visual Studio 17 2022', '-A', 'x64', '-DBUILD_TESTING=ON',
    "-DCMAKE_PREFIX_PATH=$Sdk;$DepsRoot;$QtRoot")
RunCMake -Arguments @('--build', $Build, '--config', 'Release', '--parallel', '6')
$OldPath = $env:PATH
try {
    $env:PATH = "$ObsRoot\bin\64bit;$OldPath"
    $CTest = Join-Path (Split-Path (Get-Command $CMake).Source -Parent) 'ctest.exe'
    & $CTest --test-dir $Build -C Release --output-on-failure
    if ($LASTEXITCODE) { throw 'Windows private state tests failed' }
} finally { $env:PATH = $OldPath }
$Stage = Join-Path $WorkRoot 'stage'
RunCMake -Arguments @('--install', $Build, '--config', 'Release', '--prefix', $Stage)
$Package = Join-Path $WorkRoot 'package\obs-live-production'
New-Item "$Package\bin\64bit" -ItemType Directory -Force | Out-Null
Copy-Item "$Stage\bin\64bit\*" "$Package\bin\64bit" -Force
Copy-Item "$DepsRoot\bin\srt.dll" "$Package\bin\64bit" -Force
New-Item "$Package\bin\64bit\tls" -ItemType Directory -Force | Out-Null
Copy-Item "$QtRoot\plugins\tls\qschannelbackend.dll" "$Package\bin\64bit\tls" -Force
New-Item "$Package\data" -ItemType Directory -Force | Out-Null
Copy-Item "$Stage\data\obs-plugins\obs-live-production\*" "$Package\data" -Recurse -Force
New-Item "$Package\data\licenses" -ItemType Directory -Force | Out-Null
Copy-Item "$DepsRoot\licenses\srt\LICENSE" "$Package\data\licenses\srt-LICENSE.txt" -Force
New-Item "$Package\data\licenses\qt6" -ItemType Directory -Force | Out-Null
Copy-Item "$QtRoot\licenses\qt6\*" "$Package\data\licenses\qt6" -Force
Compress-Archive $Package "$WorkRoot\obs-live-production-obs33-windows-x64.zip" -Force
Write-Host "Windows package: $WorkRoot\obs-live-production-obs33-windows-x64.zip"
