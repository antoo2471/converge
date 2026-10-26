use super::codex_transport::CodexTransport;
use serde_json::{json, Value};
pub async fn discover(transport: &CodexTransport) -> Result<Vec<Value>, String> {
    let mut cursor = Value::Null;
    let mut models = vec![];
    for _ in 0..20 {
        let page = transport
            .rpc(
                "model/list",
                json!({"limit":100,"includeHidden":false,"cursor":cursor}),
            )
            .await?;
        for m in page["data"].as_array().ok_or("Catalogue Codex invalide")? {
            if m["hidden"] == true {
                continue;
            }
            let Some(model) = m["model"].as_str().or(m["id"].as_str()) else {
                continue;
            };
            models.push(json!({"id":model,"name":m["displayName"].as_str().unwrap_or(model),"efforts":m["supportedReasoningEfforts"].as_array().map(|a| a.iter().filter_map(|e|e["reasoningEffort"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),"defaultEffort":m["defaultReasoningEffort"],"vision":m["inputModalities"].as_array().map(|a|a.iter().any(|v|v=="image")).unwrap_or(true)}));
        }
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            return Ok(models);
        }
    }
    Err("Catalogue Codex trop volumineux".into())
}
