param(
  [Parameter(Mandatory = $true)]
  [string]$Target,

  [string]$Version = "",

  [string]$AppName = "Rovdex"
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$DistDir = Join-Path $RootDir "dist"
$BuildDir = Join-Path $DistDir "msi-$Target"
$ExeSource = Join-Path $RootDir "target\$Target\release\rovdex-cli.exe"
$IconSource = Join-Path $RootDir "assets\icons\Rovdex.ico"
$MsiWxsPath = Join-Path $BuildDir "Rovdex.msi.wxs"
$BundleWxsPath = Join-Path $BuildDir "Rovdex.bundle.wxs"

if ([string]::IsNullOrWhiteSpace($Version)) {
  $cargoToml = Get-Content (Join-Path $RootDir "Cargo.toml")
  $inWorkspacePackage = $false
  foreach ($line in $cargoToml) {
    if ($line -match '^\[workspace\.package\]') {
      $inWorkspacePackage = $true
      continue
    }
    if ($inWorkspacePackage -and $line -match '^\[') {
      $inWorkspacePackage = $false
    }
    if ($inWorkspacePackage -and $line -match '^\s*version\s*=\s*"(.+)"') {
      $Version = $Matches[1]
      break
    }
  }
}

if (!(Test-Path $ExeSource)) {
  throw "missing Windows executable: $ExeSource"
}

if (!(Test-Path $IconSource)) {
  throw "missing Windows icon: $IconSource"
}

$null = New-Item -ItemType Directory -Force -Path $BuildDir
Copy-Item $ExeSource (Join-Path $BuildDir "Rovdex.exe") -Force
Copy-Item $IconSource (Join-Path $BuildDir "Rovdex.ico") -Force

$arch = switch ($Target) {
  "x86_64-pc-windows-msvc" { "x64" }
  "aarch64-pc-windows-msvc" { "arm64" }
  default { throw "unsupported Windows MSI target: $Target" }
}

$upgradeCode = switch ($Target) {
  "x86_64-pc-windows-msvc" { "19E20EAF-3D3E-4B4A-9A0C-3E3D84D6A001" }
  "aarch64-pc-windows-msvc" { "19E20EAF-3D3E-4B4A-9A0C-3E3D84D6A002" }
}

$bundleUpgradeCode = switch ($Target) {
  "x86_64-pc-windows-msvc" { "B27818E3-D4E1-4D0F-95EF-0D4A9A52A001" }
  "aarch64-pc-windows-msvc" { "B27818E3-D4E1-4D0F-95EF-0D4A9A52A002" }
}

$mainComponentGuid = switch ($Target) {
  "x86_64-pc-windows-msvc" { "A73EA0A4-9E16-47D1-8C64-C9A7BF4E1001" }
  "aarch64-pc-windows-msvc" { "A73EA0A4-9E16-47D1-8C64-C9A7BF4E1011" }
}

$shortcutComponentGuid = switch ($Target) {
  "x86_64-pc-windows-msvc" { "A73EA0A4-9E16-47D1-8C64-C9A7BF4E1002" }
  "aarch64-pc-windows-msvc" { "A73EA0A4-9E16-47D1-8C64-C9A7BF4E1012" }
}

$msiName = switch ($Target) {
  "x86_64-pc-windows-msvc" { "Rovdex-Windows-x64.msi" }
  "aarch64-pc-windows-msvc" { "Rovdex-Windows-arm64.msi" }
}

$installerExeName = switch ($Target) {
  "x86_64-pc-windows-msvc" { "Rovdex-Windows-x64.exe" }
  "aarch64-pc-windows-msvc" { "Rovdex-Windows-arm64.exe" }
}

$msiWxs = @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package
    Name="$AppName"
    Manufacturer="Rovdex"
    Version="$Version"
    UpgradeCode="$upgradeCode"
    Language="1033"
    Scope="perMachine"
    InstallerVersion="500">

    <SummaryInformation Description="$AppName Installer" Manufacturer="Rovdex" />
    <MediaTemplate EmbedCab="yes" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of $AppName is already installed." />

    <Icon Id="AppIcon.ico" SourceFile="Rovdex.ico" />
    <Property Id="ARPPRODUCTICON" Value="AppIcon.ico" />

    <StandardDirectory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="$AppName">
        <Component Id="MainExecutable" Guid="$mainComponentGuid">
          <File Id="RovdexExe" Source="Rovdex.exe" KeyPath="yes" />
        </Component>
      </Directory>
    </StandardDirectory>

    <StandardDirectory Id="ProgramMenuFolder">
      <Directory Id="AppProgramMenuDir" Name="$AppName">
        <Component Id="ProgramMenuShortcut" Guid="$shortcutComponentGuid">
          <Shortcut
            Id="ApplicationStartMenuShortcut"
            Name="$AppName"
            Target="[INSTALLFOLDER]Rovdex.exe"
            WorkingDirectory="INSTALLFOLDER"
            Icon="AppIcon.ico"
            IconIndex="0" />
          <RemoveFolder Id="RemoveProgramMenuDir" On="uninstall" />
          <RegistryValue
            Root="HKCU"
            Key="Software\Rovdex"
            Name="installed"
            Type="integer"
            Value="1"
            KeyPath="yes" />
        </Component>
      </Directory>
    </StandardDirectory>

    <Feature Id="MainFeature" Title="$AppName" Level="1">
      <ComponentRef Id="MainExecutable" />
      <ComponentRef Id="ProgramMenuShortcut" />
    </Feature>
  </Package>
</Wix>
"@

Set-Content -Path $MsiWxsPath -Value $msiWxs -Encoding UTF8

$bundleWxs = @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"
     xmlns:bal="http://wixtoolset.org/schemas/v4/wxs/bal">
  <Bundle
    Name="$AppName"
    Manufacturer="Rovdex"
    Version="$Version"
    UpgradeCode="$bundleUpgradeCode"
    IconSourceFile="Rovdex.ico">
    <BootstrapperApplication>
      <bal:WixStandardBootstrapperApplication
        Theme="hyperlinkLicense"
        LicenseUrl="https://raw.githubusercontent.com/rovdex/rovdex/main/LICENSE" />
    </BootstrapperApplication>
    <Chain>
      <MsiPackage SourceFile="$msiName" />
    </Chain>
  </Bundle>
</Wix>
"@

Set-Content -Path $BundleWxsPath -Value $bundleWxs -Encoding UTF8

$outputMsi = Join-Path $DistDir $msiName
if (Test-Path $outputMsi) {
  Remove-Item $outputMsi -Force
}

$outputInstallerExe = Join-Path $DistDir $installerExeName
if (Test-Path $outputInstallerExe) {
  Remove-Item $outputInstallerExe -Force
}

Push-Location $BuildDir
try {
  wix build -arch $arch -o $outputMsi $MsiWxsPath
  if ($LASTEXITCODE -ne 0) {
    throw "wix build failed for MSI package"
  }

  wix build -arch $arch -ext WixToolset.Bal.wixext -o $outputInstallerExe $BundleWxsPath
  if ($LASTEXITCODE -ne 0) {
    throw "wix build failed for bundle installer"
  }
}
finally {
  Pop-Location
}

Write-Host "Created MSI: $outputMsi"
Write-Host "Created installer EXE: $outputInstallerExe"
