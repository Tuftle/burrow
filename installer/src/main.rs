//! Burrow's installer: one small window in the tuftle look. It carries burrow.exe inside it, puts it in the person's own
//! folder (no administrator needed), adds the shortcuts and the entry in Windows' list of apps, and shows how far it
//! has got as an acorn walking to its burrow.
#![windows_subsystem = "windows"]

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, epaint::CubicBezierShape, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Mesh, Pos2, Rect,
    Sense, Shape, Stroke, Vec2,
};

const MAGIC: &[u8; 8] = b"BURROWSP";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Burrow";

// ---- colours: the same palette as tuftle.vercel.app, dark or cream like the system's setting --------------------------

fn light() -> bool {
    static LIGHT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LIGHT.get_or_init(|| {
        // `--light` and `--dark` choose by hand, for previews
        if std::env::args().any(|a| a == "--light") {
            return true;
        }
        if std::env::args().any(|a| a == "--dark") {
            return false;
        }
        Command::new("reg")
            .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().ends_with("0x1"))
            .unwrap_or(false)
    })
}

fn pick(dark: [u8; 3], light_: [u8; 3]) -> Color32 {
    let c = if light() { light_ } else { dark };
    Color32::from_rgb(c[0], c[1], c[2])
}

fn bg() -> Color32 { pick([0x21, 0x1c, 0x17], [0xf4, 0xef, 0xe6]) }
fn card() -> Color32 { pick([0x2a, 0x23, 0x1c], [0xfb, 0xf8, 0xf2]) }
fn line() -> Color32 { pick([0x3a, 0x31, 0x28], [0xdd, 0xd2, 0xc1]) }
fn text() -> Color32 { pick([0xe6, 0xdc, 0xcd], [0x3b, 0x2f, 0x25]) }
fn soft() -> Color32 { pick([0x9a, 0x8b, 0x7a], [0x8a, 0x7b, 0x6b]) }
fn amber() -> Color32 { pick([0xd9, 0xa0, 0x73], [0x8a, 0x4b, 0x2a]) }
fn hill() -> Color32 { pick([0x6b, 0x3d, 0x24], [0x8a, 0x4b, 0x2a]) }
fn door() -> Color32 { Color32::from_rgb(0x5e, 0x3a, 0x22) }
fn dark_inside() -> Color32 { Color32::from_rgb(0x1e, 0x14, 0x0c) }
fn glow() -> Color32 { Color32::from_rgb(0xff, 0xc6, 0x6e) }
fn leaf() -> Color32 { Color32::from_rgb(0x7a, 0x9a, 0x5a) }

#[derive(Clone, PartialEq)]
enum Screen {
    Welcome,
    Installing,
    Done,
    Failed(String),
    Confirm,
    Removing,
    Removed(bool),
}

#[derive(Default)]
struct Shared {
    fraction: f32,
    finished: Option<Result<(), String>>,
}

struct App {
    screen: Screen,
    desktop_shortcut: bool,
    shared: Arc<Mutex<Shared>>,
    shown: f32,
    done_at: f64,
    auto: bool,
    delete_data: bool,
    install_dir: PathBuf,
}

fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Burrow"));
    base.join("Programs").join("Burrow")
}

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Burrow")
}

fn is_update() -> bool {
    std::env::args().any(|a| a == "--update")
}

fn hidden(program: &str) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Where burrow.exe starts and how long it is: the file ends with [payload length: u64]["BURROWSP"].
fn payload_span(file: &mut File) -> Result<(u64, u64), String> {
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    if len < 16 {
        return Err("this installer is incomplete".into());
    }
    file.seek(SeekFrom::Start(len - 16)).map_err(|e| e.to_string())?;
    let mut footer = [0u8; 16];
    file.read_exact(&mut footer).map_err(|e| e.to_string())?;
    if &footer[8..16] != MAGIC {
        return Err("this installer is incomplete: burrow is not inside it".into());
    }
    let payload = u64::from_le_bytes(footer[0..8].try_into().unwrap());
    if payload == 0 || payload > len - 16 {
        return Err("this installer is damaged".into());
    }
    Ok((len - 16 - payload, payload))
}

/// The length of this window without anything stuck on its end (what the uninstaller copies).
fn stub_len(file: &mut File) -> u64 {
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    payload_span(file).map(|(start, _)| start).unwrap_or(len)
}

