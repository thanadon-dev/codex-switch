$ErrorActionPreference = "Stop"

$repository = "thanadon-dev/codex-switch"
$headers = @{ "User-Agent" = "codex-switch-installer" }
$release = Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$repository/releases/latest"
$asset = $release.assets | Where-Object { $_.name -match "(?i)(setup|installer).*\.exe$" } | Select-Object -First 1

if (-not $asset) {
    throw "ไม่พบ Windows installer ใน GitHub Release ล่าสุด"
}

$installer = Join-Path $env:TEMP $asset.name
Write-Host "กำลังดาวน์โหลด Codex Switch $($release.tag_name)..."
Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $installer

Write-Host "กำลังติดตั้ง Codex Switch..."
$process = Start-Process -FilePath $installer -ArgumentList "/S" -Wait -PassThru
if ($process.ExitCode -ne 0) {
    throw "ติดตั้งไม่สำเร็จ รหัส $($process.ExitCode)"
}

Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue
Write-Host "ติดตั้ง Codex Switch เรียบร้อยแล้ว"
