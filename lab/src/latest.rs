use serde_json::Value;

pub fn latest(items: &[Value], key: impl Fn(&Value) -> String) -> Vec<&Value> {
    let mut newest: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
    for item in items {
        let slot = newest.entry(key(item)).or_insert(item);
        if item["id"].as_u64() > slot["id"].as_u64() {
            *slot = item;
        }
    }
    newest.into_values().collect()
}