/// Is the installed burrow.exe running? Matches by path, so a development build is left alone.
fn burrow_is_running(exe: &Path) -> bool {
    let script = format!("if (Get-Process burrow -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{}' }}) {{ 'yes' }}", exe.display());
    hidden("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("yes"))
        .unwrap_or(false)
}

fn stop_burrow(exe: &Path, force: bool) {
    let flag = if force { " -Force" } else { "" };
    let script = format!(
        "Get-Process burrow -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{}' }} | ForEach-Object {{ if ({}) {{ $_ | Stop-Process{} }} else {{ $_.CloseMainWindow() | Out-Null }} }}",
        exe.display(),
        if force { "$true" } else { "$false" },
        flag
    );
    let _ = hidden("powershell").args(["-NoProfile", "-Command", &script]).status();
}

fn shortcut_script(link: &str, target: &Path, dir: &Path) -> String {
    format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut({link}); $s.TargetPath = '{t}'; $s.WorkingDirectory = '{d}'; $s.IconLocation = '{t}'; $s.Description = 'burrow, a quiet password manager'; $s.Save()",
        t = target.display(),
        d = dir.display()
    )
}

fn make_shortcuts(dir: &Path, desktop: bool) {
    let exe = dir.join("burrow.exe");
    let start = "(Join-Path ([Environment]::GetFolderPath('Programs')) 'Burrow.lnk')";
    let mut script = shortcut_script(start, &exe, dir);
    if desktop {
        script.push_str("; ");
        script.push_str(&shortcut_script("(Join-Path ([Environment]::GetFolderPath('Desktop')) 'Burrow.lnk')", &exe, dir));
    }
    let _ = hidden("powershell").args(["-NoProfile", "-Command", &script]).status();
}

fn remove_shortcuts() {
    let script = "Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path ([Environment]::GetFolderPath('Programs')) 'Burrow.lnk'), (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Burrow.lnk')";
    let _ = hidden("powershell").args(["-NoProfile", "-Command", script]).status();
}

fn reg_add(name: &str, kind: &str, value: &str) {
    let _ = hidden("reg").args(["add", UNINSTALL_KEY, "/v", name, "/t", kind, "/d", value, "/f"]).status();
}

fn register_with_windows(dir: &Path, uninstaller: &Path, size_kb: u64) {
    reg_add("DisplayName", "REG_SZ", "Burrow");
    reg_add("DisplayVersion", "REG_SZ", VERSION);
    reg_add("Publisher", "REG_SZ", "tuftle");
    reg_add("InstallLocation", "REG_SZ", &dir.display().to_string());
    reg_add("DisplayIcon", "REG_SZ", &dir.join("burrow.exe").display().to_string());
    reg_add("UninstallString", "REG_SZ", &format!("\"{}\" --uninstall", uninstaller.display()));
    reg_add("URLInfoAbout", "REG_SZ", "https://tuftle.vercel.app");
    reg_add("EstimatedSize", "REG_DWORD", &size_kb.to_string());
    reg_add("NoModify", "REG_DWORD", "1");
    reg_add("NoRepair", "REG_DWORD", "1");
}

