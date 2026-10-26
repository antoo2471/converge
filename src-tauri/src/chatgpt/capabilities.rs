use serde_json::{json, Value};
pub fn detected(models: &[Value]) -> Value {
    json!({"vision":models.iter().any(|m|m["vision"]==true),"tools":true,"images":false,"context":32768,"note":"Images en entrée selon le modèle ; génération dans le chat lorsque Codex la propose ; pas d’endpoint autonome d’images."})
}
