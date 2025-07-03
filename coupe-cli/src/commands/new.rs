use coupe::{Config, CoupeError, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub async fn execute(name: String, path: Option<String>) -> Result<()> {
    println!("Creating new project: {}", name);

    let base_path = match path {
        Some(p) => {
            let path = Path::new(&p);
            if !path.exists() {
                fs::create_dir_all(path)?;
            }
            path.to_path_buf()
        }
        None => PathBuf::from("."),
    };

    let project_path = base_path.join(&name);
    if project_path.exists() {
        return Err(CoupeError::InvalidInput(format!(
            "Directory '{}' already exists",
            project_path.display()
        )));
    }

    fs::create_dir(&project_path)?;

    let config = Config {
        name: name.clone(),
        version: Some("0.0.1".to_string()),
        description: Some(format!("A new Coupe project: {}", name)),
        ..Default::default()
    };

    let config_path = project_path.join("coupe.yaml");
    let yaml_content = config
        .to_yaml()
        .map_err(|e| CoupeError::Config(e.to_string()))?;
    fs::write(&config_path, yaml_content)?;

    println!("✓ Created project directory: {}", project_path.display());
    println!("✓ Generated coupe.yaml configuration file");
    println!("\nNext steps:");
    println!("  cd {}", project_path.display());
    println!("  # Add your functions to coupe.yaml");
    println!("  # Run 'coupe up' to start your project");

    Ok(())
}
