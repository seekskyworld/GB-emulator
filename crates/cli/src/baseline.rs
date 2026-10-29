//! 棘轮：与基线（gates.json 格式）比较，基线中通过而本次失败的项为 REGRESSION（PLAN §4.1）。

use crate::json::{self, Value};

/// 从 gates.json 的 `results` 里取出 (rom, pass) 列表。格式不对时报错（不静默当作空基线）。
pub fn entries(v: &Value) -> Result<Vec<(String, bool)>, String> {
    let results = v
        .get("results")
        .and_then(Value::as_array)
        .ok_or("baseline has no `results` array")?;
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let rom = r.get("rom").and_then(Value::as_str).ok_or(format!("results[{i}] has no `rom`"))?;
            let pass = r.get("pass").and_then(Value::as_bool).ok_or(format!("results[{i}] has no `pass`"))?;
            Ok((rom.to_string(), pass))
        })
        .collect()
}

/// 基线中通过、本次运行到了但失败的 ROM（按本次顺序）。本次没有运行的项（`--fast`、`--only`）不算回归；
/// 基线里同一个 ROM 出现多次时，任意一次通过即视为通过。
/// 本次结果中 `"scored": false` 的项（未纳入范围的 Mooneye ROM）不参与比较。
pub fn regressions(baseline: &Value, current: &Value) -> Result<Vec<String>, String> {
    let base = entries(baseline)?;
    let cur = entries(current)?;
    let items = current.get("results").and_then(Value::as_array).unwrap_or(&[]);
    Ok(cur
        .into_iter()
        .zip(items)
        .filter(|(_, item)| item.get("scored").and_then(Value::as_bool) != Some(false))
        .map(|(c, _)| c)
        .filter(|(rom, pass)| !*pass && base.iter().any(|(b, bp)| b == rom && *bp))
        .map(|(rom, _)| rom)
        .collect())
}

/// 读取并解析基线文件。
pub fn read(path: &std::path::Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    json::parse(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

/// 新基线：旧基线的条目按 ROM 被本次结果覆盖，本次未运行的条目保留；顺序为旧基线顺序加本次新增。
pub fn merge(old: Option<&Value>, current: &Value) -> Result<Value, String> {
    let cur_items = current.get("results").and_then(Value::as_array).ok_or("current has no results")?;
    let rom_of = |v: &Value| v.get("rom").and_then(Value::as_str).map(str::to_string);
    let mut items: Vec<Value> = Vec::new();
    if let Some(old) = old {
        entries(old)?; // 校验格式
        for o in old.get("results").and_then(Value::as_array).unwrap_or(&[]) {
            let rom = rom_of(o);
            let replacement = cur_items.iter().find(|c| rom_of(c) == rom);
            items.push(replacement.unwrap_or(o).clone());
        }
    }
    for c in cur_items {
        if !items.iter().any(|i| rom_of(i) == rom_of(c)) {
            items.push(c.clone());
        }
    }
    let mut parts = Vec::new();
    for g in crate::gates::GROUPS.iter().copied().chain([crate::gates::MOONEYE]) {
        let in_group: Vec<&Value> = items
            .iter()
            .filter(|i| i.get("group").and_then(Value::as_str) == Some(g))
            .filter(|i| i.get("scored").and_then(Value::as_bool) != Some(false))
            .collect();
        if !in_group.is_empty() {
            let pass = in_group.iter().filter(|i| i.get("pass").and_then(Value::as_bool) == Some(true)).count();
            parts.push(format!("{g} {pass}/{}", in_group.len()));
        }
    }
    Ok(Value::Obj(vec![
        ("summary".to_string(), Value::Str(parts.join(" "))),
        ("results".to_string(), Value::Arr(items)),
    ]))
}
