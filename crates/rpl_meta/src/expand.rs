use std::collections::HashMap;

use regex::Regex;

pub fn expand_operations(
    source: &str,
    ops: &HashMap<String, Vec<String>>,
) -> Result<Vec<(String, String)>, Vec<String>> {
    let re = Regex::new(r"@([a-zA-Z_]\w*)").expect("invalid regex");

    // 1. Collect all unique @op references in order of first appearance
    let mut unique_ops: Vec<String> = Vec::new();
    for cap in re.captures_iter(source) {
        let name = cap[1].to_string();
        if !unique_ops.contains(&name) {
            unique_ops.push(name);
        }
    }

    // 2. No @ops → return unchanged
    if unique_ops.is_empty() {
        return Ok(vec![("".to_string(), source.to_string())]);
    }

    // 3. Look up each op; collect errors for undefined/empty ones
    let mut undefined = Vec::new();
    let mut resolved: Vec<(&str, &[String])> = Vec::new();
    for name in &unique_ops {
        match ops.get(name) {
            Some(paths) if !paths.is_empty() => {
                resolved.push((name.as_str(), paths.as_slice()));
            },
            _ => {
                undefined.push(name.clone());
            },
        }
    }
    if !undefined.is_empty() {
        return Err(undefined);
    }

    // 4. Compute cartesian product of all operation replacements
    let mut combos: Vec<Vec<(&str, &str)>> = vec![vec![]];
    for (op_name, paths) in &resolved {
        let mut new_combos = Vec::new();
        for combo in &combos {
            for path in *paths {
                let mut new_combo = combo.clone();
                new_combo.push((op_name, path.as_str()));
                new_combos.push(new_combo);
            }
        }
        combos = new_combos;
    }

    // 5. Generate expanded source for each combination
    let total = combos.len();
    let results: Vec<(String, String)> = combos
        .into_iter()
        .enumerate()
        .map(|(idx, combo)| {
            let mut expanded = source.to_string();
            for (op_name, path) in &combo {
                let token = format!("@{}", op_name);
                expanded = expanded.replace(&token, path);
            }
            let suffix = if total == 1 {
                String::new()
            } else {
                format!("_v{}", idx)
            };
            (suffix, expanded)
        })
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ops_returns_source_unchanged() {
        let ops = HashMap::new();
        let source = "pattern test\npatt {\n  p = fn _(..) -> _ { }\n}";
        let result = expand_operations(source, &ops).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "");
        assert_eq!(result[0].1, source);
    }

    #[test]
    fn single_op_expands_to_n_variants() {
        let mut ops = HashMap::new();
        ops.insert("lock".to_string(), vec![
            "std::sync::Mutex::lock".to_string(),
            "std::sync::RwLock::read".to_string(),
        ]);
        let source = "_ = @lock(move $x);";
        let result = expand_operations(source, &ops).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "_ = std::sync::Mutex::lock(move $x);");
        assert_eq!(result[1].1, "_ = std::sync::RwLock::read(move $x);");
    }

    #[test]
    fn two_ops_cartesian_product() {
        let mut ops = HashMap::new();
        ops.insert("lock".to_string(), vec![
            "Mutex::lock".to_string(),
            "RwLock::read".to_string(),
        ]);
        ops.insert("unlock".to_string(), vec![
            "MutexGuard::drop".to_string(),
        ]);
        let source = "_ = @lock(_);\n_ = @unlock(_);";
        let result = expand_operations(source, &ops).unwrap();
        // 2 lock paths * 1 unlock path = 2 variants
        assert_eq!(result.len(), 2);
        assert!(result[0].1.contains("Mutex::lock"));
        assert!(result[0].1.contains("MutexGuard::drop"));
        assert!(result[1].1.contains("RwLock::read"));
        assert!(result[1].1.contains("MutexGuard::drop"));
    }

    #[test]
    fn undefined_op_returns_error() {
        let ops = HashMap::new();
        let source = "_ = @lock(move $x);";
        let result = expand_operations(source, &ops);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors, vec!["lock".to_string()]);
    }

    #[test]
    fn multiple_occurrences_of_same_op_all_replaced() {
        let mut ops = HashMap::new();
        ops.insert("lock".to_string(), vec![
            "Mutex::lock".to_string(),
        ]);
        let source = "_ = @lock(_);\n_ = @lock(_);";
        let result = expand_operations(source, &ops).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].1.contains('@'));
        assert_eq!(result[0].1.matches("Mutex::lock").count(), 2);
    }

    #[test]
    fn empty_op_list_returns_error() {
        let mut ops = HashMap::new();
        ops.insert("lock".to_string(), vec![]);
        let source = "_ = @lock(_);";
        let result = expand_operations(source, &ops);
        assert!(result.is_err());
    }
}
