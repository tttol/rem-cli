use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Config {
    tasks_dir: PathBuf,
}

pub fn tasks_dir() -> io::Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))?;
    tasks_dir_from(&home_dir)
}

fn tasks_dir_from(home_dir: &Path) -> io::Result<PathBuf> {
    let default_tasks_dir = home_dir.join(".rem-cli/tasks");
    let config_path = home_dir.join(".rem-cli/config.yaml");
    if !config_path.exists() {
        return Ok(default_tasks_dir);
    }
    let content = fs::read_to_string(&config_path)?;
    let config = serde_yaml::from_str::<Config>(&content).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {error}", config_path.display()),
        )
    })?;
    Ok(config.tasks_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temporary_home_dir() -> PathBuf {
        std::env::temp_dir().join(format!("rem-cli-config-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn missing_config_uses_default_tasks_dir() {
        // GIVEN
        let home_dir = temporary_home_dir();
        let expected = home_dir.join(".rem-cli/tasks");

        // WHEN
        let actual = tasks_dir_from(&home_dir).unwrap();

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn config_tasks_dir_overrides_default_tasks_dir() {
        // GIVEN
        let home_dir = temporary_home_dir();
        let config_dir = home_dir.join(".rem-cli");
        let expected = home_dir.join("iCloud/rem-cli/tasks");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.yaml"),
            format!("tasks_dir: \"{}\"\n", expected.display()),
        )
        .unwrap();

        // WHEN
        let actual = tasks_dir_from(&home_dir).unwrap();

        // THEN
        assert_eq!(actual, expected);

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    fn config_without_tasks_dir_returns_error() {
        // GIVEN
        let home_dir = temporary_home_dir();
        let config_dir = home_dir.join(".rem-cli");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.yaml"), "theme: dark\n").unwrap();

        // WHEN
        let result = tasks_dir_from(&home_dir);

        // THEN
        assert!(result.is_err());

        fs::remove_dir_all(home_dir).unwrap();
    }

    #[test]
    fn invalid_config_yaml_returns_error() {
        // GIVEN
        let home_dir = temporary_home_dir();
        let config_dir = home_dir.join(".rem-cli");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.yaml"), "tasks_dir: [").unwrap();

        // WHEN
        let result = tasks_dir_from(&home_dir);

        // THEN
        assert!(result.is_err());

        fs::remove_dir_all(home_dir).unwrap();
    }
}