fn run_install(shared: Arc<Mutex<Shared>>, desktop: bool, dir: PathBuf, ctx: egui::Context) {
    let set = |f: f32| {
        shared.lock().unwrap().fraction = f;
        ctx.request_repaint();
    };
    let result = (|| -> Result<(), String> {
        let target = dir.join("burrow.exe");
        if is_update() {
            let mut waited = 0;
            while burrow_is_running(&target) {
                if waited >= 240 {
                    return Err("burrow is still open. close it and run the update again".into());
                }
                std::thread::sleep(Duration::from_millis(500));
                waited += 1;
            }
        }
        let mut me = File::open(std::env::current_exe().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let (start, length) = payload_span(&mut me)?;
        me.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
        fs::create_dir_all(&dir).map_err(|e| format!("could not make the folder: {e}"))?;

        // write beside the old one first, so a failed copy never leaves a half burrow
        let part = dir.join("burrow.exe.part");
        let mut out = File::create(&part).map_err(|e| format!("could not write burrow: {e}"))?;
        let mut left = length;
        let mut buf = vec![0u8; 256 * 1024];
        while left > 0 {
            let take = left.min(buf.len() as u64) as usize;
            me.read_exact(&mut buf[..take]).map_err(|e| e.to_string())?;
            out.write_all(&buf[..take]).map_err(|e| e.to_string())?;
            left -= take as u64;
            set(0.88 * (1.0 - left as f32 / length as f32));
            std::thread::sleep(Duration::from_millis(4)); // a small pause so the walk can be seen
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out);
        let _ = fs::remove_file(&target);
        fs::rename(&part, &target).map_err(|e| format!("could not put burrow in place: {e}"))?;

        set(0.9);
        let uninstaller = write_uninstaller(&dir)?;
        set(0.94);
        make_shortcuts(&dir, desktop);
        register_with_windows(&dir, &uninstaller, length / 1024);
        set(0.98);
        if !target.exists() {
            return Err("burrow.exe was not found after copying".into());
        }
        Ok(())
    })();
    let mut s = shared.lock().unwrap();
    s.fraction = 1.0;
    s.finished = Some(result);
    ctx.request_repaint();
}

// ---- the uninstaller ------------------------------------------------------------------------------------------------

/// Puts a copy of this window (without burrow on its end) into Burrow's folder, for Windows' "Uninstall" button.
fn write_uninstaller(dir: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut file = File::open(&exe).map_err(|e| e.to_string())?;
    let keep = stub_len(&mut file);
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let folder = dir.join("uninstall");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let target = folder.join("Burrow-Uninstall.exe");
    let mut out = File::create(&target).map_err(|e| e.to_string())?;
    std::io::copy(&mut file.take(keep), &mut out).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(target)
}

/// When the uninstaller is started from inside Burrow's folder it moves to a temporary place first, so that the folder
/// can be removed while it runs.
fn move_out_of_the_way() {
    let args: Vec<String> = std::env::args().collect();
    let Ok(exe) = std::env::current_exe() else { return };
    let dir = match args.iter().position(|a| a == "--dir").and_then(|i| args.get(i + 1)) {
        Some(d) => PathBuf::from(d),
        None => match exe.parent().and_then(|p| p.parent()) {
            Some(d) => d.to_path_buf(),
            None => return,
        },
    };
    if exe.starts_with(&dir) {
        let temp = std::env::temp_dir().join(format!("burrow-uninstall-{}.exe", std::process::id()));
        if fs::copy(&exe, &temp).is_ok() {
            let mut again = Command::new(&temp);
            again.args(&args[1..]);
            if !args.iter().any(|a| a == "--dir") {
                again.arg("--dir").arg(&dir);
            }
            again.creation_flags(0x0000_0008);
            if again.spawn().is_ok() {
                std::process::exit(0);
            }
        }
    }
}

fn uninstall_dir() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if let Some(d) = args.iter().position(|a| a == "--dir").and_then(|i| args.get(i + 1)) {
        return PathBuf::from(d);
    }
    std::env::current_exe().ok().and_then(|e| e.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf())).unwrap_or_else(install_dir)
}

