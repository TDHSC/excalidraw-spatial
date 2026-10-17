$ErrorActionPreference = 'Stop'

$Repo = if ($env:EXCAL_REPO) { $env:EXCAL_REPO } else { "TDHSC/excalidraw-spatial" }
$BinName = "excal.exe"
$InstallDir = if ($env:EXCAL_INSTALL_DIR) { $env:EXCAL_INSTALL_DIR } else { Join-Path $HOME ".local\bin" }
$Version = if ($env:EXCAL_VERSION) { $env:EXCAL_VERSION } else { "latest" }
$Archive = "excal-x86_64-pc-windows-msvc.zip"

if ($Version -eq "latest") {
    $Url = "https://github.com/$Repo/releases/latest/download/$Archive"
} else {
    $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    $Url = "https://github.com/$Repo/releases/download/$Tag/$Archive"
}

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("excal-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null

try {
    $ZipPath = Join-Path $TmpDir $Archive
    Write-Host "Downloading $BinName (x86_64-pc-windows-msvc) from $Url..."
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing

    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Path (Join-Path $TmpDir $BinName) -Destination (Join-Path $InstallDir $BinName) -Force

    Write-Host "Installed $BinName to $InstallDir\$BinName"

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not ($UserPath -split ";" | Where-Object { $_ -eq $InstallDir })) {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        $env:Path = "$env:Path;$InstallDir"
        Write-Host "Added $InstallDir to your User PATH."
    }
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
