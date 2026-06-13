# นโยบายความปลอดภัย

## การรายงาน

หากพบช่องโหว่ กรุณาอย่าเปิดเผย access token, refresh token หรือไฟล์ `auth.json` ใน GitHub Issue

ให้รายงานผ่าน GitHub Security Advisory ของ repository นี้ พร้อมขั้นตอนทำซ้ำ ผลกระทบ และรุ่นที่พบปัญหา

## ขอบเขตข้อมูล

Codex Switch เก็บ credential ใน `CODEX_HOME` ของแต่ละโปรไฟล์บนเครื่องผู้ใช้ และส่ง access token เฉพาะไปยัง endpoint ของ OpenAI เมื่อผู้ใช้เปิด Live quota

โปรเจกต์ไม่มี telemetry และไม่มี server กลางของผู้พัฒนา
