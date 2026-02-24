use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SPEC_PATHS: [&str; 2] = ["docs/openapi.yaml", "docs/swagger/openapi.yaml"];
const REQUIRED_ENDPOINTS: [(&str, &str); 5] = [
    ("get", "/healthz"),
    ("get", "/readyz"),
    ("get", "/metrics"),
    ("post", "/jobs"),
    ("get", "/jobs/{job_id}"),
];

#[derive(Debug, Default, Clone)]
struct Param {
    in_location: Option<String>,
    name: Option<String>,
    required: Option<bool>,
}

#[derive(Debug, Default)]
struct PathSpec {
    methods: BTreeSet<String>,
    path_params: Vec<Param>,
    operation_params: BTreeMap<String, Vec<Param>>,
}

fn main() {
    let mut failures = Vec::new();

    for spec_path in SPEC_PATHS {
        if let Err(err) = validate_spec(spec_path) {
            failures.push(format!("{spec_path}: {err}"));
        }
    }

    if failures.is_empty() {
        println!("Contract drift checks passed for {}", SPEC_PATHS.join(", "));
        return;
    }

    eprintln!("Contract drift checks failed:");
    for failure in failures {
        eprintln!("- {failure}");
    }
    std::process::exit(1);
}

fn validate_spec(spec_path: &str) -> Result<(), String> {
    let raw = fs::read_to_string(spec_path).map_err(|e| format!("cannot read file: {e}"))?;
    let paths = parse_paths_block(&raw);

    for (method, path) in REQUIRED_ENDPOINTS {
        let path_spec = paths
            .get(path)
            .ok_or_else(|| format!("missing required path `{path}`"))?;
        if !path_spec.methods.contains(method) {
            return Err(format!(
                "missing required method `{method}` on path `{path}`"
            ));
        }
    }

    validate_job_id_param(spec_path, &paths)?;
    Ok(())
}

fn validate_job_id_param(
    spec_path: &str,
    paths: &BTreeMap<String, PathSpec>,
) -> Result<(), String> {
    let endpoint = "/jobs/{job_id}";
    let path_spec = paths
        .get(endpoint)
        .ok_or_else(|| format!("missing required path `{endpoint}`"))?;

    let placeholders = extract_path_param_names(endpoint);
    if placeholders != vec!["job_id"] {
        return Err(format!(
            "path `{endpoint}` must declare only `{{job_id}}` placeholder, found: {:?}",
            placeholders
        ));
    }

    let has_job_id = path_spec
        .path_params
        .iter()
        .chain(
            path_spec
                .operation_params
                .get("get")
                .map(|v| v.iter())
                .into_iter()
                .flatten(),
        )
        .any(|p| {
            p.in_location.as_deref() == Some("path")
                && p.name.as_deref() == Some("job_id")
                && p.required == Some(true)
        });

    if !has_job_id {
        return Err(format!(
            "{spec_path} is missing required path parameter definition `in: path`, `name: job_id`, `required: true` for `GET {endpoint}`"
        ));
    }

    Ok(())
}

fn parse_paths_block(raw: &str) -> BTreeMap<String, PathSpec> {
    let mut paths: BTreeMap<String, PathSpec> = BTreeMap::new();
    let mut in_paths = false;
    let mut current_path: Option<String> = None;
    let mut current_method: Option<String> = None;
    let mut param_scope: Option<ParamScope> = None;
    let mut current_param: Option<Param> = None;

    for line in raw.lines() {
        let indent = line.chars().take_while(|c| *c == ' ').count();
        let trimmed = line.trim();

        if !in_paths {
            if trimmed == "paths:" {
                in_paths = true;
            }
            continue;
        }

        if indent == 0 && !trimmed.is_empty() {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
            break;
        }

        if indent == 2 && trimmed.ends_with(':') {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
            current_method = None;
            param_scope = None;
            let path_name = trimmed.trim_end_matches(':').to_string();
            current_path = Some(path_name.clone());
            paths.entry(path_name).or_default();
            continue;
        }

        if current_path.is_none() {
            continue;
        }

        if indent <= 4 {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
        }

        if indent == 4 && (trimmed == "get:" || trimmed == "post:") {
            let method = trimmed.trim_end_matches(':').to_string();
            current_method = Some(method.clone());
            param_scope = None;
            if let Some(path_name) = current_path.as_ref() {
                paths
                    .entry(path_name.clone())
                    .or_default()
                    .methods
                    .insert(method);
            }
            continue;
        }

        if indent == 4 && trimmed == "parameters:" {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
            param_scope = Some(ParamScope::Path);
            continue;
        }

        if indent == 6 && trimmed == "parameters:" && current_method.is_some() {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
            param_scope = Some(ParamScope::Operation);
            continue;
        }

        let expected_item_indent = match param_scope {
            Some(ParamScope::Path) => 6,
            Some(ParamScope::Operation) => 8,
            None => continue,
        };

        if indent == expected_item_indent && trimmed.starts_with("- in:") {
            commit_param(
                &mut paths,
                &current_path,
                &current_method,
                &param_scope,
                &mut current_param,
            );
            current_param = Some(Param {
                in_location: Some(trimmed.trim_start_matches("- in:").trim().to_string()),
                ..Param::default()
            });
            continue;
        }

        let expected_field_indent = expected_item_indent + 2;
        if indent == expected_field_indent && trimmed.starts_with("name:") {
            if let Some(param) = current_param.as_mut() {
                param.name = Some(trimmed.trim_start_matches("name:").trim().to_string());
            }
            continue;
        }

        if indent == expected_field_indent && trimmed.starts_with("required:") {
            if let Some(param) = current_param.as_mut() {
                param.required = Some(trimmed.trim_start_matches("required:").trim() == "true");
            }
            continue;
        }
    }

    commit_param(
        &mut paths,
        &current_path,
        &current_method,
        &param_scope,
        &mut current_param,
    );
    paths
}

fn commit_param(
    paths: &mut BTreeMap<String, PathSpec>,
    current_path: &Option<String>,
    current_method: &Option<String>,
    scope: &Option<ParamScope>,
    current_param: &mut Option<Param>,
) {
    let Some(param) = current_param.take() else {
        return;
    };
    let Some(path_name) = current_path.as_ref() else {
        return;
    };
    let entry = paths.entry(path_name.clone()).or_default();
    match scope {
        Some(ParamScope::Path) => entry.path_params.push(param),
        Some(ParamScope::Operation) => {
            if let Some(method) = current_method.as_ref() {
                entry
                    .operation_params
                    .entry(method.clone())
                    .or_default()
                    .push(param);
            }
        }
        None => {}
    }
}

fn extract_path_param_names(endpoint: &str) -> Vec<&str> {
    endpoint
        .split('/')
        .filter_map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2 {
                Some(&segment[1..segment.len() - 1])
            } else {
                None
            }
        })
        .collect()
}

enum ParamScope {
    Path,
    Operation,
}

#[cfg(test)]
mod tests {
    use super::{extract_path_param_names, parse_paths_block};

    #[test]
    fn extracts_path_params() {
        assert_eq!(extract_path_param_names("/jobs/{job_id}"), vec!["job_id"]);
    }

    #[test]
    fn parses_required_methods_and_params() {
        let raw = r#"
openapi: 3.0.3
paths:
  /jobs/{job_id}:
    get:
      parameters:
        - in: path
          name: job_id
          required: true
"#;
        let paths = parse_paths_block(raw);
        let p = paths.get("/jobs/{job_id}").expect("path should exist");
        assert!(p.methods.contains("get"));
        assert_eq!(p.operation_params["get"][0].name.as_deref(), Some("job_id"));
    }
}
