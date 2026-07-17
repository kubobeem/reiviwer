#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{Color32, Margin, ProgressBar, Rect, RichText, Shape, Stroke, Vec2, ViewportCommand, pos2};
use std::time::{Duration, Instant};
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};
use windows::core::Interface;

fn main() -> eframe::Result {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(500));
        set_click_through(true);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_always_on_top()
            .with_decorations(false)
            .with_resizable(true)
            .with_inner_size([410.0, 275.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ReiView",
        options,
        Box::new(|_cc| Ok(Box::new(PcMonApp::new()))),
    )
}

fn set_click_through(enabled: bool) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::w;
    unsafe {
        let hwnd = FindWindowW(None, w!("ReiView")).unwrap_or_default();
        if hwnd.0.is_null() {
            return;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let new_ex = if enabled {
            ex | WS_EX_TRANSPARENT.0
        } else {
            ex & !WS_EX_TRANSPARENT.0
        };
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex as i32);
        let alpha = if enabled { 200 } else { 240 };
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

fn move_window() {
    use windows::Win32::Foundation::WPARAM;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::w;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ReleaseCapture() -> i32;
    }
    unsafe {
        let hwnd = FindWindowW(None, w!("ReiView")).unwrap_or_default();
        if hwnd.0.is_null() {
            return;
        }
        ReleaseCapture();
        let _ = SendMessageW(hwnd, WM_SYSCOMMAND, Some(WPARAM(0xF010 | 0x0002)), None);
    }
}

struct History {
    data: Vec<f32>,
    max: usize,
}

impl History {
    fn new(max: usize) -> Self {
        Self { data: Vec::with_capacity(max), max }
    }
    fn push(&mut self, val: f32) {
        if self.data.len() >= self.max {
            self.data.remove(0);
        }
        self.data.push(val);
    }
    fn data(&self) -> &[f32] {
        &self.data
    }
}

struct PcMonApp {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,

    cpu: f32,
    cpu_freq: u64,
    cores: Vec<f32>,
    cpu_temp: Option<f32>,
    cpu_history: History,

    mem_pct: f32,
    mem_label: String,

    swap_pct: f32,
    swap_label: String,

    vram_pct: f32,
    vram_label: String,

    disk_pct: f32,
    disk_label: String,

    net_rx: f64,
    net_tx: f64,
    prev_net_rx: u64,
    prev_net_tx: u64,
    prev_time: Instant,

    battery_pct: Option<u8>,
    battery_charging: bool,

    top_cpu: Vec<(String, f32)>,
    top_mem: Vec<(String, u64)>,

    tick: Instant,
    click_through: bool,
    last_content_size: Vec2,
    manual_resize: bool,
}

impl PcMonApp {
    fn new() -> Self {
        let mut system = System::new_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();

        system.refresh_cpu_usage();
        system.refresh_memory();

        let cpu = system.global_cpu_usage();
        let cores: Vec<f32> = system.cpus().iter().map(|c| c.cpu_usage()).collect();
        let cpu_freq = system.cpus().first().map_or(0, |c| c.frequency());
        let (cpu_temp, _) = cpu_temp_info(&components);
        let (mem_pct, mem_label) = mem_info(&system);
        let (swap_pct, swap_label) = swap_info(&system);
        let (vram_pct, vram_label) = vram_info();
        let (disk_pct, disk_label) = disk_info(&disks);
        let (rx, tx) = (total_rx(&networks), total_tx(&networks));
        let (battery_pct, battery_charging) = battery_info();
        let top_cpu = top_cpu_procs(&system);
        let top_mem = top_mem_procs(&system);

        Self {
            system, disks, networks, components,
            cpu, cpu_freq, cores, cpu_temp,
            cpu_history: History::new(60),
            mem_pct, mem_label,
            swap_pct, swap_label,
            vram_pct, vram_label,
            disk_pct, disk_label,
            net_rx: 0.0, net_tx: 0.0,
            prev_net_rx: rx, prev_net_tx: tx,
            prev_time: Instant::now(),
            battery_pct, battery_charging,
            top_cpu, top_mem,
            tick: Instant::now(),
            click_through: true,
            last_content_size: Vec2::ZERO,
            manual_resize: false,
        }
    }
}

impl eframe::App for PcMonApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        if now - self.tick >= Duration::from_secs(1) {
            self.system.refresh_cpu_usage();
            self.system.refresh_processes(ProcessesToUpdate::All, true);
            self.system.refresh_memory();
            self.disks.refresh(false);
            self.networks.refresh(false);
            self.components.refresh(false);

