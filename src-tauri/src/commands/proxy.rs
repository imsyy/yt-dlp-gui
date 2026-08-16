//! 代理连通性测试

use std::time::Duration;

/// 通过代理请求一个已知地址，验证代理是否可用
#[tauri::command]
pub async fn test_proxy(proxy: String) -> Result<String, String> {
    if proxy.trim().is_empty() {
        return Err("err_proxy_empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .proxy(reqwest::Proxy::all(&proxy).map_err(|e| format!("err_proxy_config:{}", e))?)
        .build()
        .map_err(|e| format!("err_create_http_client:{}", e))?;

    let resp = client
        .get("https://www.gstatic.com/generate_204")
        .send()
        .await
        .map_err(|e| format!("err_proxy_test:{}", e))?;

    if resp.status().is_success() {
        Ok(format!("{}", resp.status().as_u16()))
    } else {
        Err(format!("err_proxy_status:{}", resp.status().as_u16()))
    }
}