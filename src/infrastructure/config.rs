use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Config {
    tasks_dir: PathBuf,
}

pub(crate) fn tasks_dir() -> io::Result<PathBuf> {
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
    serde_yaml::from_str::<Config>(&content)
        .map(|config| config.tasks_dir)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {error}", config_path.display()),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            Self {
                path: std::env::temp_dir().join(format!("rem-cli-config-test-{}", Uuid::new_v4())),
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _cleanup_result = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn missing_config_uses_default_tasks_dir() {
        // GIVEN
        let home = TestDirectory::new();
        let expected = home.path.join(".rem-cli/tasks");

        // WHEN
        let actual = tasks_dir_from(&home.path).expect("default path should load");

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn configured_tasks_dir_overrides_the_default() {
        // GIVEN
        let home = TestDirectory::new();
        let config_dir = home.path.join(".rem-cli");
        let expected = home.path.join("iCloud/rem-cli/tasks");
        fs::create_dir_all(&config_dir).expect("config directory should be created");
        fs::write(
            config_dir.join("config.yaml"),
            format!("tasks_dir: \"{}\"\n", expected.display()),
        )
        .expect("config should be written");

        // WHEN
        let actual = tasks_dir_from(&home.path).expect("config should load");

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_or_incomplete_configs_return_errors() {
        // GIVEN
        let cases = ["theme: dark\n", "tasks_dir: ["];

        // WHEN
        let actual: Vec<_> = cases
            .into_iter()
            .map(|content| {
                let home = TestDirectory::new();
                let config_dir = home.path.join(".rem-cli");
                fs::create_dir_all(&config_dir).expect("config directory should be created");
                fs::write(config_dir.join("config.yaml"), content)
                    .expect("config should be written");
                tasks_dir_from(&home.path).map(|_| ())
            })
            .collect();

        // THEN
        assert!(actual.into_iter().all(|result| result.is_err()));
    }
}
