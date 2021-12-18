use std::collections::HashMap;

pub fn apply_template(s: &str, variables: &HashMap<&str, &str>) {
    let mut s = s.to_string();
    for (key, replacement) in variables.iter() {
        s = s.replace(key, replacement)
    }
    s
}
