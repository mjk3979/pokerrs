use std::collections::HashMap;

pub fn apply_template(s: &str, variables: &HashMap<String, String>) -> String {
    let mut s = s.to_string();
    for (key, replacement) in variables.iter() {
        let full_key = format!("${{{}}}", key);
        s = s.replace(key, replacement)
    }
    s
}
