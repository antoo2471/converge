use serde_json::{json, Value};
#[derive(Default)]
pub struct Session {
    target: Option<usize>,
    frame: Option<Frame>,
}
#[derive(Clone)]
struct Frame {
    id: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    image_width: i32,
    image_height: i32,
    window: Option<usize>,
}
impl Frame {
    fn point(&self, args: &Value) -> Result<(i32, i32), String> {
        if args["frameId"].as_str().is_some_and(|id| id != self.id) {
            return Err("Capture ancienne : observez de nouveau la fenêtre avant d’agir".into());
        }
        let x = args["x"].as_f64().ok_or("x manquant")?;
        let y = args["y"].as_f64().ok_or("y manquant")?;
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= self.image_width as f64
            || y >= self.image_height as f64
        {
            return Err("Coordonnées hors de la capture".into());
        }
        Ok((
            self.x + (x * self.width as f64 / self.image_width as f64).floor() as i32,
            self.y + (y * self.height as f64 / self.image_height as f64).floor() as i32,
        ))
    }
}
#[cfg(windows)]
fn activate(id: usize) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::HWND,
        System::Threading::*,
        UI::{Input::KeyboardAndMouse::SetActiveWindow, WindowsAndMessaging::*},
    };
    unsafe {
        let window = id as HWND;
        if IsWindow(window) == 0 || IsWindowVisible(window) == 0 {
            return Err("Fenêtre cible indisponible".into());
        }
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        if GetForegroundWindow() == window {
            return Ok(());
        }
        // A worker thread needs an input queue before AttachThreadInput can work.
        let mut message: MSG = std::mem::zeroed();
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        SetForegroundWindow(window);
        for _ in 0..10 {
            if GetForegroundWindow() == window {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let thread = GetCurrentThreadId();
        let foreground = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        let target = GetWindowThreadProcessId(window, std::ptr::null_mut());
        let attached = foreground != 0
            && foreground != thread
            && AttachThreadInput(thread, foreground, 1) != 0;
        let target_attached = target != 0
            && target != thread
            && target != foreground
            && AttachThreadInput(thread, target, 1) != 0;
        BringWindowToTop(window);
        SetActiveWindow(window);
        SetForegroundWindow(window);
        if target_attached {
            AttachThreadInput(thread, target, 0);
        }
        if attached {
            AttachThreadInput(thread, foreground, 0);
        }
        for _ in 0..20 {
            if GetForegroundWindow() == window {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if GetForegroundWindow() != window {
            return Err(
                "Windows a refusé le focus. Action non envoyée ; activez la fenêtre cible.".into(),
            );
        }
        Ok(())
    }
}
#[cfg(windows)]
pub fn execute(
    name: &str,
    args: &Value,
    session: &mut Session,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<String, String> {
    use windows_sys::Win32::UI::{
        HiDpi::*, Input::KeyboardAndMouse::GetAsyncKeyState, WindowsAndMessaging::*,
    };
    struct Dpi(windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
    impl Drop for Dpi {
        fn drop(&mut self) {
            unsafe {
                SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
    let _dpi =
        Dpi(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) });
    if cancel.is_cancelled() {
        return Err("CANCELLED: Computer use annulé".into());
    }
    if unsafe { GetAsyncKeyState(0x1B) } < 0 {
        return Err("CANCELLED: Arrêt par Échap".into());
    }
    if name == "computer.actions" {
        let actions = args["actions"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 12)
            .ok_or("Une séquence contient entre 1 et 12 actions")?;
        for action in actions {
            if !matches!(
                action["type"].as_str(),
                Some(
                    "click" | "type" | "key" | "scroll" | "wait" | "move" | "double_click" | "drag"
                )
            ) {
                return Err("Action non prise en charge".into());
            }
        }
        if let Some(window) = args["windowId"].as_u64() {
            activate(window as usize)?;
            session.target = Some(window as usize);
        }
        for action in actions {
            if cancel.is_cancelled() || unsafe { GetAsyncKeyState(0x1B) } < 0 {
                return Err("CANCELLED: Arrêt du contrôle de l’ordinateur".into());
            }
            let mut action = action.clone();
            if action.get("frameId").is_none() {
                if let Some(id) = args.get("frameId") {
                    action["frameId"] = id.clone();
                }
            }
            if let Err(error) = action_only(
                &format!("computer.{}", action["type"].as_str().unwrap()),
                &action,
                session,
            ) {
                session.frame = None;
                return Err(format!("Séquence interrompue : {error}. Les actions précédentes ont pu être exécutées. Capturez de nouveau la fenêtre avant de continuer."));
            }
        }
    } else {
        if name == "computer.capture" {
            settle(args, cancel)?;
        }
        let result = action_only(name, args, session)?;
        if matches!(name, "computer.capture" | "computer.windows") {
            return Ok(result);
        }
    }
    settle(args, cancel)?;
    let foreground = unsafe { GetForegroundWindow() } as usize;
    if foreground != 0 {
        session.target = Some(foreground);
    }
    action_only("computer.capture", &json!({}), session)
}
fn settle(args: &Value, cancel: &tokio_util::sync::CancellationToken) -> Result<(), String> {
    let milliseconds = args["settleMs"].as_u64().unwrap_or(700).min(5000);
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < milliseconds as u128 {
        if cancel.is_cancelled() {
            return Err("CANCELLED: Contrôle annulé pendant la stabilisation".into());
        }
        #[cfg(windows)]
        if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x1B) } < 0 {
            return Err("CANCELLED: Arrêt par Échap".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}
#[cfg(windows)]
fn action_only(name: &str, args: &Value, session: &mut Session) -> Result<String, String> {
    use base64::Engine;
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };
    unsafe {
        if let Some(id) = args["windowId"].as_u64() {
            activate(id as usize)?;
            session.target = Some(id as usize);
        }
        if matches!(name, "computer.type" | "computer.key" | "computer.scroll") {
            activate(
                session
                    .target
                    .ok_or("Choisissez une fenêtre avec computer.focus")?,
            )?;
        }
        match name {
            "computer.capture" => {
                let desktop = args["scope"].as_str() == Some("desktop");
                let window = if desktop {
                    None
                } else {
                    session
                        .target
                        .or(Some(GetForegroundWindow() as usize))
                        .filter(|id| *id != 0)
                };
                let (mut x, mut y, mut w, mut h) = (
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                );
                if let Some(id) = window {
                    activate(id)?;
                    let mut rect: RECT = std::mem::zeroed();
                    if GetWindowRect(id as HWND, &mut rect) == 0 {
                        return Err("Dimensions de la fenêtre indisponibles".into());
                    }
                    let right = rect.right.min(x + w);
                    let bottom = rect.bottom.min(y + h);
                    x = rect.left.max(x);
                    y = rect.top.max(y);
                    w = right - x;
                    h = bottom - y;
                    session.target = Some(id);
                }
                if w <= 0 || h <= 0 || w as u64 * h as u64 > 80_000_000 {
                    return Err("Dimensions du bureau invalides".into());
                }
                let dc = GetDC(std::ptr::null_mut());
                if dc.is_null() {
                    return Err("Capture indisponible sur ce bureau".into());
                }
                let memory = CreateCompatibleDC(dc);
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = w;
                info.bmiHeader.biHeight = -h;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut bits = std::ptr::null_mut();
                let bitmap = CreateDIBSection(
                    dc,
                    &info,
                    DIB_RGB_COLORS,
                    &mut bits,
                    std::ptr::null_mut(),
                    0,
                );
                if bitmap.is_null() || bits.is_null() {
                    DeleteDC(memory);
                    ReleaseDC(std::ptr::null_mut(), dc);
                    return Err("Allocation de capture impossible".into());
                }
                let old = SelectObject(memory, bitmap);
                let success = BitBlt(memory, 0, 0, w, h, dc, x, y, SRCCOPY | CAPTUREBLT);
                GdiFlush();
                let mut pixels =
                    std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
                SelectObject(memory, old);
                DeleteObject(bitmap);
                DeleteDC(memory);
                ReleaseDC(std::ptr::null_mut(), dc);
                if success == 0 {
                    return Err("Windows a refusé la capture".into());
                }
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255
                }
                let image = image::RgbaImage::from_raw(w as u32, h as u32, pixels)
                    .ok_or("Capture invalide")?;
                let scaled = image::DynamicImage::ImageRgba8(image)
                    .resize(
                        (w as u32).min(1568),
                        (h as u32).min(1100),
                        image::imageops::FilterType::Triangle,
                    )
                    .to_rgb8();
                let (iw, ih) = scaled.dimensions();
                let mut jpeg = vec![];
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 88)
                    .encode(&scaled, iw, ih, image::ExtendedColorType::Rgb8)
                    .map_err(|e| e.to_string())?;
                let frame = Frame {
                    id: uuid::Uuid::new_v4().to_string(),
                    x,
                    y,
                    width: w,
                    height: h,
                    image_width: iw as i32,
                    image_height: ih as i32,
                    window,
                };
                let result = json!({"kind":"computer_observation","image":format!("data:image/jpeg;base64,{}",base64::engine::general_purpose::STANDARD.encode(jpeg)),"frameId":frame.id,"windowId":window,"width":iw,"height":ih,"desktopOrigin":{"x":x,"y":y},"physicalWidth":w,"physicalHeight":h,"coordinateSpace":"image pixels: top-left (0,0). Coordinates are mapped to physical screen pixels by Converge.","instruction":"Inspect this screenshot. If the requested outcome is achieved, STOP and answer without more tools. Otherwise perform only the next necessary step. Do not repeatedly recheck success."});
                session.frame = Some(frame);
                Ok(result.to_string())
            }
            "computer.windows" => {
                unsafe extern "system" fn callback(hwnd: HWND, param: LPARAM) -> i32 {
                    if IsWindowVisible(hwnd) == 0 {
                        return 1;
                    }
                    let mut title = [0u16; 1024];
                    let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 1024);
                    if len > 0 {
                        let list = &mut *(param as *mut Vec<Value>);
                        let mut rect: RECT = std::mem::zeroed();
                        GetWindowRect(hwnd, &mut rect);
                        list.push(serde_json::json!({"windowId":hwnd as usize,"title":String::from_utf16_lossy(&title[..len as usize]),"x":rect.left,"y":rect.top,"width":rect.right-rect.left,"height":rect.bottom-rect.top,"foreground":GetForegroundWindow()==hwnd}));
                    }
                    1
                }
                let mut list: Vec<Value> = vec![];
                EnumWindows(Some(callback), &mut list as *mut _ as isize);
                Ok(serde_json::json!(list).to_string())
            }
            "computer.focus" => {
                let id = args["windowId"].as_u64().ok_or("windowId manquant")? as usize;
                activate(id)?;
                session.target = Some(id);
                session.frame = None;
                Ok("Fenêtre activée".into())
            }
            "computer.click" | "computer.double_click" | "computer.move" => {
                let frame = session
                    .frame
                    .as_ref()
                    .ok_or("Capturez la fenêtre avant de choisir des coordonnées")?;
                if let Some(id) = frame.window {
                    activate(id)?;
                    let mut rect: RECT = std::mem::zeroed();
                    GetWindowRect(id as HWND, &mut rect);
                    if rect.left.max(GetSystemMetrics(SM_XVIRTUALSCREEN)) != frame.x
                        || rect.top.max(GetSystemMetrics(SM_YVIRTUALSCREEN)) != frame.y
                        || (rect.right.min(
                            GetSystemMetrics(SM_XVIRTUALSCREEN)
                                + GetSystemMetrics(SM_CXVIRTUALSCREEN),
                        ) - frame.x)
                            != frame.width
                        || (rect.bottom.min(
                            GetSystemMetrics(SM_YVIRTUALSCREEN)
                                + GetSystemMetrics(SM_CYVIRTUALSCREEN),
                        ) - frame.y)
                            != frame.height
                    {
                        return Err(
                            "La fenêtre a bougé. Demandez une nouvelle capture avant de cliquer."
                                .into(),
                        );
                    }
                }
                let (x, y) = frame.point(args)?;
                if SetCursorPos(x, y) == 0 {
                    return Err("Déplacement du curseur refusé".into());
                }
                if name == "computer.move" {
                    return Ok("Pointeur déplacé".into());
                }
                let (down, up) = match args["button"].as_str().unwrap_or("left") {
                    "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                    "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                    _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                };
                let mut input: [INPUT; 2] = std::mem::zeroed();
                for (i, flags) in [down, up].into_iter().enumerate() {
                    input[i].r#type = INPUT_MOUSE;
                    input[i].Anonymous.mi.dwFlags = flags;
                }
                if SendInput(2, input.as_ptr(), std::mem::size_of::<INPUT>() as i32) != 2 {
                    return Err("Clic refusé par Windows".into());
                }
                if name == "computer.double_click" {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    if SendInput(2, input.as_ptr(), std::mem::size_of::<INPUT>() as i32) != 2 {
                        return Err("Double clic refusé".into());
                    }
                }
                session.target = Some(GetForegroundWindow() as usize);
                Ok("Clic effectué".into())
            }
            "computer.drag" => {
                let path = args["path"]
                    .as_array()
                    .filter(|p| p.len() >= 2 && p.len() <= 100)
                    .ok_or("Le glisser-déposer nécessite 2 à 100 points")?;
                let frame = session
                    .frame
                    .as_ref()
                    .ok_or("Capturez la fenêtre avant de glisser")?
                    .clone();
                let mut points = vec![];
                for point in path {
                    let mut point = point.clone();
                    point["frameId"] = args["frameId"].clone();
                    points.push(frame.point(&point)?);
                }
                let mut start = path[0].clone();
                start["frameId"] = args["frameId"].clone();
                action_only("computer.move", &start, session)?;
                struct MouseUp;
                impl Drop for MouseUp {
                    fn drop(&mut self) {
                        unsafe {
                            let mut input: INPUT = std::mem::zeroed();
                            input.r#type = INPUT_MOUSE;
                            input.Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;
                            SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
                        }
                    }
                }
                let mut input: INPUT = std::mem::zeroed();
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
                if SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) != 1 {
                    return Err("Glisser-déposer refusé".into());
                }
                let _release = MouseUp;
                for (x, y) in points {
                    if GetAsyncKeyState(VK_ESCAPE as i32) < 0 {
                        return Err("CANCELLED: Arrêt du glisser-déposer".into());
                    }
                    if SetCursorPos(x, y) == 0 {
                        return Err("Déplacement refusé".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(8));
                }
                Ok("Glisser-déposer effectué".into())
            }
            "computer.type" => {
                let text = args["text"].as_str().ok_or("text manquant")?;
                if text.len() > 16000 {
                    return Err("Texte limité à 16 Ko".into());
                }
                let mut input = vec![];
                for code in text.encode_utf16() {
                    for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
                        let mut i: INPUT = std::mem::zeroed();
                        i.r#type = INPUT_KEYBOARD;
                        i.Anonymous.ki.wScan = code;
                        i.Anonymous.ki.dwFlags = flags;
                        input.push(i)
                    }
                }
                if !input.is_empty()
                    && SendInput(
                        input.len() as u32,
                        input.as_ptr(),
                        std::mem::size_of::<INPUT>() as i32,
                    ) != input.len() as u32
                {
                    return Err("Saisie refusée par Windows".into());
                }
                Ok("Texte saisi".into())
            }
            "computer.key" => {
                let key = args["key"].as_str().ok_or("key manquant")?;
                let codes = key
                    .split('+')
                    .map(|k| match k.to_lowercase().as_str() {
                        "ctrl" | "control" => Ok(VK_CONTROL),
                        "alt" => Ok(VK_MENU),
                        "shift" => Ok(VK_SHIFT),
                        "win" | "meta" => Ok(VK_LWIN),
                        "enter" | "return" => Ok(VK_RETURN),
                        "pageup" => Ok(VK_PRIOR),
                        "pagedown" => Ok(VK_NEXT),
                        "f1" => Ok(VK_F1),
                        "f2" => Ok(VK_F2),
                        "f3" => Ok(VK_F3),
                        "f4" => Ok(VK_F4),
                        "f5" => Ok(VK_F5),
                        "f6" => Ok(VK_F6),
                        "f7" => Ok(VK_F7),
                        "f8" => Ok(VK_F8),
                        "f9" => Ok(VK_F9),
                        "f10" => Ok(VK_F10),
                        "f11" => Ok(VK_F11),
                        "f12" => Ok(VK_F12),
                        "tab" => Ok(VK_TAB),
                        "escape" | "esc" => Ok(VK_ESCAPE),
                        "backspace" => Ok(VK_BACK),
                        "delete" => Ok(VK_DELETE),
                        "up" => Ok(VK_UP),
                        "down" => Ok(VK_DOWN),
                        "left" => Ok(VK_LEFT),
                        "right" => Ok(VK_RIGHT),
                        "space" => Ok(VK_SPACE),
                        "home" => Ok(VK_HOME),
                        "end" => Ok(VK_END),
                        v if v.len() == 1 && v.as_bytes()[0].is_ascii_alphanumeric() => {
                            Ok(v.as_bytes()[0].to_ascii_uppercase() as u16)
                        }
                        _ => Err("Touche non reconnue"),
                    })
                    .collect::<Result<Vec<u16>, _>>()?;
                if codes.len() > 5 {
                    return Err("Raccourci trop long".into());
                }
                let mut input = vec![];
                for (code, up) in codes
                    .iter()
                    .map(|c| (*c, false))
                    .chain(codes.iter().rev().map(|c| (*c, true)))
                {
                    let mut i: INPUT = std::mem::zeroed();
                    i.r#type = INPUT_KEYBOARD;
                    i.Anonymous.ki.wVk = code;
                    i.Anonymous.ki.dwFlags = if up { KEYEVENTF_KEYUP } else { 0 };
                    input.push(i);
                }
                if SendInput(
                    input.len() as u32,
                    input.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                ) != input.len() as u32
                {
                    return Err("Raccourci refusé par Windows".into());
                }
                Ok("Raccourci envoyé".into())
            }
            "computer.wait" => {
                std::thread::sleep(std::time::Duration::from_millis(
                    args["milliseconds"].as_u64().unwrap_or(250).clamp(1, 2000),
                ));
                Ok("Attente terminée".into())
            }
            "computer.scroll" => {
                let frame = session
                    .frame
                    .as_ref()
                    .ok_or("Capturez la fenêtre avant de faire défiler")?;
                let point = if args.get("x").is_some() {
                    args.clone()
                } else {
                    json!({"x":frame.image_width/2,"y":frame.image_height/2})
                };
                let (x, y) = frame.point(&point)?;
                if SetCursorPos(x, y) == 0 {
                    return Err("Position de défilement indisponible".into());
                }
                let amount = args["amount"]
                    .as_i64()
                    .ok_or("amount manquant")?
                    .clamp(-100, 100);
                let mut input: INPUT = std::mem::zeroed();
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
                input.Anonymous.mi.mouseData = (amount * 120) as u32;
                if SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) != 1 {
                    return Err("Défilement refusé".into());
                }
                Ok("Défilement effectué".into())
            }
            _ => Err("Outil Computer Use inconnu".into()),
        }
    }
}
#[cfg(not(windows))]
pub fn execute(
    _name: &str,
    _args: &Value,
    _session: &mut Session,
    _cancel: &tokio_util::sync::CancellationToken,
) -> Result<String, String> {
    Err("L’adaptateur Computer Use de cette distribution cible Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observation_delay_waits_and_can_be_cancelled() {
        let token = tokio_util::sync::CancellationToken::new();
        let start = std::time::Instant::now();
        super::settle(&json!({"settleMs":100}), &token).unwrap();
        assert!(start.elapsed().as_millis() >= 100);
        let other = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(40));
            other.cancel();
        });
        let start = std::time::Instant::now();
        assert!(super::settle(&json!({"settleMs":1000}), &token)
            .unwrap_err()
            .contains("CANCELLED:"));
        assert!(start.elapsed().as_millis() < 500);
    }
    #[test]
    fn maps_scaled_coordinates_with_negative_monitor_origin_and_rejects_stale_points() {
        let frame = Frame {
            id: "fresh".into(),
            x: -1920,
            y: -200,
            width: 3840,
            height: 2160,
            image_width: 1568,
            image_height: 882,
            window: None,
        };
        assert_eq!(
            frame
                .point(&json!({"x":784,"y":441,"frameId":"fresh"}))
                .unwrap(),
            (0, 880)
        );
        assert!(frame.point(&json!({"x":1568,"y":0})).is_err());
        assert!(frame.point(&json!({"x":0,"y":-1})).is_err());
        assert!(frame.point(&json!({"x":0,"y":0,"frameId":"old"})).is_err());
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "interactive test, uses only its own disposable window"]
    fn native_focus_screenshot_click_type_and_observe() {
        use windows_sys::Win32::{
            Foundation::*, System::Threading::GetCurrentThreadId, UI::WindowsAndMessaging::*,
        };
        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(Some(0)).collect()
        }
        struct TestWindow {
            window: usize,
            thread: u32,
            join: Option<std::thread::JoinHandle<()>>,
        }
        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    PostMessageW(self.window as HWND, WM_CLOSE, 0, 0);
                    PostThreadMessageW(self.thread, WM_QUIT, 0, 0);
                }
                if let Some(join) = self.join.take() {
                    let _ = join.join();
                }
            }
        }
        let (send, recv) = std::sync::mpsc::channel();
        let join = std::thread::spawn(move || unsafe {
            use windows_sys::Win32::UI::HiDpi::*;
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let window = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("Converge computer use contract test").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                720,
                300,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!window.is_null());
            let edit = CreateWindowExW(
                0,
                wide("EDIT").as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | 0x80,
                30,
                55,
                600,
                45,
                window,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!edit.is_null());
            send.send((window as usize, edit as usize, GetCurrentThreadId()))
                .unwrap();
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            DestroyWindow(window);
        });
        let (window, edit, thread) = recv.recv().unwrap();
        let _window = TestWindow {
            window,
            thread,
            join: Some(join),
        };
        let mut session = Session::default();
        let cancel = tokio_util::sync::CancellationToken::new();
        let observation = execute(
            "computer.focus",
            &json!({"windowId":window}),
            &mut session,
            &cancel,
        )
        .unwrap();
        let value: Value = serde_json::from_str(&observation).unwrap();
        assert_eq!(value["kind"], "computer_observation");
        assert!(value["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        let frame = session.frame.clone().unwrap();
        let mut bounds: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetWindowRect(edit as HWND, &mut bounds);
        }
        let x = ((bounds.left + 20 - frame.x) as f64 * frame.image_width as f64
            / frame.width as f64)
            .round();
        let y = ((bounds.top + 20 - frame.y) as f64 * frame.image_height as f64
            / frame.height as f64)
            .round();
        let result=execute("computer.actions",&json!({"windowId":window,"frameId":frame.id,"actions":[{"type":"click","x":x,"y":y},{"type":"type","text":"CONVERGE_CLICK_AND_TYPE_OK"}]}),&mut session,&cancel).unwrap();
        assert_ne!(
            serde_json::from_str::<Value>(&result).unwrap()["frameId"],
            value["frameId"]
        );
        let mut buffer = [0u16; 128];
        let len = unsafe { GetWindowTextW(edit as HWND, buffer.as_mut_ptr(), 128) };
        assert_eq!(
            String::from_utf16_lossy(&buffer[..len as usize]),
            "CONVERGE_CLICK_AND_TYPE_OK"
        );
        assert!(execute(
            "computer.click",
            &json!({"x":x,"y":y,"frameId":frame.id}),
            &mut session,
            &cancel
        )
        .is_err());
    }
}
