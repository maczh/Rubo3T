//! BSON <-> JSON（扩展 JSON）互转。
//!
//! 借助 `bson` crate 对扩展 JSON（canonical / relaxed）的原生 serde 支持，
//! 实现文档与 `serde_json::Value` 之间的可靠互转。这样既能展示各版本 MongoDB
//! 文档（ObjectId / 日期 / Decimal128 等都用扩展 JSON 表示），又能把用户在
//! 编辑器里敲的 JSON（含 `{"$oid": "..."}` 等形式）解析回 BSON。
//!
//! 注意：旧版 Robo3T 用 C++ 驱动，文档表示与 JSON 互转逻辑分散且不完整；
//! 这里统一收敛到一处，并天然支持所有版本通用的扩展 JSON 表示。

use bson::{Bson, Document};
use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::Value;

use crate::error::{RuboError, RuboResult};

/// 把 BSON 文档转为 `serde_json::Value`（扩展 JSON 形式）。
pub fn document_to_json(doc: &Document) -> RuboResult<Value> {
    Ok(serde_json::to_value(Bson::Document(doc.clone()))?)
}

/// 把 `serde_json::Value`（扩展 JSON 形式）解析为 BSON 文档。
pub fn json_to_document(value: &Value) -> RuboResult<Document> {
    let b: Bson = serde_json::from_value(value.clone())?;
    match b {
        Bson::Document(d) => Ok(d),
        other => Err(RuboError::Parse(format!("期望文档对象，却得到: {other:?}"))),
    }
}

/// 解析用户编辑的文档文本（普通 JSON 或扩展 JSON 均可）。
///
/// 额外支持 `ISODate("...")` 与 `Timestamp(t, i)` 便捷语法：它们会在解析前被
/// 转换为对应的扩展 JSON（字段真实类型），使时间字段可直接用字符串表达。
pub fn parse_document_text(text: &str) -> RuboResult<Document> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        // 空输入视为空文档
        return Ok(Document::new());
    }
    let normalized = preprocess_date_extensions(trimmed);
    let value: Value = serde_json::from_str(&normalized)?;
    json_to_document(&value)
}

/// 把 UTC 毫秒格式化为本地时区字符串 `YYYY-MM-DD HH:mm:ss.sss`。
///
/// 无效毫秒（超出可表示范围）返回空串。供表格/树/文本视图统一使用，
/// 保证各处时间显示格式一致。
pub fn millis_to_local_str(utc_millis: i64) -> String {
    match chrono::DateTime::<Utc>::from_timestamp_millis(utc_millis) {
        Some(dt) => dt
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        None => String::new(),
    }
}

/// 把 BSON 文档渲染为格式化的扩展 JSON 文本（用于编辑器/展示）。
///
/// 其中的日期类型会转换成本地时间字符串表达，便于阅读且可被
/// [`parse_document_text`] 的 `ISODate("...")` 语法原样解析回真实类型：
/// - canonical `{"$date": {"$numberLong": "<ms>"}}`
/// - relaxed   `{"$date": "<iso>"}`
///
/// 均渲染为 `ISODate("YYYY-MM-DD HH:mm:ss.sss")`（本地时区）。
pub fn document_to_text(doc: &Document) -> RuboResult<String> {
    let mut v = document_to_json(doc)?;
    let original = serde_json::to_string(&v)?;
    let mut marker = String::from("rubo3t_date:");
    while original.contains(&marker) {
        marker.push('_');
    }
    mark_dates_in_value(&mut v, &marker);
    let pretty = serde_json::to_string_pretty(&v)?;
    Ok(substitute_date_markers(&pretty, &marker))
}

/// 递归地把 Value 中的扩展 JSON 日期对象替换为占位字符串（NUL 包裹毫秒数）。
///
/// 之所以用占位符而非直接写入 `ISODate(...)`：后者不是合法 JSON，无法作为
/// `serde_json::Value` 表示；先占位、序列化后再做字符串替换可绕开 JSON 转义。
fn mark_dates_in_value(v: &mut Value, marker: &str) {
    match v {
        Value::Object(map) => {
            if let Some(ms) =
                extract_date_millis(map).filter(|ms| !millis_to_local_str(*ms).is_empty())
            {
                *v = Value::String(format!("\u{0}{marker}{ms}\u{0}"));
                return;
            }
            for (_, child) in map.iter_mut() {
                mark_dates_in_value(child, marker);
            }
        }
        Value::Array(arr) => {
            for child in arr.iter_mut() {
                mark_dates_in_value(child, marker);
            }
        }
        _ => {}
    }
}

