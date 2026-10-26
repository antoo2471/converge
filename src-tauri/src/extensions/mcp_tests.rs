use super::*;
#[tokio::test]
async fn google_uses_the_initialize_protocol_required_by_legacy_servers() {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let (client, server) = tokio::io::duplex(8192);
    let task = tokio::spawn(async move {
        let (reader, mut writer) = tokio::io::split(server);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["method"], "initialize");
        assert_eq!(request["params"]["protocolVersion"], "2025-03-26");
        writer
            .write_all(format!("{}\n", reply(&request)).as_bytes())
            .await
            .unwrap();
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let notification: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(notification["method"], "notifications/initialized");
    });
    let mut c = cfg("http");
    c.url = "https://gmailmcp.googleapis.com/mcp/v1".into();
    let connected = tokio::time::timeout(Duration::from_secs(3), info_for(&c).serve(client))
        .await
        .unwrap()
        .unwrap();
    connected.cancellation_token().cancel();
    task.await.unwrap();
    c.url = "https://gmailmcp.googleapis.com.evil.test/mcp/v1".into();
    assert_eq!(info_for(&c).protocol_version, info().protocol_version);
}
#[test]
fn google_diagnostics_never_expose_upstream_secrets() {
    let body = json!({"error":{"message":"secret-token https://private.example/?code=private", "details":[{"reason":"SERVICE_DISABLED","metadata":{"consumer":"private-project"}}]}});
    let message = google_diagnostic_message(403, &body);
    assert!(message.contains("SERVICE_DISABLED"));
    assert!(message.contains("HTTP 403"));
    assert!(!message.contains("private"));
    assert!(!message.contains("secret-token"));
    assert!(google_diagnostic_message(
        403,
        &json!({"error":{"details":[{"reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}})
    )
    .contains("Ajoutez les scopes"));
}

#[tokio::test]
#[ignore = "Opt-in diagnosis of the user's existing Gmail authorization; never logs credentials"]
async fn diagnose_saved_gmail_connection() {
    let Ok(path) = std::env::var("CONVERGE_GOOGLE_DIAG_DB") else {
        return;
    };
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut query = db
        .prepare("SELECT data FROM extensions WHERE kind='mcp' ORDER BY rowid DESC")
        .unwrap();
    let entries = query.query_map([], |row| row.get::<_, String>(0)).unwrap();
    let c = entries
        .filter_map(Result::ok)
        .filter_map(|raw| serde_json::from_str::<Config>(&raw).ok())
        .find(|c| c.url == "https://gmailmcp.googleapis.com/mcp/v1" && c.auth == "oauth")
        .expect("Saved Gmail config required");
    let mut manager = auth_manager(&c, Vault::new(&c)).await.unwrap();
    assert!(
        manager.initialize_from_store().await.unwrap_or(false),
        "No saved authorization; reconnect from Converge"
    );
    let client = AuthClient::new(http().unwrap(), manager);
    let transport = StreamableHttpClientTransport::with_client(
        client.clone(),
        http_config(&c, &secrets(&c.id).unwrap()).unwrap(),
    );
    match tokio::time::timeout(Duration::from_secs(40), info_for(&c).serve(transport)).await {
        Ok(Ok(session)) => {
            let tools = tokio::time::timeout(Duration::from_secs(25), session.list_all_tools())
                .await
                .unwrap_or_else(|_| panic!("Gmail tools discovery timed out"))
                .unwrap_or_else(|_| panic!("Gmail tools discovery failed"));
            assert!(!tools.is_empty());
            println!(
                "SDK initialize succeeded; {} Gmail tools discovered",
                tools.len()
            );
            session.cancellation_token().cancel();
        }
        Ok(Err(error)) => {
            use rmcp::service::ClientInitializeError as E;
            println!(
                "SDK initialize failure kind: {}",
                match &error {
                    E::ExpectedInitResponse(_) => "unexpected-response",
                    E::ExpectedInitResult(_) => "unexpected-result",
                    E::ConnectionClosed(_) => "connection-closed",
                    E::TransportError { .. } => "transport",
                    E::JsonRpcError(e) => {
                        println!("JSON RPC code: {}", e.code.0);
                        "json-rpc"
                    }
                    E::NoCompatibleProtocolVersion { .. } => "protocol-version",
                    E::LegacyFallbackFailed { .. } => "legacy-fallback",
                    _ => "other",
                }
            );
            // Print only presence of known public transport diagnostics, never raw errors.
            let description = error.to_string();
            for known in [
                "403",
                "401",
                "404",
                "400",
                "SSE",
                "Deserialize",
                "content type",
                "protocol",
                "tools",
                "discover",
                "initialize",
                "channel closed",
            ] {
                if description.contains(known) {
                    println!("Known diagnostic category: {known}");
                }
            }
            panic!("Saved Gmail MCP initialization failed");
        }
        Err(_) => panic!("SDK initialize timed out"),
    }
    let message = google_connection_diagnostic(&c, &secrets(&c.id).unwrap(), &client).await;
    println!(
        "Gmail MCP diagnostic: {}",
        message.unwrap_or_else(
            || "No HTTP refusal was reproduced with the saved authorization".into()
        )
    );
}
fn cfg(transport: &str) -> Config {
    Config {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Contract server".into(),
        enabled: true,
        auto_connect: true,
        transport: transport.into(),
        url: String::new(),
        command: String::new(),
        args: vec![],
        cwd: String::new(),
        auth: "none".into(),
        client_id: String::new(),
        scopes: vec![],
        redirect_port: None,
    }
}
fn reply(request: &Value) -> Value {
    let result = match request["method"].as_str().unwrap_or("") {
        "initialize" => {
            json!({"protocolVersion":request["params"]["protocolVersion"],"serverInfo":{"name":"Converge test server","version":"1"},"capabilities":{"tools":{},"resources":{},"prompts":{}}})
        }
        "tools/list" => {
            json!({"tools":[{"name":"echo","description":"Echo input","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}]})
        }
        "resources/list" => json!({"resources":[{"uri":"fixture://hello","name":"hello"}]}),
        "prompts/list" => json!({"prompts":[{"name":"welcome","description":"Welcome"}]}),
        "tools/call" => {
            json!({"content":[{"type":"text","text":request["params"]["arguments"]["text"]}]})
        }
        "resources/read" => {
            json!({"contents":[{"uri":"fixture://hello","text":"Resource content"}]})
        }
        "prompts/get" => {
            json!({"messages":[{"role":"user","content":{"type":"text","text":"Welcome content"}}]})
        }
        _ => json!({}),
    };
    json!({"jsonrpc":"2.0","id":request["id"],"result":result})
}
#[tokio::test]
async fn streamable_http_tools_resources_and_prompts_reach_the_real_server() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let stop = CancellationToken::new();
    let done = stop.clone();
    let requests = Arc::new(Mutex::new(vec![]));
    let captured = requests.clone();
    let server = tokio::spawn(async move {
        loop {
            let accepted =
                tokio::select! {_=done.cancelled()=>break,socket=listener.accept()=>socket};
            let (mut socket, _) = accepted.unwrap();
            let mut bytes = vec![];
            let mut buf = [0u8; 2048];
            let (header_end, length) = loop {
                let n = socket.read(&mut buf).await.unwrap();
                if n == 0 {
                    break (0, 0);
                }
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..i]);
                    let len = headers
                        .lines()
                        .find_map(|l| {
                            l.split_once(':')
                                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    break (i + 4, len);
                }
            };
            if header_end == 0 {
                continue;
            }
            while bytes.len() < header_end + length {
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            let headers = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
            if !headers.starts_with("post ") {
                socket.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                continue;
            }
            assert!(headers.contains("authorization: bearer contract-token\r\n"));
            assert!(!headers.contains("bearer bearer"));
            let request: Value =
                serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
            captured.lock().unwrap().push(request.clone());
            if request.get("id").is_none() {
                socket
                    .write_all(
                        b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            } else {
                let body = reply(&request).to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        }
    });
    let mut c = cfg("http");
    c.url = url;
    c.auth = "token".into();
    let connected = connect_with_credentials(
        &c,
        Secrets {
            token: "contract-token".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(connected.tools[0]["name"], "echo");
    let runtime = Runtime::default();
    runtime
        .inner
        .sessions
        .lock()
        .await
        .insert(c.id.clone(), connected);
    let state = super::super::test_state();
    super::super::put(&state, "mcp", &c.id, &serde_json::to_value(&c).unwrap()).unwrap();
    let list = execute(&runtime, &state, "mcp.list", &json!({"serverId":c.id}))
        .await
        .unwrap();
    assert!(list.contains("fixture://hello"));
    let result = execute(
        &runtime,
        &state,
        "mcp.call",
        &json!({"serverId":c.id,"toolName":"echo","arguments":{"text":"A real MCP result"}}),
    )
    .await
    .unwrap();
    assert!(result.contains("A real MCP result"));
    assert!(execute(
        &runtime,
        &state,
        "mcp.call",
        &json!({"serverId":c.id,"toolName":"unregistered","arguments":{}})
    )
    .await
    .is_err());
    assert!(execute(
        &runtime,
        &state,
        "mcp.read",
        &json!({"serverId":c.id,"uri":"fixture://hello"})
    )
    .await
    .unwrap()
    .contains("Resource content"));
    assert!(execute(
        &runtime,
        &state,
        "mcp.prompt",
        &json!({"serverId":c.id,"name":"welcome","arguments":{}})
    )
    .await
    .unwrap()
    .contains("Welcome content"));
    c.enabled = false;
    super::super::put(&state, "mcp", &c.id, &serde_json::to_value(&c).unwrap()).unwrap();
    assert!(
        execute(&runtime, &state, "mcp.list", &json!({"serverId":c.id}))
            .await
            .is_err()
    );
    disconnect(&runtime, &c.id, false).await.unwrap();
    stop.cancel();
    server.await.unwrap();
    assert!(requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["method"] == "tools/call"));
}
#[tokio::test]
async fn stdio_server_streams_real_json_rpc_and_stops_on_disconnect() {
    let root = std::env::temp_dir().join(format!("converge-mcp-contract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("server.cjs");
    std::fs::write(&path,r#"const rl=require('node:readline').createInterface({input:process.stdin});rl.on('line',line=>{const r=JSON.parse(line);if(r.id===undefined)return;let result={};if(r.method==='initialize')result={protocolVersion:r.params.protocolVersion,serverInfo:{name:'stdio fixture',version:'1'},capabilities:{tools:{}}};if(r.method==='tools/list')result={tools:[{name:'echo',inputSchema:{type:'object',properties:{text:{type:'string'}}}}]};if(r.method==='tools/call')result={content:[{type:'text',text:r.params.arguments.text}],isError:false};process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:r.id,result})+'\n');});"#).unwrap();
    let mut c = cfg("stdio");
    c.command = "node".into();
    c.args = vec![path.to_string_lossy().into()];
    c.cwd = root.to_string_lossy().into();
    let connected = connect_with_credentials(&c, Default::default())
        .await
        .unwrap();
    assert_eq!(connected.tools.len(), 1);
    let result = connected
        .client
        .call_tool(
            CallToolRequestParams::new("echo")
                .with_arguments(json!({"text":"stdio works"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert!(serde_json::to_string(&result)
        .unwrap()
        .contains("stdio works"));
    connected.client.cancellation_token().cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(connected.client.is_closed());
    drop(connected);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&root).unwrap();
}
#[test]
fn partial_secret_updates_preserve_unedited_fields_and_allow_explicit_clear() {
    let value = Secrets {
        token: "token".into(),
        headers: HashMap::from([("X-Test".into(), "header".into())]),
        env: HashMap::from([("TEST".into(), "value".into())]),
        client_secret: "oauth".into(),
    };
    let value = merge_secret(
        value,
        SecretsPatch {
            token: Some("replacement".into()),
            ..Default::default()
        },
    );
    assert_eq!(value.env["TEST"], "value");
    assert_eq!(value.client_secret, "oauth");
    let value = merge_secret(
        value,
        SecretsPatch {
            headers: Some(HashMap::new()),
            ..Default::default()
        },
    );
    assert!(value.headers.is_empty());
    assert_eq!(value.token, "replacement");
}
#[tokio::test]
async fn revoked_oauth_store_cannot_save_a_late_callback() {
    let mut c = cfg("http");
    c.url = "https://example.org/mcp".into();
    let vault = Vault::new(&c);
    vault.invalidate();
    assert!(vault
        .save(StoredCredentials::new("client".into(), None, vec![], None))
        .await
        .is_err());
    assert!(vault.load().await.unwrap().is_none());
}
#[test]
fn mcp_urls_allow_configuration_but_not_embedded_credentials() {
    let mut c = cfg("http");
    c.url = "https://mcp.supabase.com/mcp?read_only=true&project_ref=project".into();
    validate(&c).unwrap();
    for url in [
        "http://outside.test/mcp",
        "https://user:password@example.org/mcp",
        "https://example.org/mcp?api_key=secret",
        "https://example.org/mcp#fragment",
    ] {
        c.url = url.into();
        assert!(validate(&c).is_err());
    }
}

#[test]
fn google_oauth_requests_offline_access_only_for_its_official_resources() {
    let raw = "https://accounts.google.com/o/oauth2/v2/auth?state=private&client_id=own-client";
    let url = oauth_browser_url(raw, "https://gmailmcp.googleapis.com/mcp/v1").unwrap();
    let parsed = url::Url::parse(&url).unwrap();
    let pairs = parsed.query_pairs().collect::<Vec<_>>();
    assert!(pairs
        .iter()
        .any(|(k, v)| k == "access_type" && v == "offline"));
    assert!(pairs.iter().any(|(k, v)| k == "state" && v == "private"));
    assert_eq!(
        oauth_browser_url(raw, "https://evil.test/mcp").unwrap(),
        raw
    );
}

#[tokio::test]
#[ignore = "Reads official Google MCP/OIDC metadata; creates no registration and authorizes no account"]
async fn live_google_mcp_oauth_metadata_and_pkce() {
    assert_eq!(
        std::env::var("CONVERGE_MCP_METADATA_TEST").as_deref(),
        Ok("1")
    );
    let manager = AuthorizationManager::new("https://gmailmcp.googleapis.com/mcp/v1")
        .await
        .unwrap();
    let mut oauth = OAuthState::Unauthorized(manager);
    oauth
        .start_authorization(
            AuthorizationRequest::new("http://127.0.0.1:32145/oauth/callback")
                .with_client_name("Converge")
                .with_preregistered_client("converge-metadata-test.apps.googleusercontent.com")
                .with_scopes(["https://www.googleapis.com/auth/gmail.readonly"]),
        )
        .await
        .unwrap();
    let raw = oauth.get_authorization_url().await.unwrap();
    let url = url::Url::parse(
        &oauth_browser_url(&raw, "https://gmailmcp.googleapis.com/mcp/v1").unwrap(),
    )
    .unwrap();
    assert_eq!(url.host_str(), Some("accounts.google.com"));
    let pairs = url.query_pairs().collect::<Vec<_>>();
    assert!(pairs
        .iter()
        .any(|(k, v)| k == "code_challenge_method" && v == "S256"));
    assert!(pairs
        .iter()
        .any(|(k, v)| k == "access_type" && v == "offline"));
}