            self.cpu = self.system.global_cpu_usage();
            self.cores = self.system.cpus().iter().map(|c| c.cpu_usage()).collect();
            self.cpu_freq = self.system.cpus().first().map_or(0, |c| c.frequency());
            self.cpu_history.push(self.cpu);
            let (ct, _) = cpu_temp_info(&self.components);
            self.cpu_temp = ct;

            let (mp, ml) = mem_info(&self.system);
            self.mem_pct = mp;
            self.mem_label = ml;

            let (sp, sl) = swap_info(&self.system);
            self.swap_pct = sp;
            self.swap_label = sl;

            let (vp, vl) = vram_info();
            self.vram_pct = vp;
            self.vram_label = vl;

            let (dp, dl) = disk_info(&self.disks);
            self.disk_pct = dp;
            self.disk_label = dl;

            let new_rx = total_rx(&self.networks);
            let new_tx = total_tx(&self.networks);
            let dt = (now - self.prev_time).as_secs_f64().max(0.001);
            self.net_rx = (new_rx - self.prev_net_rx) as f64 / dt;
            self.net_tx = (new_tx - self.prev_net_tx) as f64 / dt;
            self.prev_net_rx = new_rx;
            self.prev_net_tx = new_tx;
            self.prev_time = now;

            let (bp, bc) = battery_info();
            self.battery_pct = bp;
            self.battery_charging = bc;

            self.top_cpu = top_cpu_procs(&self.system);
            self.top_mem = top_mem_procs(&self.system);

