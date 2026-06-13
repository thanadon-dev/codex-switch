import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import "./styles.css";

type Profile = {
  id: string;
  name: string;
  color: string;
  projectPath?: string;
  createdAt: number;
};

type ProfileView = {
  profile: Profile;
  signedIn: boolean;
  email?: string;
  plan?: string;
};

type QuotaWindow = {
  usedPercent: number;
  windowMinutes?: number;
  resetsAt?: number;
};

type QuotaSnapshot = {
  profileId: string;
  source: "live" | "local";
  plan?: string;
  primary?: QuotaWindow;
  secondary?: QuotaWindow;
  fetchedAt: number;
  stale: boolean;
  error?: string;
};

type Settings = {
  liveQuotaEnabled: boolean;
  refreshSeconds: number;
  minimizeToTray: boolean;
};

const state: {
  profiles: ProfileView[];
  quotas: Map<string, QuotaSnapshot>;
  settings: Settings;
  refreshing: Set<string>;
  notifiedLevels: Map<string, number>;
  timer?: number;
} = {
  profiles: [],
  quotas: new Map(),
  settings: { liveQuotaEnabled: false, refreshSeconds: 60, minimizeToTray: true },
  refreshing: new Set(),
  notifiedLevels: new Map(),
};

const app = document.querySelector<HTMLDivElement>("#app")!;

const icons = {
  refresh: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 4v7h-7"/></svg>`,
  plus: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>`,
  play: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 11 7-11 7V5Z"/></svg>`,
  more: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3v-4h.09A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3h4v.09A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 .6 1 1.7 1.7 0 0 0 1.1.4H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z"/></svg>`,
};

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#039;", '"': "&quot;",
  })[character]!);
}

function formatPlan(plan?: string): string {
  if (!plan) return "ไม่ทราบแผน";
  return plan.charAt(0).toUpperCase() + plan.slice(1);
}

