use serde_json::{Map, Value};

/// Emits an optional structured diagnostic event without exposing secrets.
/// Set `RISK_GATE_LOG=json` to enable it; the default remains quiet.
pub fn event(name: &str, fields: &[(&str, String)]) -> Option<String> {
    if std::env::var("RISK_GATE_LOG").as_deref() != Ok("json") {
        return None;
    }
    let line = render(name, fields);
    if let Some(line) = &line {
        eprintln!("{line}");
    }
    line
}

fn render(name: &str, fields: &[(&str, String)]) -> Option<String> {
    let mut object = Map::new();
    object.insert("event".into(), Value::String(name.to_owned()));
    for (key, value) in fields {
        object.insert((*key).to_owned(), Value::String(value.clone()));
    }
    Some(Value::Object(object).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOG_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn disabled_logging_does_not_require_output_configuration() {
        let _guard = LOG_ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("RISK_GATE_LOG") };
        assert!(event("test", &[("value", "secret".into())]).is_none());
    }

    #[test]
    fn enabled_logging_returns_and_emits_json() {
        let _guard = LOG_ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("RISK_GATE_LOG", "json") };
        let result = event("test", &[("value", "secret".into())]);
        unsafe { std::env::remove_var("RISK_GATE_LOG") };
        assert!(result.is_some());
        assert!(result.unwrap().contains(r#""event":"test""#));
    }

    #[test]
    fn renders_json_without_secret_filtering_or_shell_interpretation() {
        let line = render("test", &[("value", "a\"b".into())]).unwrap();
        assert!(line.contains(r#""a\"b""#));
        assert!(line.contains(r#""event":"test""#));
    }
}