/// 若 `map` 是扩展 JSON 日期对象，返回其 UTC 毫秒。
fn extract_date_millis(map: &serde_json::Map<String, Value>) -> Option<i64> {
    if map.len() != 1 {
        return None;
    }
    match map.get("$date")? {
        // canonical: {"$date": {"$numberLong": "<ms>"}}
        Value::Object(inner) => {
            if inner.len() == 1 {
                if let Some(Value::String(s)) = inner.get("$numberLong") {
                    return s.parse::<i64>().ok();
                }
            }
            None
        }
        // relaxed: {"$date": "<iso>"}
        Value::String(s) => parse_isodate_to_millis(s),
        // 少数序列化器直接给数字毫秒
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

/// 把 pretty 文本中的日期占位符 `"\u0000<ms>\u0000"` 替换为 `ISODate("<本地时间>")`。
///
/// serde_json 会把 NUL(0x00) 转义为字面量 `\u0000`，故占位符在输出里是单行、
/// 可用固定字面量反查（避免原始 `{"$date": {...}}` 跨行难以匹配的问题）。
fn substitute_date_markers(s: &str, marker: &str) -> String {
    let open = format!("\"\\u0000{marker}");
    let open = open.as_str();
    const CLOSE: &str = "\\u0000\"";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(open) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + open.len()..];
        let mut replaced = false;
        if let Some(end) = after.find(CLOSE) {
            let digits = &after[..end];
            let all_digits = !digits.is_empty()
                && digits
                    .strip_prefix('-')
                    .unwrap_or(digits)
                    .bytes()
                    .all(|b| b.is_ascii_digit());
            if all_digits {
                if let Ok(ms) = digits.parse::<i64>() {
                    out.push_str("ISODate(\"");
                    out.push_str(&millis_to_local_str(ms));
                    out.push_str("\")");
                    rest = &after[end + CLOSE.len()..];
                    replaced = true;
                }
            }
        }
        if !replaced {
            // 非日期占位符（极少见）：原样保留，继续向后扫描
            out.push_str(open);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// 解析一个筛选/排序/命令 JSON 文本为 BSON 文档（可为空文档）。
pub fn parse_filter_text(text: &str) -> RuboResult<Document> {
    parse_document_text(text)
}

/// 解析聚合管道文本（JSON 数组，每个元素为一个阶段文档）。
/// 空输入返回空管道（等价于不过滤）。解析失败返回错误。
pub fn parse_pipeline_text(text: &str) -> RuboResult<Vec<Document>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let normalized = preprocess_date_extensions(trimmed);
    let value: Value = serde_json::from_str(&normalized)?;
    let arr = value
        .as_array()
        .ok_or_else(|| RuboError::Parse("聚合管道必须是一个 JSON 数组".to_string()))?;
    let mut out = Vec::new();
    for v in arr {
        out.push(json_to_document(v)?);
    }
    Ok(out)
}

/// 解析 ISODate 字符串为 UTC 毫秒。
///
/// 支持：
/// - RFC3339（含 `Z` 或时区偏移）：`2024-01-01T12:00:00.000Z`、`2024-01-01T20:00:00+08:00`
/// - 本地格式（空格分隔、无时区，按运行环境本地时区解释）：
///   `2024-01-01 12:00:00.123`、`2024-01-01 12:00`、`2024-01-01`
fn parse_isodate_to_millis(s: &str) -> Option<i64> {
    let s = s.trim();
    // RFC3339（带 Z 或偏移）
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    // 本地格式：空格换 T 后按 naive 解析，再按本地时区解释
    let iso = s.replace(' ', "T");
    let naive_formats = [
        "%Y-%m-%dT%H:%M:%S%.3f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H",
    ];
    for fmt in naive_formats {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&iso, fmt) {
            return Local
                .from_local_datetime(&naive)
                .single()
                .map(|d| d.timestamp_millis());
        }
    }
    // 仅日期
    if let Ok(date) = NaiveDate::parse_from_str(&iso, "%Y-%m-%d") {
        if let Some(naive) = date.and_hms_opt(0, 0, 0) {
            return Local
                .from_local_datetime(&naive)
                .single()
                .map(|d| d.timestamp_millis());
        }
    }
    None
}

/// 把用户输入中的 `ISODate("...")` 与 `Timestamp(t, i)` 语法转换为扩展 JSON，
/// 使日期/时间戳字段可用字符串便捷表达，并自动转换为字段的真实类型。
///
/// 仅在「值位置」（即前一个非空白字符为 `{` `[` `,` `:` `(` 或文本开头）做替换，
/// 避免误改字符串字面量内部的内容。
fn preprocess_date_extensions(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let mut in_string = false;
    let mut escaped = false;
    let mut previous = None;
    while !rest.is_empty() {
        let c = rest.chars().next().expect("nonempty input");
        if !in_string && matches!(previous, None | Some('{' | '[' | ',' | ':' | '(')) {
            let replacement = if let Some(after) = rest.strip_prefix("ISODate(") {
                try_replace_isodate(after).map(|(text, n)| (text, n + "ISODate(".len()))
            } else if let Some(after) = rest.strip_prefix("Timestamp(") {
                try_replace_timestamp(after).map(|(text, n)| (text, n + "Timestamp(".len()))
            } else {
                None
            };
            if let Some((text, consumed)) = replacement {
                out.push_str(&text);
                rest = &rest[consumed..];
                previous = Some(')');
                continue;
            }
        }
        out.push(c);
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        }
        if !c.is_whitespace() {
            previous = Some(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// 尝试把 `ISODate(` 之后的内容（形如 `"<date>"`）转换为
/// `{"$date": {"$numberLong": "<millis>"}}`，返回 (替换文本, 已消费字节数)。
fn try_replace_isodate(after: &str) -> Option<(String, usize)> {
    let after_trim = after.trim_start();
    let ws1 = after.len() - after_trim.len();
    let inner = after_trim.strip_prefix('"')?;
    let end_quote = inner.find('"')?;
    let content = &inner[..end_quote];
    let after_quote = &inner[end_quote + 1..];
    let after_quote_trim = after_quote.trim_start();
    let ws2 = after_quote.len() - after_quote_trim.len();
    if !after_quote_trim.starts_with(')') {
        return None;
    }
    let millis = parse_isodate_to_millis(content)?;
    let repl = format!("{{\"$date\": {{\"$numberLong\": \"{}\"}}}}", millis);
    let consumed = ws1 + 1 /* " */ + end_quote + 1 /* " */ + ws2 + 1 /* ) */;
    Some((repl, consumed))
}

/// 尝试把 `Timestamp(` 之后的内容（形如 `t, i`）转换为
/// `{"$timestamp": {"t": t, "i": i}}`，返回 (替换文本, 已消费字节数)。
fn try_replace_timestamp(after: &str) -> Option<(String, usize)> {
    let end = after.find(')')?;
    let inner = after[..end].trim();
    let (t_s, i_s) = inner.split_once(',')?;
    let t_v: u32 = t_s.trim().parse().ok()?;
    let i_v: u32 = i_s.trim().parse().ok()?;
    if !t_s.trim().bytes().all(|b| b.is_ascii_digit() || b == b'-')
        || !i_s.trim().bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let repl = format!("{{\"$timestamp\": {{\"t\": {}, \"i\": {}}}}}", t_v, i_v);
    Some((repl, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn date_markers_do_not_change_string_values() {
        let d = doc! {
            "note": "\u{0}1600000000000\u{0}",
            "marker": "rubo3t_date:",
            "date": bson::DateTime::from_millis(1600000000000),
            "extreme": bson::DateTime::from_millis(i64::MAX),
        };
        assert_eq!(
            parse_document_text(&document_to_text(&d).unwrap()).unwrap(),
            d
        );
    }

    #[test]
    fn timestamp_before_date_is_converted() {
        let d = parse_document_text(
            r#"{"ts": Timestamp(1, 2), "date": ISODate("2024-01-01T00:00:00Z")}"#,
        )
        .unwrap();
        assert!(matches!(d.get("ts"), Some(Bson::Timestamp(_))));
        assert!(matches!(d.get("date"), Some(Bson::DateTime(_))));
    }

    #[test]
    fn constructor_text_inside_json_strings_is_preserved() {
        let original = bson::doc! {"note": "example: Timestamp(1, 2), ISODate(123)"};
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(parse_document_text(&json).unwrap(), original);
    }

    #[test]
    fn timestamp_rejects_out_of_range_values() {
        for text in [
            r#"{"t": Timestamp(-1, 0)}"#,
            r#"{"t": Timestamp(4294967296, 0)}"#,
            r#"{"t": Timestamp(0, 4294967296)}"#,
        ] {
            assert!(parse_document_text(text).is_err());
        }
    }

    #[test]
    fn roundtrip_objectid_and_date() {
        let d = doc! {
            "name": "alice",
            "_id": bson::oid::ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap(),
            "created": bson::DateTime::from_millis(1_600_000_000_000),
            "score": 99.5,
            "n": 42,
            "big": 9_000_000_000i64,
        };
        let text = document_to_text(&d).unwrap();
        let parsed = parse_document_text(&text).unwrap();
        assert_eq!(d, parsed);
    }

    #[test]
    fn document_to_text_dates_as_local_isodate() {
        // 2020-09-13T12:26:40.000Z == 1600000000000 ms
        let d = doc! { "t": bson::DateTime::from_millis(1_600_000_000_000) };
        let text = document_to_text(&d).unwrap();
        // 展示为 ISODate("本地时间")，不再出现原始扩展 JSON
        assert!(text.contains("ISODate("), "应含 ISODate(：{text}");
        assert!(
            !text.contains("$numberLong"),
            "不应再出现 $numberLong：{text}"
        );
        assert!(!text.contains("$date"), "不应再出现 $date：{text}");
        // 本地时间字符串形如 YYYY-MM-DD HH:mm:ss.sss
        let iso = text
            .split("ISODate(\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or_default();
        assert_eq!(
            iso.len(),
            "2020-09-13 12:26:40.000".len(),
            "时间串长度不符：{iso}"
        );
        // 往返：解析回来仍是同一 DateTime（时区无关，因格式与解析都用本地时区）
        let parsed = parse_document_text(&text).unwrap();
        assert_eq!(d, parsed);
    }

    #[test]
    fn parse_extended_json() {
        let text =
            r#"{"_id": {"$oid": "507f1f77bcf86cd799439011"}, "amt": {"$numberDecimal": "123.45"}}"#;
        let d = parse_document_text(text).unwrap();
        assert!(matches!(d.get("_id"), Some(bson::Bson::ObjectId(_))));
        assert!(matches!(d.get("amt"), Some(bson::Bson::Decimal128(_))));
    }

    #[test]
    fn plain_json_numbers() {
        let d = parse_document_text(r#"{"a": 1, "b": 3.14, "c": "x"}"#).unwrap();
        assert!(matches!(d.get("a"), Some(bson::Bson::Int32(1))));
        assert!(matches!(d.get("b"), Some(bson::Bson::Double(_))));
    }

    #[test]
    fn isodate_rfc3339_to_datetime() {
        // 2020-09-13T12:26:40.000Z == 1600000000000 ms
        let d = parse_document_text(r#"{"t": ISODate("2020-09-13T12:26:40.000Z")}"#).unwrap();
        match d.get("t") {
            Some(bson::Bson::DateTime(dt)) => assert_eq!(dt.timestamp_millis(), 1_600_000_000_000),
            other => panic!("期望 DateTime，却得到 {:?}", other),
        }
    }

    #[test]
    fn isodate_local_format_to_datetime() {
        // 本地格式（无时区）按运行环境本地时区解释；这里仅校验能被解析为 DateTime。
        let d = parse_document_text(r#"{"t": ISODate("2024-01-01 12:00:00")}"#).unwrap();
        assert!(matches!(d.get("t"), Some(bson::Bson::DateTime(_))));
    }

    #[test]
    fn isodate_inside_string_not_converted() {
        // 字符串字面量内部的 ISODate 不应被改写（前一个非空白字符是引号）。
        let d = parse_document_text(r#"{"note": "use ISODate(\"2024-01-01\") here"}"#).unwrap();
        assert!(matches!(d.get("note"), Some(bson::Bson::String(_))));
    }

    #[test]
    fn timestamp_syntax_to_bson_timestamp() {
        let d = parse_document_text(r#"{"ts": Timestamp(1600000000, 5)}"#).unwrap();
        match d.get("ts") {
            Some(bson::Bson::Timestamp(ts)) => {
                assert_eq!(ts.time, 1_600_000_000);
                assert_eq!(ts.increment, 5);
            }
            other => panic!("期望 Timestamp，却得到 {:?}", other),
        }
    }

    #[test]
    fn mixed_isodate_and_timestamp() {
        let d = parse_document_text(
            r#"{"a": ISODate("2020-09-13T12:26:40.000Z"), "b": Timestamp(1, 2)}"#,
        )
        .unwrap();
        assert!(matches!(d.get("a"), Some(bson::Bson::DateTime(_))));
        assert!(matches!(d.get("b"), Some(bson::Bson::Timestamp(_))));
    }
}