function formatAgo(timestamp: number): string {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - timestamp));
  if (seconds < 10) return "เมื่อสักครู่";
  if (seconds < 60) return `${seconds} วินาทีที่แล้ว`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} นาทีที่แล้ว`;
  return `${Math.floor(seconds / 3600)} ชั่วโมงที่แล้ว`;
}

function formatReset(timestamp?: number): string {
  if (!timestamp) return "ไม่ทราบเวลารีเซ็ต";
  const seconds = Math.max(0, timestamp - Math.floor(Date.now() / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours >= 24) return `รีเซ็ตใน ${Math.floor(hours / 24)} วัน ${hours % 24} ชม.`;
  return `รีเซ็ตใน ${hours} ชม. ${minutes} นาที`;
}

function quotaClass(percent: number): string {
  if (percent >= 90) return "danger";
  if (percent >= 70) return "warning";
  return "normal";
}

function quotaBlock(label: string, quota?: QuotaWindow, error?: string): string {
  if (!quota) {
    return `<div class="quota-cell empty"><span>${label}</span><strong>—</strong><small>${error ? "ไม่มีข้อมูล" : "กำลังรอข้อมูล"}</small></div>`;
  }
  const used = Math.max(0, Math.min(100, quota.usedPercent));
  return `<div class="quota-cell">
    <div class="quota-heading"><span>${label}</span><strong>${Math.round(used)}%</strong></div>
    <div class="progress" aria-label="ใช้โควตา ${Math.round(used)} เปอร์เซ็นต์"><i class="${quotaClass(used)}" style="width:${used}%"></i></div>
    <small>${formatReset(quota.resetsAt)}</small>
  </div>`;
}

function renderProfile(item: ProfileView): string {
  const quota = state.quotas.get(item.profile.id);
  const sourceLabel = quota?.source === "live" ? "Live" : quota?.source === "local" ? "Local" : "Waiting";
  const sourceClass = quota?.source === "live" && !quota.stale ? "live" : "local";
  const isRefreshing = state.refreshing.has(item.profile.id);
  return `<article class="profile-row" data-id="${item.profile.id}">
    <div class="identity">
      <span class="avatar" style="--profile-color:${escapeHtml(item.profile.color)}">${escapeHtml(item.profile.name.slice(0, 1).toUpperCase())}</span>
      <div>
        <div class="name-line"><strong>${escapeHtml(item.profile.name)}</strong><span>${formatPlan(quota?.plan ?? item.plan)}</span></div>
        <small>${escapeHtml(item.email ?? (item.signedIn ? "เข้าสู่ระบบแล้ว" : "ยังไม่ได้เข้าสู่ระบบ"))}</small>
      </div>
    </div>
    ${quotaBlock("5 ชั่วโมง", quota?.primary, quota?.error)}
    ${quotaBlock("รายสัปดาห์", quota?.secondary, quota?.error)}
    <div class="row-actions">
      <button class="icon-button refresh-one ${isRefreshing ? "spinning" : ""}" title="รีเฟรช" aria-label="รีเฟรช ${escapeHtml(item.profile.name)}">${icons.refresh}</button>
      <button class="launch-button" ${item.signedIn ? "" : "data-login=\"true\""}>${icons.play}<span>${item.signedIn ? "เปิด" : "เข้าสู่ระบบ"}</span></button>
      <button class="icon-button more-button" title="ตัวเลือก" aria-label="ตัวเลือก ${escapeHtml(item.profile.name)}">${icons.more}</button>
      <div class="profile-menu" popover>
        <button data-action="edit">แก้ไขโปรไฟล์</button>
        <button data-action="login">เข้าสู่ระบบใหม่</button>
        <button data-action="delete" class="danger-text">ลบโปรไฟล์</button>
      </div>
    </div>
    <div class="row-meta"><span class="source ${sourceClass}">${sourceLabel}</span><span>${quota ? formatAgo(quota.fetchedAt) : "ยังไม่รีเฟรช"}</span>${quota?.error ? `<span class="error-text" title="${escapeHtml(quota.error)}">เกิดข้อผิดพลาด</span>` : ""}</div>
  </article>`;
}

function render(): void {
  app.innerHTML = `<main class="shell">
    <header class="topbar">
      <div><h1>Codex Switch</h1><p>โควตาและโปรไฟล์ Codex ของคุณ</p></div>
      <div class="top-actions">
        <button class="button secondary" id="refresh-all">${icons.refresh}<span>รีเฟรช</span></button>
        <button class="button primary" id="add-profile">${icons.plus}<span>เพิ่มโปรไฟล์</span></button>
        <button class="icon-button" id="open-settings" aria-label="ตั้งค่า" title="ตั้งค่า">${icons.settings}</button>
      </div>
    </header>
    ${!state.settings.liveQuotaEnabled ? `<aside class="privacy-banner"><div><strong>Live quota ยังปิดอยู่</strong><p>เปิดเพื่อดึงโควตาล่าสุดจาก ChatGPT โดยตรง หรือใช้ข้อมูล Local ที่อาจล่าช้า</p></div><button class="button secondary" id="enable-live">เปิด Live quota</button></aside>` : ""}
    <section class="list-header"><span>บัญชี</span><span>โควตา</span><span>การทำงาน</span></section>
    <section class="profile-list">
      ${state.profiles.length ? state.profiles.map(renderProfile).join("") : `<div class="empty-state"><div class="empty-mark">CS</div><h2>ยังไม่มีโปรไฟล์</h2><p>นำเข้าบัญชี Codex ปัจจุบัน หรือสร้างโปรไฟล์ใหม่เพื่อเข้าสู่ระบบ</p><button class="button primary" id="empty-add">${icons.plus}<span>เพิ่มโปรไฟล์แรก</span></button></div>`}
    </section>
    <footer><span>${state.profiles.length} โปรไฟล์</span><span>รีเฟรชอัตโนมัติทุก ${state.settings.refreshSeconds} วินาทีเมื่อเปิดหน้าต่าง</span></footer>
  </main>
  ${dialogs()}
  <div id="toast-region" aria-live="polite"></div>`;
  bindEvents();
}

function dialogs(): string {
  return `<dialog id="profile-dialog">
    <form method="dialog" id="profile-form">
      <div class="dialog-heading"><div><h2 id="profile-dialog-title">เพิ่มโปรไฟล์</h2><p>แต่ละโปรไฟล์เก็บข้อมูลใน CODEX_HOME แยกกัน</p></div><button type="button" class="icon-button close-dialog" aria-label="ปิด">×</button></div>
      <input type="hidden" id="profile-id" />
      <label>ชื่อโปรไฟล์<input id="profile-name" required maxlength="40" placeholder="เช่น งาน หรือ ส่วนตัว" /></label>
      <label>โฟลเดอร์โปรเจกต์<input id="project-path" placeholder="D:\\Projects\\my-app (ไม่บังคับ)" /></label>
      <fieldset><legend>สีประจำโปรไฟล์</legend><div class="colors">${["#5965d8", "#16866b", "#c16a24", "#b34d70", "#596273"].map((color, index) => `<label><input type="radio" name="profile-color" value="${color}" ${index === 0 ? "checked" : ""}/><i style="background:${color}"></i></label>`).join("")}</div></fieldset>
      <label class="check-row" id="import-row"><input type="checkbox" id="import-current" checked /><span><strong>นำเข้าบัญชีปัจจุบัน</strong><small>คัดลอก auth.json จาก Codex ที่ใช้อยู่ในเครื่อง</small></span></label>
      <div class="dialog-actions"><button type="button" class="button secondary close-dialog">ยกเลิก</button><button type="submit" class="button primary" id="save-profile">บันทึก</button></div>
    </form>
  </dialog>
  <dialog id="settings-dialog">
    <form method="dialog" id="settings-form">
      <div class="dialog-heading"><div><h2>ตั้งค่า</h2><p>ควบคุมความสดของข้อมูลและการทำงานเบื้องหลัง</p></div><button type="button" class="icon-button close-dialog" aria-label="ปิด">×</button></div>
      <label class="check-row"><input type="checkbox" id="live-enabled" /><span><strong>Live quota</strong><small>ส่ง access token ไปยัง endpoint ของ OpenAI เพื่ออ่านโควตา</small></span></label>
      <label>ช่วงรีเฟรช<select id="refresh-seconds"><option value="30">30 วินาที</option><option value="60">1 นาที</option><option value="120">2 นาที</option><option value="300">5 นาที</option></select></label>
      <div class="notice"><strong>ข้อมูลสำคัญ</strong><p>Live quota ใช้ private endpoint ที่อาจเปลี่ยนได้ Codex Switch จะ fallback ไปยังข้อมูล Local เมื่อเรียกไม่สำเร็จ</p></div>
      <div class="dialog-actions"><button type="button" class="button secondary close-dialog">ยกเลิก</button><button type="submit" class="button primary" id="save-settings">บันทึก</button></div>
    </form>
  </dialog>`;
}

function dialog(id: string): HTMLDialogElement {
  return document.querySelector<HTMLDialogElement>(`#${id}`)!;
}