fn run_uninstall(shared: Arc<Mutex<Shared>>, dir: PathBuf, delete_data: bool, ctx: egui::Context) {
    let set = |f: f32| {
        shared.lock().unwrap().fraction = f;
        ctx.request_repaint();
    };
    let result = (|| -> Result<(), String> {
        let exe = dir.join("burrow.exe");
        // ask burrow to close, and after a few seconds make sure it has
        stop_burrow(&exe, false);
        for _ in 0..8 {
            if !burrow_is_running(&exe) {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        stop_burrow(&exe, true);
        set(0.25);
        std::thread::sleep(Duration::from_millis(500));
        remove_shortcuts();
        set(0.45);
        let _ = hidden("reg").args(["delete", UNINSTALL_KEY, "/f"]).status();
        set(0.6);
        for i in 0..20 {
            if !dir.exists() || fs::remove_dir_all(&dir).is_ok() {
                break;
            }
            set(0.6 + 0.02 * i as f32);
            std::thread::sleep(Duration::from_millis(300));
        }
        if delete_data {
            let _ = fs::remove_dir_all(data_dir());
        }
        if exe.exists() {
            return Err("some files could not be removed. close anything that is using them and try again".into());
        }
        Ok(())
    })();
    let mut s = shared.lock().unwrap();
    s.fraction = 1.0;
    s.finished = Some(result);
    ctx.request_repaint();
}

/// Removes this window's own temporary copy once it has closed.
fn forget_myself() {
    if let Ok(exe) = std::env::current_exe() {
        if exe.starts_with(std::env::temp_dir()) {
            let _ = Command::new("cmd")
                .args(["/C", &format!("ping -n 4 127.0.0.1 >nul & del /f /q \"{}\"", exe.display())])
                .creation_flags(CREATE_NO_WINDOW | 0x0000_0008)
                .spawn();
        }
    }
}

// ---- drawing: a small hill with a round door, and an acorn --------------------------------------------------------

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 below `a`, 1 above `b`, smooth in between.
fn span(f: f32, a: f32, b: f32) -> f32 {
    ease((f - a) / (b - a))
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// A doorway: straight sides, rounded top. `rx` is half the width, `ry` how tall the rounded part is.
fn arch(x: f32, base: f32, rx: f32, ry: f32, height: f32) -> Vec<Pos2> {
    let top = base - height + ry;
    let mut pts = vec![Pos2::new(x - rx, base)];
    for i in 0..=24 {
        let a = std::f32::consts::PI * (1.0 - i as f32 / 24.0);
        pts.push(Pos2::new(x + rx * a.cos(), top - ry * a.sin()));
    }
    pts.push(Pos2::new(x + rx, base));
    pts
}

const DOOR_W: f32 = 70.0;
const DOOR_H: f32 = 92.0;

/// The hill, its round door (`open` 0 shut, 1 wide open), the light behind it, and a sprout on top.
fn draw_burrow(painter: &egui::Painter, hx: f32, ground: f32, open: f32, lit: f32, sprout: f32, time: f64) {
    // the hill: half an ellipse
    let (a, b) = (150.0, 128.0);
    let pts: Vec<Pos2> = (0..=64)
        .map(|i| {
            let t = std::f32::consts::PI * (1.0 - i as f32 / 64.0);
            Pos2::new(hx + a * t.cos(), ground - b * t.sin())
        })
        .collect();
    painter.add(Shape::convex_polygon(pts, hill(), Stroke::NONE));

    // inside the door: dark, or warm when lit
    let rx = DOOR_W / 2.0;
    let inside = mix(dark_inside(), Color32::from_rgb(0x8c, 0x5a, 0x2a), lit * 0.55);
    painter.add(Shape::convex_polygon(arch(hx, ground, rx + 5.0, rx + 5.0, DOOR_H + 5.0), mix(hill(), Color32::BLACK, 0.35), Stroke::NONE));
    painter.add(Shape::convex_polygon(arch(hx, ground, rx, rx, DOOR_H), inside, Stroke::NONE));
    if lit > 0.0 {
        let pulse = 0.85 + 0.15 * (time * 2.2).sin() as f32;
        painter.add(Shape::ellipse_filled(Pos2::new(hx, ground - 26.0), Vec2::new(22.0, 30.0), glow().gamma_multiply(0.22 * lit * pulse)));
    }

    // the door itself swings open to the left, so it narrows as it opens
    let door_w = DOOR_W * (1.0 - 0.84 * open);
    let left = hx - rx;
    let dx = left + door_w / 2.0;
    painter.add(Shape::convex_polygon(arch(dx, ground, door_w / 2.0, rx, DOOR_H), door(), Stroke::new(1.5, mix(door(), Color32::BLACK, 0.4))));
    if door_w > 22.0 {
        // planks and the keyhole, shown while the door faces us
        let k = ((door_w - 22.0) / (DOOR_W - 22.0)).clamp(0.0, 1.0);
        for off in [-0.33_f32, 0.33] {
            painter.add(Shape::line_segment(
                [Pos2::new(dx + off * door_w, ground - DOOR_H * 0.62), Pos2::new(dx + off * door_w, ground - 2.0)],
                Stroke::new(1.2, mix(door(), Color32::BLACK, 0.35).gamma_multiply(k)),
            ));
        }
        let key = Pos2::new(dx, ground - 38.0);
        let kc = mix(Color32::from_rgb(0xe8, 0xc8, 0x9a), glow(), lit);
        painter.add(Shape::circle_filled(key, 5.0 * k.max(0.3), kc.gamma_multiply(k)));
        painter.add(Shape::convex_polygon(
            vec![Pos2::new(key.x - 2.6 * k, key.y + 3.0), Pos2::new(key.x + 2.6 * k, key.y + 3.0), Pos2::new(key.x + 3.8 * k, key.y + 13.0), Pos2::new(key.x - 3.8 * k, key.y + 13.0)],
            kc.gamma_multiply(k),
            Stroke::NONE,
        ));
    }

    // the sprout on top of the hill
    if sprout > 0.0 {
        let base = Pos2::new(hx + 6.0, ground - b + 4.0);
        let h = 34.0 * sprout;
        let sway = ((time * 1.6).sin() as f32) * 2.0 * sprout;
        let tip = Pos2::new(base.x + sway, base.y - h);
        painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [base, Pos2::new(base.x, base.y - h * 0.4), Pos2::new(tip.x - 3.0, tip.y + h * 0.3), tip], false, Color32::TRANSPARENT, Stroke::new(3.0, leaf()),
        )));
        let l = 13.0 * sprout;
        painter.add(Shape::ellipse_filled(Pos2::new(tip.x - l * 0.9, tip.y + 2.0), Vec2::new(l, l * 0.5), leaf()));
        painter.add(Shape::ellipse_filled(Pos2::new(tip.x + l * 0.9, tip.y - 3.0), Vec2::new(l, l * 0.5), mix(leaf(), Color32::WHITE, 0.12)));
    }

    // the ground line, with a few tufts
    let g = line();
    painter.add(Shape::line_segment([Pos2::new(hx - 420.0, ground), Pos2::new(hx + 420.0, ground)], Stroke::new(2.0, g)));
    for (i, off) in [-262.0_f32, -190.0, -118.0, 176.0, 232.0].iter().enumerate() {
        let x = hx + off;
        let tall = 7.0 + (i % 2) as f32 * 3.0;
        for d in [-3.5_f32, 0.0, 3.5] {
            painter.add(Shape::line_segment([Pos2::new(x, ground), Pos2::new(x + d * 1.4, ground - tall + d.abs())], Stroke::new(1.6, leaf().gamma_multiply(0.8))));
        }
    }
}