            self.tick = now;
        }

        ui.ctx().request_repaint_after(Duration::from_secs(1));

        let bg = Color32::from_rgba_premultiplied(0x0a, 0x0a, 0x0f, 0xdd);
        let cyan = Color32::from_rgb(0x22, 0xd3, 0xee);
        let green = Color32::from_rgb(0x22, 0xc5, 0x5e);
        let dim = Color32::from_gray(140);
        let dimmer = Color32::from_gray(90);
        let frame = egui::Frame::new().fill(bg).inner_margin(Margin::symmetric(10, 6));
        frame.show(ui, |ui| {
            let avail = ui.available_width();
            let bar_w = (avail - 110.0).clamp(50.0, 200.0);

            // Drag handle (≡) — entire top bar is draggable when unlocked
            ui.horizontal(|ui| {
                let sense = if !self.click_through { egui::Sense::click_and_drag() } else { egui::Sense::hover() };
                let id = ui.next_auto_id();
                let l = ui.label(RichText::new("≡").color(dimmer));
                let drag_rect = l.rect.expand2(Vec2::new(40.0, 2.0));
                if ui.interact(drag_rect, id, sense).drag_started() {
                    move_window();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("[x]").clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                    let label = if self.click_through { "[locked]" } else { "[movable]" };
                    if ui.button(label).clicked() {
                        self.click_through = !self.click_through;
                        set_click_through(self.click_through);
                    }
                });
            });
            // CPU row 1: bar + percentage + temp + freq
            ui.horizontal(|ui| {
                ui.label(RichText::new("CPU").color(cyan));
                ui.add(ProgressBar::new((self.cpu / 100.0).clamp(0.0, 1.0))
                    .fill(cyan).desired_width(bar_w));
                ui.label(RichText::new(format!("{:5.0}%", self.cpu)).color(dim));
                let temp_str = self.cpu_temp.map_or(String::new(), |t| format!(" {t:.0}C"));
                ui.label(RichText::new(temp_str).color(dimmer));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}MHz", self.cpu_freq)).color(dimmer));
                });
            });

            // CPU row 2: sparkline
            let spark_w = (avail - 40.0).clamp(60.0, 600.0);
            ui.horizontal(|ui| {
                ui.add_space(32.0);
                draw_sparkline(ui, self.cpu_history.data(), spark_w, 18.0, cyan);
            });

            // CPU row 3: per-core
            let core_advance = 35.0;
            let cores_per_row = ((ui.available_width() - 32.0) / core_advance) as usize;
            ui.horizontal(|ui| {
                ui.add_space(32.0);
                for (i, &core) in self.cores.iter().enumerate().take(16) {
                    let c = if core > 80.0 { cyan } else if core > 50.0 { green } else { dim };
                    ui.label(RichText::new(format!("{:02}:{:.0}", i + 1, core)).color(c).size(10.0));
                    if i % cores_per_row == cores_per_row - 1 && i != self.cores.len() - 1 && i < 15 {
                        ui.add_space(2.0);
                    }
                }
            });
            ui.add_space(3.0);

            // MEM
            metric_row(ui, "MEM", self.mem_pct, Some(&self.mem_label), bar_w, cyan, dim);
            ui.add_space(3.0);

            // SWP
            metric_row(ui, "SWP", self.swap_pct, Some(&self.swap_label), bar_w, cyan, dim);
            ui.add_space(3.0);

            // VRM
            metric_row(ui, "VRM", self.vram_pct, Some(&self.vram_label), bar_w, cyan, dim);
            ui.add_space(3.0);

            // DSK
            metric_row(ui, "DSK", self.disk_pct, Some(&self.disk_label), bar_w, cyan, dim);
            ui.add_space(3.0);

            // NET
            ui.horizontal(|ui| {
                ui.label(RichText::new("NET").color(cyan));
                ui.label(RichText::new(format!(
                    "down {}  up {}", fmt_rate(self.net_rx), fmt_rate(self.net_tx)
                )).color(dim));
            });
            ui.add_space(2.0);

            // Clock + uptime
            ui.horizontal(|ui| {
                let now = local_time();
                ui.label(RichText::new(&now).color(dim));
                let up = system_uptime();
                ui.label(RichText::new(up).color(dimmer).size(10.0));
            });
            ui.add_space(2.0);

            // BAT
            ui.horizontal(|ui| {
                ui.label(RichText::new("BAT").color(cyan));
                if let Some(pct) = self.battery_pct {
                    let bat_color = if pct > 20 { green } else { Color32::from_rgb(0xe9, 0x45, 0x60) };
                    ui.add(ProgressBar::new(pct as f32 / 100.0)
                        .fill(bat_color).desired_width(bar_w));
                    let status = if self.battery_charging { "charging" } else { "discharging" };
                    ui.label(RichText::new(format!("{pct:3}% {status}")).color(dim));
                } else {
                    ui.label(RichText::new("--").color(dim));
                }
            });
            ui.add_space(2.0);

            // Top processes
            ui.horizontal(|ui| {
                ui.label(RichText::new("CPU:").color(cyan).size(11.0));
                for (name, usage) in &self.top_cpu {
                    if *usage > 0.01 {
                        ui.label(RichText::new(format!("{} {:.1}%", name, usage)).color(dim).size(10.0));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("MEM:").color(cyan).size(11.0));
                for (name, bytes) in &self.top_mem {
                    if *bytes > 0 {
                        let mem_str = if *bytes > 1_000_000_000 {
                            format!("{:.1}G", *bytes as f64 / 1_000_000_000.0)
                        } else {
                            format!("{:.0}M", *bytes as f64 / 1_000_000.0)
                        };
                        ui.label(RichText::new(format!("{name} {mem_str}")).color(dim).size(10.0));
                    }
                }
            });

            // Resize grip
            let grip_sz = Vec2::splat(12.0);
            let bottom_right = ui.max_rect().right_bottom();
            let grip_rect = Rect::from_two_pos(bottom_right - grip_sz, bottom_right);
            let grip_resp = ui.interact(grip_rect, ui.next_auto_id(), egui::Sense::click_and_drag());
            if grip_resp.drag_started() || grip_resp.dragged() {
                self.manual_resize = true;
                let delta = grip_resp.drag_delta();
                let cur = ui.ctx().viewport_rect();
                let new_sz = (cur.size() + delta).max(Vec2::new(300.0, 200.0));
                self.last_content_size = new_sz;
                ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(new_sz));
            }
            // Draw grip indicator
            let painter = ui.painter();
            let c = Color32::from_gray(100);
            for i in 0..3 {
                let y = grip_rect.bottom() - 3.0 - i as f32 * 3.0;
                painter.line_segment([
                    pos2(grip_rect.right() - 3.0 - i as f32 * 3.0, y),
                    pos2(grip_rect.right(), y),
                ], Stroke::new(1.0, c));
            }

            // Auto-resize to content (only if user hasn't manually resized)
            if !self.manual_resize {
                let used = ui.min_rect().size() + Vec2::new(20.0, 12.0);
                if (used - self.last_content_size).length() > 1.0 {
                    self.last_content_size = used;
                    ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(used));
                }
            }
        });
    }
}


// ---- UI helpers ----

fn metric_row(
    ui: &mut egui::Ui,
    label: &str,
    pct: f32,
    extra: Option<&str>,
    bar_w: f32,
    color: Color32,
    dim: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(color));
        ui.add(
            ProgressBar::new((pct / 100.0).clamp(0.0, 1.0))
                .fill(color)
                .desired_width(bar_w),
        );
        ui.label(RichText::new(format!("{pct:5.0}%")).color(dim));
        if let Some(ext) = extra {
            ui.label(RichText::new(ext).color(dim));
        }
    });
}