function openProfileDialog(profile?: ProfileView): void {
  const target = dialog("profile-dialog");
  target.querySelector<HTMLHeadingElement>("#profile-dialog-title")!.textContent = profile ? "แก้ไขโปรไฟล์" : "เพิ่มโปรไฟล์";
  target.querySelector<HTMLInputElement>("#profile-id")!.value = profile?.profile.id ?? "";
  target.querySelector<HTMLInputElement>("#profile-name")!.value = profile?.profile.name ?? "";
  target.querySelector<HTMLInputElement>("#project-path")!.value = profile?.profile.projectPath ?? "";
  target.querySelector<HTMLElement>("#import-row")!.hidden = Boolean(profile);
  if (profile) {
    const color = target.querySelector<HTMLInputElement>(`input[name=profile-color][value="${profile.profile.color}"]`);
    if (color) color.checked = true;
  }
  target.showModal();
  setTimeout(() => target.querySelector<HTMLInputElement>("#profile-name")?.focus(), 0);
}

function showToast(message: string, type: "success" | "error" = "success"): void {
  const region = document.querySelector("#toast-region");
  if (!region) return;
  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.textContent = message;
  region.append(toast);
  window.setTimeout(() => toast.remove(), 3500);
}

async function refreshProfile(id: string): Promise<void> {
  if (state.refreshing.has(id)) return;
  state.refreshing.add(id);
  render();
  try {
    const quota = await invoke<QuotaSnapshot>("refresh_quota", {
      id,
      mode: state.settings.liveQuotaEnabled ? "live" : "local",
    });
    state.quotas.set(id, quota);
    void notifyQuota(quota);
  } catch (error) {
    showToast(String(error), "error");
  } finally {
    state.refreshing.delete(id);
    render();
  }
}

