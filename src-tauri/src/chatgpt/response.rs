use serde_json::{json, Value};

pub fn safe_error(value: &Value) -> String {
    let code = value["codexErrorInfo"].as_str().unwrap_or("");
    // Inspect for classification, never relay arbitrary server text, tokens, URLs or headers.
    let description = format!("{code} {}", value["message"].as_str().unwrap_or("")).to_lowercase();
    if description.contains("429")
        || description.contains("usage")
        || description.contains("quota")
        || description.contains("rate limit")
    {
        "Limite d’utilisation Codex atteinte. Consultez les limites du compte et réessayez après leur réinitialisation.".into()
    } else if description.contains("401")
        || description.contains("auth")
        || description.contains("expired")
    {
        "Session Codex expirée ou révoquée. Reconnectez votre compte ChatGPT.".into()
    } else if description.contains("403")
        || description.contains("access")
        || description.contains("not supported")
        || description.contains("modelnotfound")
    {
        "Accès refusé à ce modèle ou à cette capacité pour ce compte Codex. Choisissez un autre modèle.".into()
    } else {
        "Codex a refusé la demande ou interrompu la connexion. Vérifiez le compte, le modèle et la version de Codex CLI.".into()
    }
}
pub fn normalize(value: &Value) -> Option<Value> {
    let params = &value["params"];
    match value["method"].as_str()? {
        "item/agentMessage/delta" => Some(json!({"type":"delta","text":params["delta"]})),
        "item/reasoning/summaryTextDelta" => {
            Some(json!({"type":"reasoning","text":params["delta"]}))
        }
        "turn/started" => Some(json!({"type":"execution","text":"Codex travaille…"})),
        _ => None,
    }
}
pub async fn generated_image(item: &Value, root: &std::path::Path) -> Result<Value, String> {
    use base64::Engine;
    if !item["failure"].is_null() || item["status"] == "failed" {
        return Err(if item["failure"]["type"] == "usageLimitExceeded" {
            "Limite de génération d’images atteinte pour ce compte ChatGPT."
        } else {
            "La génération d’image a échoué côté Codex."
        }
        .into());
    }
    let result = item["result"].as_str().unwrap_or("");
    let bytes = if !result.is_empty() {
        let data = result
            .strip_prefix("data:")
            .and_then(|s| s.split_once(",").map(|(_, data)| data))
            .unwrap_or(result);
        if data.len() > 36 * 1024 * 1024 {
            return Err("Image générée trop volumineuse".into());
        }
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "Codex a renvoyé une image illisible")?
    } else if let Some(path) = item["savedPath"].as_str() {
        let root = root
            .canonicalize()
            .map_err(|_| "Dossier privé Codex indisponible")?;
        let path = std::path::PathBuf::from(path)
            .canonicalize()
            .map_err(|_| "Le fichier d’image annoncé par Codex est introuvable")?;
        if !path.starts_with(&root) {
            return Err("L’image annoncée est située en dehors du dossier privé Codex".into());
        }
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|_| "Image Codex inaccessible")?;
        if metadata.len() > 25 * 1024 * 1024 {
            return Err("Image générée trop volumineuse".into());
        }
        tokio::fs::read(path)
            .await
            .map_err(|_| "Image Codex inaccessible")?
    } else {
        return Err("Codex n’a renvoyé aucun contenu pour l’image générée".into());
    };
    if bytes.len() > 25 * 1024 * 1024 {
        return Err("Image générée trop volumineuse".into());
    }
    let format = image::guess_format(&bytes).map_err(|_| "Format de l’image Codex non reconnu")?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err("Format d’image Codex non pris en charge".into()),
    };
    let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format)
        .into_dimensions()
        .map_err(|_| "Image Codex endommagée")?;
    if u64::from(width) * u64::from(height) > 50_000_000 {
        return Err("Image Codex trop grande".into());
    }
    Ok(
        json!({"type":"image","data":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"prompt":item["revisedPrompt"].as_str().unwrap_or("Image générée")}),
    )
}
pub fn tool_output(result: &str, success: bool) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(result) {
        if v["kind"] == "computer_observation"
            && v["image"]
                .as_str()
                .is_some_and(|s| s.starts_with("data:image/"))
        {
            let mut meta = v.clone();
            meta.as_object_mut().unwrap().remove("image");
            return json!({"success":success,"contentItems":[{"type":"inputText","text":meta.to_string()},{"type":"inputImage","imageUrl":v["image"]}]});
        }
    }
    json!({"success":success,"contentItems":[{"type":"inputText","text":result}]})
}
