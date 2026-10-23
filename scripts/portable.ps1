# Packs a Windows release build into a zip that runs without installing: the executable, the
# third-party notices and a short readme, in one folder. Run after `npx tauri build`; the
# release workflow does the same. Prints the zip's path.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src-tauri/target' }
$exe = Join-Path $target 'release/DS2SaveManager.exe'
if (-not (Test-Path $exe)) { throw "$exe is missing; run npx tauri build first." }

$out = Join-Path $target 'release/bundle/portable'
$stage = Join-Path $out 'DS2 Save Manager'
$zip = Join-Path $out "DS2 Save Manager_${version}_x64-portable.zip"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage
Copy-Item (Join-Path $root 'NOTICE') (Join-Path $stage 'NOTICE.txt')
$readme = @"
DS2 Save Manager $version (portable)

Run DS2SaveManager.exe; nothing is installed. It needs the Microsoft Edge WebView2 Runtime,
which Windows 10 and 11 already have. If the window does not open, install the runtime from
https://developer.microsoft.com/microsoft-edge/webview2/

Settings are kept where the installed version keeps them (%APPDATA%\com.mert.ds2savemanager),
so both share them. Backups sit next to each save, in its "DS2 Save Manager Backups" folder.

---

DS2 Save Manager $version (taşınabilir)

DS2SaveManager.exe dosyasını çalıştırın; hiçbir şey kurulmaz. Microsoft Edge WebView2
Runtime gerekir; Windows 10 ve 11'de zaten vardır. Pencere açılmazsa çalışma zamanını
https://developer.microsoft.com/microsoft-edge/webview2/ adresinden kurun.

Ayarlar kurulu sürümün kullandığı yerde tutulur (%APPDATA%\com.mert.ds2savemanager); ikisi
aynı ayarları paylaşır. Yedekler her kaydın yanındaki "DS2 Save Manager Backups" klasöründedir.
"@
Set-Content -Path (Join-Path $stage 'README.txt') -Value $readme -Encoding utf8

Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item $stage -Recurse -Force
Write-Output $zip
