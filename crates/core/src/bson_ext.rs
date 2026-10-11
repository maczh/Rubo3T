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
        other => Err(RuboError::Parse(format!(
            "期望文档对象，却得到: {other:?}"
        ))),
    }
}

/// 解析用户编辑的文档文本（普通 JSON 或扩展 JSON 均可）。
pub fn parse_document_text(text: &str) -> RuboResult<Document> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        // 空输入视为空文档
        return Ok(Document::new());
    }
    let value: Value = serde_json::from_str(trimmed)?;
    json_to_document(&value)
}

/// 把 BSON 文档渲染为格式化的扩展 JSON 文本（用于编辑器/展示）。
pub fn document_to_text(doc: &Document) -> RuboResult<String> {
    let v = document_to_json(doc)?;
    Ok(serde_json::to_string_pretty(&v)?)
}

/// 解析一个筛选/排序/命令 JSON 文本为 BSON 文档（可为空文档）。
pub fn parse_filter_text(text: &str) -> RuboResult<Document> {
    parse_document_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

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
    fn parse_extended_json() {
        let text = r#"{"_id": {"$oid": "507f1f77bcf86cd799439011"}, "amt": {"$numberDecimal": "123.45"}}"#;
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
}
