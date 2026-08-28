use serde::{Deserialize, Serialize};

/// 单条弹幕的对外响应
#[derive(Debug, Clone, Serialize)]
pub struct DanmakuItemResponse {
    /// 弹幕 ID（hashid 字符串）
    pub id: String,
    /// 弹幕文本
    pub text: String,
    /// 出现时间（秒）
    pub time: f64,
    /// 颜色（十六进制，如 #ffffff）
    pub color: Option<String>,
    /// 字号
    #[serde(rename = "fontSize")]
    pub font_size: Option<i32>,
}

/// 视频弹幕列表响应
#[derive(Debug, Clone, Serialize)]
pub struct DanmakuListResponse {
    pub items: Vec<DanmakuItemResponse>,
}

/// 发送弹幕请求体
#[derive(Debug, Clone, Deserialize)]
pub struct SendDanmakuRequest {
    pub text: String,
    pub time: f64,
    pub color: Option<String>,
    #[serde(rename = "fontSize")]
    pub font_size: Option<i32>,
}

/// 发送弹幕响应
#[derive(Debug, Clone, Serialize)]
pub struct SendDanmakuResponse {
    pub id: String,
}
