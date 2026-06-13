# Design System

## Direction

เครื่องมือเดสก์ท็อปแบบ restrained ใช้รายการแนวนอนที่อ่านเร็วและพื้นที่ว่างพอเหมาะ หน้าต่างหลักต้องดูเหมือน utility ที่เชื่อถือได้ ไม่ใช่หน้า analytics ขนาดใหญ่

## Color

- Canvas: `oklch(0.98 0.004 255)`
- Surface: `oklch(1 0 0)`
- Ink: `oklch(0.22 0.02 255)`
- Muted: `oklch(0.49 0.02 255)`
- Border: `oklch(0.9 0.01 255)`
- Accent: `oklch(0.58 0.18 258)`
- Success: `oklch(0.58 0.14 155)`
- Warning: `oklch(0.68 0.15 72)`
- Danger: `oklch(0.58 0.2 28)`

Dark theme ใช้ neutral ที่ tint ไปทาง accent เล็กน้อยและรักษาความหมายของ semantic colors เดิม

## Typography

ใช้ `Inter`, `Segoe UI`, system-ui เพียง family เดียว ขนาดหลัก 13-14px หัวข้อหน้า 22px ตัวเลขโควตา 20px ใช้ tabular numerals กับเปอร์เซ็นต์และเวลา

## Layout

หน้าต่างเริ่มต้น 820x620px แถบหัวสูงประมาณ 72px เนื้อหาเป็น account list ไม่มี sidebar ในรุ่นแรก รายการบัญชีใช้เส้นคั่นแทนการ์ดลอย

## Components

- Button: สูง 34px radius 8px primary ใช้ accent, secondary ใช้ neutral fill
- Account row: grid สำหรับ identity, 5-hour, weekly และ actions
- Progress: สูง 6px radius เต็ม แสดง used percentage
- Status badge: ข้อความและจุดสถานะ ใช้สีเป็นข้อมูลเสริม
- Dialog: native dialog สำหรับเพิ่มโปรไฟล์และตั้งค่า

## Motion

ใช้ 160-200ms ease-out เฉพาะ hover, dialog และ progress update ปิด animation เมื่อ `prefers-reduced-motion: reduce`