async function refreshAll(): Promise<void> {
  await Promise.all(state.profiles.map((item) => refreshProfile(item.profile.id)));
}

function scheduleRefresh(): void {
  if (state.timer) window.clearInterval(state.timer);
  state.timer = window.setInterval(() => {
    if (!state.settings.liveQuotaEnabled && document.visibilityState === "visible") void refreshAll();
    else if (document.visibilityState === "visible") render();
  }, state.settings.refreshSeconds * 1000);
}

async function notifyQuota(quota: QuotaSnapshot): Promise<void> {
  const used = Math.max(quota.primary?.usedPercent ?? 0, quota.secondary?.usedPercent ?? 0);
  const level = used >= 95 ? 2 : used >= 80 ? 1 : 0;
  const previous = state.notifiedLevels.get(quota.profileId) ?? 0;
  state.notifiedLevels.set(quota.profileId, level);
  if (level === 0 || level <= previous) return;
  let granted = await isPermissionGranted();
  if (!granted) granted = (await requestPermission()) === "granted";
  if (!granted) return;
  const profile = state.profiles.find((item) => item.profile.id === quota.profileId);
  sendNotification({
    title: `โควตา ${profile?.profile.name ?? "Codex"} ใกล้เต็ม`,
    body: `ใช้งานแล้ว ${Math.round(used)}% เปิด Codex Switch เพื่อตรวจสอบเวลารีเซ็ต`,
  });
}

async function reloadProfiles(): Promise<void> {
  state.profiles = await invoke<ProfileView[]>("list_profiles");
  render();
}