fn draw_sparkline(ui: &mut egui::Ui, data: &[f32], width: f32, height: f32, color: Color32) {
    if data.len() < 2 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let min_v = data.iter().cloned().fold(f32::MAX, f32::min);
    let max_v = data.iter().cloned().fold(f32::MIN, f32::max);
    let range = (max_v - min_v).max(1.0);

    let pts: Vec<egui::Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = rect.left() + (i as f32 / (data.len() - 1).max(1) as f32) * rect.width();
            let y = rect.bottom() - ((v - min_v) / range) * rect.height();
            egui::pos2(x, y)
        })
        .collect();

    ui.painter().add(Shape::line(pts, Stroke::new(1.5, color)));
}

// ---- Data helpers ----

fn cpu_temp_info(components: &Components) -> (Option<f32>, String) {
    let temp = components.iter().find_map(|c| {
        let l = c.label().to_lowercase();
        (l.contains("cpu") || l.contains("package"))
            .then(|| c.temperature())
            .flatten()
    });
    let label = temp.map_or(String::new(), |t| format!("{t:.0}C"));
    (temp, label)
}

fn mem_info(system: &System) -> (f32, String) {
    let used = system.used_memory();
    let total = system.total_memory();
    let pct = if total > 0 { used as f32 / total as f32 * 100.0 } else { 0.0 };
    let used_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
    let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
    (pct, format!("{used_gb:.1}/{total_gb:.1}G"))
}

fn swap_info(system: &System) -> (f32, String) {
    let used = system.used_swap();
    let total = system.total_swap();
    if total == 0 {
        return (0.0, "none".into());
    }
    let pct = used as f32 / total as f32 * 100.0;
    let used_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
    let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
    (pct, format!("{used_gb:.1}/{total_gb:.1}G"))
}

fn vram_info() -> (f32, String) {
    get_gpu_vram().map_or((0.0, "--".into()), |(used, total)| {
        if total == 0 {
            return (0.0, "--".into());
        }
        let pct = used as f32 / total as f32 * 100.0;
        let used_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
        let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
        (pct, format!("{used_gb:.1}/{total_gb:.1}G"))
    })
}

fn get_gpu_vram() -> Option<(u64, u64)> {
    use windows::Win32::Graphics::Dxgi::*;
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        if let Ok(adapter) = factory.EnumAdapters1(0) {
            let adapter3: IDXGIAdapter3 = adapter.cast().ok()?;
            let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            adapter3
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                .ok()?;
            Some((info.CurrentUsage, info.Budget))
        } else {
            None
        }
    }
}

fn battery_info() -> (Option<u8>, bool) {
    use windows::Win32::System::Power::*;
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_ok() {
            let pct = if status.BatteryLifePercent <= 100 {
                Some(status.BatteryLifePercent)
            } else {
                None
            };
            let charging = status.ACLineStatus == 1;
            (pct, charging)
        } else {
            (None, false)
        }
    }
}

fn disk_info(disks: &Disks) -> (f32, String) {
    for disk in disks {
        let name = disk.name().to_string_lossy();
        if name.contains("C:") || name.contains("C:\\") {
            let total = disk.total_space();
            let avail = disk.available_space();
            let used = total - avail;
            let pct = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
            let used_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
            let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
            return (pct as f32, format!("{used_gb:.0}/{total_gb:.0}G"));
        }
    }
    if let Some(disk) = disks.iter().next() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total - avail;
        let pct = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
        (pct as f32, format!("{pct:.0}%"))
    } else {
        (0.0, "--".into())
    }
}

fn top_cpu_procs(system: &System) -> Vec<(String, f32)> {
    let mut procs: Vec<_> = system
        .processes()
        .iter()
        .map(|(_, p)| (p.name().to_string_lossy().into_owned(), p.cpu_usage()))
        .collect();
    procs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    procs.truncate(3);
    procs
}

fn top_mem_procs(system: &System) -> Vec<(String, u64)> {
    let mut procs: Vec<_> = system
        .processes()
        .iter()
        .map(|(_, p)| (p.name().to_string_lossy().into_owned(), p.memory()))
        .collect();
    procs.sort_by(|a, b| b.1.cmp(&a.1));
    procs.truncate(3);
    procs
}

fn total_rx(networks: &Networks) -> u64 {
    networks.iter().map(|(_, n)| n.total_received()).sum()
}

fn total_tx(networks: &Networks) -> u64 {
    networks.iter().map(|(_, n)| n.total_transmitted()).sum()
}

fn fmt_rate(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.1}G", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1}M", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0}K", bps / 1_000.0)
    } else {
        format!("{bps:.0}B")
    }
}

fn local_time() -> String {
    use windows::Win32::Foundation::SYSTEMTIME;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    }
    unsafe {
        let mut st = SYSTEMTIME::default();
        GetLocalTime(&mut st);
        format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond)
    }
}

fn system_uptime() -> String {
    let secs = System::uptime();
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    format!("up {d}d {h}h {m}m")
}
