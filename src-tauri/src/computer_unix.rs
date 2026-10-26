use super::*;
use base64::Engine;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use xcap::Window;

fn windows() -> Result<Vec<Window>, String> {
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland") {
        return Err("Le contrôle natif nécessite une session Linux X11. Wayland interdit ce contrôle global sans un portail de bureau distant. Utilisez une session X11 ; aucune action n’a été envoyée.".into());
    }
    Window::all().map_err(|_| "Accès aux fenêtres refusé. Sur macOS, autorisez Converge dans Accessibilité et Enregistrement de l’écran.".into())
}
fn target(session: &Session) -> Result<Window, String> {
    let id = session
        .target
        .ok_or("Choisissez une fenêtre avec computer.focus")?;
    windows()?
        .into_iter()
        .find(|w| w.id().ok().map(|v| v as usize) == Some(id))
        .ok_or("Fenêtre indisponible".into())
}
fn focus(id: usize, session: &mut Session) -> Result<(), String> {
    let window = windows()?
        .into_iter()
        .find(|w| w.id().ok().map(|v| v as usize) == Some(id))
        .ok_or("Fenêtre indisponible")?;
    if !window.is_focused().unwrap_or(false) {
        #[cfg(target_os="macos")]
        let status=std::process::Command::new("/usr/bin/osascript").args(["-e",&format!("tell application \"System Events\" to set frontmost of first application process whose unix id is {} to true",window.pid().map_err(|_| "Processus indisponible")?)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map_err(|_| "Activation de la fenêtre impossible")?;
        #[cfg(target_os = "linux")]
        let status = std::process::Command::new(
            crate::host_environment::executable("wmctrl")
                .ok_or("Installez wmctrl pour activer les fenêtres Linux X11")?,
        )
        .args(["-ia", &format!("0x{id:x}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "Activation de la fenêtre impossible")?;
        if !status.success() {
            return Err("L’OS a refusé l’activation de la fenêtre. Sur macOS, vérifiez l’autorisation Accessibilité.".into());
        }
    }
    session.target = Some(id);
    session.frame = None;
    Ok(())
}
fn capture(args: &Value, session: &mut Session) -> Result<String, String> {
    let window = if args["scope"] == "desktop" {
        None
    } else if session.target.is_some() {
        Some(target(session)?)
    } else {
        windows()?
            .into_iter()
            .find(|w| w.is_focused().unwrap_or(false))
    };
    let (image, x, y, w, h, id) = if let Some(window) = window {
        let image = window.capture_image().map_err(|_| {
            "Capture refusée. Autorisez l’enregistrement de l’écran dans les réglages système."
        })?;
        (
            image,
            window.x().map_err(|_| "Position indisponible")?,
            window.y().map_err(|_| "Position indisponible")?,
            window.width().map_err(|_| "Taille indisponible")? as i32,
            window.height().map_err(|_| "Taille indisponible")? as i32,
            window.id().ok().map(|v| v as usize),
        )
    } else {
        let monitor = xcap::Monitor::all()
            .map_err(|_| "Écrans indisponibles")?
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .ok_or("Écran principal indisponible")?;
        let image = monitor
            .capture_image()
            .map_err(|_| "Capture de l’écran refusée")?;
        #[cfg(target_os = "macos")]
        let scale = monitor.scale_factor().unwrap_or(1.0).max(1.0);
        #[cfg(target_os = "linux")]
        let scale = 1.0f32;
        let w = (image.width() as f32 / scale).round() as i32;
        let h = (image.height() as f32 / scale).round() as i32;
        (
            image,
            monitor.x().unwrap_or(0),
            monitor.y().unwrap_or(0),
            w,
            h,
            None,
        )
    };
    if w <= 0 || h <= 0 || image.width() as u64 * image.height() as u64 > 80_000_000 {
        return Err("Dimensions de capture invalides".into());
    }
    let image = image::DynamicImage::ImageRgba8(image)
        .resize(1568, 1100, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let (iw, ih) = image.dimensions();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
        .encode(image.as_raw(), iw, ih, image::ExtendedColorType::Rgb8)
        .map_err(|_| "Encodage de capture impossible")?;
    let frame = Frame {
        id: uuid::Uuid::new_v4().to_string(),
        x,
        y,
        width: w,
        height: h,
        image_width: iw as i32,
        image_height: ih as i32,
        window: id,
    };
    let result = json!({"kind":"computer_observation","image":format!("data:image/jpeg;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"frameId":frame.id,"windowId":id,"width":iw,"height":ih,"desktopOrigin":{"x":x,"y":y},"nativeWidth":w,"nativeHeight":h,"coordinateSpace":"image pixels, mapped by Converge to native desktop coordinates","instruction":"Act on the visible target using this frameId. Do not repeatedly focus/capture. Stop and explain the result when the requested outcome is visible."});
    session.frame = Some(frame);
    if id.is_some() {
        session.target = id;
    }
    Ok(result.to_string())
}
fn key(value: &str) -> Result<Key, String> {
    Ok(match value.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Key::Control,
        "alt" | "option" => Key::Alt,
        "shift" => Key::Shift,
        "meta" | "cmd" | "command" | "super" => Key::Meta,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "escape" | "esc" => {
            return Err("Échap est réservé à la reprise manuelle du contrôle".into())
        }
        _ => {
            let mut chars = value.chars();
            let c = chars.next().ok_or("Touche manquante")?;
            if chars.next().is_some() {
                return Err("Touche non prise en charge".into());
            }
            Key::Unicode(c)
        }
    })
}
fn action(
    name: &str,
    args: &Value,
    session: &mut Session,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), String> {
    if name == "computer.wait" {
        return settle(
            &json!({"settleMs":args["milliseconds"].as_u64().unwrap_or(700).min(5000)}),
            cancel,
        );
    }
    let window = target(session)?;
    if !window.is_focused().unwrap_or(false) {
        return Err("La fenêtre cible n’est plus au premier plan. Aucune action envoyée.".into());
    }
    if let Some(frame) = &session.frame {
        if frame.window.is_some()
            && (window.x().ok() != Some(frame.x)
                || window.y().ok() != Some(frame.y)
                || window.width().ok().map(|v| v as i32) != Some(frame.width)
                || window.height().ok().map(|v| v as i32) != Some(frame.height))
        {
            return Err("Fenêtre déplacée : capturez-la de nouveau avant d’agir".into());
        }
    }
    let mut input=Enigo::new(&Settings::default()).map_err(|_| "Contrôle refusé. Vérifiez les autorisations Accessibilité macOS ou votre session Linux X11.")?;
    match name {
        "computer.click" | "computer.double_click" | "computer.move" => {
            let (x, y) = session
                .frame
                .as_ref()
                .ok_or("Capturez la fenêtre avant d’agir")?
                .point(args)?;
            input
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|_| "Déplacement refusé")?;
            if name != "computer.move" {
                let button = match args["button"].as_str().unwrap_or("left") {
                    "left" => Button::Left,
                    "right" => Button::Right,
                    "middle" => Button::Middle,
                    _ => return Err("Bouton inconnu".into()),
                };
                input
                    .button(button, Direction::Click)
                    .map_err(|_| "Clic refusé")?;
                if name == "computer.double_click" {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    input
                        .button(button, Direction::Click)
                        .map_err(|_| "Double-clic refusé")?;
                }
            }
        }
        "computer.type" => {
            let text = args["text"]
                .as_str()
                .filter(|s| s.len() <= 20000)
                .ok_or("Texte invalide")?;
            input.text(text).map_err(|_| "Saisie refusée")?;
        }
        "computer.key" => {
            let keys = args["key"]
                .as_str()
                .ok_or("Touche manquante")?
                .split('+')
                .map(key)
                .collect::<Result<Vec<_>, _>>()?;
            if keys.len() > 5 {
                return Err("Raccourci trop long".into());
            }
            let mut pressed = vec![];
            let result: Result<(), String> = (|| {
                for key in keys {
                    input
                        .key(key, Direction::Press)
                        .map_err(|_| "Raccourci refusé")?;
                    pressed.push(key);
                }
                Ok(())
            })();
            for key in pressed.into_iter().rev() {
                let _ = input.key(key, Direction::Release);
            }
            result?;
        }
        "computer.scroll" => {
            let amount = args["amount"]
                .as_i64()
                .ok_or("Défilement manquant")?
                .clamp(-100, 100) as i32;
            input
                .scroll(-amount, Axis::Vertical)
                .map_err(|_| "Défilement refusé")?;
        }
        "computer.drag" => {
            let path = args["path"]
                .as_array()
                .filter(|p| p.len() >= 2 && p.len() <= 100)
                .ok_or("Trajet invalide")?;
            let frame = session.frame.as_ref().ok_or("Capture manquante")?;
            let points = path
                .iter()
                .map(|p| frame.point(&json!({"x":p["x"],"y":p["y"],"frameId":args["frameId"]})))
                .collect::<Result<Vec<_>, _>>()?;
            input
                .move_mouse(points[0].0, points[0].1, Coordinate::Abs)
                .map_err(|_| "Déplacement refusé")?;
            input
                .button(Button::Left, Direction::Press)
                .map_err(|_| "Glisser-déposer refusé")?;
            let result: Result<(), String> = (|| {
                for (x, y) in points.into_iter().skip(1) {
                    if cancel.is_cancelled() {
                        return Err("CANCELLED: Contrôle arrêté".into());
                    }
                    input
                        .move_mouse(x, y, Coordinate::Abs)
                        .map_err(|_| "Glisser-déposer refusé")?;
                    std::thread::sleep(std::time::Duration::from_millis(16));
                }
                Ok(())
            })();
            let _ = input.button(Button::Left, Direction::Release);
            result?;
        }
        _ => return Err("Action Computer Use inconnue".into()),
    }
    Ok(())
}
pub(super) fn execute(
    name: &str,
    args: &Value,
    session: &mut Session,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<String, String> {
    if cancel.is_cancelled() {
        return Err("CANCELLED: Contrôle arrêté".into());
    }
    if name == "computer.windows" {
        let rows=windows()?.into_iter().filter_map(|w|{let title=w.title().ok()?;if title.is_empty()||title=="Converge computer control"{return None;}Some(json!({"windowId":w.id().ok()?,"title":title,"app":w.app_name().ok(),"x":w.x().ok(),"y":w.y().ok(),"width":w.width().ok(),"height":w.height().ok(),"foreground":w.is_focused().unwrap_or(false)}))}).take(100).collect::<Vec<_>>();
        return Ok(json!(rows).to_string());
    }
    if let Some(id) = args["windowId"].as_u64() {
        if name == "computer.focus" || session.target != Some(id as usize) {
            focus(id as usize, session)?;
        }
    }
    if name == "computer.focus" {
        if args["windowId"].as_u64().is_none() {
            return Err("windowId manquant".into());
        }
        settle(args, cancel)?;
        return capture(&json!({}), session);
    }
    if name == "computer.capture" {
        settle(args, cancel)?;
        return capture(args, session);
    }
    if name == "computer.actions" {
        let actions = args["actions"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 12)
            .ok_or("Une séquence contient 1 à 12 actions")?;
        for item in actions {
            if cancel.is_cancelled() {
                return Err("CANCELLED: Contrôle arrêté".into());
            }
            let mut item = item.clone();
            if item.get("frameId").is_none() {
                item["frameId"] = args["frameId"].clone();
            }
            if let Err(error) = action(
                &format!("computer.{}", item["type"].as_str().unwrap_or("")),
                &item,
                session,
                cancel,
            ) {
                session.frame = None;
                return Err(error);
            }
        }
    } else {
        action(name, args, session, cancel)?;
    }
    settle(args, cancel)?;
    capture(&json!({}), session)
}
