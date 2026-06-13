# Codex Switch

Codex Switch เป็นแอปเดสก์ท็อปขนาดเล็กสำหรับติดตามโควตาและจัดการ Codex CLI หลายบัญชีบน Windows แต่ละบัญชีใช้ `CODEX_HOME` แยกจากกัน จึงเปิดพร้อมกันได้โดยไม่ต้องสลับ `auth.json` กลาง

## ความสามารถ

- แสดงโควตา 5 ชั่วโมงและรายสัปดาห์
- รีเฟรชแบบ Live และ fallback ไปยังข้อมูล Local อัตโนมัติ
- ทำงานเบื้องหลังผ่าน system tray
- ลดความถี่การรีเฟรชอัตโนมัติเมื่อซ่อนหน้าต่างเพื่อลดการใช้ทรัพยากร
- แจ้งเตือนเมื่อใช้งานถึง 80% และ 95%
- สร้างหรือนำเข้าบัญชี Codex ปัจจุบัน
- เปิด Codex CLI หลายบัญชีพร้อมกัน
- จำโฟลเดอร์โปรเจกต์ของแต่ละบัญชี
- รองรับ Light และ Dark mode ตามระบบ

## ติดตั้ง

เปิด PowerShell แล้วรันคำสั่งเดียว:

```powershell
irm https://raw.githubusercontent.com/thanadon-dev/codex-switch/main/install.ps1 | iex
```

สคริปต์จะดาวน์โหลด installer รุ่นล่าสุดจาก GitHub Releases และติดตั้งให้โดยอัตโนมัติ

สามารถดาวน์โหลดไฟล์ติดตั้งด้วยตนเองได้จากหน้า [Releases](https://github.com/thanadon-dev/codex-switch/releases)

## วิธีใช้งาน

1. เปิด Codex Switch
2. เลือก `เพิ่มโปรไฟล์`
3. นำเข้าบัญชี Codex ปัจจุบัน หรือสร้างโปรไฟล์เปล่าแล้วเข้าสู่ระบบ
4. เปิด `Live quota` ในหน้าตั้งค่า หากต้องการข้อมูลล่าสุด
5. กด `เปิด` เพื่อเริ่ม Codex CLI ด้วยบัญชีและโฟลเดอร์ของโปรไฟล์นั้น

## แหล่งข้อมูลโควตา

### Live

เรียก `https://chatgpt.com/backend-api/wham/usage` ด้วย access token ของบัญชีโดยตรง ข้อมูลมีความสดกว่า แต่ endpoint นี้ไม่ได้เป็น public API และอาจเปลี่ยนได้ในอนาคต ผู้ใช้ต้องเปิดโหมดนี้ด้วยตนเอง

Codex Switch ไม่ส่ง token ไปยัง server ของผู้พัฒนา คำขอถูกส่งจากเครื่องผู้ใช้ไปยัง OpenAI โดยตรง

### Local

อ่านข้อมูลจาก `sessions/**/rollout-*.jsonl` ภายใน `CODEX_HOME` ของโปรไฟล์ ไม่ส่ง token ผ่านเครือข่ายเพิ่มเติม แต่ข้อมูลอาจล่าช้าหรือไม่มีค่า `rate_limits` ใน Codex บางรุ่น

## ตำแหน่งข้อมูล

ข้อมูลแอปถูกเก็บใน:

```text
%APPDATA%\dev.thanadon.codex-switch\
└── profiles\
    └── <profile-id>\
        ├── auth.json
        ├── profile.json
        ├── quota-cache.json
        └── sessions\
```

ไฟล์ `auth.json` มีข้อมูลรับรองสิทธิ์ ควรป้องกันโฟลเดอร์ผู้ใช้และไม่ส่งไฟล์นี้ให้ผู้อื่น

## พัฒนาจาก source

ต้องมี Node.js 20 ขึ้นไป, Rust stable, Microsoft C++ Build Tools และ WebView2

```powershell
git clone https://github.com/thanadon-dev/codex-switch.git
cd codex-switch
npm run setup
npm run tauri dev
```

คำสั่งตรวจสอบ:

```powershell
npm run check
npm run tauri build
```

## เทคโนโลยี

- Tauri 2
- Rust
- TypeScript แบบไม่ใช้ frontend framework
- Vite

## ข้อจำกัด

- รุ่นแรกเน้น Windows
- Live quota พึ่งพา private endpoint ของ ChatGPT
- การเข้าสู่ระบบและเปิด Codex ต้องติดตั้ง Codex CLI และเรียกคำสั่ง `codex` ได้จาก `PATH`
- Codex Switch ไม่ได้พัฒนา รับรอง หรือสนับสนุนโดย OpenAI

## ความปลอดภัย

กรุณารายงานปัญหาด้านความปลอดภัยตาม [SECURITY.md](SECURITY.md) และหลีกเลี่ยงการแนบ token หรือ `auth.json` ใน issue

## License

[MIT](LICENSE)