/// A little acorn with two feet. `walk` makes it bob; `alpha` fades it as it goes through the door.
fn draw_acorn(painter: &egui::Painter, foot: Pos2, scale: f32, walk: f64, alpha: f32) {
    let t = walk as f32;
    let hop = if walk != 0.0 { (t * 9.0).sin().abs() * 5.0 } else { 0.0 };
    let at = |x: f32, y: f32| Pos2::new(foot.x + x * scale, foot.y + (y - hop) * scale);
    let a = |c: Color32| c.gamma_multiply(alpha);
    // feet
    let step = if walk != 0.0 { (t * 9.0).sin() * 3.0 } else { 0.0 };
    for (x, s) in [(-5.0, step), (5.0, -step)] {
        painter.add(Shape::ellipse_filled(at(x + s, -1.0), Vec2::new(3.6 * scale, 2.2 * scale), a(Color32::from_rgb(0x4a, 0x2a, 0x16))));
    }
    // the nut
    painter.add(Shape::ellipse_filled(at(0.0, -17.0), Vec2::new(12.5 * scale, 16.0 * scale), a(Color32::from_rgb(0xc8, 0x8f, 0x55))));
    painter.add(Shape::ellipse_filled(at(-4.0, -20.0), Vec2::new(3.0 * scale, 6.0 * scale), a(Color32::from_rgb(0xe0, 0xb0, 0x7c))));
    // the cap, with a few dots
    painter.add(Shape::ellipse_filled(at(0.0, -29.0), Vec2::new(14.5 * scale, 8.5 * scale), a(Color32::from_rgb(0x6f, 0x3b, 0x20))));
    for x in [-7.0, -2.0, 4.0, 9.0] {
        painter.add(Shape::circle_filled(at(x, -28.5 + (x * 0.37).sin() * 2.0), 1.1 * scale, a(Color32::from_rgb(0x9a, 0x5c, 0x34))));
    }
    // the stem
    painter.add(Shape::line_segment([at(0.0, -36.5), at(2.5, -42.0)], Stroke::new(3.0 * scale, a(Color32::from_rgb(0x4a, 0x2a, 0x16)))));
    // the eyes, two small dots
    painter.add(Shape::circle_filled(at(-4.5, -18.0), 1.5 * scale, a(Color32::from_rgb(0x2a, 0x12, 0x06))));
    painter.add(Shape::circle_filled(at(4.5, -18.0), 1.5 * scale, a(Color32::from_rgb(0x2a, 0x12, 0x06))));
}

/// The whole scene for a given point in the install (`f` from 0 to 1): the acorn walks over, the door opens, it goes in,
/// the door shuts and the keyhole starts to glow. Played backwards for removing.
fn scene(painter: &egui::Painter, hx: f32, ground: f32, f: f32, time: f64, show_acorn: bool) {
    let start = hx - 235.0;
    let door_x = hx - 8.0;
    let walk = span(f, 0.0, 0.72);
    let open = span(f, 0.45, 0.62) * (1.0 - span(f, 0.9, 0.98));
    let lit = span(f, 0.94, 1.0);
    draw_burrow(painter, hx, ground, open, lit, 0.0, time);
    if show_acorn && f < 0.9 {
        let x = start + (door_x - start) * walk;
        let going_in = span(f, 0.72, 0.84);
        let moving = if walk > 0.0 && walk < 1.0 { time } else if going_in > 0.0 { time } else { 0.0 };
        let sx = x + 6.0 * going_in;
        draw_acorn(painter, Pos2::new(sx, ground), 1.2 * (1.0 - 0.35 * going_in), moving, 1.0 - going_in);
    }
}

