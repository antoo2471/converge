use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Byte-oriented framing preserves UTF-8 split across transport chunks. SSE data
/// is collected until its blank-line delimiter; Ollama uses NDJSON instead.
pub struct Decoder {
    protocol: String,
    pending: Vec<u8>,
    data: Vec<String>,
}
impl Decoder {
    pub fn new(protocol: &str) -> Self {
        Self {
            protocol: protocol.into(),
            pending: vec![],
            data: vec![],
        }
    }
    fn flush(&mut self, events: &mut Vec<Value>) -> Result<(), String> {
        if self.data.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.data).join("\n");
        if data.trim() == "[DONE]" {
            events.push(json!({"_end":true}));
        } else {
            events.push(serde_json::from_str(&data).map_err(|_| "Événement SSE JSON invalide")?);
        }
        Ok(())
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Value>, String> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > 4 * 1024 * 1024 {
            return Err("Événement trop volumineux".into());
        }
        let mut events = vec![];
        while let Some(index) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=index).collect();
            let line = std::str::from_utf8(&line)
                .map_err(|_| "Flux UTF-8 invalide")?
                .trim_end_matches(['\r', '\n']);
            if self.protocol == "ollama" {
                if !line.trim().is_empty() {
                    events
                        .push(serde_json::from_str(line).map_err(|_| "Événement NDJSON invalide")?);
                }
            } else if line.is_empty() {
                self.flush(&mut events)?
            } else if let Some(data) = line.strip_prefix("data:") {
                self.data
                    .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
            }
        }
        Ok(events)
    }
    pub fn finish(&mut self) -> Result<Vec<Value>, String> {
        let mut events = self.feed(b"\n")?;
        self.flush(&mut events)?;
        Ok(events)
    }
}
pub struct Accumulator {
    protocol: String,
    pub done: bool,
    calls: BTreeMap<usize, (String, String)>,
}
impl Accumulator {
    pub fn new(protocol: &str) -> Self {
        Self {
            protocol: protocol.into(),
            done: false,
            calls: BTreeMap::new(),
        }
    }
    pub fn ingest(&mut self, value: &Value) -> Result<Option<String>, String> {
        if !value["error"].is_null() {
            return Err("Le moteur a signalé une erreur pendant la génération".into());
        }
        self.done |=
            value["_end"] == true || value["done"] == true || value["type"] == "message_stop";
        if self.protocol == "gemini" {
            if let Some(reason) = value["candidates"][0]["finishReason"].as_str() {
                if reason != "STOP" {
                    return Err(
                        "La réponse Gemini a été interrompue ou filtrée par le service".into(),
                    );
                }
                self.done = true;
            }
            let mut text = String::new();
            for part in value["candidates"][0]["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if part["thought"] != true {
                    if let Some(s) = part["text"].as_str() {
                        text.push_str(s);
                    }
                }
                if let Some(call) = part.get("functionCall") {
                    if self.calls.len() >= 16 {
                        return Err("Trop d’appels Gemini".into());
                    }
                    self.calls.insert(
                        self.calls.len(),
                        (
                            call["name"].as_str().unwrap_or_default().into(),
                            call["args"].to_string(),
                        ),
                    );
                }
            }
            return Ok(if text.is_empty() { None } else { Some(text) });
        }
        if self.protocol == "responses" {
            if matches!(
                value["type"].as_str(),
                Some("response.failed" | "response.incomplete" | "error")
            ) {
                return Err("La génération Responses a échoué ou a été tronquée".into());
            }
            self.done |= value["type"] == "response.completed";
            let index = value["output_index"].as_u64().unwrap_or(0) as usize;
            if index > 64 {
                return Err("Trop de blocs Responses".into());
            }
            if value["type"] == "response.output_item.added"
                && value["item"]["type"] == "function_call"
            {
                self.calls.insert(
                    index,
                    (
                        value["item"]["name"].as_str().unwrap_or_default().into(),
                        String::new(),
                    ),
                );
            }
            if value["type"] == "response.function_call_arguments.delta" {
                self.calls
                    .entry(index)
                    .or_default()
                    .1
                    .push_str(value["delta"].as_str().unwrap_or_default());
            }
            if value["type"] == "response.function_call_arguments.done" {
                self.calls.entry(index).or_default().1 =
                    value["arguments"].as_str().unwrap_or_default().into();
            }
            if self.calls.len() > 16 || self.calls.values().any(|(_, a)| a.len() > 1024 * 1024) {
                return Err("Appel Responses trop volumineux".into());
            }
            return Ok(if value["type"] == "response.output_text.delta" {
                value["delta"].as_str().map(str::to_owned)
            } else {
                None
            });
        }
        let text = match self.protocol.as_str() {
            "ollama" => value["message"]["content"].as_str(),
            "anthropic" => value["delta"]["text"].as_str(),
            _ => value["choices"][0]["delta"]["content"].as_str(),
        }
        .map(str::to_string);
        if self.protocol == "openai" {
            if let Some(list) = value["choices"][0]["delta"]["tool_calls"].as_array() {
                for call in list {
                    let index = call["index"].as_u64().unwrap_or(0) as usize;
                    if index >= 16 {
                        return Err("Trop d’appels d’outils".into());
                    }
                    let entry = self.calls.entry(index).or_default();
                    if let Some(name) = call["function"]["name"].as_str() {
                        entry.0.push_str(name)
                    }
                    if let Some(args) = call["function"]["arguments"].as_str() {
                        entry.1.push_str(args)
                    }
                }
            }
        } else if self.protocol == "anthropic" {
            let index = value["index"].as_u64().unwrap_or(0) as usize;
            if index >= 16 {
                return Err("Trop de blocs de réponse".into());
            }
            if value["type"] == "content_block_start"
                && value["content_block"]["type"] == "tool_use"
            {
                self.calls.insert(
                    index,
                    (
                        value["content_block"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        String::new(),
                    ),
                );
            }
            if let Some(args) = value["delta"]["partial_json"].as_str() {
                self.calls.entry(index).or_default().1.push_str(args);
            }
        } else if let Some(list) = value["message"]["tool_calls"].as_array() {
            for call in list {
                let name = call["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let arguments = &call["function"]["arguments"];
                let args = if let Some(s) = arguments.as_str() {
                    s.to_owned()
                } else {
                    arguments.to_string()
                };
                self.calls.insert(self.calls.len(), (name, args));
            }
        }
        if self.calls.len() > 16
            || self
                .calls
                .values()
                .any(|(name, args)| name.len() > 120 || args.len() > 1024 * 1024)
        {
            return Err("Appel d’outil trop volumineux".into());
        }
        Ok(text)
    }
    pub fn progress(&self) -> Vec<Value> {
        self.calls.iter().filter(|(_, (name,_))|!name.is_empty()).map(|(index,(name,args))|json!({"type":"tool_progress","index":index,"name":name,"arguments":args})).collect()
    }
    pub fn tools(&self) -> Result<Vec<Value>, String> {
        self.calls
            .values()
            .map(|(name, args)| {
                let args: Value = serde_json::from_str(if args.is_empty() { "{}" } else { args })
                    .map_err(|_| "Arguments d’outil JSON invalides")?;
                if !args.is_object() {
                    return Err("Les arguments d’outil doivent être un objet".into());
                }
                Ok(json!({"type":"tool","name":name,"args":args}))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_preserves_fragmented_unicode_and_multi_line_data() {
        let mut d = Decoder::new("openai");
        let bytes = "data: {\"text\":\n data: \"été\"}\n\n"
            .replace("\n data:", "\ndata:")
            .into_bytes();
        let mut all = vec![];
        for b in bytes.chunks(1) {
            all.extend(d.feed(b).unwrap())
        }
        assert_eq!(all[0]["text"], "été");
    }
    #[test]
    fn accepts_final_ndjson_without_newline() {
        let mut d = Decoder::new("ollama");
        assert!(d.feed(b"{\"done\":true}").unwrap().is_empty());
        assert_eq!(d.finish().unwrap()[0]["done"], true);
    }
    #[test]
    fn handles_comments_and_done() {
        let mut d = Decoder::new("openai");
        let v = d
            .feed(b": ping\r\nevent: message\r\ndata: [DONE]\r\n\r\n")
            .unwrap();
        let mut a = Accumulator::new("openai");
        a.ingest(&v[0]).unwrap();
        assert!(a.done);
    }
    #[test]
    fn assembles_native_tool_arguments() {
        let mut a = Accumulator::new("openai");
        a.ingest(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"filesystem_read","arguments":"{\"path\":"}}]}}]})).unwrap();
        assert_eq!(a.progress()[0]["arguments"], "{\"path\":");
        a.ingest(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"README.md\"}"}}]}}]})).unwrap();
        assert_eq!(a.tools().unwrap()[0]["args"]["path"], "README.md");
    }
    #[test]
    fn propagates_server_errors() {
        assert!(Accumulator::new("anthropic")
            .ingest(&json!({"type":"error","error":{"message":"overloaded"}}))
            .is_err());
    }
    #[test]
    fn responses_stream_assembles_text_and_native_tools() {
        let mut a = Accumulator::new("responses");
        assert_eq!(
            a.ingest(&json!({"type":"response.output_text.delta","delta":"Bonjour"}))
                .unwrap()
                .unwrap(),
            "Bonjour"
        );
        a.ingest(&json!({"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","name":"filesystem_read"}})).unwrap();
        a.ingest(&json!({"type":"response.function_call_arguments.delta","output_index":1,"delta":"{\"path\":"})).unwrap();
        a.ingest(&json!({"type":"response.function_call_arguments.delta","output_index":1,"delta":"\"README.md\"}"})).unwrap();
        a.ingest(&json!({"type":"response.completed"})).unwrap();
        assert!(a.done);
        assert_eq!(a.tools().unwrap()[0]["args"]["path"], "README.md");
        assert!(Accumulator::new("responses")
            .ingest(&json!({"type":"response.incomplete"}))
            .is_err());
    }
    #[test]
    fn gemini_stream_keeps_tools_and_rejects_truncation() {
        let mut a = Accumulator::new("gemini");
        assert_eq!(a.ingest(&json!({"candidates":[{"content":{"parts":[{"text":"Privé","thought":true},{"text":"Visible"},{"functionCall":{"name":"filesystem_read","args":{"path":"a"}}}]},"finishReason":"STOP"}]})).unwrap().unwrap(),"Visible");
        assert!(a.done);
        assert_eq!(a.tools().unwrap()[0]["args"]["path"], "a");
        assert!(Accumulator::new("gemini")
            .ingest(&json!({"candidates":[{"finishReason":"MAX_TOKENS"}]}))
            .is_err());
    }
}
