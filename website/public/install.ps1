# caldir installer
# Usage: powershell -c "irm https://caldir.org/install.ps1 | iex"

function Install-Caldir {
  $ErrorActionPreference = "Stop"
  $ProgressPreference = "SilentlyContinue"

  $repo = "t4t5/caldir"
  # Only x86_64 is built; Windows on ARM runs it through emulation.
  $target = "x86_64-pc-windows-msvc"
  $installDir = Join-Path $env:LOCALAPPDATA "Programs\caldir"

  # Windows PowerShell 5.1 may default to TLS 1.0, which GitHub rejects.
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

  Write-Host "Detecting platform: $target"

  # Fetch latest version from GitHub API
  # Try the API first, fall back to the releases redirect for rate-limited IPs
  $version = $null
  try {
    $version = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -UseBasicParsing).tag_name
  } catch {}

  if (-not $version) {
    try {
      $request = [Net.HttpWebRequest]::Create("https://github.com/$repo/releases/latest")
      $request.AllowAutoRedirect = $false
      $location = $request.GetResponse().Headers["Location"]
      if ($location) { $version = $location.Split("/")[-1] }
    } catch {}
  }

  if (-not $version) {
    throw "Could not determine latest version (GitHub API may be rate-limiting your IP)"
  }

  Write-Host "Latest version: $version"

  $archive = "caldir-$target.zip"
  $url = "https://github.com/$repo/releases/download/$version/$archive"

  $tmp = Join-Path ([IO.Path]::GetTempPath()) ("caldir-" + [IO.Path]::GetRandomFileName())
  New-Item -ItemType Directory -Path $tmp | Out-Null

  try {
    Write-Host "Downloading $url..."
    Invoke-WebRequest $url -OutFile (Join-Path $tmp $archive) -UseBasicParsing

    Write-Host "Extracting..."
    Expand-Archive (Join-Path $tmp $archive) -DestinationPath $tmp

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null

    # Install everything the release archive ships — the archive is the source
    # of truth for which binaries make up a caldir install.
    foreach ($exe in Get-ChildItem $tmp -Filter "caldir*.exe") {
      $dest = Join-Path $installDir $exe.Name
      # A running exe can't be deleted, but it can be renamed aside.
      if (Test-Path $dest) {
        try { Remove-Item $dest -Force } catch { Move-Item $dest "$dest.old" -Force }
      }
      Move-Item $exe.FullName $dest -Force
      Write-Host "  Installed $($exe.Name) to $dest"
    }
  } finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
  }

  Write-Host ""
  Write-Host "caldir $version installed successfully!"

  # Add install dir to the user PATH if missing
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if (($userPath -split ";") -notcontains $installDir) {
    $newPath = ("$userPath".TrimEnd(";") + ";" + $installDir).TrimStart(";")
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host ""
    Write-Host "Added $installDir to your PATH. Restart your terminal to start using caldir."
  }
  if (($env:Path -split ";") -notcontains $installDir) {
    $env:Path += ";$installDir"
  }
}

Install-Caldir