fn button(ui: &mut egui::Ui, rect: Rect, label: &str, primary: bool) -> bool {
    let response = ui.allocate_rect(rect, Sense::click());
    let hover = response.hovered();
    let (fill, text, stroke) = if primary {
        (if hover { pick([0xe6, 0xb5, 0x8a], [0x9c, 0x58, 0x33]) } else { amber() }, bg(), Stroke::NONE)
    } else {
        (if hover { card() } else { Color32::TRANSPARENT }, text(), Stroke::new(1.0, if hover { amber() } else { line() }))
    };
    let radius = (rect.height() / 2.0) as u8;
    ui.painter().add(Shape::rect_filled(rect, CornerRadius::same(radius), fill));
    if stroke.width > 0.0 {
        ui.painter().add(Shape::rect_stroke(rect, CornerRadius::same(radius), stroke, egui::StrokeKind::Inside));
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(18.0), text);
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Georgia is on every Windows computer; it is what the tuftle pages are written in
        let mut fonts = FontDefinitions::default();
        if let Ok(bytes) = fs::read(r"C:\Windows\Fonts\georgia.ttf") {
            fonts.font_data.insert("georgia".into(), Arc::new(FontData::from_owned(bytes)));
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "georgia".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        let mut visuals = if light() { egui::Visuals::light() } else { egui::Visuals::dark() };
        visuals.widgets.inactive.bg_fill = card();
        visuals.widgets.inactive.weak_bg_fill = card();
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, line());
        visuals.selection.bg_fill = amber();
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, amber());
        visuals.override_text_color = Some(text());
        cc.egui_ctx.set_visuals(visuals);
        let mut screen = if std::env::args().any(|a| a == "--uninstall") { Screen::Confirm } else { Screen::Welcome };
        let mut preview = 0.0;
        for arg in std::env::args().skip(1) {
            if let Some(v) = arg.strip_prefix("--preview-installing=") {
                screen = Screen::Installing;
                preview = v.parse().unwrap_or(0.5);
            } else if arg == "--preview-done" {
                screen = Screen::Done;
            } else if let Some(v) = arg.strip_prefix("--preview-removing=") {
                screen = Screen::Removing;
                preview = v.parse().unwrap_or(0.5);
            } else if arg == "--preview-removed" {
                screen = Screen::Removed(true);
            } else if arg == "--preview-failed" {
                screen = Screen::Failed("the folder could not be written to.".into());
            }
        }
        App {
            screen,
            desktop_shortcut: true,
            shared: Arc::new(Mutex::new(Shared { fraction: preview, finished: None })),
            shown: preview,
            done_at: 0.0,
            auto: std::env::args().any(|a| a == "--install"),
            delete_data: false,
            install_dir: install_dir(),
        }
    }

    fn start_removal(&mut self, ctx: &egui::Context) {
        self.screen = Screen::Removing;
        self.shown = 0.0;
        let (shared, dir, delete, ctx2) = (self.shared.clone(), uninstall_dir(), self.delete_data, ctx.clone());
        std::thread::spawn(move || run_uninstall(shared, dir, delete, ctx2));
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.screen = Screen::Installing;
        let (shared, dir, desktop, ctx2) = (self.shared.clone(), self.install_dir.clone(), self.desktop_shortcut, ctx.clone());
        std::thread::spawn(move || run_install(shared, desktop, dir, ctx2));
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = ctx.input(|i| i.time);
        if self.screen == Screen::Confirm && std::env::args().any(|a| a == "--remove") {
            self.start_removal(ctx); // `--uninstall --remove` skips the question
        }
        if self.auto && self.screen == Screen::Welcome {
            self.start(ctx); // `Burrow-Setup.exe --install` goes straight to installing
        }
        // progress shown eases towards the real one, so it never jumps
        let (target, finished) = {
            let s = self.shared.lock().unwrap();
            (s.fraction, s.finished.clone())
        };
        if self.screen == Screen::Installing || self.screen == Screen::Removing {
            self.shown += (target - self.shown) * 0.08;
            ctx.request_repaint();
            if let Some(result) = finished {
                if self.shown > 0.985 || result.is_err() {
                    self.shown = 1.0;
                    self.done_at = time;
                    let installing = self.screen == Screen::Installing;
                    self.screen = match result {
                        Ok(()) if installing => Screen::Done,
                        Ok(()) => Screen::Removed(!self.delete_data),
                        Err(e) => Screen::Failed(e),
                    };
                }
            }
        }
        if self.screen == Screen::Welcome || self.screen == Screen::Confirm || matches!(self.screen, Screen::Removed(_)) {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if self.screen == Screen::Done {
            ctx.request_repaint_after(Duration::from_millis(33));
            if is_update() && time - self.done_at > 1.6 {
                let _ = Command::new(self.install_dir.join("burrow.exe")).spawn();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let full = ui.max_rect();
            let painter = ui.painter().clone();
            painter.add(Shape::rect_filled(full, CornerRadius::same(18), bg()));
            painter.add(Shape::rect_stroke(full.shrink(0.5), CornerRadius::same(18), Stroke::new(1.0, line()), egui::StrokeKind::Inside));
            // a warm glow at the top, like the tuftle pages
            {
                let mut mesh = Mesh::default();
                let centre = Pos2::new(full.center().x, full.top() + 10.0);
                mesh.colored_vertex(centre, Color32::from_rgba_premultiplied(26, 18, 10, 26));
                for i in 0..48 {
                    let a = std::f32::consts::TAU * i as f32 / 48.0;
                    mesh.colored_vertex(centre + Vec2::new(a.cos() * 330.0, a.sin() * 230.0), Color32::TRANSPARENT);
                }
                for i in 1..48u32 {
                    mesh.add_triangle(0, i, i + 1);
                }
                mesh.add_triangle(0, 48, 1);
                painter.with_clip_rect(full).add(Shape::mesh(mesh));
            }

            // the title bar: drag it to move the window; the cross closes it (not while working)
            let bar = Rect::from_min_size(full.min, Vec2::new(full.width() - 60.0, 52.0));
            if ui.interact(bar, egui::Id::new("bar"), Sense::click_and_drag()).drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if self.screen != Screen::Installing && self.screen != Screen::Removing {
                let cross = Rect::from_center_size(Pos2::new(full.right() - 30.0, full.top() + 28.0), Vec2::splat(32.0));
                let r = ui.allocate_rect(cross, Sense::click());
                painter.text(cross.center(), Align2::CENTER_CENTER, "\u{00d7}", FontId::proportional(26.0), if r.hovered() { text() } else { soft() });
                if r.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            painter.text(Pos2::new(full.left() + 26.0, full.top() + 28.0), Align2::LEFT_CENTER, "burrow", FontId::proportional(18.0), soft());

            let cx = full.center().x;
            let ground = full.top() + 262.0;
            let hx = cx + 60.0;
            match self.screen.clone() {
                Screen::Welcome => {
                    // the acorn waits by the door, bobbing a little
                    draw_burrow(&painter, hx, ground, 0.0, 0.0, 0.0, time);
                    draw_acorn(&painter, Pos2::new(hx - 118.0, ground), 1.2, 0.0, 1.0);
                    painter.text(Pos2::new(cx, full.top() + 282.0), Align2::CENTER_TOP, "welcome to burrow", FontId::proportional(32.0), text());
                    painter.text(Pos2::new(cx, full.top() + 324.0), Align2::CENTER_TOP, "a quiet place for your passwords.", FontId::proportional(16.0), soft());
                    let check = Rect::from_min_size(Pos2::new(cx - 130.0, full.top() + 352.0), Vec2::new(300.0, 24.0));
                    ui.put(check, egui::Checkbox::new(&mut self.desktop_shortcut, egui::RichText::new("add a shortcut on the desktop").size(15.0).color(text())));
                    painter.text(Pos2::new(cx, full.bottom() - 16.0), Align2::CENTER_CENTER, "installs just for you, in your own folder. no administrator needed.", FontId::proportional(13.0), soft());
                    if button(ui, Rect::from_center_size(Pos2::new(cx, full.top() + 404.0), Vec2::new(210.0, 40.0)), "install burrow", true) {
                        self.start(ctx);
                    }
                }
                Screen::Installing => {
                    let f = self.shown.clamp(0.0, 1.0);
                    scene(&painter, hx, ground, f, time, true);
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "tucking things in\u{2026}", FontId::proportional(26.0), text());
                    painter.text(Pos2::new(cx, full.top() + 330.0), Align2::CENTER_TOP, format!("{}%   a few moments, then it is yours.", (f * 100.0).round() as u32), FontId::proportional(16.0), soft());
                    bar_track(&painter, Pos2::new(cx, full.top() + 392.0), f);
                }
                Screen::Done => {
                    let since = (time - self.done_at) as f32;
                    draw_burrow(&painter, hx, ground, 0.0, 1.0, ease(since / 1.6), time);
                    let a = (255.0 * ease(since / 0.7)) as u8;
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "burrow is ready", FontId::proportional(32.0), Color32::from_rgba_unmultiplied(text().r(), text().g(), text().b(), a));
                    painter.text(Pos2::new(cx, full.top() + 336.0), Align2::CENTER_TOP, "everything is tucked in. nothing leaves this computer.", FontId::proportional(16.0), Color32::from_rgba_unmultiplied(soft().r(), soft().g(), soft().b(), a));
                    if button(ui, Rect::from_center_size(Pos2::new(cx - 75.0, full.top() + 408.0), Vec2::new(138.0, 42.0)), "open burrow", true) {
                        let _ = Command::new(self.install_dir.join("burrow.exe")).spawn();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if button(ui, Rect::from_center_size(Pos2::new(cx + 75.0, full.top() + 408.0), Vec2::new(138.0, 42.0)), "close", false) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Screen::Confirm => {
                    draw_burrow(&painter, hx, ground, 0.55, 0.0, 0.0, time);
                    draw_acorn(&painter, Pos2::new(hx - 118.0, ground), 1.2, 0.0, 1.0);
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "remove burrow?", FontId::proportional(32.0), text());
                    painter.text(Pos2::new(cx, full.top() + 334.0), Align2::CENTER_TOP, "the app goes. your vault stays, unless you tick the box.", FontId::proportional(16.0), soft());
                    let check = Rect::from_min_size(Pos2::new(cx - 190.0, full.top() + 366.0), Vec2::new(420.0, 24.0));
                    ui.put(check, egui::Checkbox::new(&mut self.delete_data, egui::RichText::new("also delete my vault (this cannot be undone)").size(15.0).color(text())));
                    if button(ui, Rect::from_center_size(Pos2::new(cx - 100.0, full.top() + 428.0), Vec2::new(180.0, 42.0)), "keep burrow", true) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if button(ui, Rect::from_center_size(Pos2::new(cx + 100.0, full.top() + 428.0), Vec2::new(180.0, 42.0)), "remove burrow", false) {
                        self.start_removal(ctx);
                    }
                }
                Screen::Removing => {
                    let f = self.shown.clamp(0.0, 1.0);
                    // the acorn comes back out and wanders off as burrow is taken away
                    scene(&painter, hx, ground, 1.0 - f, time, true);
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "packing up\u{2026}", FontId::proportional(26.0), text());
                    painter.text(Pos2::new(cx, full.top() + 330.0), Align2::CENTER_TOP, format!("{}%", (f * 100.0).round() as u32), FontId::proportional(16.0), soft());
                    bar_track(&painter, Pos2::new(cx, full.top() + 392.0), f);
                }
                Screen::Removed(kept) => {
                    draw_burrow(&painter, hx, ground, 1.0, 0.0, 0.0, time);
                    draw_acorn(&painter, Pos2::new(hx - 118.0, ground), 1.2, 0.0, 1.0);
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "burrow is removed", FontId::proportional(32.0), text());
                    let msg = if kept { "your vault file is still on this computer." } else { "your vault was deleted too." };
                    painter.text(Pos2::new(cx, full.top() + 336.0), Align2::CENTER_TOP, msg, FontId::proportional(16.0), soft());
                    painter.text(Pos2::new(cx, full.top() + 362.0), Align2::CENTER_TOP, "come back whenever you like.", FontId::proportional(16.0), soft());
                    if button(ui, Rect::from_center_size(Pos2::new(cx, full.top() + 420.0), Vec2::new(170.0, 42.0)), "close", true) {
                        forget_myself();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Screen::Failed(message) => {
                    draw_burrow(&painter, hx, ground, 0.0, 0.0, 0.0, time);
                    draw_acorn(&painter, Pos2::new(hx - 118.0, ground), 1.2, 0.0, 1.0);
                    painter.text(Pos2::new(cx, full.top() + 292.0), Align2::CENTER_TOP, "that didn't work", FontId::proportional(28.0), text());
                    painter.text(Pos2::new(cx, full.top() + 334.0), Align2::CENTER_TOP, message, FontId::proportional(15.0), soft());
                    painter.text(Pos2::new(cx, full.top() + 360.0), Align2::CENTER_TOP, "you can try again, or write to tuftle@proton.me.", FontId::proportional(15.0), soft());
                    if button(ui, Rect::from_center_size(Pos2::new(cx, full.top() + 420.0), Vec2::new(160.0, 42.0)), "close", true) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        });
    }
}

fn bar_track(painter: &egui::Painter, centre: Pos2, f: f32) {
    let track = Rect::from_center_size(centre, Vec2::new(300.0, 6.0));
    painter.add(Shape::rect_filled(track, CornerRadius::same(3), line()));
    let mut filled = track;
    filled.set_width(track.width() * f);
    painter.add(Shape::rect_filled(filled, CornerRadius::same(3), amber()));
}

fn main() -> eframe::Result {
    let removing = std::env::args().any(|a| a == "--uninstall");
    if removing {
        move_out_of_the_way();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(if removing { "Remove Burrow" } else { "Burrow Setup" })
            .with_inner_size([640.0, 470.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_decorations(false)
            .with_transparent(true),
        centered: true,
        ..Default::default()
    };
    eframe::run_native("Burrow Setup", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
