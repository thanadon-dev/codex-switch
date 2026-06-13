$ErrorActionPreference = "Stop"

foreach ($command in @("node", "npm", "cargo", "rustc")) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "ไม่พบ $command กรุณาติดตั้งเครื่องมือที่ระบุใน README"
    }
}

npm install
Write-Host "เตรียมโปรเจกต์เรียบร้อยแล้ว ใช้ npm run tauri dev เพื่อเริ่มพัฒนา"