function bindEvents(): void {
  document.querySelectorAll<HTMLButtonElement>(".close-dialog").forEach((button) => {
    button.addEventListener("click", () => button.closest<HTMLDialogElement>("dialog")?.close());
  });
  document.querySelector("#add-profile")?.addEventListener("click", () => openProfileDialog());
  document.querySelector("#empty-add")?.addEventListener("click", () => openProfileDialog());
  document.querySelector("#refresh-all")?.addEventListener("click", () => void refreshAll());
  document.querySelector("#enable-live")?.addEventListener("click", () => openSettings());
  document.querySelector("#open-settings")?.addEventListener("click", () => openSettings());

  document.querySelectorAll<HTMLElement>(".profile-row").forEach((row) => {
    const id = row.dataset.id!;
    const profile = state.profiles.find((item) => item.profile.id === id)!;
    row.querySelector(".refresh-one")?.addEventListener("click", () => void refreshProfile(id));
    row.querySelector(".launch-button")?.addEventListener("click", async (event) => {
      try {
        const login = (event.currentTarget as HTMLElement).dataset.login === "true";
        await invoke(login ? "login_profile" : "launch_profile", { id });
        showToast(login ? "เปิดหน้าต่างเข้าสู่ระบบแล้ว" : `เปิด ${profile.profile.name} แล้ว`);
      } catch (error) { showToast(String(error), "error"); }
    });
    const menu = row.querySelector<HTMLElement>(".profile-menu")!;
    row.querySelector(".more-button")?.addEventListener("click", () => menu.togglePopover());
    menu.querySelector('[data-action="edit"]')?.addEventListener("click", () => openProfileDialog(profile));
    menu.querySelector('[data-action="login"]')?.addEventListener("click", async () => {
      await invoke("login_profile", { id });
      showToast("เปิดหน้าต่างเข้าสู่ระบบแล้ว");
    });
    menu.querySelector('[data-action="delete"]')?.addEventListener("click", async () => {
      if (!confirm(`ลบโปรไฟล์ ${profile.profile.name} และข้อมูลใน CODEX_HOME หรือไม่`)) return;
      await invoke("delete_profile", { id });
      state.quotas.delete(id);
      await reloadProfiles();
      showToast("ลบโปรไฟล์แล้ว");
    });
  });

  document.querySelector<HTMLFormElement>("#profile-form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const id = form.querySelector<HTMLInputElement>("#profile-id")!.value;
    const name = form.querySelector<HTMLInputElement>("#profile-name")!.value;
    const projectPath = form.querySelector<HTMLInputElement>("#project-path")!.value || null;
    const color = form.querySelector<HTMLInputElement>('input[name="profile-color"]:checked')!.value;
    try {
      if (id) {
        await invoke("update_profile", { input: { id, name, color, projectPath } });
      } else {
        const importCurrent = form.querySelector<HTMLInputElement>("#import-current")!.checked;
        await invoke("create_profile", { input: { name, color, projectPath, importCurrent } });
      }
      dialog("profile-dialog").close();
      await reloadProfiles();
      void refreshAll();
      showToast(id ? "อัปเดตโปรไฟล์แล้ว" : "เพิ่มโปรไฟล์แล้ว");
    } catch (error) { showToast(String(error), "error"); }
  });

  document.querySelector<HTMLFormElement>("#settings-form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    state.settings.liveQuotaEnabled = form.querySelector<HTMLInputElement>("#live-enabled")!.checked;
    state.settings.refreshSeconds = Number(form.querySelector<HTMLSelectElement>("#refresh-seconds")!.value);
    await invoke("save_settings", { settings: state.settings });
    dialog("settings-dialog").close();
    render();
    scheduleRefresh();
    void refreshAll();
    showToast("บันทึกการตั้งค่าแล้ว");
  });
}

function openSettings(): void {
  const target = dialog("settings-dialog");
  target.querySelector<HTMLInputElement>("#live-enabled")!.checked = state.settings.liveQuotaEnabled;
  target.querySelector<HTMLSelectElement>("#refresh-seconds")!.value = String(state.settings.refreshSeconds);
  target.showModal();
}

async function initialize(): Promise<void> {
  try {
    state.settings = await invoke<Settings>("get_settings");
    await reloadProfiles();
    const cached = await invoke<QuotaSnapshot[]>("get_cached_quotas");
    cached.forEach((quota) => state.quotas.set(quota.profileId, quota));
    render();
    await listen<QuotaSnapshot>("quota-updated", ({ payload }) => {
      state.quotas.set(payload.profileId, payload);
      void notifyQuota(payload);
      if (document.visibilityState === "visible") render();
    });
    scheduleRefresh();
    if (state.profiles.length) void refreshAll();
  } catch (error) {
    app.innerHTML = `<div class="fatal"><h1>เปิด Codex Switch ไม่สำเร็จ</h1><p>${escapeHtml(String(error))}</p></div>`;
  }
}

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible" && state.profiles.length) void refreshAll();
});

void initialize();
